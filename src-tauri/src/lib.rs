use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::path::PathBuf;
use tauri::{State, Manager, AppHandle};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct ServerState {
    process: Mutex<Option<Child>>,
    info: Mutex<Option<ServerInfo>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    url: String,
    ip: String,
    port: u16,
}

fn get_local_ip() -> String {
    use std::net::UdpSocket;
    
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

fn get_resource_path(app: &AppHandle, resource: &str) -> Option<PathBuf> {
    app.path().resource_dir().ok().map(|p| p.join(resource))
}

#[tauri::command]
fn start_server(app: AppHandle, state: State<ServerState>) -> Result<ServerInfo, String> {
    let mut process_guard = state.process.lock().map_err(|e| e.to_string())?;
    
    // Check if already running
    if process_guard.is_some() {
        let info_guard = state.info.lock().map_err(|e| e.to_string())?;
        if let Some(info) = info_guard.as_ref() {
            return Ok(info.clone());
        }
    }

    // Get paths to bundled Node.js and server
    let node_path = get_resource_path(&app, "binaries/node/node.exe")
        .ok_or("Node.js not found in bundle")?;
    
    let server_path = get_resource_path(&app, "binaries/server/dist/index.js")
        .ok_or("Server not found in bundle")?;

    if !node_path.exists() {
        return Err(format!("Node.js not found at: {:?}", node_path));
    }

    if !server_path.exists() {
        return Err(format!("Server not found at: {:?}", server_path));
    }

    // Start the server
    let child = Command::new(&node_path)
        .arg(&server_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start server: {}", e))?;

    *process_guard = Some(child);

    // Wait for server to start
    std::thread::sleep(std::time::Duration::from_millis(2000));

    let ip = get_local_ip();
    let port = 3016u16;
    let info = ServerInfo {
        url: format!("ws://{}:{}", ip, port),
        ip,
        port,
    };

    let mut info_guard = state.info.lock().map_err(|e| e.to_string())?;
    *info_guard = Some(info.clone());

    Ok(info)
}

#[tauri::command]
fn stop_server(state: State<ServerState>) -> Result<(), String> {
    let mut process_guard = state.process.lock().map_err(|e| e.to_string())?;
    
    if let Some(mut child) = process_guard.take() {
        let _ = child.kill();
    }

    let mut info_guard = state.info.lock().map_err(|e| e.to_string())?;
    *info_guard = None;

    Ok(())
}

#[tauri::command]
fn get_server_info(state: State<ServerState>) -> Result<ServerInfo, String> {
    let info_guard = state.info.lock().map_err(|e| e.to_string())?;
    info_guard.clone().ok_or_else(|| "Server not running".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(ServerState::default())
        .invoke_handler(tauri::generate_handler![
            start_server,
            stop_server,
            get_server_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
