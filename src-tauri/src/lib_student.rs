// Student app - no server management needed
use std::process::Command;

#[tauri::command]
fn shutdown_computer() -> Result<(), String> {
    println!("[shutdown_computer] Command called");
    
    #[cfg(target_os = "windows")]
    {
        println!("[shutdown_computer] Windows: Executing shutdown /s /t 0");
        Command::new("shutdown")
            .args(["/s", "/t", "0"])
            .spawn()
            .map_err(|e| {
                let err_msg = format!("Failed to shutdown: {}", e);
                println!("[shutdown_computer] Error: {}", err_msg);
                err_msg
            })?;
        println!("[shutdown_computer] Windows: Shutdown command executed");
    }

    #[cfg(target_os = "macos")]
    {
        println!("[shutdown_computer] macOS: Attempting shutdown");
        // Try using osascript first (more reliable on macOS)
        let osascript_result = Command::new("osascript")
            .args(["-e", "tell application \"System Events\" to shut down"])
            .spawn();
        
        if osascript_result.is_ok() {
            println!("[shutdown_computer] macOS: Using osascript method");
            return Ok(());
        }
        
        // Fallback to shutdown command
        println!("[shutdown_computer] macOS: osascript failed, trying shutdown command");
        Command::new("shutdown")
            .args(["-h", "now"])
            .spawn()
            .map_err(|e| {
                let err_msg = format!("Failed to shutdown: {}", e);
                println!("[shutdown_computer] Error: {}", err_msg);
                err_msg
            })?;
        println!("[shutdown_computer] macOS: Shutdown command executed");
    }

    #[cfg(target_os = "linux")]
    {
        println!("[shutdown_computer] Linux: Executing shutdown -h now");
        Command::new("shutdown")
            .args(["-h", "now"])
            .spawn()
            .map_err(|e| {
                let err_msg = format!("Failed to shutdown: {}", e);
                println!("[shutdown_computer] Error: {}", err_msg);
                err_msg
            })?;
        println!("[shutdown_computer] Linux: Shutdown command executed");
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let err_msg = "Shutdown not supported on this platform".to_string();
        println!("[shutdown_computer] Error: {}", err_msg);
        return Err(err_msg);
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![shutdown_computer])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
