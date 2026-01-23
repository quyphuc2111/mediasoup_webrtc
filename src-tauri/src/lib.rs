use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use tauri::{State, Manager, AppHandle};
use serde::{Deserialize, Serialize};

mod udp_audio;
mod lan_discovery;
mod database;
mod audio_capture;

use database::{init_database, save_device, get_all_devices, delete_device, SavedDevice};
use lan_discovery::{discover_devices, respond_to_discovery, DiscoveredDevice};

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

#[derive(Default)]
pub struct DatabaseState {
    conn: Mutex<Option<rusqlite::Connection>>,
}

#[derive(Default)]
pub struct DiscoveryState {
    listener_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    is_running: Mutex<bool>,
}

pub struct AudioCaptureState {
    capture: Arc<Mutex<Option<audio_capture::AudioCapture>>>,
    is_running: Mutex<bool>,
    sender_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    should_stop: Arc<Mutex<bool>>,
}

impl Default for AudioCaptureState {
    fn default() -> Self {
        Self {
            capture: Arc::new(Mutex::new(None)),
            is_running: Mutex::new(false),
            sender_handle: Mutex::new(None),
            should_stop: Arc::new(Mutex::new(false)),
        }
    }
}

// UDP Audio Commands - RustDesk approach: capture audio in Rust and auto-send via UDP
#[tauri::command]
fn start_udp_audio_capture(
    ip: String,
    port: u16,
    state: State<AudioCaptureState>,
) -> Result<String, String> {
    let mut is_running = state.is_running.lock().map_err(|e| e.to_string())?;
    if *is_running {
        return Ok("Audio capture already running".to_string());
    }

    // Create audio capture
    let mut capture_guard = state.capture.lock().map_err(|e| e.to_string())?;
    let mut capture = audio_capture::AudioCapture::new()
        .map_err(|e| format!("Failed to create audio capture: {}", e))?;
    
    capture.start_capture()
        .map_err(|e| format!("Failed to start audio capture: {}", e))?;
    
    let sample_rate = capture.get_sample_rate();
    let channels = capture.get_channels();
    
    // Calculate frame size: 10ms of audio (like RustDesk)
    let frame_size = (sample_rate as usize / 100) * channels as usize;
    
    *capture_guard = Some(capture);
    *is_running = true;
    
    // Create background task to send audio via UDP
    let capture_arc = Arc::clone(&state.capture);
    let should_stop = Arc::clone(&state.should_stop);
    *should_stop.lock().unwrap() = false;
    
    // Clone values for move into closure
    let ip_clone = ip.clone();
    let port_clone = port;
    
    let sender_handle = std::thread::spawn(move || {
        let socket = match std::net::UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to bind UDP socket for audio: {}", e);
                return;
            }
        };
        
        let addr = format!("{}:{}", ip_clone, port_clone);
        log::info!("Starting audio sender to {}", addr);
        
        loop {
            // Check if should stop
            if *should_stop.lock().unwrap() {
                log::info!("Audio sender stopped");
                break;
            }
            
            // Read samples from buffer
            let samples = {
                let capture_guard = capture_arc.lock().unwrap();
                if let Some(capture) = capture_guard.as_ref() {
                    if capture.has_samples(frame_size) {
                        capture.read_samples(frame_size)
                    } else {
                        // Not enough samples yet, wait a bit
                        std::thread::sleep(std::time::Duration::from_millis(5));
                        continue;
                    }
                } else {
                    // Capture stopped
                    break;
                }
            };
            
            // Convert i16 samples to bytes (little-endian)
            let mut bytes = Vec::with_capacity(samples.len() * 2);
            for sample in samples {
                bytes.extend_from_slice(&sample.to_le_bytes());
            }
            
            // Send via UDP
            if let Err(e) = socket.send_to(&bytes, &addr) {
                log::warn!("Failed to send audio: {}", e);
            }
            
            // Sleep to maintain ~10ms frame rate
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });
    
    let mut sender_handle_guard = state.sender_handle.lock().map_err(|e| e.to_string())?;
    *sender_handle_guard = Some(sender_handle);
    
    Ok(format!("Audio capture started, sending to {}:{}", ip, port))
}

#[tauri::command]
fn stop_udp_audio_capture(
    state: State<AudioCaptureState>,
) -> Result<(), String> {
    // Stop sender thread
    {
        *state.should_stop.lock().map_err(|e| e.to_string())? = true;
        let mut sender_handle_guard = state.sender_handle.lock().map_err(|e| e.to_string())?;
        if let Some(handle) = sender_handle_guard.take() {
            handle.thread().unpark(); // Wake up thread to check should_stop
            let _ = handle.join(); // Wait for thread to finish
        }
    }
    
    // Stop audio capture
    let mut is_running = state.is_running.lock().map_err(|e| e.to_string())?;
    let mut capture_guard = state.capture.lock().map_err(|e| e.to_string())?;
    
    if let Some(mut capture) = capture_guard.take() {
        capture.stop_capture();
    }
    
    *is_running = false;
    Ok(())
}

