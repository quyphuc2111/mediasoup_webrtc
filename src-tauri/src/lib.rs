#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

mod audio_capture;
mod auto_update;
mod autostart;
mod crypto;
mod database;
mod document_distribution;
mod file_transfer;
mod h264_decoder;
mod h264_encoder;
mod lan_discovery;
mod ldap_auth;
mod screen_capture;
mod student_agent;
mod student_auto_connect;
mod student_tray;
mod teacher_connector;
mod udp_audio;
mod udp_frame_transport;

use crypto::{KeyPairInfo, VerifyResult};
use document_distribution::{Document, DocumentServerState};
use file_transfer::FileTransferState;
use student_agent::{AgentConfig, AgentState, AgentStatus};
use teacher_connector::{ConnectorState, StudentConnection};

use database::{delete_device, get_all_devices, init_database, save_device, SavedDevice, authenticate_user, get_all_users, UserAccount, LoginResponse};
use lan_discovery::{discover_devices, respond_to_discovery, DiscoveredDevice};

#[derive(Default)]
pub struct ServerState {
    process: Mutex<Option<Child>>,
    info: Mutex<Option<ServerInfo>>,
}

/// Global AppHandle for logging (set during app initialization)
static APP_HANDLE: Mutex<Option<AppHandle>> = Mutex::new(None);

/// Initialize logging system with AppHandle
pub fn init_logging(app: AppHandle) {
    *APP_HANDLE.lock().unwrap() = Some(app);
}

/// Log to both console and frontend DebugPanel
pub fn log_debug(level: &str, message: &str) {
    // Always log to console
    match level {
        "error" => eprintln!("[{}] {}", level.to_uppercase(), message),
        "warn" => println!("[{}] {}", level.to_uppercase(), message),
        _ => println!("[{}] {}", level.to_uppercase(), message),
    }

    // Emit to frontend if AppHandle is available
    if let Ok(handle) = APP_HANDLE.lock() {
        if let Some(app) = handle.as_ref() {
            let _ = app.emit(
                "debug-log",
                serde_json::json!({
                    "level": level,
                    "message": message,
                    "timestamp": std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis())
                        .unwrap_or(0),
                }),
            );
        }
    }
}

/// Macro for easy logging (use this instead of println!)
/// Usage: log_debug!("info", "message {}", arg);
#[macro_export]
macro_rules! log_debug {
    ($level:expr, $($arg:tt)*) => {
        $crate::log_debug($level, &format!($($arg)*));
    };
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
fn start_server(#[allow(unused)] app: AppHandle, state: State<ServerState>) -> Result<ServerInfo, String> {
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
        // If we are in src-tauri, go up one level to find mediasoup-rust-server
        if root_dir.ends_with("src-tauri") {
            root_dir.pop();
        }

        let rust_server_dir = root_dir.join("mediasoup-rust-server");
        if !rust_server_dir.exists() {
            return Err(format!("Rust server directory not found at: {:?}", rust_server_dir));
        }

        println!("[Server] Starting Rust mediasoup server from: {:?}", rust_server_dir);
        
        let mut command = Command::new("cargo");
        command
            .args(["run", "--release"])
            .current_dir(&rust_server_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        command
            .spawn()
            .map_err(|e| format!("Failed to start Rust server: {}", e))?
    };

    #[cfg(not(debug_assertions))]
    let child = {
        let rust_binary_name = if cfg!(target_os = "windows") {
            "binaries/server/mediasoup-rust-server.exe"
        } else {
            "binaries/server/mediasoup-rust-server"
        };

        let binary_path = get_resource_path(&app, rust_binary_name)
            .ok_or("Mediasoup server binary path not found")?;

        if !binary_path.exists() {
            return Err(format!("Mediasoup server binary not found at: {:?}", binary_path));
        }

        println!("[Server] Starting Rust mediasoup server: {:?}", binary_path);

        let mut command = Command::new(&binary_path);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        command
            .spawn()
            .map_err(|e| format!("Failed to start server: {}", e))?
    };

    *process_guard = Some(child);

    // Wait for server to start (Rust server starts faster than Node.js)
    std::thread::sleep(std::time::Duration::from_millis(1000));

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
    info_guard
        .clone()
        .ok_or_else(|| "Server not running".to_string())
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

    log::info!("[UDP Audio] Starting audio capture...");
    capture
        .start_capture()
        .map_err(|e| format!("Failed to start audio capture: {}", e))?;

    let sample_rate = capture.get_sample_rate();
    let channels = capture.get_channels();

    // Calculate frame size: 10ms of audio (like RustDesk)
    let frame_size = (sample_rate as usize / 100) * channels as usize;
    log::info!(
        "[UDP Audio] Audio capture started: {}Hz, {} channels, frame_size: {}",
        sample_rate,
        channels,
        frame_size
    );

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
        log::info!("[UDP Audio] Starting audio sender thread to {}", addr);

        let mut sent_count = 0u64;
        let mut empty_count = 0u64;

        loop {
            // Check if should stop
            if *should_stop.lock().unwrap() {
                log::info!(
                    "[UDP Audio] Audio sender stopped (sent {} packets, {} empty)",
                    sent_count,
                    empty_count
                );
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
                        empty_count += 1;
                        if empty_count % 100 == 0 {
                            log::debug!(
                                "[UDP Audio] Waiting for samples... (empty_count: {})",
                                empty_count
                            );
                        }
                        std::thread::sleep(std::time::Duration::from_millis(5));
                        continue;
                    }
                } else {
                    // Capture stopped
                    log::warn!("[UDP Audio] Capture stopped, exiting sender thread");
                    break;
                }
            };

            // Convert i16 samples to bytes (little-endian)
            let mut bytes = Vec::with_capacity(samples.len() * 2);
            for sample in samples {
                bytes.extend_from_slice(&sample.to_le_bytes());
            }

            // Send via UDP
            match socket.send_to(&bytes, &addr) {
                Ok(_) => {
                    sent_count += 1;
                    if sent_count % 100 == 0 {
                        log::debug!(
                            "[UDP Audio] Sent {} packets ({} bytes each)",
                            sent_count,
                            bytes.len()
                        );
                    }
                }
                Err(e) => {
                    log::warn!(
                        "[UDP Audio] Failed to send audio packet #{}: {}",
                        sent_count,
                        e
                    );
                }
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
fn stop_udp_audio_capture(state: State<AudioCaptureState>) -> Result<(), String> {
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
fn read_audio_samples(count: usize, state: State<AudioCaptureState>) -> Result<Vec<i16>, String> {
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

    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Failed to bind: {}", e))?;

    let addr = format!("{}:{}", ip, port);
    socket
        .send_to(&bytes, &addr)
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
    let conn = init_database(&app).map_err(|e| format!("Failed to init database: {}", e))?;

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

    save_device(conn, &device).map_err(|e| format!("Failed to save device: {}", e))
}

#[tauri::command]
fn get_saved_devices(state: State<DatabaseState>) -> Result<Vec<SavedDevice>, String> {
    let db_state = state.conn.lock().map_err(|e| e.to_string())?;
    let conn = db_state.as_ref().ok_or("Database not initialized")?;

    get_all_devices(conn).map_err(|e| format!("Failed to get devices: {}", e))
}

#[tauri::command]
fn remove_device_from_db(id: i64, state: State<DatabaseState>) -> Result<(), String> {
    let db_state = state.conn.lock().map_err(|e| e.to_string())?;
    let conn = db_state.as_ref().ok_or("Database not initialized")?;

    delete_device(conn, id).map_err(|e| format!("Failed to delete device: {}", e))
}

// ============================================================
// User Authentication Commands
// ============================================================

/// Login with username and password
#[tauri::command]
fn login(username: String, password: String, state: State<DatabaseState>) -> Result<LoginResponse, String> {
    let db_state = state.conn.lock().map_err(|e| e.to_string())?;
    let conn = db_state.as_ref().ok_or("Database not initialized")?;
    
    Ok(authenticate_user(conn, &username, &password))
}

/// Get all users (admin only)
#[tauri::command]
fn get_users(state: State<DatabaseState>) -> Result<Vec<UserAccount>, String> {
    let db_state = state.conn.lock().map_err(|e| e.to_string())?;
    let conn = db_state.as_ref().ok_or("Database not initialized")?;
    
    get_all_users(conn).map_err(|e| format!("Failed to get users: {}", e))
}

// ============================================================
// Crypto Commands - View Client Authentication
// ============================================================

/// Generate a new keypair for teacher
#[tauri::command]
fn crypto_generate_keypair() -> Result<KeyPairInfo, String> {
    let keypair = crypto::generate_keypair();
    crypto::save_keypair(&keypair)?;
    Ok(keypair)
}

/// Load existing keypair
#[tauri::command]
fn crypto_load_keypair() -> Result<KeyPairInfo, String> {
    crypto::load_keypair()
}

/// Check if keypair exists
#[tauri::command]
fn crypto_has_keypair() -> bool {
    crypto::has_keypair()
}

/// Export public key in shareable format
#[tauri::command]
fn crypto_export_public_key() -> Result<String, String> {
    let keypair = crypto::load_keypair()?;
    crypto::export_public_key(&keypair.public_key)
}

/// Import teacher's public key (for students)
#[tauri::command]
fn crypto_import_teacher_key(key_data: String) -> Result<String, String> {
    let normalized = crypto::import_public_key(&key_data)?;
    crypto::save_teacher_public_key(&normalized)?;
    Ok(normalized)
}

/// Load teacher's public key (for students)
#[tauri::command]
fn crypto_load_teacher_key() -> Result<String, String> {
    crypto::load_teacher_public_key()
}

/// Check if teacher's public key exists (for students)
#[tauri::command]
fn crypto_has_teacher_key() -> bool {
    crypto::has_teacher_public_key()
}

/// Sign a challenge (for teacher connecting to student)
#[tauri::command]
fn crypto_sign_challenge(challenge_base64: String) -> Result<String, String> {
    let keypair = crypto::load_keypair()?;
    let challenge = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &challenge_base64,
    )
    .map_err(|e| format!("Invalid challenge base64: {}", e))?;

    crypto::sign_challenge(&keypair.private_key, &challenge)
}

/// Verify a signature (for student verifying teacher)
#[tauri::command]
fn crypto_verify_signature(
    challenge_base64: String,
    signature_base64: String,
) -> Result<VerifyResult, String> {
    let public_key = crypto::load_teacher_public_key()?;
    let challenge = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &challenge_base64,
    )
    .map_err(|e| format!("Invalid challenge base64: {}", e))?;

    Ok(crypto::verify_signature(
        &public_key,
        &challenge,
        &signature_base64,
    ))
}

/// Generate a random challenge
#[tauri::command]
fn crypto_generate_challenge() -> String {
    let challenge = crypto::generate_challenge();
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &challenge)
}

