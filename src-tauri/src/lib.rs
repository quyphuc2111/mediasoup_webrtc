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

// Helper function to start server with Node.js
fn start_server_with_node(app: &AppHandle) -> Result<Child, String> {
    // Try to find Node.js - first bundled, then system
    let node_path = if cfg!(target_os = "windows") {
        // Windows: look for bundled node.exe
        let bundled_path = get_resource_path(app, "binaries/node/node.exe");
        if let Some(path) = bundled_path.as_ref() {
            if path.exists() {
                path.clone()
            } else {
                // Bundled path doesn't exist, try system node
                match Command::new("node").arg("--version").output() {
                    Ok(_) => {
                        println!("[Server] Using system Node.js");
                        PathBuf::from("node")
                    }
                    Err(_) => return Err("Node.js not found. Please install Node.js from https://nodejs.org/".to_string()),
                }
            }
        } else {
            // No bundled path, try system node
            match Command::new("node").arg("--version").output() {
                Ok(_) => {
                    println!("[Server] Using system Node.js");
                    PathBuf::from("node")
                }
                Err(_) => return Err("Node.js not found. Please install Node.js from https://nodejs.org/".to_string()),
            }
        }
    } else {
        // macOS/Linux: try bundled node first
        let bundled_path = get_resource_path(app, "binaries/node/bin/node")
            .or_else(|| get_resource_path(app, "binaries/node/node"));
        
        if let Some(path) = bundled_path.as_ref() {
            if path.exists() {
                path.clone()
            } else {
                // Bundled path doesn't exist, try system node
                match Command::new("node").arg("--version").output() {
                    Ok(_) => {
                        println!("[Server] Using system Node.js (bundled not found)");
                        PathBuf::from("node")
                    }
                    Err(_) => return Err("Node.js not found. Please install Node.js from https://nodejs.org/ or bundle it in the app.".to_string()),
                }
            }
        } else {
            // No bundled path, try system node
            match Command::new("node").arg("--version").output() {
                Ok(_) => {
                    println!("[Server] Using system Node.js");
                    PathBuf::from("node")
                }
                Err(_) => return Err("Node.js not found. Please install Node.js from https://nodejs.org/ or bundle it in the app.".to_string()),
            }
        }
    };

    // Check server path (dist/index.js)
    let server_path = get_resource_path(app, "binaries/server/dist/index.js")
        .ok_or("Server not found in bundle")?;

    if !server_path.exists() {
        return Err(format!("Server not found at: {:?}", server_path));
    }

    println!("[Server] Starting server with Node.js: {:?} {:?}", node_path, server_path);

    let mut command = Command::new(&node_path);
    command
        .arg(&server_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    
    // On Unix systems, detach from terminal to run in background
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    
    // On Windows, create no window
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command.spawn()
        .map_err(|e| format!("Failed to start server: {}", e))
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

    #[cfg(debug_assertions)]
    let child = {
        let mut root_dir = std::env::current_dir().map_err(|e| e.to_string())?;
        // If we are in src-tauri, go up one level to find mediasoup-server
        if root_dir.ends_with("src-tauri") {
            root_dir.pop();
        }
        
        let server_dir = root_dir.join("mediasoup-server");
        if !server_dir.exists() {
            return Err(format!("Server directory not found at: {:?}", server_dir));
        }

        let cmd_name = if cfg!(target_os = "windows") { "npm.cmd" } else { "npm" };

        let mut command = Command::new(cmd_name);
        command
            .args(["run", "dev"])
            .current_dir(server_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        
        // On Unix systems, detach from terminal to run in background
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        
        // On Windows, create no window
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        command.spawn()
            .map_err(|e| format!("Failed to start dev server: {}", e))?
    };

    #[cfg(not(debug_assertions))]
    let child = {
        // Priority 1: Try to use bundled mediasoup-server binary (no Node.js needed)
        let server_binary_name = if cfg!(target_os = "windows") {
            "binaries/server/mediasoup-server-win.exe"
        } else if cfg!(target_os = "macos") {
            "binaries/server/mediasoup-server-macos"
        } else {
            "binaries/server/mediasoup-server-linux"
        };

        let bundled_binary = get_resource_path(&app, server_binary_name);
        
        // Try bundled binary first
        if let Some(binary_path) = bundled_binary.as_ref() {
            if binary_path.exists() {
                // Use bundled binary directly (no Node.js needed)
                println!("[Server] Using bundled mediasoup-server binary: {:?}", binary_path);
                
                let mut command = Command::new(binary_path);
                command
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                
                // On Unix systems, detach from terminal to run in background
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    command.process_group(0);
                }
                
                // On Windows, create no window
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    const CREATE_NO_WINDOW: u32 = 0x08000000;
                    command.creation_flags(CREATE_NO_WINDOW);
                }

                command.spawn()
                    .map_err(|e| format!("Failed to start bundled server binary: {}", e))?
            } else {
                // Binary path doesn't exist, fallback to Node.js
                println!("[Server] Bundled binary not found at {:?}, using Node.js + dist/index.js", binary_path);
                start_server_with_node(&app)?
            }
        } else {
            // No bundled binary path, fallback to Node.js
            println!("[Server] Bundled binary path not found, using Node.js + dist/index.js");
            start_server_with_node(&app)?
        }
    };

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
