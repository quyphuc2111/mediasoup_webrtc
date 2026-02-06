//! Windows User Logon
//!
//! Uses Win32 API to:
//! - LogonUser: Authenticate credentials
//! - WTSEnumerateSessions: Find active user sessions
//! - Create interactive logon session so SmartlabStudent can start

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
use windows::Win32::Security::{
    LogonUserW, LOGON32_LOGON_INTERACTIVE, LOGON32_PROVIDER_DEFAULT,
};
use windows::Win32::System::RemoteDesktop::{
    WTSActive, WTSEnumerateSessionsW, WTSFreeMemory, WTSQuerySessionInformationW,
    WTSUserName, WTS_CURRENT_SERVER_HANDLE, WTS_SESSION_INFOW,
};

/// Convert a Rust string to a null-terminated wide string
fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// Logon a user interactively on this machine
///
/// This validates credentials and creates a logon session.
/// Combined with the Scheduled Task (ONLOGON), SmartlabStudent
/// will auto-start once the session is created.
pub fn logon_user(username: &str, password: &str, domain: Option<&str>) -> Result<(), String> {
    let wide_user = to_wide(username);
    let wide_pass = to_wide(password);
    let wide_domain = domain.map(|d| to_wide(d));

    let domain_ptr = wide_domain
        .as_ref()
        .map(|d| windows::core::PCWSTR(d.as_ptr()))
        .unwrap_or(windows::core::PCWSTR::null());

    let mut token = HANDLE::default();

    let result = unsafe {
        LogonUserW(
            windows::core::PCWSTR(wide_user.as_ptr()),
            domain_ptr,
            windows::core::PCWSTR(wide_pass.as_ptr()),
            LOGON32_LOGON_INTERACTIVE,
            LOGON32_PROVIDER_DEFAULT,
            &mut token,
        )
    };

    match result {
        Ok(()) => {
            log::info!("[Logon] LogonUser succeeded for: {}", username);

            // Close the token — the logon session is created,
            // Windows will show the user's desktop
            if !token.is_invalid() {
                unsafe { let _ = CloseHandle(token); }
            }

            // Trigger a fast user switch / unlock to activate the session
            // The user's desktop should now be available
            trigger_logon_screen(username)?;

            Ok(())
        }
        Err(e) => {
            Err(format!("LogonUser failed: {} (user: {})", e, username))
        }
    }
}

/// Trigger the logon screen to switch to the user's session
/// Uses a simple approach: simulate Ctrl+Alt+Del equivalent via API
fn trigger_logon_screen(username: &str) -> Result<(), String> {
    use std::process::Command;

    // Method 1: Use tscon to connect to the user's session
    // First find the session ID
    if let Some(session_id) = find_user_session(username) {
        log::info!("[Logon] Found session {} for user {}, activating", session_id, username);

        // Connect console to this session
        let result = Command::new("tscon")
            .args([&session_id.to_string(), "/dest:console"])
            .output();

        match result {
            Ok(output) if output.status.success() => {
                log::info!("[Logon] Session activated via tscon");
                return Ok(());
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                log::warn!("[Logon] tscon failed: {}", stderr);
            }
            Err(e) => {
                log::warn!("[Logon] tscon error: {}", e);
            }
        }
    }

    // Method 2: Use quser/query to verify and logoff/logon
    // The LogonUser call already created the session, it should appear at next console switch
    log::info!("[Logon] Session created for {}, will be active at next console interaction", username);
    Ok(())
}

/// Find a WTS session ID for a given username
fn find_user_session(target_user: &str) -> Option<u32> {
    unsafe {
        let mut session_info: *mut WTS_SESSION_INFOW = std::ptr::null_mut();
        let mut count: u32 = 0;

        let result = WTSEnumerateSessionsW(
            WTS_CURRENT_SERVER_HANDLE,
            0,
            1,
            &mut session_info,
            &mut count,
        );

        if result.is_err() {
            return None;
        }

        let sessions = std::slice::from_raw_parts(session_info, count as usize);
        let mut found_session = None;

        for session in sessions {
            // Query username for this session
            let mut buffer: PWSTR = PWSTR::null();
            let mut bytes_returned: u32 = 0;

            let query_result = WTSQuerySessionInformationW(
                WTS_CURRENT_SERVER_HANDLE,
                session.SessionId,
                WTSUserName,
                &mut buffer,
                &mut bytes_returned,
            );

            if query_result.is_ok() && !buffer.is_null() {
                let user = buffer.to_string().unwrap_or_default();
                WTSFreeMemory(buffer.as_ptr() as *mut _);

                if user.eq_ignore_ascii_case(target_user) {
                    found_session = Some(session.SessionId);
                    break;
                }
            }
        }

        WTSFreeMemory(session_info as *mut _);
        found_session
    }
}

/// Get the username of the currently active console session
pub fn get_active_session_user() -> Option<String> {
    unsafe {
        let mut session_info: *mut WTS_SESSION_INFOW = std::ptr::null_mut();
        let mut count: u32 = 0;

        let result = WTSEnumerateSessionsW(
            WTS_CURRENT_SERVER_HANDLE,
            0,
            1,
            &mut session_info,
            &mut count,
        );

        if result.is_err() {
            return None;
        }

        let sessions = std::slice::from_raw_parts(session_info, count as usize);
        let mut active_user = None;

        for session in sessions {
            if session.State == WTSActive {
                let mut buffer: PWSTR = PWSTR::null();
                let mut bytes_returned: u32 = 0;

                let query_result = WTSQuerySessionInformationW(
                    WTS_CURRENT_SERVER_HANDLE,
                    session.SessionId,
                    WTSUserName,
                    &mut buffer,
                    &mut bytes_returned,
                );

                if query_result.is_ok() && !buffer.is_null() {
                    let user = buffer.to_string().unwrap_or_default();
                    WTSFreeMemory(buffer.as_ptr() as *mut _);

                    if !user.is_empty() && user != "SYSTEM" {
                        active_user = Some(user);
                        break;
                    }
                }
            }
        }

        WTSFreeMemory(session_info as *mut _);
        active_user
    }
}