// ============================================================
// Authentication Mode Commands
// ============================================================

/// Set authentication mode (Ed25519 or LDAP)
#[tauri::command]
fn auth_set_mode(mode: crypto::AuthMode) -> Result<(), String> {
    crypto::save_auth_mode(mode)
}

/// Get current authentication mode
#[tauri::command]
fn auth_get_mode() -> crypto::AuthMode {
    crypto::load_auth_mode()
}

// ============================================================
// LDAP Authentication Commands
// ============================================================

/// Save LDAP configuration
#[tauri::command]
fn ldap_save_config(config: ldap_auth::LdapConfig) -> Result<(), String> {
    ldap_auth::save_ldap_config(&config)
}

/// Load LDAP configuration
#[tauri::command]
fn ldap_load_config() -> Result<ldap_auth::LdapConfig, String> {
    ldap_auth::load_ldap_config()
}

/// Test LDAP connection
#[tauri::command]
async fn ldap_test_connection(config: ldap_auth::LdapConfig) -> Result<String, String> {
    ldap_auth::test_ldap_connection(&config).await
}

/// Authenticate user with LDAP
#[tauri::command]
async fn ldap_authenticate(
    config: ldap_auth::LdapConfig,
    username: String,
    password: String,
) -> ldap_auth::LdapAuthResult {
    ldap_auth::authenticate_ldap(&config, &username, &password).await
}

// ============================================================
// Student Agent Commands
// ============================================================

/// Global agent state handle for async operations
static AGENT_RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

fn get_agent_runtime() -> &'static tokio::runtime::Runtime {
    AGENT_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Failed to create agent runtime")
    })
}

/// Start the student agent
#[tauri::command]
fn start_student_agent(
    app: AppHandle,
    port: u16,
    student_name: String,
    agent_state: State<Arc<AgentState>>,
    transfer_state: State<Arc<FileTransferState>>,
) -> Result<(), String> {
    // Update config
    {
        let mut config = agent_state.config.lock().map_err(|e| e.to_string())?;
        config.port = port;
        config.student_name = student_name;
    }

    // Reset auto-connect stop flag
    agent_state.auto_connect_stop.store(false, Ordering::Relaxed);

    // Start file receiver for chunked transfers
    let transfer_state_clone = Arc::clone(&transfer_state);
    let app_clone = app.clone();
    get_agent_runtime().spawn(async move {
        if let Err(e) = file_transfer::start_file_receiver(
            transfer_state_clone,
            app_clone,
            port,
        ).await {
            log::error!("[FileTransfer] Failed to start file receiver: {}", e);
        }
    });

    // Start agent in background
    let state_clone = Arc::clone(&agent_state);
    get_agent_runtime().spawn(async move {
        if let Err(e) = student_agent::start_agent(state_clone).await {
            log::error!("[StudentAgent] Error: {}", e);
        }
    });

    // Start auto-connect service to find teacher
    let auto_connect_config = student_auto_connect::AutoConnectConfig {
        teacher_port: 3018, // Teacher discovery port (different from student agent port)
        retry_interval_secs: 10,
        discovery_timeout_ms: 3000,
    };
    let stop_flag = Arc::clone(&agent_state.auto_connect_stop);
    get_agent_runtime().spawn(async move {
        if let Err(e) = student_auto_connect::start_auto_connect(auto_connect_config, stop_flag).await {
            log::error!("[StudentAutoConnect] Error: {}", e);
        }
    });

    log::info!("[StudentAgent] Started with auto-connect enabled");

    Ok(())
}

/// Stop the student agent
#[tauri::command]
fn stop_student_agent(state: State<Arc<AgentState>>) -> Result<(), String> {
    // Stop auto-connect service
    state.auto_connect_stop.store(true, Ordering::Relaxed);
    log::info!("[StudentAgent] Stopping auto-connect service");
    
    // Stop agent
    student_agent::stop_agent(&state)
}

/// Quit the application (for student tray)
#[tauri::command]
fn quit_app(app: AppHandle) -> Result<(), String> {
    log::info!("[App] Quit requested");
    app.exit(0);
    Ok(())
}