#[tauri::command]
fn read_audio_samples(
    count: usize,
    state: State<AudioCaptureState>,
) -> Result<Vec<i16>, String> {
    let capture_guard = state.capture.lock().map_err(|e| e.to_string())?;
    
    if let Some(capture) = capture_guard.as_ref() {
        Ok(capture.read_samples(count))
    } else {
        Err("Audio capture not started".to_string())
    }
}

#[tauri::command]
fn send_udp_audio(ip: String, port: u16, audio_data: Vec<i16>) -> Result<(), String> {
    use std::net::UdpSocket;
    
    // Convert i16 to bytes (little-endian)
    let mut bytes = Vec::with_capacity(audio_data.len() * 2);
    for sample in audio_data {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Failed to bind: {}", e))?;
    
    let addr = format!("{}:{}", ip, port);
    socket.send_to(&bytes, &addr)
        .map_err(|e| format!("Failed to send: {}", e))?;
    
    Ok(())
}

// LAN Discovery Commands
#[tauri::command]
fn discover_lan_devices(port: u16, timeout_ms: u64) -> Result<Vec<DiscoveredDevice>, String> {
    discover_devices(port, timeout_ms)
}

#[tauri::command]
fn start_discovery_listener(
    name: String,
    port: u16,
    state: State<DiscoveryState>,
) -> Result<(), String> {
    // Check if already running
    let mut is_running = state.is_running.lock().map_err(|e| e.to_string())?;
    if *is_running {
        return Ok(()); // Already running, return success
    }

    // Check if port is available
    let test_socket = std::net::UdpSocket::bind(format!("0.0.0.0:{}", port));
    if test_socket.is_err() {
        return Err(format!("Port {} is already in use or not available", port));
    }
    drop(test_socket); // Release the test socket

    let name_clone = name.clone();
    let port_clone = port;

    // Start in background thread
    let handle = std::thread::spawn(move || {
        // Set up signal handling for graceful shutdown
        let result = respond_to_discovery(&name_clone, port_clone);
        if let Err(e) = result {
            eprintln!("Discovery listener error: {}", e);
        }
    });

    // Store handle and mark as running
    let mut handle_guard = state.listener_handle.lock().map_err(|e| e.to_string())?;
    *handle_guard = Some(handle);
    *is_running = true;

    Ok(())
}

#[tauri::command]
fn stop_discovery_listener(state: State<DiscoveryState>) -> Result<(), String> {
    let mut is_running = state.is_running.lock().map_err(|e| e.to_string())?;
    if !*is_running {
        return Ok(()); // Not running, return success
    }

    let mut handle_guard = state.listener_handle.lock().map_err(|e| e.to_string())?;
    if let Some(handle) = handle_guard.take() {
        // Note: We can't easily stop the thread, but we mark it as stopped
        // The thread will continue until it errors or the app closes
        drop(handle);
    }

    *is_running = false;
    Ok(())
}

// Database Commands
#[tauri::command]
fn init_db(app: AppHandle, state: State<DatabaseState>) -> Result<(), String> {
    let conn = init_database(&app)
        .map_err(|e| format!("Failed to init database: {}", e))?;
    
    let mut db_state = state.conn.lock().map_err(|e| e.to_string())?;
    *db_state = Some(conn);
    
    Ok(())
}

#[tauri::command]
fn save_device_to_db(
    ip: String,
    name: String,
    port: u16,
    state: State<DatabaseState>,
) -> Result<i64, String> {
    let db_state = state.conn.lock().map_err(|e| e.to_string())?;
    let conn = db_state.as_ref().ok_or("Database not initialized")?;
    
    let device = SavedDevice {
        id: None,
        ip,
        name,
        port,
        last_used: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    
    save_device(conn, &device)
        .map_err(|e| format!("Failed to save device: {}", e))
}

#[tauri::command]
fn get_saved_devices(state: State<DatabaseState>) -> Result<Vec<SavedDevice>, String> {
    let db_state = state.conn.lock().map_err(|e| e.to_string())?;
    let conn = db_state.as_ref().ok_or("Database not initialized")?;
    
    get_all_devices(conn)
        .map_err(|e| format!("Failed to get devices: {}", e))
}

#[tauri::command]
fn remove_device_from_db(id: i64, state: State<DatabaseState>) -> Result<(), String> {
    let db_state = state.conn.lock().map_err(|e| e.to_string())?;
    let conn = db_state.as_ref().ok_or("Database not initialized")?;
    
    delete_device(conn, id)
        .map_err(|e| format!("Failed to delete device: {}", e))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(ServerState::default())
        .manage(DatabaseState::default())
        .manage(DiscoveryState::default())
        .manage(AudioCaptureState::default())
        .invoke_handler(tauri::generate_handler![
            start_server,
            stop_server,
            get_server_info,
            start_udp_audio_capture,
            stop_udp_audio_capture,
            read_audio_samples,
            send_udp_audio,
            discover_lan_devices,
            start_discovery_listener,
            stop_discovery_listener,
            init_db,
            save_device_to_db,
            get_saved_devices,
            remove_device_from_db
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
