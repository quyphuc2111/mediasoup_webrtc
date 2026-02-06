//! Autostart Registration Module
//!
//! Implements Veyon/NetSupport-style autostart:
//! - Windows: Registry HKLM/HKCU Run key + Scheduled Task + Firewall
//! - macOS: LaunchDaemon (system-level) or LaunchAgent (user-level fallback)
//!
//! The app self-registers on first run, so no manual script is needed.
//! Uses the actual product name and binary path at runtime.

use std::path::PathBuf;

/// Get the current executable path
fn get_exe_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("Failed to get exe path: {}", e))
}

/// Get the product name from the exe filename (e.g. "SmartlabStudent.exe" -> "SmartlabStudent")
fn get_product_name() -> String {
    get_exe_path()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_else(|| "SmartlabStudent".to_string())
}

/// Check if autostart is already configured
pub fn is_autostart_configured() -> bool {
    #[cfg(target_os = "windows")]
    { return is_autostart_configured_windows(); }
    #[cfg(target_os = "macos")]
    { return is_autostart_configured_macos(); }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    { false }
}

/// Register autostart (called automatically on first run)
pub fn register_autostart() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    { return register_autostart_windows(); }
    #[cfg(target_os = "macos")]
    { return register_autostart_macos(); }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    { Ok(()) }
}


// ============================================================
// Windows Implementation
// ============================================================

#[cfg(target_os = "windows")]
fn is_autostart_configured_windows() -> bool {
    use std::process::Command;
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let name = get_product_name();

    if let Ok(output) = Command::new("schtasks")
        .args(["/Query", "/TN", &name])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        if output.status.success() { return true; }
    }

    if let Ok(output) = Command::new("reg")
        .args(["query", r"HKLM\Software\Microsoft\Windows\CurrentVersion\Run", "/v", &name])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        if output.status.success() { return true; }
    }

    if let Ok(output) = Command::new("reg")
        .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", &name])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        if output.status.success() { return true; }
    }

    false
}

#[cfg(target_os = "windows")]
fn register_autostart_windows() -> Result<(), String> {
    use std::process::Command;
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let exe_path = get_exe_path()?;
    let exe_str = exe_path.to_string_lossy();
    let name = get_product_name();

    log::info!("[Autostart] Registering '{}' autostart for: {}", name, exe_str);

    // 1. Registry: try HKLM (system-wide), fallback HKCU
    let hklm_ok = Command::new("reg")
        .args(["add", r"HKLM\Software\Microsoft\Windows\CurrentVersion\Run",
               "/v", &name, "/t", "REG_SZ",
               "/d", &format!("\"{}\"", exe_str), "/f"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if hklm_ok {
        log::info!("[Autostart] Registered in HKLM (system-wide)");
    } else {
        let hkcu_ok = Command::new("reg")
            .args(["add", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                   "/v", &name, "/t", "REG_SZ",
                   "/d", &format!("\"{}\"", exe_str), "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if hkcu_ok {
            log::info!("[Autostart] Registered in HKCU (user-level fallback)");
        } else {
            log::warn!("[Autostart] Failed to register in registry");
        }
    }

    // 2. Scheduled Task (like Veyon)
    let _ = Command::new("schtasks")
        .args(["/Delete", "/TN", &name, "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let task_ok = Command::new("schtasks")
        .args(["/Create", "/TN", &name,
               "/TR", &format!("\"{}\"", exe_str),
               "/SC", "ONLOGON", "/RL", "HIGHEST", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if task_ok {
        log::info!("[Autostart] Created scheduled task (ONLOGON, HIGHEST)");
    } else {
        let task_ok = Command::new("schtasks")
            .args(["/Create", "/TN", &name,
                   "/TR", &format!("\"{}\"", exe_str),
                   "/SC", "ONLOGON", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if task_ok {
            log::info!("[Autostart] Created scheduled task (ONLOGON, normal)");
        } else {
            log::warn!("[Autostart] Could not create scheduled task");
        }
    }

    // 3. Firewall rules
    let _ = Command::new("netsh")
        .args(["advfirewall", "firewall", "delete", "rule", &format!("name={}", name)])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let _ = Command::new("netsh")
        .args(["advfirewall", "firewall", "add", "rule",
               &format!("name={}", name), "dir=in", "action=allow",
               &format!("program={}", exe_str), "enable=yes", "profile=any"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let _ = Command::new("netsh")
        .args(["advfirewall", "firewall", "add", "rule",
               &format!("name={} Out", name), "dir=out", "action=allow",
               &format!("program={}", exe_str), "enable=yes", "profile=any"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    log::info!("[Autostart] Firewall rules configured");

    Ok(())
}


// ============================================================
// macOS Implementation
// ============================================================

#[cfg(target_os = "macos")]
const DAEMON_LABEL: &str = "com.zenadev.smartlabstudent";
#[cfg(target_os = "macos")]
const DAEMON_PATH: &str = "/Library/LaunchDaemons/com.zenadev.smartlabstudent.plist";
#[cfg(target_os = "macos")]
const AGENT_REL_PATH: &str = "Library/LaunchAgents/com.zenadev.smartlabstudent.plist";

#[cfg(target_os = "macos")]
fn is_autostart_configured_macos() -> bool {
    use std::path::Path;
    if Path::new(DAEMON_PATH).exists() { return true; }
    if let Some(home) = dirs::home_dir() {
        if home.join(AGENT_REL_PATH).exists() { return true; }
    }
    false
}

#[cfg(target_os = "macos")]
fn make_plist(exe: &str, log_dir: &str) -> String {
    format!(
r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>StandardOutPath</key>
    <string>{log_dir}/smartlabstudent.log</string>
    <key>StandardErrorPath</key>
    <string>{log_dir}/smartlabstudent.error.log</string>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>"#,
        label = DAEMON_LABEL,
        exe = exe,
        log_dir = log_dir,
    )
}

#[cfg(target_os = "macos")]
fn register_autostart_macos() -> Result<(), String> {
    use std::process::Command;

    let exe_path = get_exe_path()?;
    let exe_str = exe_path.to_string_lossy();

    log::info!("[Autostart] Registering autostart for: {}", exe_str);

    // Try LaunchDaemon first (system-level, like Veyon)
    if std::fs::write(DAEMON_PATH, make_plist(&exe_str, "/var/log")).is_ok() {
        let _ = Command::new("chmod").args(["644", DAEMON_PATH]).output();
        let _ = Command::new("launchctl").args(["load", DAEMON_PATH]).output();
        log::info!("[Autostart] Registered LaunchDaemon (system-level)");
        return Ok(());
    }

    // Fallback: LaunchAgent (user-level)
    log::info!("[Autostart] LaunchDaemon failed (no root), falling back to LaunchAgent");
    let home = dirs::home_dir().ok_or("Failed to get home directory")?;
    let agent_dir = home.join("Library/LaunchAgents");
    let agent_path = home.join(AGENT_REL_PATH);

    std::fs::create_dir_all(&agent_dir)
        .map_err(|e| format!("Failed to create LaunchAgents dir: {}", e))?;
    std::fs::write(&agent_path, make_plist(&exe_str, "/tmp"))
        .map_err(|e| format!("Failed to write LaunchAgent plist: {}", e))?;

    let agent_str = agent_path.to_string_lossy();
    let _ = Command::new("chmod").args(["644", &agent_str]).output();
    let _ = Command::new("launchctl").args(["load", &agent_str]).output();

    log::info!("[Autostart] Registered LaunchAgent (user-level)");
    Ok(())
}