// ============================================================
// Remote Login Command (via SmartlabService on student machine)
// ============================================================

/// Send remote login command to a student machine's service
/// The service runs at boot level and can login users before they're logged in
#[tauri::command]
async fn remote_login_student(
    ip: String,
    username: String,
    password: String,
    domain: Option<String>,
) -> Result<String, String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpStream;

    let addr = format!("{}:3019", ip);
    log::info!("[RemoteLogin] Connecting to service at {}", addr);

    let stream = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        TcpStream::connect(&addr),
    )
    .await
    .map_err(|_| format!("Connection to student service at {} timed out (5s)", addr))?
    .map_err(|e| format!("Cannot connect to student service at {}: {}", addr, e))?;

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Send login command
    let cmd = serde_json::json!({
        "command": "login",
        "username": username,
        "password": password,
        "domain": domain,
    });
    let mut cmd_str = serde_json::to_string(&cmd).map_err(|e| e.to_string())?;
    cmd_str.push('\n');

    writer.write_all(cmd_str.as_bytes()).await
        .map_err(|e| format!("Failed to send command: {}", e))?;

    // Read response
    let mut response_line = String::new();
    reader.read_line(&mut response_line).await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    let resp: serde_json::Value = serde_json::from_str(response_line.trim())
        .map_err(|e| format!("Invalid response: {}", e))?;

    if resp["success"].as_bool().unwrap_or(false) {
        Ok(resp["message"].as_str().unwrap_or("OK").to_string())
    } else {
        Err(resp["message"].as_str().unwrap_or("Unknown error").to_string())
    }
}

/// Ping a student machine's service to check if it's reachable (even before login)
#[tauri::command]
async fn ping_student_service(ip: String) -> Result<serde_json::Value, String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpStream;

    let addr = format!("{}:3019", ip);

    let stream = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        TcpStream::connect(&addr),
    )
    .await
    .map_err(|_| format!("Service not reachable at {} (timed out after 5s)", addr))?
    .map_err(|e| format!("Service not reachable at {}: {}", addr, e))?;

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Send status command
    let cmd = "{\"command\":\"status\"}\n";
    writer.write_all(cmd.as_bytes()).await
        .map_err(|e| format!("Failed to send: {}", e))?;

    let mut response_line = String::new();
    reader.read_line(&mut response_line).await
        .map_err(|e| format!("Failed to read: {}", e))?;

    serde_json::from_str(response_line.trim())
        .map_err(|e| format!("Invalid response: {}", e))
}

/// Get current agent status
#[tauri::command]
fn get_agent_status(state: State<Arc<AgentState>>) -> AgentStatus {
    state.get_status()
}

/// Get agent configuration
#[tauri::command]
fn get_agent_config(state: State<Arc<AgentState>>) -> Result<AgentConfig, String> {
    state
        .config
        .lock()
        .map(|c| c.clone())
        .map_err(|e| e.to_string())
}

// ============================================================
// Teacher Connector Commands
// ============================================================

/// Global connector runtime for async WebSocket operations
static CONNECTOR_RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

fn get_connector_runtime() -> &'static tokio::runtime::Runtime {
    CONNECTOR_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .expect("Failed to create connector runtime")
    })
}

/// Connect to a student agent
#[tauri::command]
fn connect_to_student(
    app: AppHandle,
    ip: String,
    port: u16,
    state: State<'_, Arc<ConnectorState>>,
) -> Result<String, String> {
    // Generate connection ID
    let id = format!("{}:{}", ip, port);

    // Check if already connected
    if let Some(conn) = state.get_connection(&id) {
        if conn.status != teacher_connector::ConnectionStatus::Disconnected
            && !matches!(
                conn.status,
                teacher_connector::ConnectionStatus::Error { .. }
            )
        {
            return Err("Already connected to this student".to_string());
        }
    }

    // Check if we have a keypair
    if !crypto::has_keypair() {
        return Err("No keypair found. Please generate one first.".to_string());
    }

    // Create connection entry with Connecting status
    let connection = teacher_connector::StudentConnection {
        id: id.clone(),
        ip: ip.clone(),
        port,
        name: None,
        status: teacher_connector::ConnectionStatus::Connecting,
        current_version: None,
        machine_name: None,
        update_status: None,
    };

    // Store connection
    {
        let mut conns = state.connections.lock().map_err(|e| e.to_string())?;
        conns.insert(id.clone(), connection);
    }

    // Spawn connection in dedicated runtime
    let state_clone = Arc::clone(&state);
    let ip_clone = ip.clone();
    let id_clone = id.clone();
    let app_handle = app.clone();

    println!("[TeacherConnector] Spawning connection to {}:{}", ip, port);

    get_connector_runtime().spawn(async move {
        println!(
            "[TeacherConnector] Starting WebSocket connection to {}:{}",
            ip_clone, port
        );
        match teacher_connector::handle_connection_async(
            state_clone,
            id_clone.clone(),
            ip_clone.clone(),
            port,
            app_handle,
        )
        .await
        {
            Ok(()) => println!(
                "[TeacherConnector] Connection to {}:{} closed normally",
                ip_clone, port
            ),
            Err(e) => println!(
                "[TeacherConnector] Connection to {}:{} failed: {}",
                ip_clone, port, e
            ),
        }
    });

    Ok(id)
}

/// Disconnect from a student
#[tauri::command]
fn disconnect_from_student(
    connection_id: String,
    state: State<Arc<ConnectorState>>,
) -> Result<(), String> {
    teacher_connector::disconnect_student(&state, &connection_id)
}

/// Request screen from a student
#[tauri::command]
fn request_student_screen(
    connection_id: String,
    state: State<Arc<ConnectorState>>,
) -> Result<(), String> {
    teacher_connector::request_screen(&state, &connection_id)
}

/// Stop viewing student screen
#[tauri::command]
fn stop_student_screen(
    connection_id: String,
    state: State<Arc<ConnectorState>>,
) -> Result<(), String> {
    teacher_connector::stop_screen(&state, &connection_id)
}

/// Get all student connections
#[tauri::command]
fn get_student_connections(state: State<Arc<ConnectorState>>) -> Vec<StudentConnection> {
    state.get_all_connections()
}

/// Get a single student connection
#[tauri::command]
fn get_student_connection(
    connection_id: String,
    state: State<Arc<ConnectorState>>,
) -> Option<StudentConnection> {
    state.get_connection(&connection_id)
}

/// Get the latest screen frame for a student
#[tauri::command]
fn get_student_screen_frame(
    connection_id: String,
    state: State<Arc<ConnectorState>>,
) -> Option<teacher_connector::ScreenFrame> {
    teacher_connector::get_screen_frame(&state, &connection_id)
}

/// Send mouse input to a student (remote control)
#[tauri::command]
fn send_remote_mouse_event(
    student_id: String,
    event: teacher_connector::MouseInputEvent,
    state: State<Arc<ConnectorState>>,
) -> Result<(), String> {
    teacher_connector::send_mouse_input(&state, &student_id, event)
}

/// Send keyboard input to a student (remote control)
#[tauri::command]
fn send_remote_keyboard_event(
    student_id: String,
    event: teacher_connector::KeyboardInputEvent,
    state: State<Arc<ConnectorState>>,
) -> Result<(), String> {
    teacher_connector::send_keyboard_input(&state, &student_id, event)
}

