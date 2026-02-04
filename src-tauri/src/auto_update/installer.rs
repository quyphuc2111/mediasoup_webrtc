// Installer Runner for Windows
// Executes Windows installers in silent mode
// Requirements: 4.1, 4.3

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
#[cfg(target_os = "windows")]
use std::io::Result as IoResult;

/// Error types for installation operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "message")]
pub enum InstallError {
    /// Installer file not found
    InstallerNotFound(String),
    
    /// Failed to execute installer
    ExecutionFailed(String),
    
    /// Installer returned non-zero exit code
    InstallerFailed { exit_code: i32, message: String },
    
    /// Failed to detect installer type
    UnknownInstallerType(String),
    
    /// Failed to restart application
    RestartFailed(String),
    
    /// Invalid installer path
    InvalidPath(String),
    
    /// Permission denied
    PermissionDenied(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::InstallerNotFound(path) => {
                write!(f, "Installer not found: {}", path)
            }
            InstallError::ExecutionFailed(msg) => {
                write!(f, "Failed to execute installer: {}", msg)
            }
            InstallError::InstallerFailed { exit_code, message } => {
                write!(f, "Installer failed with exit code {}: {}", exit_code, message)
            }
            InstallError::UnknownInstallerType(ext) => {
                write!(f, "Unknown installer type: {}", ext)
            }
            InstallError::RestartFailed(msg) => {
                write!(f, "Failed to restart application: {}", msg)
            }
            InstallError::InvalidPath(msg) => {
                write!(f, "Invalid installer path: {}", msg)
            }
            InstallError::PermissionDenied(msg) => {
                write!(f, "Permission denied: {}", msg)
            }
        }
    }
}

impl std::error::Error for InstallError {}

/// Types of Windows installers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallerType {
    /// Windows Installer (MSI)
    Msi,
    /// NSIS installer (EXE with /S flag)
    NsisExe,
    /// Inno Setup installer (EXE with /SILENT or /VERYSILENT flag)
    InnoSetupExe,
    /// Generic EXE installer (tries common silent flags)
    Generic,
    /// macOS DMG disk image
    MacDmg,
    /// macOS PKG installer
    MacPkg,
    /// macOS App bundle (.app)
    MacApp,
}

impl InstallerType {
    /// Get the silent installation arguments for this installer type
    pub fn silent_args(&self) -> Vec<&'static str> {
        match self {
            InstallerType::Msi => vec!["/quiet", "/norestart"],
            InstallerType::NsisExe => vec!["/S"],
            InstallerType::InnoSetupExe => vec!["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"],
            InstallerType::Generic => vec!["/S", "/silent", "/quiet"],
            InstallerType::MacDmg => vec![],
            InstallerType::MacPkg => vec![],
            InstallerType::MacApp => vec![],
        }
    }
}

/// Installer runner for Windows
/// Handles silent installation and application restart
pub struct InstallerRunner;

