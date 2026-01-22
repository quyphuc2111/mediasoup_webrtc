// Student app - no server management needed
use std::process::Command;

#[tauri::command]
fn shutdown_computer() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("shutdown")
            .args(["/s", "/t", "0"])
            .spawn()
            .map_err(|e| format!("Failed to shutdown: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("shutdown")
            .args(["-h", "now"])
            .spawn()
            .map_err(|e| format!("Failed to shutdown: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("shutdown")
            .args(["-h", "now"])
            .spawn()
            .map_err(|e| format!("Failed to shutdown: {}", e))?;
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        return Err("Shutdown not supported on this platform".to_string());
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
