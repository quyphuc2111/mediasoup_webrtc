//! Autostart Registration Module
//!
//! Implements Veyon/NetSupport-style autostart:
//! - Windows: Registry HKLM/HKCU Run key + Scheduled Task + Firewall
//! - macOS: LaunchDaemon (system-level) or LaunchAgent (user-level fallback)
//!
//! The app self-registers on first run, so no manual script is needed.

use std::path::PathBuf;

/// Check if autostart is already configured
pub fn is_autostart_configured() -> bool {
    #[cfg(target_os = "windows")]
    {
        is_autostart_configured_windows()
    }
    #[cfg(target_os = "macos")]
    {
        is_autostart_configured_macos()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        false
    }
}

/// Register autostart (called automatically on first run)
pub fn register_autostart() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        register_autostart_windows()
    }
    #[cfg(target_os = "macos")]
    {
        register_autostart_macos()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Ok(())
    }
}

/// Register firewall exception
pub fn register_firewall() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        register_firewall_windows()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}

/// Get the current executable path
fn get_exe_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("Failed to get exe path: {}", e))
}


// ============================================================
// Windows Implementation
// ============================================================

#[cfg(target_os = "windows")]
fn is_autostart_configured_windows() -> bool {
    use std::process::Command;
    
    // Check if scheduled task exists
    let output = Command::new("schtasks")
        .args(["/Query", "/TN", "SmartlabStudent"])
        .output();
    
    if let Ok(output) = output {
        if output.status.success() {
            return true;
        }
    }
    
    // Check registry as fallback
    let output = Command::new("reg")
        .args(["query", r"HKLM\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "SmartlabStudent"])
        .output();
    
    if let Ok(output) = output {
        if output.status.success() {
            return true;
        }
    }
    
    // Check HKCU
    let output = Command::new("reg")
        .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "SmartlabStudent"])
        .output();
    
    if let Ok(output) = output {
        return output.status.success();
    }
    
    false
}

#[cfg(target_os = "windows")]
fn register_autostart_windows() -> Result<(), String> {
    use std::process::Command;
    
    let exe_path = get_exe_path()?;
    let exe_str = exe_path.to_string_lossy();
    
    log::info!("[Autostart] Registering autostart for: {}", exe_str);
    
    // 1. Try HKLM registry (system-wide, needs admin)
    let hklm_result = Command::new("reg")
        .args([
            "add",
            r"HKLM\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v", "SmartlabStudent",
            "/t", "REG_SZ",
            "/d", &format!("\"{}\"", exe_str),
            "/f",
        ])
        .output();
    
    match hklm_result {
        Ok(output) if output.status.success() => {
            log::info!("[Autostart] Registered in HKLM (system-wide)");
        }
        _ => {
            // Fallback to HKCU (user-level)
            let hkcu_result = Command::new("reg")
                .args([
                    "add",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v", "SmartlabStudent",
                    "/t", "REG_SZ",
                    "/d", &format!("\"{}\"", exe_str),
                    "/f",
                ])
                .output()
                .map_err(|e| format!("Failed to write HKCU registry: {}", e))?;
            
            if hkcu_result.status.success() {
                log::info!("[Autostart] Registered in HKCU (user-level fallback)");
            } else {
                log::warn!("[Autostart] Failed to register in registry");
            }
        }
    }
    
    // 2. Create Scheduled Task (like Veyon)
    // Delete existing task first
    let _ = Command::new("schtasks")
        .args(["/Delete", "/TN", "SmartlabStudent", "/F"])
        .output();
    
    // Create task with highest privileges
    let task_result = Command::new("schtasks")
        .args([
            "/Create",
            "/TN", "SmartlabStudent",
            "/TR", &format!("\"{}\"", exe_str),
            "/SC", "ONLOGON",
            "/RL", "HIGHEST",
            "/F",
        ])
        .output();
    
    match task_result {
        Ok(output) if output.status.success() => {
            log::info!("[Autostart] Created scheduled task (ONLOGON, HIGHEST)");
        }
        _ => {
            // Fallback: create without HIGHEST privilege
            let task_result = Command::new("schtasks")
                .args([
                    "/Create",
                    "/TN", "SmartlabStudent",
                    "/TR", &format!("\"{}\"", exe_str),
                    "/SC", "ONLOGON",
                    "/F",
                ])
                .output();
            
            match task_result {
                Ok(output) if output.status.success() => {
                    log::info!("[Autostart] Created scheduled task (ONLOGON, normal)");
                }
                _ => {
                    log::warn!("[Autostart] Could not create scheduled task");
                }
            }
        }
    }
    
    // 3. Register firewall
    let _ = register_firewall_windows();
    
    log::info!("[Autostart] Autostart registration complete");
    Ok(())
}