impl InstallerRunner {
    /// Run a program with elevated (admin) privileges on Windows
    /// Uses ShellExecuteW with "runas" verb to trigger UAC prompt
    #[cfg(target_os = "windows")]
    fn run_elevated(exe_path: &str, args: &[&str]) -> IoResult<std::process::Output> {
        use std::os::windows::ffi::OsStrExt;
        use std::ffi::OsStr;
        use std::ptr::null_mut;
        
        // For elevated execution, we need to use a different approach
        // Since ShellExecuteW doesn't give us output, we'll use a workaround:
        // Run the installer and wait for it to complete
        
        log::info!("[Installer] Attempting elevated execution of: {}", exe_path);
        
        // Build the arguments string
        let args_str = args.join(" ");
        
        // Use PowerShell Start-Process with -Verb RunAs for elevation
        let ps_command = format!(
            "Start-Process -FilePath '{}' -ArgumentList '{}' -Verb RunAs -Wait -PassThru | Select-Object -ExpandProperty ExitCode",
            exe_path.replace("'", "''"),
            args_str.replace("'", "''")
        );
        
        log::info!("[Installer] PowerShell command: {}", ps_command);
        
        Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_command])
            .output()
    }

    /// Detect installer type from file extension and header
    /// 
    /// # Arguments
    /// * `installer_path` - Path to the installer file
    /// 
    /// # Returns
    /// * `Ok(InstallerType)` - Detected installer type
    /// * `Err(InstallError)` - If detection fails
    pub fn detect_installer_type(installer_path: &Path) -> Result<InstallerType, InstallError> {
        // Check if file exists
        if !installer_path.exists() {
            return Err(InstallError::InstallerNotFound(
                installer_path.display().to_string(),
            ));
        }

        // Get file extension
        let extension = installer_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase());

        match extension.as_deref() {
            Some("msi") => Ok(InstallerType::Msi),
            Some("exe") => {
                // Try to detect EXE installer type from file header/content
                Self::detect_exe_installer_type(installer_path)
            }
            // macOS installer types
            Some("dmg") => Ok(InstallerType::MacDmg),
            Some("pkg") => Ok(InstallerType::MacPkg),
            Some("app") => Ok(InstallerType::MacApp),
            Some(ext) => Err(InstallError::UnknownInstallerType(ext.to_string())),
            None => Err(InstallError::UnknownInstallerType(
                "no extension".to_string(),
            )),
        }
    }

    /// Detect the type of EXE installer by examining file content
    fn detect_exe_installer_type(installer_path: &Path) -> Result<InstallerType, InstallError> {
        // Read first few KB of the file to check for signatures
        let content = std::fs::read(installer_path).map_err(|e| {
            InstallError::ExecutionFailed(format!("Failed to read installer: {}", e))
        })?;

        // Check for NSIS signature ("Nullsoft" appears in NSIS installers)
        if content.windows(8).any(|w| w == b"Nullsoft") {
            return Ok(InstallerType::NsisExe);
        }

        // Check for Inno Setup signature
        // Inno Setup installers contain "Inno Setup" string
        if content.windows(10).any(|w| w == b"Inno Setup") {
            return Ok(InstallerType::InnoSetupExe);
        }

        // Default to generic if we can't detect the type
        Ok(InstallerType::Generic)
    }

    /// Run installer silently (Windows)
    /// 
    /// For MSI: msiexec /i <path> /quiet /norestart
    /// For NSIS EXE: <path> /S
    /// For Inno Setup EXE: <path> /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
    /// 
    /// # Arguments
    /// * `installer_path` - Path to the installer file
    /// * `installer_type` - Type of installer (MSI, NSIS, Inno Setup, Generic)
    /// 
    /// # Returns
    /// * `Ok(())` - Installation completed successfully
    /// * `Err(InstallError)` - Installation failed
    /// 
    /// Requirements: 4.1
    #[cfg(target_os = "windows")]
    pub fn run_silent(
        installer_path: &Path,
        installer_type: InstallerType,
    ) -> Result<(), InstallError> {
        // Validate installer path
        if !installer_path.exists() {
            return Err(InstallError::InstallerNotFound(
                installer_path.display().to_string(),
            ));
        }

        let installer_path_str = installer_path
            .to_str()
            .ok_or_else(|| InstallError::InvalidPath("Path contains invalid UTF-8".to_string()))?;

        log::info!("[Installer] Running installer: {} (type: {:?})", installer_path_str, installer_type);
        log::info!("[Installer] Silent args: {:?}", installer_type.silent_args());

        let output = match installer_type {
            InstallerType::Msi => {
                // Use msiexec for MSI files
                log::info!("[Installer] Using msiexec for MSI installation");
                Command::new("msiexec")
                    .arg("/i")
                    .arg(installer_path_str)
                    .args(installer_type.silent_args())
                    .output()
            }
            InstallerType::NsisExe => {
                // NSIS installer - run with /S flag
                // For per-user installers (Student app), no elevation needed
                // For per-machine installers (Teacher app), request elevation
                log::info!("[Installer] Running NSIS installer with /S flag");
                
                // First try without elevation (works for per-user installers)
                let result = Command::new(installer_path_str)
                    .arg("/S")
                    .output();
                
                match &result {
                    Ok(output) if output.status.success() => {
                        log::info!("[Installer] NSIS installer completed successfully (no elevation needed)");
                        result
                    }
                    Ok(output) => {
                        let exit_code = output.status.code().unwrap_or(-1);
                        // Exit code 740 means elevation required
                        if exit_code == 740 {
                            log::info!("[Installer] Elevation required, requesting admin rights...");
                            Self::run_elevated(installer_path_str, &["/S"])
                        } else {
                            result
                        }
                    }
                    Err(_) => {
                        // Try with elevation as fallback
                        log::info!("[Installer] Direct execution failed, trying with elevation...");
                        Self::run_elevated(installer_path_str, &["/S"])
                    }
                }
            }
            InstallerType::InnoSetupExe => {
                // Inno Setup - run with /VERYSILENT
                log::info!("[Installer] Running Inno Setup installer");
                Command::new(installer_path_str)
                    .args(installer_type.silent_args())
                    .output()
            }
            _ => {
                // For other EXE installers, run directly with silent args
                log::info!("[Installer] Running generic EXE installer");
                Command::new(installer_path_str)
                    .args(installer_type.silent_args())
                    .output()
            }
        };

        match output {
            Ok(output) => {
                log::info!("[Installer] Installer exit status: {:?}", output.status);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stdout.is_empty() {
                    log::info!("[Installer] stdout: {}", stdout);
                }
                if !stderr.is_empty() {
                    log::warn!("[Installer] stderr: {}", stderr);
                }
                
                if output.status.success() {
                    log::info!("[Installer] Installation completed successfully");
                    Ok(())
                } else {
                    let exit_code = output.status.code().unwrap_or(-1);
                    log::error!("[Installer] Installation failed with exit code: {}", exit_code);
                    Err(InstallError::InstallerFailed {
                        exit_code,
                        message: if stderr.is_empty() {
                            format!("Installer exited with code {}", exit_code)
                        } else {
                            stderr.to_string()
                        },
                    })
                }
            }
            Err(e) => {
                log::error!("[Installer] Failed to execute installer: {}", e);
                Err(InstallError::ExecutionFailed(e.to_string()))
            }
        }
    }

    /// Run installer silently (non-Windows stub)
    #[cfg(not(target_os = "windows"))]
    pub fn run_silent(
        installer_path: &Path,
        installer_type: InstallerType,
    ) -> Result<(), InstallError> {
        // Validate installer path
        if !installer_path.exists() {
            return Err(InstallError::InstallerNotFound(
                installer_path.display().to_string(),
            ));
        }

        #[cfg(target_os = "macos")]
        {
            match installer_type {
                InstallerType::MacDmg => {
                    // Mount DMG, copy app to /Applications, unmount
                    Self::install_dmg(installer_path)
                }
                InstallerType::MacPkg => {
                    // Use installer command for PKG
                    Self::install_pkg(installer_path)
                }
                InstallerType::MacApp => {
                    // Copy .app to /Applications
                    Self::install_app(installer_path)
                }
                _ => Err(InstallError::ExecutionFailed(format!(
                    "Installer type {:?} is not supported on macOS",
                    installer_type
                ))),
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            Err(InstallError::ExecutionFailed(format!(
                "Installer type {:?} is not supported on this platform: {}",
                installer_type,
                installer_path.display()
            )))
        }
    }

    /// Install DMG on macOS
    #[cfg(target_os = "macos")]
    fn install_dmg(dmg_path: &Path) -> Result<(), InstallError> {
        let dmg_path_str = dmg_path.to_str()
            .ok_or_else(|| InstallError::InvalidPath("Path contains invalid UTF-8".to_string()))?;

        // Create a temporary mount point
        let mount_point = format!("/Volumes/SmartLabUpdate_{}", std::process::id());

        // Mount the DMG
        log::info!("[Installer] Mounting DMG: {}", dmg_path_str);
        let mount_output = Command::new("hdiutil")
            .args(["attach", dmg_path_str, "-mountpoint", &mount_point, "-nobrowse", "-quiet"])
            .output()
            .map_err(|e| InstallError::ExecutionFailed(format!("Failed to mount DMG: {}", e)))?;

        if !mount_output.status.success() {
            let stderr = String::from_utf8_lossy(&mount_output.stderr);
            return Err(InstallError::ExecutionFailed(format!("Failed to mount DMG: {}", stderr)));
        }

        // Find .app in mounted volume
        let app_result = std::fs::read_dir(&mount_point)
            .map_err(|e| InstallError::ExecutionFailed(format!("Failed to read mount point: {}", e)))?
            .filter_map(|entry| entry.ok())
            .find(|entry| {
                entry.path().extension()
                    .map(|ext| ext == "app")
                    .unwrap_or(false)
            });

        let cleanup = || {
            // Unmount DMG
            let _ = Command::new("hdiutil")
                .args(["detach", &mount_point, "-quiet"])
                .output();
        };

        let app_entry = match app_result {
            Some(entry) => entry,
            None => {
                cleanup();
                return Err(InstallError::ExecutionFailed("No .app found in DMG".to_string()));
            }
        };

        let app_name = app_entry.file_name();
        let source_app = app_entry.path();
        let dest_app = Path::new("/Applications").join(&app_name);

        // Remove existing app if present
        if dest_app.exists() {
            log::info!("[Installer] Removing existing app: {:?}", dest_app);
            let _ = std::fs::remove_dir_all(&dest_app);
        }

        // Copy app to /Applications
        log::info!("[Installer] Copying {:?} to /Applications", app_name);
        let copy_output = Command::new("cp")
            .args(["-R", source_app.to_str().unwrap(), dest_app.to_str().unwrap()])
            .output()
            .map_err(|e| {
                cleanup();
                InstallError::ExecutionFailed(format!("Failed to copy app: {}", e))
            })?;

        if !copy_output.status.success() {
            let stderr = String::from_utf8_lossy(&copy_output.stderr);
            cleanup();
            return Err(InstallError::ExecutionFailed(format!("Failed to copy app: {}", stderr)));
        }

        // Unmount DMG
        cleanup();
        
        // Store the installed app path for restart
        // We'll use an environment variable to pass this to restart_app
        std::env::set_var("SMARTLAB_INSTALLED_APP_PATH", dest_app.to_str().unwrap());
        
        log::info!("[Installer] DMG installation completed successfully. App installed at: {:?}", dest_app);

        Ok(())
    }

    /// Install PKG on macOS
    #[cfg(target_os = "macos")]
    fn install_pkg(pkg_path: &Path) -> Result<(), InstallError> {
        let pkg_path_str = pkg_path.to_str()
            .ok_or_else(|| InstallError::InvalidPath("Path contains invalid UTF-8".to_string()))?;

        log::info!("[Installer] Installing PKG: {}", pkg_path_str);
        
        // Use installer command (requires admin privileges)
        let output = Command::new("installer")
            .args(["-pkg", pkg_path_str, "-target", "/"])
            .output()
            .map_err(|e| InstallError::ExecutionFailed(format!("Failed to run installer: {}", e)))?;

        if output.status.success() {
            log::info!("[Installer] PKG installation completed successfully");
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(InstallError::InstallerFailed {
                exit_code: output.status.code().unwrap_or(-1),
                message: stderr.to_string(),
            })
        }
    }

    /// Install .app bundle on macOS
    #[cfg(target_os = "macos")]
    fn install_app(app_path: &Path) -> Result<(), InstallError> {
        let app_name = app_path.file_name()
            .ok_or_else(|| InstallError::InvalidPath("Invalid app path".to_string()))?;
        
        let dest_app = Path::new("/Applications").join(app_name);

        // Remove existing app if present
        if dest_app.exists() {
            log::info!("[Installer] Removing existing app: {:?}", dest_app);
            std::fs::remove_dir_all(&dest_app)
                .map_err(|e| InstallError::ExecutionFailed(format!("Failed to remove existing app: {}", e)))?;
        }

        // Copy app to /Applications
        log::info!("[Installer] Copying {:?} to /Applications", app_name);
        let output = Command::new("cp")
            .args(["-R", app_path.to_str().unwrap(), dest_app.to_str().unwrap()])
            .output()
            .map_err(|e| InstallError::ExecutionFailed(format!("Failed to copy app: {}", e)))?;

        if output.status.success() {
            log::info!("[Installer] App installation completed successfully");
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(InstallError::ExecutionFailed(format!("Failed to copy app: {}", stderr)))
        }
    }

    /// Schedule restart after installation completes
    /// 
    /// # Arguments
    /// * `delay_seconds` - Delay before restart in seconds
    /// 
    /// # Returns
    /// * `Ok(())` - Restart scheduled successfully
    /// * `Err(InstallError)` - Failed to schedule restart
    #[cfg(target_os = "windows")]
    pub fn schedule_restart(delay_seconds: u32) -> Result<(), InstallError> {
        // Use Windows shutdown command with restart flag
        let output = Command::new("shutdown")
            .args(["/r", "/t", &delay_seconds.to_string()])
            .output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    Ok(())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(InstallError::RestartFailed(stderr.to_string()))
                }
            }
            Err(e) => Err(InstallError::RestartFailed(e.to_string())),
        }
    }

    /// Schedule restart (non-Windows stub)
    #[cfg(not(target_os = "windows"))]
    pub fn schedule_restart(_delay_seconds: u32) -> Result<(), InstallError> {
        Err(InstallError::RestartFailed(
            "Scheduled restart is not supported on this platform".to_string(),
        ))
    }
}


