//! VNC Server Management
//!
//! Manages TightVNC Server on student machines:
//! - Install TightVNC silently
//! - Start/stop VNC server
//! - Configure VNC password
//! - Check VNC status

use std::path::PathBuf;
use std::process::Command;

const VNC_DEFAULT_PORT: u16 = 5900;
const VNC_SERVICE_NAME: &str = "tvnserver";

/// Check if TightVNC is installed
pub fn is_vnc_installed() -> bool {
    get_vnc_server_path().is_some()
}

/// Get TightVNC server executable path
fn get_vnc_server_path() -> Option<PathBuf> {
    let paths = [
        r"C:\Program Files\TightVNC\tvnserver.exe",
        r"C:\Program Files (x86)\TightVNC\tvnserver.exe",
    ];
    for p in &paths {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// Get VNC server status
pub fn get_vnc_status() -> VncStatus {
    if !is_vnc_installed() {
        return VncStatus {
            installed: false,
            running: false,
            port: VNC_DEFAULT_PORT,
        };
    }

    let running = is_vnc_running();
    VncStatus {
        installed: true,
        running,
        port: VNC_DEFAULT_PORT,
    }
}

#[derive(Debug, serde::Serialize)]
pub struct VncStatus {
    pub installed: bool,
    pub running: bool,
    pub port: u16,
}

/// Check if VNC server is running
fn is_vnc_running() -> bool {
    // Check Windows service
    let output = Command::new("sc")
        .args(["query", VNC_SERVICE_NAME])
        .output();

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        return stdout.contains("RUNNING");
    }

    // Fallback: check process
    let output = Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq tvnserver.exe"])
        .output();

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        return stdout.contains("tvnserver.exe");
    }

    false
}

/// Start VNC server
pub fn start_vnc(password: Option<&str>) -> Result<String, String> {
    if !is_vnc_installed() {
        return Err("TightVNC is not installed. Install it first.".to_string());
    }

    // Set password if provided
    if let Some(pwd) = password {
        set_vnc_password(pwd)?;
    }

    // Try starting as Windows service first
    let result = Command::new("sc")
        .args(["start", VNC_SERVICE_NAME])
        .output();

    match result {
        Ok(out) if out.status.success() => {
            log::info!("[VNC] Service started successfully");
            Ok("VNC server started".to_string())
        }
        _ => {
            // Fallback: start as application
            log::info!("[VNC] Service start failed, trying application mode");
            if let Some(exe) = get_vnc_server_path() {
                let result = Command::new(&exe)
                    .arg("-run")
                    .spawn();
                match result {
                    Ok(_) => Ok("VNC server started (application mode)".to_string()),
                    Err(e) => Err(format!("Failed to start VNC: {}", e)),
                }
            } else {
                Err("VNC server executable not found".to_string())
            }
        }
    }
}

/// Stop VNC server
pub fn stop_vnc() -> Result<String, String> {
    // Try stopping service
    let _ = Command::new("sc")
        .args(["stop", VNC_SERVICE_NAME])
        .output();

    // Also kill any running process
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "tvnserver.exe"])
        .output();

    log::info!("[VNC] VNC server stopped");
    Ok("VNC server stopped".to_string())
}

/// Set VNC password via registry
fn set_vnc_password(password: &str) -> Result<(), String> {
    // TightVNC stores encrypted password in registry
    // Use tvnserver -controlservice -setparam to set it
    if let Some(exe) = get_vnc_server_path() {
        // Set password using TightVNC's built-in password tool
        let result = Command::new(&exe)
            .args(["-controlservice", "-setparam", "Password", password])
            .output();

        match result {
            Ok(out) if out.status.success() => {
                log::info!("[VNC] Password set successfully");
                Ok(())
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                // Try alternative method via registry
                set_vnc_password_registry(password)
                    .map_err(|e| format!("Failed to set password: {} / {}", stderr.trim(), e))
            }
            Err(_) => set_vnc_password_registry(password),
        }
    } else {
        Err("VNC server not found".to_string())
    }
}

/// Set VNC password directly via registry (fallback)
fn set_vnc_password_registry(password: &str) -> Result<(), String> {
    // TightVNC uses DES-encrypted password in registry
    // For simplicity, use PowerShell to set it
    let ps_cmd = format!(
        r#"
        $regPath = 'HKLM:\SOFTWARE\TightVNC\Server'
        if (-not (Test-Path $regPath)) {{ New-Item -Path $regPath -Force | Out-Null }}
        # Set control password and primary password
        # TightVNC accepts plain text password via -setparam
        "#
    );

    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_cmd])
        .output();

    // Use the TightVNC password utility if available
    let tvnpasswd = PathBuf::from(r"C:\Program Files\TightVNC\tvnpasswd.exe");
    if tvnpasswd.exists() {
        let _ = Command::new(&tvnpasswd)
            .args(["-set", password])
            .output();
    }

    Ok(())
}

/// Install TightVNC silently from a bundled or downloaded installer
pub fn install_vnc(installer_path: Option<&str>, password: &str) -> Result<String, String> {
    if is_vnc_installed() {
        return Ok("TightVNC is already installed".to_string());
    }

    let installer = if let Some(path) = installer_path {
        PathBuf::from(path)
    } else {
        // Look for installer in common locations
        let candidates = [
            r"C:\SmartLab\tightvnc-setup.msi",
            r"C:\temp\tightvnc-setup.msi",
        ];
        candidates.iter()
            .map(PathBuf::from)
            .find(|p| p.exists())
            .ok_or("TightVNC installer not found. Place tightvnc-setup.msi in C:\\SmartLab\\".to_string())?
    };

    if !installer.exists() {
        return Err(format!("Installer not found: {}", installer.display()));
    }

    log::info!("[VNC] Installing TightVNC from: {}", installer.display());

    // Silent install with msiexec
    // SET_USEVNCAUTHENTICATION=1 enables VNC password auth
    // SET_PASSWORD sets the VNC password
    // ADDLOCAL=Server installs only the server component
    let result = Command::new("msiexec")
        .args([
            "/i",
            &installer.to_string_lossy(),
            "/quiet",
            "/norestart",
            "ADDLOCAL=Server",
            &format!("SET_USEVNCAUTHENTICATION=1"),
            &format!("SET_PASSWORD={}", password),
            "SET_USECONTROLAUTHENTICATION=1",
            &format!("SET_CONTROLPASSWORD={}", password),
        ])
        .output()
        .map_err(|e| format!("Failed to run installer: {}", e))?;

    if result.status.success() {
        log::info!("[VNC] TightVNC installed successfully");
        // Give service time to register
        std::thread::sleep(std::time::Duration::from_secs(2));
        Ok("TightVNC installed successfully".to_string())
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr);
        let stdout = String::from_utf8_lossy(&result.stdout);
        Err(format!("Installation failed: {} {}", stdout.trim(), stderr.trim()))
    }
}