#[cfg(target_os = "windows")]
fn register_firewall_windows() -> Result<(), String> {
    use std::process::Command;
    
    let exe_path = get_exe_path()?;
    let exe_str = exe_path.to_string_lossy();
    
    // Remove old rules
    let _ = Command::new("netsh")
        .args(["advfirewall", "firewall", "delete", "rule", "name=SmartlabStudent"])
        .output();
    
    // Add inbound rule
    let inbound = Command::new("netsh")
        .args([
            "advfirewall", "firewall", "add", "rule",
            "name=SmartlabStudent",
            "dir=in",
            "action=allow",
            &format!("program={}", exe_str),
            "enable=yes",
            "profile=any",
        ])
        .output();
    
    match inbound {
        Ok(output) if output.status.success() => {
            log::info!("[Autostart] Added inbound firewall rule");
        }
        _ => {
            log::warn!("[Autostart] Could not add inbound firewall rule (may need admin)");
        }
    }
    
    // Add outbound rule
    let _ = Command::new("netsh")
        .args([
            "advfirewall", "firewall", "add", "rule",
            "name=SmartlabStudent Out",
            "dir=out",
            "action=allow",
            &format!("program={}", exe_str),
            "enable=yes",
            "profile=any",
        ])
        .output();
    
    Ok(())
}


// ============================================================
// macOS Implementation
// ============================================================

#[cfg(target_os = "macos")]
const LAUNCH_DAEMON_LABEL: &str = "com.zenadev.smartlabstudent";
#[cfg(target_os = "macos")]
const LAUNCH_DAEMON_PATH: &str = "/Library/LaunchDaemons/com.zenadev.smartlabstudent.plist";
#[cfg(target_os = "macos")]
const LAUNCH_AGENT_PATH: &str = "Library/LaunchAgents/com.zenadev.smartlabstudent.plist";

#[cfg(target_os = "macos")]
fn is_autostart_configured_macos() -> bool {
    use std::path::Path;
    
    // Check LaunchDaemon first (system-level)
    if Path::new(LAUNCH_DAEMON_PATH).exists() {
        return true;
    }
    
    // Check LaunchAgent (user-level fallback)
    if let Some(home) = dirs::home_dir() {
        let agent_path = home.join(LAUNCH_AGENT_PATH);
        if agent_path.exists() {
            return true;
        }
    }
    
    false
}

#[cfg(target_os = "macos")]
fn register_autostart_macos() -> Result<(), String> {
    use std::process::Command;
    
    let exe_path = get_exe_path()?;
    let exe_str = exe_path.to_string_lossy();
    
    log::info!("[Autostart] Registering autostart for: {}", exe_str);
    
    // Try LaunchDaemon first (system-level, like Veyon)
    let daemon_plist = format!(
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
    <string>/var/log/smartlabstudent.log</string>
    <key>StandardErrorPath</key>
    <string>/var/log/smartlabstudent.error.log</string>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>"#,
        label = LAUNCH_DAEMON_LABEL,
        exe = exe_str,
    );
    
    // Try writing LaunchDaemon (needs root/sudo)
    let daemon_written = std::fs::write(LAUNCH_DAEMON_PATH, &daemon_plist).is_ok();
    
    if daemon_written {
        // Set permissions
        let _ = Command::new("chmod")
            .args(["644", LAUNCH_DAEMON_PATH])
            .output();
        
        // Load the daemon
        let _ = Command::new("launchctl")
            .args(["load", LAUNCH_DAEMON_PATH])
            .output();
        
        log::info!("[Autostart] Registered LaunchDaemon (system-level)");
        return Ok(());
    }
    
    // Fallback: LaunchAgent (user-level)
    log::info!("[Autostart] LaunchDaemon failed (no root), falling back to LaunchAgent");
    
    let home = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;
    
    let agent_dir = home.join("Library/LaunchAgents");
    let agent_path = home.join(LAUNCH_AGENT_PATH);
    
    // Create directory if needed
    std::fs::create_dir_all(&agent_dir)
        .map_err(|e| format!("Failed to create LaunchAgents dir: {}", e))?;
    
    let agent_plist = format!(
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
    <string>/tmp/smartlabstudent.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/smartlabstudent.error.log</string>
</dict>
</plist>"#,
        label = LAUNCH_DAEMON_LABEL,
        exe = exe_str,
    );
    
    std::fs::write(&agent_path, &agent_plist)
        .map_err(|e| format!("Failed to write LaunchAgent plist: {}", e))?;
    
    // Set permissions
    let _ = Command::new("chmod")
        .args(["644", &agent_path.to_string_lossy()])
        .output();
    
    // Load the agent
    let _ = Command::new("launchctl")
        .args(["load", &agent_path.to_string_lossy()])
        .output();
    
    log::info!("[Autostart] Registered LaunchAgent (user-level)");
    Ok(())
}