#[tauri::command]
fn send_remote_keyframe_request(
    connection_id: String,
    state: State<Arc<ConnectorState>>,
) -> Result<(), String> {
    teacher_connector::request_keyframe(&state, &connection_id)
}

/// Get the transport protocol for a connection ("udp" or "websocket")
#[tauri::command]
fn get_transport_protocol(
    connection_id: String,
    state: State<Arc<ConnectorState>>,
) -> String {
    state.transport_protocols
        .lock()
        .ok()
        .and_then(|protos| protos.get(&connection_id).cloned())
        .unwrap_or_else(|| "websocket".to_string())
}

/// Send shutdown command to a student
#[tauri::command]
fn send_shutdown_command(
    student_id: String,
    delay_seconds: Option<u32>,
    state: State<Arc<ConnectorState>>,
) -> Result<(), String> {
    teacher_connector::send_shutdown(&state, &student_id, delay_seconds)
}

/// Send restart command to a student
#[tauri::command]
fn send_restart_command(
    student_id: String,
    delay_seconds: Option<u32>,
    state: State<Arc<ConnectorState>>,
) -> Result<(), String> {
    teacher_connector::send_restart(&state, &student_id, delay_seconds)
}

/// Send lock screen command to a student
#[tauri::command]
fn send_lock_screen_command(
    student_id: String,
    state: State<Arc<ConnectorState>>,
) -> Result<(), String> {
    teacher_connector::send_lock_screen(&state, &student_id)
}

/// Send logout command to a student
#[tauri::command]
fn send_logout_command(
    student_id: String,
    state: State<Arc<ConnectorState>>,
) -> Result<(), String> {
    teacher_connector::send_logout(&state, &student_id)
}

/// Start teacher discovery service to respond to student auto-connect requests
#[tauri::command]
fn start_teacher_discovery(
    app: AppHandle,
    teacher_name: String,
    connector_state: State<Arc<ConnectorState>>,
) -> Result<(), String> {
    log::info!("[TeacherDiscovery] Starting unified discovery service as '{}'", teacher_name);
    
    let state = Arc::clone(&connector_state);
    
    // Start the unified teacher auto-connect (listens for student announcements + active scanning)
    std::thread::spawn(move || {
        let on_student_found = move |ip: String, port: u16| {
            log::info!("[TeacherDiscovery] Auto-connecting to student at {}:{}", ip, port);
            
            let state_clone = Arc::clone(&state);
            let app_clone = app.clone();
            let ip_clone = ip.clone();
            
            get_connector_runtime().spawn(async move {
                let id = format!("{}:{}", ip_clone, port);
                
                // Skip if already connected
                if let Some(conn) = state_clone.get_connection(&id) {
                    if conn.status != teacher_connector::ConnectionStatus::Disconnected
                        && !matches!(conn.status, teacher_connector::ConnectionStatus::Error { .. })
                    {
                        log::info!("[TeacherDiscovery] Already connected to {}", id);
                        return;
                    }
                }
                
                let connection = teacher_connector::StudentConnection {
                    id: id.clone(),
                    ip: ip_clone.clone(),
                    port,
                    name: None,
                    status: teacher_connector::ConnectionStatus::Connecting,
                    current_version: None,
                    machine_name: None,
                    update_status: None,
                };
                
                if let Ok(mut conns) = state_clone.connections.lock() {
                    conns.insert(id.clone(), connection);
                }
                
                log::info!("[TeacherDiscovery] Connecting to {}:{}", ip_clone, port);
                match teacher_connector::handle_connection_async(
                    state_clone, id.clone(), ip_clone.clone(), port, app_clone,
                ).await {
                    Ok(()) => log::info!("[TeacherDiscovery] Connection to {} closed", id),
                    Err(e) => log::error!("[TeacherDiscovery] Connection to {} failed: {}", id, e),
                }
            });
        };
        
        // Use the new unified discovery system
        if let Err(e) = lan_discovery::run_teacher_auto_connect(
            &teacher_name,
            lan_discovery::DISCOVERY_PORT,
            on_student_found,
        ) {
            log::error!("[TeacherDiscovery] Discovery service error: {}", e);
        }
    });
    
    Ok(())
}

/// Send file to a student (chunked TCP transfer)
#[tauri::command]
async fn send_file_to_student(
    app: AppHandle,
    student_id: String,
    file_path: String,
    connector_state: State<'_, Arc<ConnectorState>>,
    transfer_state: State<'_, Arc<FileTransferState>>,
) -> Result<String, String> {
    // Get student connection info
    let conn = connector_state.get_connection(&student_id)
        .ok_or_else(|| "Student not connected".to_string())?;
    
    // Send file via chunked TCP
    file_transfer::send_file_chunked(
        Arc::clone(&transfer_state),
        app,
        conn.ip,
        conn.port,
        file_path,
        student_id,
    ).await
}

/// Cancel a file transfer
#[tauri::command]
fn cancel_file_transfer(
    job_id: String,
    state: State<Arc<FileTransferState>>,
) -> Result<bool, String> {
    Ok(state.cancel_job(&job_id))
}

/// Get file transfer job status
#[tauri::command]
fn get_file_transfer_status(
    job_id: String,
    state: State<Arc<FileTransferState>>,
) -> Result<Option<file_transfer::FileTransferJob>, String> {
    Ok(state.get_job(&job_id))
}

// ============================================================
// File Transfer Commands
// ============================================================

/// List files in a directory
#[tauri::command]
fn list_directory(path: String) -> Result<Vec<file_transfer::FileInfo>, String> {
    file_transfer::list_directory(&path)
}

/// Get home directory
#[tauri::command]
fn get_home_directory() -> Result<String, String> {
    file_transfer::get_home_directory()
}

/// Get desktop directory
#[tauri::command]
fn get_desktop_directory() -> Result<String, String> {
    file_transfer::get_desktop_directory()
}

/// Get documents directory
#[tauri::command]
fn get_documents_directory() -> Result<String, String> {
    file_transfer::get_documents_directory()
}

/// Read file as base64
#[tauri::command]
fn read_file_as_base64(path: String) -> Result<String, String> {
    file_transfer::read_file_as_base64(&path)
}

/// Write file from base64
#[tauri::command]
fn write_file_from_base64(path: String, data: String) -> Result<(), String> {
    file_transfer::write_file_from_base64(&path, &data)
}

/// Get file info
#[tauri::command]
fn get_file_info(path: String) -> Result<file_transfer::FileInfo, String> {
    file_transfer::get_file_info(&path)
}

/// Get student's directory listing
#[tauri::command]
async fn get_student_directory(
    student_id: String,
    path: String,
    state: State<'_, Arc<ConnectorState>>,
) -> Result<Vec<file_transfer::FileInfo>, String> {
    teacher_connector::list_student_directory(&state, &student_id, path).await
}

// ============================================================
// Document Distribution Commands
// ============================================================

/// Global document server runtime
static DOC_SERVER_RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

fn get_doc_server_runtime() -> &'static tokio::runtime::Runtime {
    DOC_SERVER_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Failed to create document server runtime")
    })
}

/// Start document distribution server
#[tauri::command]
fn start_document_server(
    port: u16,
    state: State<Arc<DocumentServerState>>,
) -> Result<String, String> {
    // Check if already running
    {
        let is_running = state.is_running.lock().map_err(|e| e.to_string())?;
        if *is_running {
            let server_port = state.server_port.lock().map_err(|e| e.to_string())?;
            return Ok(format!("Server already running on port {}", *server_port));
        }
    }
    
    let state_clone = Arc::clone(&state);
    let local_ip = get_local_ip();
    
    get_doc_server_runtime().spawn(async move {
        if let Err(e) = document_distribution::start_document_server(state_clone, port).await {
            log::error!("[DocumentServer] Error: {}", e);
        }
    });
    
    // Wait a bit for server to start
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    Ok(format!("http://{}:{}", local_ip, port))
}

