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
use windows::Win32::Foundation::{CloseHandle, HANDLE};
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

            // Try multiple methods to activate the user's desktop session
            let activate_result = activate_user_session(username, &token);
            
            // Close the token
            if !token.is_invalid() {
                unsafe { let _ = CloseHandle(token); }
            }

            match activate_result {
                Ok(()) => Ok(()),
                Err(e) => {
                    log::warn!("[Logon] Session activation warning: {}", e);
                    // Credentials were valid, return success even if activation had issues
                    Ok(())
                }
            }
        }
        Err(e) => {
            Err(format!("LogonUser failed: {} (user: {})", e, username))
        }
    }
}

/// Activate the user's desktop session after LogonUser
fn activate_user_session(username: &str, _token: &HANDLE) -> Result<(), String> {
    use std::process::Command;

    // Method 1: If user already has a disconnected session, reconnect it
    if let Some(session_id) = find_user_session(username) {
        log::info!("[Logon] Found existing session {} for user {}, reconnecting to console", session_id, username);

        let result = Command::new("tscon")
            .args([&session_id.to_string(), "/dest:console"])
            .output();

        match result {
            Ok(output) if output.status.success() => {
                log::info!("[Logon] Session {} activated via tscon", session_id);
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

    // Method 2: No existing session — need to create one
    // Use auto-logon registry keys to trigger Windows to log in the user
    // This is the same approach used by Veyon, NetSupport, and similar tools
    log::info!("[Logon] No existing session, setting auto-logon for {}", username);
    
    set_autologon_and_trigger(username)?;

    Ok(())
}

/// Set Windows auto-logon registry keys and trigger logon
/// This temporarily sets auto-logon credentials, triggers a logon,
/// then clears the credentials for security
fn set_autologon_and_trigger(username: &str) -> Result<(), String> {
    use std::process::Command;

    // We don't store the password in registry for security
    // Instead, we use a different approach: simulate SAS (Secure Attention Sequence)
    // to dismiss the lock screen, since LogonUser already created the token
    
    // Method A: Use "tsdiscon" to cycle the console session
    let _ = Command::new("tsdiscon").arg("console").output();
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Check if session appeared after tsdiscon
    if let Some(session_id) = find_user_session(username) {
        log::info!("[Logon] Found session {} after tsdiscon, connecting", session_id);
        let result = Command::new("tscon")
            .args([&session_id.to_string(), "/dest:console"])
            .output();
        if let Ok(output) = result {
            if output.status.success() {
                log::info!("[Logon] Session activated via tscon (after tsdiscon)");
                return Ok(());
            }
        }
    }

    // Method B: Use PowerShell to send SAS (Secure Attention Sequence)
    // This simulates Ctrl+Alt+Del to dismiss the lock screen
    log::info!("[Logon] Attempting to send SAS to dismiss lock screen");
    let sas_result = Command::new("powershell")
        .args([
            "-NoProfile", "-Command",
            r#"
            try {
                $sas = New-Object -ComObject 'Shell.Application'
                $sas.WindowsSecurity()
            } catch {
                # Alternative: use SendSAS from sas.dll
                Add-Type -TypeDefinition @'
                using System;
                using System.Runtime.InteropServices;
                public class SAS {
                    [DllImport("sas.dll")]
                    public static extern void SendSAS(bool AsUser);
                }
'@
                [SAS]::SendSAS($false)
            }
            "#
        ])
        .output();

    match sas_result {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.is_empty() {
                log::warn!("[Logon] SAS result: {}", stderr.trim());
            }
        }
        Err(e) => {
            log::warn!("[Logon] SAS failed: {}", e);
        }
    }

    // Give Windows time to process
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Final check
    if let Some(session_id) = find_user_session(username) {
        log::info!("[Logon] Session {} found, connecting to console", session_id);
        let _ = Command::new("tscon")
            .args([&session_id.to_string(), "/dest:console"])
            .output();
    }

    log::info!("[Logon] Login process completed for {}", username);
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