impl InstallerRunner {
    /// Restart the application immediately
    /// 
    /// This function spawns a new instance of the application and exits the current one.
    /// Command line arguments from the current instance are passed to the new instance.
    /// 
    /// # Returns
    /// * `Ok(())` - New instance spawned (current process should exit)
    /// * `Err(InstallError)` - Failed to restart
    /// 
    /// Requirements: 4.3
    pub fn restart_app() -> Result<(), InstallError> {
        let current_exe = std::env::current_exe().map_err(|e| {
            InstallError::RestartFailed(format!("Failed to get current executable path: {}", e))
        })?;

        // Get command line arguments (skip the first one which is the executable path)
        let args: Vec<String> = std::env::args().skip(1).collect();

        // Spawn new instance
        let mut command = Command::new(&current_exe);
        command.args(&args);

        // On Windows, use CREATE_NEW_PROCESS_GROUP to detach from current process
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
            const DETACHED_PROCESS: u32 = 0x00000008;
            command.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
        }

        command.spawn().map_err(|e| {
            InstallError::RestartFailed(format!("Failed to spawn new instance: {}", e))
        })?;

        Ok(())
    }

    /// Restart the application after a delay
    /// 
    /// This is useful when the installer needs time to complete before the app restarts.
    /// 
    /// # Arguments
    /// * `delay_ms` - Delay in milliseconds before restarting
    /// 
    /// # Returns
    /// * `Ok(())` - Restart initiated
    /// * `Err(InstallError)` - Failed to restart
    pub fn restart_app_delayed(delay_ms: u64) -> Result<(), InstallError> {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        Self::restart_app()
    }

    /// Run installer and restart the application
    /// 
    /// This is a convenience method that combines installation and restart.
    /// It runs the installer silently, waits for completion, then restarts the app.
    /// 
    /// # Arguments
    /// * `installer_path` - Path to the installer file
    /// * `installer_type` - Type of installer
    /// 
    /// # Returns
    /// * `Ok(())` - Installation and restart initiated
    /// * `Err(InstallError)` - Installation or restart failed
    pub fn install_and_restart(
        installer_path: &Path,
        installer_type: InstallerType,
    ) -> Result<(), InstallError> {
        // Run the installer
        Self::run_silent(installer_path, installer_type)?;
        
        // Small delay to ensure installer has finished
        std::thread::sleep(std::time::Duration::from_millis(500));
        
        // Restart the application
        Self::restart_app()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_installer_type_silent_args() {
        assert_eq!(
            InstallerType::Msi.silent_args(),
            vec!["/quiet", "/norestart"]
        );
        assert_eq!(InstallerType::NsisExe.silent_args(), vec!["/S"]);
        assert_eq!(
            InstallerType::InnoSetupExe.silent_args(),
            vec!["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"]
        );
        assert_eq!(
            InstallerType::Generic.silent_args(),
            vec!["/S", "/silent", "/quiet"]
        );
    }

    #[test]
    fn test_detect_installer_type_msi() {
        // Create a temporary MSI file for testing
        let temp_dir = std::env::temp_dir();
        let msi_path = temp_dir.join("test_installer.msi");
        
        // Create an empty file with .msi extension
        std::fs::write(&msi_path, b"dummy msi content").unwrap();
        
        let result = InstallerRunner::detect_installer_type(&msi_path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), InstallerType::Msi);
        
        // Cleanup
        let _ = std::fs::remove_file(&msi_path);
    }

    #[test]
    fn test_detect_installer_type_not_found() {
        let path = PathBuf::from("/nonexistent/installer.msi");
        let result = InstallerRunner::detect_installer_type(&path);
        
        assert!(result.is_err());
        match result.unwrap_err() {
            InstallError::InstallerNotFound(_) => {}
            _ => panic!("Expected InstallerNotFound error"),
        }
    }

    #[test]
    fn test_detect_installer_type_unknown_extension() {
        let temp_dir = std::env::temp_dir();
        let unknown_path = temp_dir.join("test_file.xyz");
        
        std::fs::write(&unknown_path, b"dummy content").unwrap();
        
        let result = InstallerRunner::detect_installer_type(&unknown_path);
        assert!(result.is_err());
        match result.unwrap_err() {
            InstallError::UnknownInstallerType(ext) => {
                assert_eq!(ext, "xyz");
            }
            _ => panic!("Expected UnknownInstallerType error"),
        }
        
        // Cleanup
        let _ = std::fs::remove_file(&unknown_path);
    }

    #[test]
    fn test_install_error_display() {
        let error = InstallError::InstallerNotFound("/path/to/installer.msi".to_string());
        assert!(format!("{}", error).contains("/path/to/installer.msi"));

        let error = InstallError::InstallerFailed {
            exit_code: 1603,
            message: "Fatal error during installation".to_string(),
        };
        assert!(format!("{}", error).contains("1603"));
        assert!(format!("{}", error).contains("Fatal error"));
    }

    #[test]
    fn test_install_error_serialization() {
        let error = InstallError::InstallerFailed {
            exit_code: 1,
            message: "Test error".to_string(),
        };
        
        let json = serde_json::to_string(&error).unwrap();
        let deserialized: InstallError = serde_json::from_str(&json).unwrap();
        
        assert_eq!(error, deserialized);
    }

    #[test]
    fn test_installer_type_serialization() {
        let installer_type = InstallerType::InnoSetupExe;
        
        let json = serde_json::to_string(&installer_type).unwrap();
        let deserialized: InstallerType = serde_json::from_str(&json).unwrap();
        
        assert_eq!(installer_type, deserialized);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_run_silent_non_windows() {
        let temp_dir = std::env::temp_dir();
        let installer_path = temp_dir.join("test.msi");
        std::fs::write(&installer_path, b"dummy").unwrap();
        
        let result = InstallerRunner::run_silent(&installer_path, InstallerType::Msi);
        assert!(result.is_err());
        match result.unwrap_err() {
            InstallError::ExecutionFailed(msg) => {
                assert!(msg.contains("not supported"));
            }
            _ => panic!("Expected ExecutionFailed error"),
        }
        
        let _ = std::fs::remove_file(&installer_path);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_schedule_restart_non_windows() {
        let result = InstallerRunner::schedule_restart(5);
        assert!(result.is_err());
        match result.unwrap_err() {
            InstallError::RestartFailed(msg) => {
                assert!(msg.contains("not supported"));
            }
            _ => panic!("Expected RestartFailed error"),
        }
    }

    #[test]
    fn test_restart_app_can_get_current_exe() {
        // Test that we can get the current executable path
        // This is a prerequisite for restart_app to work
        let current_exe = std::env::current_exe();
        assert!(current_exe.is_ok());
        let exe_path = current_exe.unwrap();
        assert!(exe_path.exists());
    }

    #[test]
    fn test_restart_error_display() {
        let error = InstallError::RestartFailed("Test restart failure".to_string());
        let display = format!("{}", error);
        assert!(display.contains("restart"));
        assert!(display.contains("Test restart failure"));
    }
}