/// Stop document distribution server
#[tauri::command]
fn stop_document_server(state: State<Arc<DocumentServerState>>) -> Result<(), String> {
    let mut is_running = state.is_running.lock().map_err(|e| e.to_string())?;
    *is_running = false;
    Ok(())
}

/// Get document server status
#[tauri::command]
fn get_document_server_status(state: State<Arc<DocumentServerState>>) -> Result<(bool, u16, String), String> {
    let is_running = *state.is_running.lock().map_err(|e| e.to_string())?;
    let port = *state.server_port.lock().map_err(|e| e.to_string())?;
    let url = if is_running {
        format!("http://{}:{}", get_local_ip(), port)
    } else {
        String::new()
    };
    Ok((is_running, port, url))
}

/// Upload a document
#[tauri::command]
async fn upload_document(
    name: String,
    data: Vec<u8>,
    description: Option<String>,
    category: Option<String>,
    state: State<'_, Arc<DocumentServerState>>,
) -> Result<Document, String> {
    document_distribution::save_document(
        Arc::clone(&state),
        name,
        data,
        description,
        category,
    ).await
}

/// Upload document from file path
#[tauri::command]
async fn upload_document_from_path(
    file_path: String,
    description: Option<String>,
    category: Option<String>,
    state: State<'_, Arc<DocumentServerState>>,
) -> Result<Document, String> {
    let path = std::path::PathBuf::from(&file_path);
    
    // Get filename
    let name = path.file_name()
        .ok_or("Invalid file path")?
        .to_string_lossy()
        .to_string();
    
    // Read file
    let data = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;
    
    document_distribution::save_document(
        Arc::clone(&state),
        name,
        data,
        description,
        category,
    ).await
}

/// Delete a document
#[tauri::command]
async fn delete_document(
    id: String,
    state: State<'_, Arc<DocumentServerState>>,
) -> Result<(), String> {
    document_distribution::delete_document(Arc::clone(&state), &id).await
}

/// List all documents
#[tauri::command]
fn list_documents(state: State<Arc<DocumentServerState>>) -> Vec<Document> {
    state.list_documents()
}

/// Get a single document
#[tauri::command]
fn get_document(id: String, state: State<Arc<DocumentServerState>>) -> Option<Document> {
    state.get_document(&id)
}

/// Download document from URL to Downloads folder or custom folder
#[tauri::command]
async fn download_document_to_downloads(
    url: String,
    filename: String,
    custom_folder: Option<String>,
) -> Result<String, String> {
    // Get target directory
    let target_dir = if let Some(folder) = custom_folder {
        std::path::PathBuf::from(folder)
    } else {
        dirs::download_dir()
            .ok_or_else(|| "Failed to get Downloads directory".to_string())?
    };
    
    // Create directory if not exists
    if !target_dir.exists() {
        tokio::fs::create_dir_all(&target_dir)
            .await
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    
    // Create file path
    let mut file_path = target_dir.join(&filename);
    
    // Handle duplicate filenames
    let mut counter = 1;
    while file_path.exists() {
        let stem = std::path::Path::new(&filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file")
            .to_string();
        let ext = std::path::Path::new(&filename)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        
        let new_name = if ext.is_empty() {
            format!("{} ({})", stem, counter)
        } else {
            format!("{} ({}).{}", stem, counter, ext)
        };
        file_path = target_dir.join(new_name);
        counter += 1;
    }
    
    // Download file
    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to download: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }
    
    let bytes = response.bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    // Write to file
    tokio::fs::write(&file_path, bytes)
        .await
        .map_err(|e| format!("Failed to write file: {}", e))?;
    
    Ok(file_path.to_string_lossy().to_string())
}

// ============================================================
// Auto-Update Commands
// ============================================================

/// Check for updates from the Update API
/// Requirements: 2.1
#[tauri::command]
async fn check_for_updates(
    app: AppHandle,
    state: State<'_, Arc<auto_update::UpdateCoordinator>>,
) -> Result<Option<auto_update::UpdateInfo>, String> {
    state
        .check_for_updates(Some(&app))
        .await
        .map_err(|e| e.to_string())
}

/// Download the update package
/// Requirements: 3.1
#[tauri::command]
async fn download_update(
    app: AppHandle,
    state: State<'_, Arc<auto_update::UpdateCoordinator>>,
) -> Result<String, String> {
    state
        .download_update(app)
        .await
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

/// Get the current update state
#[tauri::command]
fn get_update_state(
    state: State<Arc<auto_update::UpdateCoordinator>>,
) -> auto_update::UpdateState {
    state.get_state()
}

/// Install the verified update
/// Requirements: 4.1
#[tauri::command]
fn install_update(
    app: AppHandle,
    state: State<Arc<auto_update::UpdateCoordinator>>,
) -> Result<(), String> {
    state.install_update(Some(&app)).map_err(|e| e.to_string())
}

/// Restart the application for update
/// Requirements: 4.3
#[tauri::command]
fn restart_for_update(
    app: AppHandle,
    state: State<Arc<auto_update::UpdateCoordinator>>,
) -> Result<(), String> {
    state.restart_app(Some(&app)).map_err(|e| e.to_string())
}

/// Reset the update coordinator to idle state
#[tauri::command]
fn reset_update_state(
    app: AppHandle,
    state: State<Arc<auto_update::UpdateCoordinator>>,
) {
    state.reset(Some(&app));
}

/// Get update configuration
#[tauri::command]
fn get_update_config() -> auto_update::UpdateConfig {
    auto_update::load_config()
}

/// Save update configuration
#[tauri::command]
fn save_update_config(config: auto_update::UpdateConfig) -> Result<(), String> {
    auto_update::save_config(&config)
}

/// Get update config file path
#[tauri::command]
fn get_update_config_path() -> String {
    auto_update::get_config_path().to_string_lossy().to_string()
}

/// Get the downloaded update package path
#[tauri::command]
fn get_update_download_path(
    state: State<Arc<auto_update::UpdateCoordinator>>,
) -> Option<String> {
    let path = state.get_download_path();
    log::info!("[UpdateCoordinator] get_update_download_path: {:?}", path);
    path.map(|p| p.to_string_lossy().to_string())
}

/// Get the latest update info from coordinator (for when frontend state is lost)
#[tauri::command]
fn get_latest_update_info(
    state: State<Arc<auto_update::UpdateCoordinator>>,
) -> Option<auto_update::UpdateInfo> {
    let info = state.get_latest_info();
    log::info!("[UpdateCoordinator] get_latest_update_info: {:?}", info.as_ref().map(|i| &i.version));
    info
}

/// Check for Student app updates from API server
/// This is used by Teacher to get Student app update info for LAN distribution
#[tauri::command]
async fn check_student_update(
    state: State<'_, Arc<auto_update::UpdateCoordinator>>,
) -> Result<Option<auto_update::UpdateInfo>, String> {
    let config = state.get_config();
    let client = auto_update::UpdateApiClient::from_config(config);
    
    // Get Student app update info
    client
        .get_latest_version_for_app("student")
        .await
        .map_err(|e| e.to_string())
}

/// Download Student app update package for LAN distribution (Teacher only)
/// Returns the path to the downloaded file
#[tauri::command]
async fn download_student_package_for_lan(
    app: AppHandle,
    download_url: String,
    sha256: String,
) -> Result<String, String> {
    log::info!("[StudentPackage] Downloading from: {}", download_url);
    
    // Create temp directory for student updates
    let temp_dir = std::env::temp_dir().join("smartlab_student_updates_dist");
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| format!("Failed to create temp dir: {}", e))?;
    
    // Extract filename from URL
    let filename = download_url
        .split('/')
        .last()
        .unwrap_or("student_update.exe");
    let dest_path = temp_dir.join(filename);
    
    // Download with progress
    let downloader = auto_update::Downloader::new();
    let app_clone = app.clone();
    
    let progress_callback = Box::new(move |progress: auto_update::DownloadProgress| {
        let _ = app_clone.emit("student-package-download-progress", &progress);
    });
    
    let path = downloader
        .download(&download_url, &dest_path, None, Some(progress_callback))
        .await
        .map_err(|e| format!("Download failed: {}", e))?;
    
    // Verify hash
    auto_update::Verifier::verify_sha256(&path, &sha256)
        .map_err(|e| format!("Hash verification failed: {}", e))?;
    
    log::info!("[StudentPackage] Downloaded and verified: {:?}", path);
    
    Ok(path.to_string_lossy().to_string())
}

// ============================================================
// LAN Distribution Commands (Teacher only)
// ============================================================

/// Start the LAN distribution server to serve updates to students
/// Requirements: 7.1
#[tauri::command]
async fn start_lan_distribution(
    package_path: String,
    sha256: String,
    state: State<'_, Arc<auto_update::LanDistributionServer>>,
    connector_state: State<'_, Arc<ConnectorState>>,
) -> Result<String, String> {
    log::info!("[LanDistribution] Starting with package_path: {}", package_path);
    
    let path = std::path::PathBuf::from(&package_path);
    
    // Verify file exists
    if !path.exists() {
        log::error!("[LanDistribution] Package file not found: {}", package_path);
        return Err(format!("Package file not found: {}", package_path));
    }
    
    log::info!("[LanDistribution] Package file verified: {:?}", path);
    
    // Start the LAN server
    state
        .start(path, sha256.clone())
        .await
        .map_err(|e| e.to_string())?;
    
    // Get the download URL
    let url = state
        .get_download_url()
        .await
        .ok_or_else(|| "Failed to get download URL".to_string())?;
    
    // Update connector state with LAN distribution info
    connector_state.set_lan_distribution(Some(url.clone()), Some(sha256));
    
    log::info!("[LanDistribution] Server started, URL: {}", url);
    
    Ok(url)
}

/// Get the LAN distribution server URL if running
#[tauri::command]
async fn get_lan_distribution_url(
    state: State<'_, Arc<auto_update::LanDistributionServer>>,
) -> Result<Option<String>, String> {
    Ok(state.get_download_url().await)
}

/// Stop the LAN distribution server
/// Requirements: 7.5
#[tauri::command]
async fn stop_lan_distribution(
    state: State<'_, Arc<auto_update::LanDistributionServer>>,
    connector_state: State<'_, Arc<ConnectorState>>,
) -> Result<(), String> {
    // Clear LAN distribution info from connector state
    connector_state.set_lan_distribution(None, None);
    
    state.stop().await.map_err(|e| e.to_string())
}

/// Broadcast update required to all connected students
/// Requirements: 14.1, 14.2
#[tauri::command]
fn broadcast_update_to_students(
    required_version: String,
    update_url: String,
    sha256: Option<String>,
    state: State<Arc<ConnectorState>>,
) -> Result<teacher_connector::BroadcastResult, String> {
    teacher_connector::broadcast_update_required(&state, required_version, update_url, sha256)
}

/// Send update required to a specific student
/// Requirements: 14.1, 14.2
#[tauri::command]
fn send_update_to_student(
    student_id: String,
    required_version: String,
    update_url: String,
    sha256: Option<String>,
    state: State<Arc<ConnectorState>>,
) -> Result<(), String> {
    teacher_connector::send_update_required(&state, &student_id, required_version, update_url, sha256)
}

/// Check if all students have acknowledged the update
/// Requirements: 14.4
#[tauri::command]
fn check_update_acknowledgments(
    state: State<Arc<ConnectorState>>,
) -> (bool, Vec<String>) {
    let all_acked = state.all_students_acknowledged();
    let pending = state.get_pending_acknowledgments();
    (all_acked, pending)
}

/// Check if all students are up to date
/// Requirements: 14.5
#[tauri::command]
fn check_all_students_updated(
    state: State<Arc<ConnectorState>>,
) -> bool {
    state.all_students_up_to_date()
}

/// Get the status of connected students' updates
/// Requirements: 10.5, 10.6
#[tauri::command]
fn get_client_update_status(
    state: State<Arc<ConnectorState>>,
) -> Vec<teacher_connector::ClientUpdateStatus> {
    state.get_all_client_update_status()
}

// ============================================================
// Student Update Commands
// ============================================================

/// Get the current student update state
/// Requirements: 11.1, 11.2
#[tauri::command]
fn get_student_update_state(
    state: State<Arc<auto_update::StudentUpdateCoordinator>>,
) -> auto_update::StudentUpdateState {
    state.get_state()
}

/// Set update required for student (called when receiving update_required from teacher)
/// Requirements: 8.1
#[tauri::command]
fn set_student_update_required(
    required_version: String,
    update_url: Option<String>,
    sha256: Option<String>,
    app: AppHandle,
    state: State<Arc<auto_update::StudentUpdateCoordinator>>,
) {
    state.set_update_required(required_version, update_url, sha256, Some(&app));
}

/// Download update from Teacher's LAN server
/// Requirements: 8.1, 8.3
#[tauri::command]
async fn download_student_update(
    app: AppHandle,
    state: State<'_, Arc<auto_update::StudentUpdateCoordinator>>,
) -> Result<String, String> {
    state
        .download_with_retry(app)
        .await
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

/// Retry student update download after failure
/// Requirements: 8.5, 11.4
#[tauri::command]
async fn retry_student_update(
    app: AppHandle,
    state: State<'_, Arc<auto_update::StudentUpdateCoordinator>>,
) -> Result<String, String> {
    state
        .retry_download(app)
        .await
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

/// Install the student update
/// Requirements: 9.1
#[tauri::command]
async fn install_student_update(
    app: AppHandle,
    state: State<'_, Arc<auto_update::StudentUpdateCoordinator>>,
) -> Result<(), String> {
    log::info!("[StudentUpdate] Starting installation...");
    
    // Get the download path
    let download_path = match state.start_install(Some(&app)) {
        Ok(path) => path,
        Err(e) => {
            log::error!("[StudentUpdate] Failed to start install: {}", e);
            return Err(e.to_string());
        }
    };

    log::info!("[StudentUpdate] Download path: {:?}", download_path);

    // Detect installer type
    let installer_type = match auto_update::InstallerRunner::detect_installer_type(&download_path) {
        Ok(t) => t,
        Err(e) => {
            log::error!("[StudentUpdate] Failed to detect installer type: {}", e);
            // Transition to Failed state
            state.transition_to_failed(e.to_string(), Some(&app));
            return Err(e.to_string());
        }
    };

    log::info!("[StudentUpdate] Installer type: {:?}", installer_type);

    // Run the installer in a blocking task to avoid blocking the async runtime
    let download_path_clone = download_path.clone();
    let install_result = tokio::task::spawn_blocking(move || {
        log::info!("[StudentUpdate] Running installer silently...");
        auto_update::InstallerRunner::run_silent(&download_path_clone, installer_type)
    })
    .await;

    match install_result {
        Ok(Ok(())) => {
            log::info!("[StudentUpdate] Installation completed successfully");
        }
        Ok(Err(e)) => {
            log::error!("[StudentUpdate] Installer failed: {}", e);
            state.transition_to_failed(e.to_string(), Some(&app));
            return Err(e.to_string());
        }
        Err(e) => {
            log::error!("[StudentUpdate] Task join error: {}", e);
            state.transition_to_failed(format!("Task join error: {}", e), Some(&app));
            return Err(format!("Task join error: {}", e));
        }
    }

    // Mark as restarting
    if let Err(e) = state.start_restart(Some(&app)) {
        log::error!("[StudentUpdate] Failed to start restart: {}", e);
        return Err(e.to_string());
    }

    log::info!("[StudentUpdate] Scheduling app restart in 2 seconds...");

    // Schedule restart
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        log::info!("[StudentUpdate] Restarting app now...");
        if let Err(e) = auto_update::InstallerRunner::restart_app() {
            log::error!("[StudentUpdate] Failed to restart app: {}", e);
        }
    });

    Ok(())
}

/// Ensure SmartlabService (Windows Service) is installed and running.
/// The service runs at boot level on port 3019, allowing teacher to connect
/// even before user login. If the service is not installed or not running,
/// this function will attempt to fix it.
#[cfg(windows)]
fn ensure_smartlab_service_running(app: &AppHandle) {
    use std::os::windows::process::CommandExt;

    log::info!("[SmartlabService] Checking service status...");

    // Step 1: Check if service is running via sc query
    let status = Command::new("sc")
        .args(["query", "SmartlabService"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output();

    match status {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            if !output.status.success() {
                // sc query returns non-zero when service doesn't exist
                log::info!("[SmartlabService] Service not installed (sc query failed)");
            } else if stdout.contains("RUNNING") {
                log::info!("[SmartlabService] Service is already running");
                return;
            } else if stdout.contains("STOPPED") || stdout.contains("STOP_PENDING") {
                // Service is installed but stopped — try to start it
                log::info!("[SmartlabService] Service is stopped, attempting to start...");
                let start_result = Command::new("sc")
                    .args(["start", "SmartlabService"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .creation_flags(0x08000000)
                    .output();

                match start_result {
                    Ok(out) if out.status.success() => {
                        log::info!("[SmartlabService] Service started successfully");
                        return;
                    }
                    Ok(out) => {
                        let err = String::from_utf8_lossy(&out.stderr);
                        log::warn!("[SmartlabService] Failed to start service: {}", err);
                        // Fall through to try console mode
                    }
                    Err(e) => {
                        log::warn!("[SmartlabService] Failed to run sc start: {}", e);
                    }
                }
            } else {
                // Service exists but in unknown state
                log::warn!("[SmartlabService] Service in unexpected state: {}", stdout.trim());
            }
        }
        Err(_) => {
            // sc command failed — service likely not installed
            log::info!("[SmartlabService] Service not found, attempting to install...");
        }
    }

    // Step 2: Try to find the service executable
    let service_exe = find_service_executable(app);

    if let Some(exe_path) = service_exe {
        log::info!("[SmartlabService] Found service at: {:?}", exe_path);

        // Step 3: Try to install the service (requires admin)
        let install_result = Command::new(&exe_path)
            .arg("--install")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(0x08000000)
            .output();

        match install_result {
            Ok(out) if out.status.success() => {
                log::info!("[SmartlabService] Service installed successfully");

                // Start the service
                let _ = Command::new("sc")
                    .args(["start", "SmartlabService"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .creation_flags(0x08000000)
                    .output();

                log::info!("[SmartlabService] Service start command sent");
                return;
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr);
                log::warn!("[SmartlabService] Install failed (may need admin): {}", err);
            }
            Err(e) => {
                log::warn!("[SmartlabService] Failed to run install: {}", e);
            }
        }

        // Step 4: Fallback — run in console mode (no admin needed, runs as background process)
        log::info!("[SmartlabService] Falling back to console mode...");
        match Command::new(&exe_path)
            .arg("--console")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
        {
            Ok(_child) => {
                log::info!("[SmartlabService] Running in console mode (fallback)");
            }
            Err(e) => {
                log::error!("[SmartlabService] Failed to start in console mode: {}", e);
            }
        }
    } else {
        log::error!("[SmartlabService] Service executable not found");
    }

    // Ensure firewall rule exists for port 3019 (SmartlabService TCP)
    ensure_service_firewall_rule();
}

/// Add Windows Firewall inbound rule for SmartlabService port 3019
#[cfg(windows)]
fn ensure_service_firewall_rule() {
    use std::os::windows::process::CommandExt;

    let rule_name = "SmartlabService TCP 3019";

    // Check if rule already exists
    let check = Command::new("netsh")
        .args(["advfirewall", "firewall", "show", "rule", &format!("name={}", rule_name)])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .output();

    if let Ok(output) = check {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains(rule_name) {
                log::info!("[SmartlabService] Firewall rule already exists");
                return;
            }
        }
    }

    // Add inbound rule for TCP port 3019
    let result = Command::new("netsh")
        .args([
            "advfirewall", "firewall", "add", "rule",
            &format!("name={}", rule_name),
            "dir=in", "action=allow", "protocol=TCP",
            "localport=3019", "enable=yes", "profile=any",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .output();

    match result {
        Ok(out) if out.status.success() => {
            log::info!("[SmartlabService] Firewall rule added for TCP 3019");
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            log::warn!("[SmartlabService] Failed to add firewall rule (may need admin): {}", err);
        }
        Err(e) => {
            log::warn!("[SmartlabService] Failed to run netsh: {}", e);
        }
    }
}

/// Find the smartlab-service.exe in known locations
#[cfg(windows)]
fn find_service_executable(app: &AppHandle) -> Option<PathBuf> {
    // 1. Check next to the main executable (same directory)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let path = exe_dir.join("smartlab-service.exe");
            if path.exists() {
                return Some(path);
            }
            // Check binaries subfolder
            let path = exe_dir.join("binaries").join("smartlab-service.exe");
            if path.exists() {
                return Some(path);
            }
            // Check resources subfolder
            let path = exe_dir.join("resources").join("smartlab-service.exe");
            if path.exists() {
                return Some(path);
            }
        }
    }

    // 2. Check in Tauri resource directory
    if let Ok(resource_dir) = app.path().resource_dir() {
        let path = resource_dir.join("smartlab-service.exe");
        if path.exists() {
            return Some(path);
        }
        let path = resource_dir.join("binaries").join("smartlab-service.exe");
        if path.exists() {
            return Some(path);
        }
    }

    // 3. Check Program Files (common install locations)
    if let Ok(pf) = std::env::var("ProgramFiles") {
        let base = PathBuf::from(&pf).join("SmartlabStudent");
        for subfolder in &["binaries", "resources", ""] {
            let path = if subfolder.is_empty() {
                base.join("smartlab-service.exe")
            } else {
                base.join(subfolder).join("smartlab-service.exe")
            };
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // Initialize logging system
            init_logging(app.handle().clone());
            log_debug("info", "Application started - logging system initialized");
            
            // Check if running in student mode (based on product name)
            let is_student_mode = app.config().product_name
                .as_ref()
                .map(|name| name.contains("Student"))
                .unwrap_or(false);
            
            if is_student_mode {
                log::info!("[Setup] Running in Student mode - initializing system tray");
                
                // Setup system tray for student
                if let Err(e) = student_tray::setup_tray(app.handle()) {
                    log::error!("[Setup] Failed to setup system tray: {}", e);
                } else {
                    log::info!("[Setup] System tray initialized successfully");
                }
                
                // Ensure SmartlabService is installed and running (Windows only)
                #[cfg(windows)]
                {
                    ensure_smartlab_service_running(app.handle());
                }
                
                // Auto-register autostart on first run (like Veyon/NetSupport)
                if !autostart::is_autostart_configured() {
                    log::info!("[Setup] Autostart not configured, registering now...");
                    match autostart::register_autostart() {
                        Ok(()) => log::info!("[Setup] Autostart registered successfully"),
                        Err(e) => log::warn!("[Setup] Failed to register autostart: {}", e),
                    }
                } else {
                    log::info!("[Setup] Autostart already configured");
                }
                
                // Start agent status monitor
                let app_handle = app.handle().clone();
                let agent_state = app.state::<Arc<AgentState>>();
                let agent_state_clone = Arc::clone(&agent_state);
                
                tauri::async_runtime::spawn(async move {
                    student_tray::monitor_agent_status(app_handle, agent_state_clone).await;
                });
                
                // Auto-start student agent
                log::info!("[Setup] Auto-starting student agent...");
                let agent_state = app.state::<Arc<AgentState>>();
                let transfer_state = app.state::<Arc<FileTransferState>>();
                let app_handle = app.handle().clone();
                
                // Get machine name for student name
                let student_name = std::env::var("COMPUTERNAME")
                    .or_else(|_| std::env::var("HOSTNAME"))
                    .unwrap_or_else(|_| "Student".to_string());
                
                // Update config
                {
                    if let Ok(mut config) = agent_state.config.lock() {
                        config.port = 3017;
                        config.student_name = student_name.clone();
                    }
                }
                
                // Reset auto-connect stop flag
                agent_state.auto_connect_stop.store(false, std::sync::atomic::Ordering::Relaxed);
                
                // Start file receiver
                let transfer_state_clone = Arc::clone(&transfer_state);
                let app_clone = app_handle.clone();
                get_agent_runtime().spawn(async move {
                    if let Err(e) = file_transfer::start_file_receiver(
                        transfer_state_clone,
                        app_clone,
                        3017,
                    ).await {
                        log::error!("[FileTransfer] Failed to start file receiver: {}", e);
                    }
                });
                
                // Start agent
                let state_clone = Arc::clone(&agent_state);
                get_agent_runtime().spawn(async move {
                    if let Err(e) = student_agent::start_agent(state_clone).await {
                        log::error!("[StudentAgent] Error: {}", e);
                    }
                });
                
                // Start discovery responder (so teacher's scan can find us)
                let discovery_name = student_name.clone();
                std::thread::spawn(move || {
                    if let Err(e) = lan_discovery::run_student_discovery_responder(
                        &discovery_name,
                        lan_discovery::DISCOVERY_PORT,
                    ) {
                        log::error!("[StudentDiscovery] Responder error: {}", e);
                    }
                });
                
                // Start auto-connect service (legacy: broadcasts looking for teacher)
                let auto_connect_config = student_auto_connect::AutoConnectConfig {
                    teacher_port: lan_discovery::DISCOVERY_PORT,
                    retry_interval_secs: 10,
                    discovery_timeout_ms: 3000,
                };
                let stop_flag = Arc::clone(&agent_state.auto_connect_stop);
                get_agent_runtime().spawn(async move {
                    if let Err(e) = student_auto_connect::start_auto_connect(auto_connect_config, stop_flag).await {
                        log::error!("[StudentAutoConnect] Error: {}", e);
                    }
                });
                
                log::info!("[Setup] Student agent auto-started with discovery + auto-connect");
            } else {
                log::info!("[Setup] Running in Teacher mode");
            }
            
            Ok(())
        })
        .manage(ServerState::default())
        .manage(DatabaseState::default())
        .manage(DiscoveryState::default())
        .manage(AudioCaptureState::default())
        .manage(Arc::new(AgentState::default()))
        .manage(Arc::new(ConnectorState::default()))
        .manage(Arc::new(FileTransferState::default()))
        .manage(Arc::new(DocumentServerState::default()))
        .manage(Arc::new(auto_update::UpdateCoordinator::with_defaults(
            env!("CARGO_PKG_VERSION").to_string()
        )))
        .manage(Arc::new(auto_update::LanDistributionServer::new(9280)))
        .manage(Arc::new(auto_update::StudentUpdateCoordinator::new(
            env!("CARGO_PKG_VERSION").to_string()
        )))
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
            remove_device_from_db,
            // User Authentication commands
            login,
            get_users,
            // Crypto commands
            crypto_generate_keypair,
            crypto_load_keypair,
            crypto_has_keypair,
            crypto_export_public_key,
            crypto_import_teacher_key,
            crypto_load_teacher_key,
            crypto_has_teacher_key,
            crypto_sign_challenge,
            crypto_verify_signature,
            crypto_generate_challenge,
            // Auth mode commands
            auth_set_mode,
            auth_get_mode,
            // LDAP commands
            ldap_save_config,
            ldap_load_config,
            ldap_test_connection,
            ldap_authenticate,
            // Student Agent commands
            start_student_agent,
            stop_student_agent,
            quit_app,
            get_agent_status,
            get_agent_config,
            // Auto-Logon commands
            remote_login_student,
            ping_student_service,
            // Teacher Connector commands
            connect_to_student,
            disconnect_from_student,
            request_student_screen,
            stop_student_screen,
            get_student_connections,
            get_student_connection,
            get_student_screen_frame,
            // Remote Control commands
            send_remote_mouse_event,
            send_remote_keyboard_event,
            send_remote_keyframe_request,
            get_transport_protocol,
            // System Control commands
            send_shutdown_command,
            send_restart_command,
            send_lock_screen_command,
            send_logout_command,
            start_teacher_discovery,
            send_file_to_student,
            cancel_file_transfer,
            get_file_transfer_status,
            // File Transfer commands
            list_directory,
            get_home_directory,
            get_desktop_directory,
            get_documents_directory,
            read_file_as_base64,
            write_file_from_base64,
            get_file_info,
            get_student_directory,
            download_document_to_downloads,
            // Document Distribution commands
            start_document_server,
            stop_document_server,
            get_document_server_status,
            upload_document,
            upload_document_from_path,
            delete_document,
            list_documents,
            get_document,
            // Auto-Update commands
            check_for_updates,
            download_update,
            get_update_state,
            install_update,
            restart_for_update,
            reset_update_state,
            get_update_config,
            save_update_config,
            get_update_config_path,
            get_update_download_path,
            get_latest_update_info,
            check_student_update,
            download_student_package_for_lan,
            // LAN Distribution commands
            start_lan_distribution,
            stop_lan_distribution,
            get_lan_distribution_url,
            get_client_update_status,
            broadcast_update_to_students,
            send_update_to_student,
            check_update_acknowledgments,
            check_all_students_updated,
            // Student Update commands
            get_student_update_state,
            set_student_update_required,
            download_student_update,
            retry_student_update,
            install_student_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
