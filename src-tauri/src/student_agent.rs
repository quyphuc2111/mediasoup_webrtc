//! Student Agent - WebSocket server that allows teacher to connect and view screen
//!
//! This module implements a mini WebSocket server on the student machine that:
//! 1. Listens for incoming connections from teacher
//! 2. Auto-accepts connections (no authentication required)
//! 3. Captures and streams screen to teacher
//! 4. Receives and executes remote input (mouse/keyboard)

use enigo::{Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::tungstenite::Message;

use crate::h264_encoder::H264Encoder;
use crate::screen_capture;

/// Agent status
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum AgentStatus {
    Stopped,
    Starting,
    WaitingForTeacher,
    Authenticating,
    Connected { teacher_name: String },
    Error { message: String },
}

/// Agent configuration
#[derive(Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub port: u16,
    pub student_name: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            port: 3017,
            student_name: "Student".to_string(),
        }
    }
}

/// Mouse button type
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Mouse input event from teacher
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MouseInputEvent {
    pub event_type: String, // "move", "click", "scroll", "down", "up"
    pub x: f64,             // Normalized 0-1
    pub y: f64,             // Normalized 0-1
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button: Option<MouseButton>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_y: Option<f64>,
}

/// Keyboard modifiers
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

/// Keyboard input event from teacher
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct KeyboardInputEvent {
    pub event_type: String, // "keydown", "keyup"
    pub key: String,
    pub code: String,
    pub modifiers: KeyModifiers,
}

/// Messages from teacher to student
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum TeacherMessage {
    #[serde(rename = "request_screen")]
    RequestScreen,

    #[serde(rename = "stop_screen")]
    StopScreen,

    #[serde(rename = "ping")]
    Ping,

    #[serde(rename = "mouse_input")]
    MouseInput { event: MouseInputEvent },

    #[serde(rename = "mouse_input_batch")]
    MouseInputBatch { events: Vec<MouseInputEvent> },

    #[serde(rename = "keyboard_input")]
    KeyboardInput { event: KeyboardInputEvent },

    #[serde(rename = "request_keyframe")]
    RequestKeyframe,

    #[serde(rename = "send_file")]
    SendFile {
        file_name: String,
        file_data: String,
        file_size: u64,
    },

    #[serde(rename = "list_directory")]
    ListDirectory {
        path: String,
    },
}

/// Messages from student to teacher
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum StudentMessage {
    #[serde(rename = "welcome")]
    Welcome {
        student_name: String,
    },

    #[serde(rename = "screen_ready")]
    ScreenReady { width: u32, height: u32 },

    #[serde(rename = "screen_frame")]
    ScreenFrame {
        /// Base64 encoded JPEG image
        data: String,
        /// Frame timestamp in milliseconds
        timestamp: u64,
    },

    #[serde(rename = "screen_stopped")]
    ScreenStopped,

    #[serde(rename = "pong")]
    Pong,

    #[serde(rename = "file_received")]
    FileReceived {
        file_name: String,
        success: bool,
        message: String,
    },

    #[serde(rename = "directory_listing")]
    DirectoryListing {
        path: String,
        files: Vec<crate::file_transfer::FileInfo>,
    },

    #[serde(rename = "error")]
    Error { message: String },
}

/// Connection state for a single teacher connection
struct TeacherConnection {
    #[allow(dead_code)]
    addr: SocketAddr,
    screen_sharing: bool,
    stop_capture: Option<Arc<AtomicBool>>,
    keyframe_request_tx: Option<broadcast::Sender<()>>,
}

/// State shared across the agent
pub struct AgentState {
    pub status: Mutex<AgentStatus>,
    pub config: Mutex<AgentConfig>,
    pub shutdown_tx: Mutex<Option<broadcast::Sender<()>>>,
    connections: Mutex<HashMap<SocketAddr, TeacherConnection>>,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            status: Mutex::new(AgentStatus::Stopped),
            config: Mutex::new(AgentConfig::default()),
            shutdown_tx: Mutex::new(None),
            connections: Mutex::new(HashMap::new()),
        }
    }
}

impl AgentState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_status(&self, status: AgentStatus) {
        if let Ok(mut s) = self.status.lock() {
            *s = status;
        }
    }

    pub fn get_status(&self) -> AgentStatus {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or(AgentStatus::Error {
                message: "Lock error".to_string(),
            })
    }
}

/// Handle a single WebSocket connection from teacher
async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    state: Arc<AgentState>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    log::info!("[StudentAgent] New connection from: {}", addr);

    // Accept WebSocket connection with increased message size limit (100MB)
    let ws_config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
        max_message_size: Some(100 * 1024 * 1024), // 100MB
        max_frame_size: Some(16 * 1024 * 1024),    // 16MB per frame
        ..Default::default()
    };
    
    let ws_stream = match tokio_tungstenite::accept_async_with_config(stream, Some(ws_config)).await {
        Ok(ws) => ws,
        Err(e) => {
            log::error!("[StudentAgent] WebSocket handshake failed: {}", e);
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();

    // Store connection state (auto-authenticated, no auth required)
    {
        let mut conns = state.connections.lock().unwrap();
        conns.insert(
            addr,
            TeacherConnection {
                addr,
                screen_sharing: false,
                stop_capture: None,
                keyframe_request_tx: None,
            },
        );
    }

    // Channel for screen frames
    let (frame_tx, mut frame_rx) = mpsc::channel::<Vec<u8>>(2);

    // Get student name
    let student_name = state
        .config
        .lock()
        .map(|c| c.student_name.clone())
        .unwrap_or_else(|_| "Student".to_string());

    // Send welcome message (no auth required)
    let welcome = StudentMessage::Welcome {
        student_name: student_name.clone(),
    };

    if let Err(e) = send_message(&mut write, &welcome).await {
        log::error!("[StudentAgent] Failed to send welcome: {}", e);
        return;
    }

    // Auto-connect: set status to Connected and start screen capture immediately
    state.set_status(AgentStatus::Connected {
        teacher_name: "Teacher".to_string(),
    });

    // AUTO-START SCREEN CAPTURE on connection
    if let Err(e) = start_screen_capture(addr, &state, &frame_tx) {
        log::error!("[StudentAgent] Failed to start screen capture: {}", e);
    } else {
        // Get screen resolution and send ready message
        let (width, height) = screen_capture::get_screen_resolution().unwrap_or((1920, 1080));
        let ready_response = StudentMessage::ScreenReady { width, height };
        if let Err(e) = send_message(&mut write, &ready_response).await {
            log::error!("[StudentAgent] Failed to send screen ready: {}", e);
        }
        log::info!("[StudentAgent] Screen sharing auto-started for {}", addr);
    }

    // Message handling loop
    loop {
        tokio::select! {
            // Handle incoming messages
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Err(e) = handle_message_with_capture(&text, addr, &state, &mut write, &frame_tx).await {
                            log::error!("[StudentAgent] Error handling message: {}", e);
                            let error_msg = StudentMessage::Error { message: e };
                            let _ = send_message(&mut write, &error_msg).await;
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        log::info!("[StudentAgent] Connection closed by teacher: {}", addr);
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = write.send(Message::Pong(data)).await;
                    }
                    Some(Err(e)) => {
                        log::error!("[StudentAgent] WebSocket error: {}", e);
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }
            // Handle screen frames - send as binary for efficiency
            // Frame format: [1 byte type] [8 bytes timestamp] [4 bytes width] [4 bytes height] [H.264 data]
            Some(frame_data) = frame_rx.recv() => {
                // Send as binary WebSocket message (already formatted by capture loop)
                if let Err(e) = write.send(Message::Binary(frame_data)).await {
                    log::error!("[StudentAgent] Failed to send frame: {}", e);
                }
            }
            // Handle shutdown signal
            _ = shutdown_rx.recv() => {
                log::info!("[StudentAgent] Shutdown signal received");
                let _ = write.close().await;
                break;
            }
        }
    }

    // Stop any running screen capture
    {
        let mut conns = state.connections.lock().unwrap();
        if let Some(conn) = conns.get_mut(&addr) {
            if let Some(stop_flag) = conn.stop_capture.take() {
                stop_flag.store(true, Ordering::Relaxed);
            }
        }
        conns.remove(&addr);
    }

    // Update status based on remaining connections
    let has_connections = state
        .connections
        .lock()
        .map(|c| !c.is_empty())
        .unwrap_or(false);

    if !has_connections {
        state.set_status(AgentStatus::WaitingForTeacher);
    }

    log::info!("[StudentAgent] Connection handler finished for: {}", addr);
}

/// Handle a single message from teacher with screen capture support
async fn handle_message_with_capture<S>(
    text: &str,
    addr: SocketAddr,
    state: &Arc<AgentState>,
    write: &mut S,
    frame_tx: &mpsc::Sender<Vec<u8>>,
) -> Result<(), String>
where
    S: SinkExt<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    let msg: TeacherMessage =
        serde_json::from_str(text).map_err(|e| format!("Invalid message: {}", e))?;

    match msg {
        TeacherMessage::RequestScreen => {
            // Check if already sharing
            let already_sharing = {
                let conns = state.connections.lock().unwrap();
                conns
                    .get(&addr)
                    .map(|c| c.screen_sharing)
                    .unwrap_or(false)
            };

            if already_sharing {
                println!("[StudentAgent] Screen already being shared");
                return Ok(());
            }

            // Start screen capture
            start_screen_capture(addr, state, frame_tx)?;

            // Get screen resolution
            let (width, height) = screen_capture::get_screen_resolution().unwrap_or((1920, 1080));

            let response = StudentMessage::ScreenReady { width, height };
            send_message(write, &response).await?;

            println!("[StudentAgent] Screen sharing started");
        }

        TeacherMessage::StopScreen => {
            // Stop screen capture
            {
                let mut conns = state.connections.lock().unwrap();
                if let Some(conn) = conns.get_mut(&addr) {
                    if let Some(stop_flag) = conn.stop_capture.take() {
                        stop_flag.store(true, Ordering::Relaxed);
                    }
                    conn.screen_sharing = false;
                }
            }

            let response = StudentMessage::ScreenStopped;
            send_message(write, &response).await?;

            println!("[StudentAgent] Screen sharing stopped");
        }

        TeacherMessage::Ping => {
            let response = StudentMessage::Pong;
            send_message(write, &response).await?;
        }

        TeacherMessage::MouseInput { event } => {
            // Handle mouse input (no auth check needed)
            if let Err(e) = handle_mouse_input(&event) {
                log::warn!("[StudentAgent] Failed to handle mouse input: {}", e);
            }
        }

        TeacherMessage::MouseInputBatch { events } => {
            // Handle batched mouse inputs
            for event in events.iter() {
                if let Err(e) = handle_mouse_input(event) {
                    log::warn!("[StudentAgent] Failed to handle mouse input: {}", e);
                }
            }
        }

        TeacherMessage::KeyboardInput { event } => {
            // Handle keyboard input (no auth check needed)
            if let Err(e) = handle_keyboard_input(&event) {
                log::warn!("[StudentAgent] Failed to handle keyboard input: {}", e);
            }
        }

        TeacherMessage::RequestKeyframe => {
            // Signal the capture loop to send a keyframe
            if let Ok(conns) = state.connections.lock() {
                if let Some(conn) = conns.get(&addr) {
                    if let Some(ref tx) = conn.keyframe_request_tx {
                        let _ = tx.send(());
                        log::info!(
                            "[StudentAgent] Keyframe request signaled to capture loop for {}",
                            addr
                        );
                    }
                }
            }
        }

        TeacherMessage::SendFile {
            file_name,
            file_data,
            file_size,
        } => {
            log::info!(
                "[StudentAgent] Receiving file: {} ({} bytes) from {}",
                file_name,
                file_size,
                addr
            );

            // Save file to Downloads folder
            match save_received_file(&file_name, &file_data).await {
                Ok(save_path) => {
                    log::info!("[StudentAgent] File saved to: {}", save_path);
                    let response = StudentMessage::FileReceived {
                        file_name: file_name.clone(),
                        success: true,
                        message: format!("File saved to: {}", save_path),
                    };
                    send_message(write, &response).await?;
                }
                Err(e) => {
                    log::error!("[StudentAgent] Failed to save file: {}", e);
                    let response = StudentMessage::FileReceived {
                        file_name: file_name.clone(),
                        success: false,
                        message: format!("Failed to save file: {}", e),
                    };
                    send_message(write, &response).await?;
                }
            }
        }

        TeacherMessage::ListDirectory { path } => {
            log::info!("[StudentAgent] Listing directory: {}", path);

            // If path is empty, use Downloads directory
            let target_path = if path.is_empty() {
                dirs::download_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "/".to_string())
            } else {
                path.clone()
            };

            match crate::file_transfer::list_directory(&target_path) {
                Ok(files) => {
                    let response = StudentMessage::DirectoryListing {
                        path: target_path,
                        files,
                    };
                    send_message(write, &response).await?;
                }
                Err(e) => {
                    log::error!("[StudentAgent] Failed to list directory: {}", e);
                    let response = StudentMessage::Error {
                        message: format!("Failed to list directory: {}", e),
                    };
                    send_message(write, &response).await?;
                }
            }
        }
    }

    Ok(())
}

/// Start screen capture for a connection
fn start_screen_capture(
    addr: SocketAddr,
    state: &Arc<AgentState>,
    frame_tx: &mpsc::Sender<Vec<u8>>,
) -> Result<(), String> {
    // Check if already sharing
    {
        let conns = state.connections.lock().unwrap();
        if let Some(conn) = conns.get(&addr) {
            if conn.screen_sharing {
                return Ok(()); // Already sharing
            }
        }
    }

    // Create stop flag and keyframe request channel
    let stop_flag = Arc::new(AtomicBool::new(false));
    let (keyframe_tx, mut keyframe_rx) = broadcast::channel(1);

    // Update connection state
    {
        let mut conns = state.connections.lock().unwrap();
        if let Some(conn) = conns.get_mut(&addr) {
            conn.screen_sharing = true;
            conn.stop_capture = Some(Arc::clone(&stop_flag));
            conn.keyframe_request_tx = Some(keyframe_tx);
        }
    }

    // Clone frame_tx for the capture thread
    let frame_tx_clone = frame_tx.clone();

    // Start capture in background thread to avoid Send issues with Monitor handles
    std::thread::spawn(move || {
        crate::log_debug(
            "info",
            "[ScreenCapture] Starting H.264 capture loop (background thread)",
        );

        // Get primary monitor once and cache it
        let monitors = match xcap::Monitor::all() {
            Ok(m) => m,
            Err(e) => {
                crate::log_debug(
                    "error",
                    &format!("[ScreenCapture] Failed to get monitors: {}", e),
                );
                return;
            }
        };

        let monitor = monitors
            .iter()
            .find(|m| m.is_primary())
            .or_else(|| monitors.first());

        let monitor = match monitor {
            Some(m) => m,
            None => {
                crate::log_debug("error", "[ScreenCapture] No monitors found");
                return;
            }
        };

        // Get initial screen resolution
        let init_width = monitor.width();
        let init_height = monitor.height();

        // Create H.264 encoder
        let mut encoder = match H264Encoder::new(init_width, init_height) {
            Ok(enc) => enc,
            Err(e) => {
                crate::log_debug(
                    "error",
                    &format!("[ScreenCapture] Failed to create H.264 encoder: {}", e),
                );
                return;
            }
        };

        crate::log_debug(
            "info",
            &format!(
                "[ScreenCapture] H.264 encoder created for {}x{}",
                init_width, init_height
            ),
        );

        let start_time = std::time::Instant::now();
        let mut frame_count: u64 = 0;

        while !stop_flag.load(Ordering::Relaxed) {
            let loop_start = std::time::Instant::now();

            // Check for keyframe request
            if let Ok(_) = keyframe_rx.try_recv() {
                encoder.request_keyframe();
                crate::log_debug(
                    "info",
                    "[ScreenCapture] Forcing keyframe due to teacher request",
                );
            }

            match screen_capture::capture_monitor_raw(monitor) {
                Ok(raw_frame) => {
                    let timestamp = start_time.elapsed().as_millis() as u64;

                    // Encode to H.264 (encoder will auto-update dimensions if needed)
                    match encoder.encode_rgba_with_size(
                        &raw_frame.rgba_data,
                        raw_frame.width,
                        raw_frame.height,
                        timestamp,
                    ) {
                        Ok(encoded) => {
                            let desc_len = encoded.sps_pps.as_ref().map(|d| d.len()).unwrap_or(0);
                            let mut binary_frame =
                                Vec::with_capacity(19 + desc_len + encoded.data.len());

                            binary_frame.push(if encoded.is_keyframe { 1 } else { 0 }); // Frame type
                            binary_frame.extend_from_slice(&timestamp.to_le_bytes()); // 8 bytes
                            binary_frame.extend_from_slice(&encoded.width.to_le_bytes()); // 4 bytes
                            binary_frame.extend_from_slice(&encoded.height.to_le_bytes()); // 4 bytes
                            binary_frame.extend_from_slice(&(desc_len as u16).to_le_bytes()); // 2 bytes description length

                            // Add AVCC description if present (keyframes only)
                            if let Some(ref desc) = encoded.sps_pps {
                                binary_frame.extend_from_slice(desc);
                            }

                            // Add Annex-B H.264 data
                            binary_frame.extend_from_slice(&encoded.data);

                            if frame_tx_clone.try_send(binary_frame).is_err() {
                                // Channel full or closed
                            }

                            frame_count += 1;
                            if frame_count % 30 == 0 {
                                let elapsed = start_time.elapsed().as_secs_f32();
                                let fps = frame_count as f32 / elapsed;
                                crate::log_debug(
                                    "info",
                                    &format!(
                                        "[ScreenCapture] {} frames, {:.1} FPS, keyframe={}",
                                        frame_count, fps, encoded.is_keyframe
                                    ),
                                );
                            }
                        }
                        Err(e) => {
                            crate::log_debug(
                                "error",
                                &format!("[ScreenCapture] Encode error: {}", e),
                            );
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[ScreenCapture] Capture error: {}", e);
                }
            }

            // Adaptive sleep for ~60 FPS (16.6 ms per frame)
            let elapsed = loop_start.elapsed();
            if elapsed < std::time::Duration::from_micros(16666) {
                std::thread::sleep(std::time::Duration::from_micros(16666) - elapsed);
            }
        }

        crate::log_debug("info", "[ScreenCapture] H.264 capture loop ended");
    });

    Ok(())
}

/// Send a message to the WebSocket
async fn send_message<S>(write: &mut S, msg: &StudentMessage) -> Result<(), String>
where
    S: SinkExt<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    let json = serde_json::to_string(msg).map_err(|e| format!("Failed to serialize: {}", e))?;

    write
        .send(Message::Text(json))
        .await
        .map_err(|e| format!("Failed to send: {}", e))
}

/// Save received file to Downloads folder
async fn save_received_file(file_name: &str, file_data: &str) -> Result<String, String> {
    use std::path::PathBuf;

    // Get Downloads directory
    let downloads_dir = dirs::download_dir()
        .ok_or_else(|| "Failed to get Downloads directory".to_string())?;

    // Create full path
    let mut file_path = downloads_dir.join(file_name);

    // If file exists, add number suffix
    let mut counter = 1;
    while file_path.exists() {
        let path_buf = PathBuf::from(file_name);
        let stem = path_buf
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let ext = path_buf
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        let new_name = if ext.is_empty() {
            format!("{} ({})", stem, counter)
        } else {
            format!("{} ({}).{}", stem, counter, ext)
        };

        file_path = downloads_dir.join(new_name);
        counter += 1;
    }

    // Decode base64 and write file
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, file_data)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    tokio::fs::write(&file_path, bytes)
        .await
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(file_path.to_string_lossy().to_string())
}

/// Handle mouse input event from teacher
fn handle_mouse_input(event: &MouseInputEvent) -> Result<(), String> {
    // Get all monitors
    let monitors = xcap::Monitor::all().map_err(|e| format!("Failed to get monitors: {}", e))?;

    // Find the primary monitor (same logic as screen capture)
    // precise match ensures we control the screen we are viewing
    let monitor = monitors
        .iter()
        .find(|m| m.is_primary())
        .or_else(|| monitors.first())
        .ok_or_else(|| "No monitors found".to_string())?;

    let screen_width = monitor.width() as f64;
    let screen_height = monitor.height() as f64;
    let offset_x = monitor.x();
    let offset_y = monitor.y();

    // Convert normalized coordinates to absolute screen coordinates
    // We must add the monitor's offset (virtual desktop position)
    let x = offset_x + (event.x * screen_width) as i32;
    let y = offset_y + (event.y * screen_height) as i32;

    // Create Enigo instance
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| format!("Failed to create Enigo: {}", e))?;

    match event.event_type.as_str() {
        "move" => {
            enigo
                .move_mouse(x, y, Coordinate::Abs)
                .map_err(|e| format!("Failed to move mouse: {}", e))?;
        }
        "click" => {
            // Move to position first
            enigo
                .move_mouse(x, y, Coordinate::Abs)
                .map_err(|e| format!("Failed to move mouse: {}", e))?;

            // Determine button
            let button = match event.button {
                Some(MouseButton::Left) | None => Button::Left,
                Some(MouseButton::Right) => Button::Right,
                Some(MouseButton::Middle) => Button::Middle,
            };

            // Click
            enigo
                .button(button, Direction::Click)
                .map_err(|e| format!("Failed to click: {}", e))?;
        }
        "down" => {
            let button = match event.button {
                Some(MouseButton::Left) | None => Button::Left,
                Some(MouseButton::Right) => Button::Right,
                Some(MouseButton::Middle) => Button::Middle,
            };

            // Only move if we aren't already there (optimization)?
            // For safety, let's just press. The preceding 'move' event should have positioned us.
            // But if packets are lost (UDP) or reordered? TCP/WS guarantees order.
            // Sending explicit move with down ensures accuracy.
            enigo
                .move_mouse(x, y, Coordinate::Abs)
                .map_err(|e| format!("Failed to move mouse (pre-down): {}", e))?;

            crate::log_debug(
                "info",
                &format!("[Mouse] MouseDown {:?} at {},{}", button, x, y),
            );
            enigo
                .button(button, Direction::Press)
                .map_err(|e| format!("Failed to press button: {}", e))?;
        }
        "up" => {
            let button = match event.button {
                Some(MouseButton::Left) | None => Button::Left,
                Some(MouseButton::Right) => Button::Right,
                Some(MouseButton::Middle) => Button::Middle,
            };

            // enigo.move_mouse(x, y, Coordinate::Abs)?; // Optional?

            crate::log_debug(
                "info",
                &format!("[Mouse] MouseUp {:?} at {},{}", button, x, y),
            );
            enigo
                .button(button, Direction::Release)
                .map_err(|e| format!("Failed to release button: {}", e))?;
        }
        "scroll" => {
            enigo
                .move_mouse(x, y, Coordinate::Abs)
                .map_err(|e| format!("Failed to move mouse: {}", e))?;

            if let Some(delta_y) = event.delta_y {
                // Negative delta means scroll up, positive means scroll down
                let scroll_amount = if delta_y > 0.0 { -1 } else { 1 };
                enigo
                    .scroll(scroll_amount, enigo::Axis::Vertical)
                    .map_err(|e| format!("Failed to scroll: {}", e))?;
            }
            if let Some(delta_x) = event.delta_x {
                let scroll_amount = if delta_x > 0.0 { -1 } else { 1 };
                enigo
                    .scroll(scroll_amount, enigo::Axis::Horizontal)
                    .map_err(|e| format!("Failed to scroll horizontal: {}", e))?;
            }
        }
        _ => {
            log::warn!(
                "[StudentAgent] Unknown mouse event type: {}",
                event.event_type
            );
        }
    }

    Ok(())
}

/// Handle keyboard input event from teacher
fn handle_keyboard_input(event: &KeyboardInputEvent) -> Result<(), String> {
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| format!("Failed to create Enigo: {}", e))?;

    // Convert key code to enigo Key
    let key = code_to_key(&event.code, &event.key);

    let direction = match event.event_type.as_str() {
        "keydown" => Direction::Press,
        "keyup" => Direction::Release,
        _ => return Ok(()),
    };

    // Press/release the key
    // NOTE: We do not manually handle modifiers here because the frontend sends explicit
    // keydown/keyup events for Shift, Control, Alt, etc.
    // Manually managing them here causes double-presses or premature releases.
    enigo
        .key(key, direction)
        .map_err(|e| format!("Failed to handle key: {}", e))?;

    Ok(())
}

/// Convert JavaScript key code to enigo Key
fn code_to_key(code: &str, key: &str) -> Key {
    match code {
        // Letters
        "KeyA" => Key::Unicode('a'),
        "KeyB" => Key::Unicode('b'),
        "KeyC" => Key::Unicode('c'),
        "KeyD" => Key::Unicode('d'),
        "KeyE" => Key::Unicode('e'),
        "KeyF" => Key::Unicode('f'),
        "KeyG" => Key::Unicode('g'),
        "KeyH" => Key::Unicode('h'),
        "KeyI" => Key::Unicode('i'),
        "KeyJ" => Key::Unicode('j'),
        "KeyK" => Key::Unicode('k'),
        "KeyL" => Key::Unicode('l'),
        "KeyM" => Key::Unicode('m'),
        "KeyN" => Key::Unicode('n'),
        "KeyO" => Key::Unicode('o'),
        "KeyP" => Key::Unicode('p'),
        "KeyQ" => Key::Unicode('q'),
        "KeyR" => Key::Unicode('r'),
        "KeyS" => Key::Unicode('s'),
        "KeyT" => Key::Unicode('t'),
        "KeyU" => Key::Unicode('u'),
        "KeyV" => Key::Unicode('v'),
        "KeyW" => Key::Unicode('w'),
        "KeyX" => Key::Unicode('x'),
        "KeyY" => Key::Unicode('y'),
        "KeyZ" => Key::Unicode('z'),

        // Numbers
        "Digit0" => Key::Unicode('0'),
        "Digit1" => Key::Unicode('1'),
        "Digit2" => Key::Unicode('2'),
        "Digit3" => Key::Unicode('3'),
        "Digit4" => Key::Unicode('4'),
        "Digit5" => Key::Unicode('5'),
        "Digit6" => Key::Unicode('6'),
        "Digit7" => Key::Unicode('7'),
        "Digit8" => Key::Unicode('8'),
        "Digit9" => Key::Unicode('9'),

        // Function keys
        "F1" => Key::F1,
        "F2" => Key::F2,
        "F3" => Key::F3,
        "F4" => Key::F4,
        "F5" => Key::F5,
        "F6" => Key::F6,
        "F7" => Key::F7,
        "F8" => Key::F8,
        "F9" => Key::F9,
        "F10" => Key::F10,
        "F11" => Key::F11,
        "F12" => Key::F12,

        // Special keys
        "Enter" => Key::Return,
        "Escape" => Key::Escape,
        "Backspace" => Key::Backspace,
        "Tab" => Key::Tab,
        "Space" => Key::Space,
        "Delete" => Key::Delete,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,

        // Arrow keys
        "ArrowUp" => Key::UpArrow,
        "ArrowDown" => Key::DownArrow,
        "ArrowLeft" => Key::LeftArrow,
        "ArrowRight" => Key::RightArrow,

        // Modifiers (handled separately but included for completeness)
        "ShiftLeft" | "ShiftRight" => Key::Shift,
        "ControlLeft" | "ControlRight" => Key::Control,
        "AltLeft" | "AltRight" => Key::Alt,
        "MetaLeft" | "MetaRight" => Key::Meta,

        // Punctuation and symbols
        "Minus" => Key::Unicode('-'),
        "Equal" => Key::Unicode('='),
        "BracketLeft" => Key::Unicode('['),
        "BracketRight" => Key::Unicode(']'),
        "Backslash" => Key::Unicode('\\'),
        "Semicolon" => Key::Unicode(';'),
        "Quote" => Key::Unicode('\''),
        "Backquote" => Key::Unicode('`'),
        "Comma" => Key::Unicode(','),
        "Period" => Key::Unicode('.'),
        "Slash" => Key::Unicode('/'),

        // Numpad
        "Numpad0" => Key::Unicode('0'),
        "Numpad1" => Key::Unicode('1'),
        "Numpad2" => Key::Unicode('2'),
        "Numpad3" => Key::Unicode('3'),
        "Numpad4" => Key::Unicode('4'),
        "Numpad5" => Key::Unicode('5'),
        "Numpad6" => Key::Unicode('6'),
        "Numpad7" => Key::Unicode('7'),
        "Numpad8" => Key::Unicode('8'),
        "Numpad9" => Key::Unicode('9'),
        "NumpadMultiply" => Key::Unicode('*'),
        "NumpadAdd" => Key::Unicode('+'),
        "NumpadSubtract" => Key::Unicode('-'),
        "NumpadDecimal" => Key::Unicode('.'),
        "NumpadDivide" => Key::Unicode('/'),
        "NumpadEnter" => Key::Return,

        // Default: try to use the key character
        _ => {
            if key.len() == 1 {
                Key::Unicode(key.chars().next().unwrap())
            } else {
                log::warn!("[StudentAgent] Unknown key code: {} (key: {})", code, key);
                Key::Unicode(' ')
            }
        }
    }
}

/// Kill any process listening on the specified port, excluding the current process
fn kill_port_holder(port: u16) {
    let current_pid = std::process::id();

    #[cfg(target_os = "windows")]
    {
        // PowerShell command to find process ID by port and kill it, EXCEPT current PID
        // Get-NetTCPConnection finds the connection, .OwningProcess gets PID
        // Where-Object filters out current PID
        // Stop-Process kills the rest
        let cmd = format!("Get-Process -Id (Get-NetTCPConnection -LocalPort {} -ErrorAction SilentlyContinue).OwningProcess -ErrorAction SilentlyContinue | Where-Object {{ $_.Id -ne {} }} | Stop-Process -Force", port, current_pid);

        println!(
            "[StudentAgent] Attempting to free port {} using PowerShell (excluding self: {})...",
            port, current_pid
        );
        let _ = std::process::Command::new("powershell")
            .args(&["-NoProfile", "-Command", &cmd])
            .output();
    }

    #[cfg(not(target_os = "windows"))]
    {
        println!(
            "[StudentAgent] Attempting to free port {} using lsof (excluding self: {})...",
            port, current_pid
        );
        // lsof finds the PID
        // grep -v excludes current PID
        // xargs kill kills it (if any remain)
        let cmd = format!(
            "lsof -t -i:{} | grep -v ^{}$ | xargs kill -9",
            port, current_pid
        );
        let _ = std::process::Command::new("sh").arg("-c").arg(cmd).output();
    }
}

/// Start the student agent server
pub async fn start_agent(state: Arc<AgentState>) -> Result<(), String> {
    println!("[StudentAgent] start_agent called");

    // Check if already running
    let current_status = state.get_status();
    if current_status != AgentStatus::Stopped {
        println!(
            "[StudentAgent] Already running, status: {:?}",
            current_status
        );
        return Err("Agent already running".to_string());
    }

    println!("[StudentAgent] Starting agent (no authentication required)...");
    state.set_status(AgentStatus::Starting);

    // Get configuration
    let default_port = state.config.lock().map(|c| c.port).unwrap_or(3017);
    let mut port = default_port;

    // Forcefully kill any process holding the port to ensure we can bind
    kill_port_holder(port);
    // Give OS a moment to release the port
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create shutdown channel
    let (shutdown_tx, _) = broadcast::channel::<()>(1);
    {
        let mut tx = state.shutdown_tx.lock().unwrap();
        *tx = Some(shutdown_tx.clone());
    }

    // Try to bind to the requested port, if that fails, try a random port
    let socket = match Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)) {
        Ok(s) => s,
        Err(e) => {
            println!("[StudentAgent] Failed to create socket: {}", e);
            return Err(format!("Failed to create socket: {}", e));
        }
    };

    if let Err(e) = socket.set_reuse_address(true) {
        println!("[StudentAgent] Failed to set SO_REUSEADDR: {}", e);
        return Err(format!("Failed to set SO_REUSEADDR: {}", e));
    }

    if let Err(e) = socket.set_nonblocking(true) {
        println!("[StudentAgent] Failed to set non-blocking: {}", e);
        return Err(format!("Failed to set non-blocking: {}", e));
    }

    let addr_str = format!("0.0.0.0:{}", port);
    let addr: std::net::SocketAddr = addr_str
        .parse()
        .map_err(|e| format!("Invalid address: {}", e))?;

    // Bind logic with fallback
    if let Err(e) = socket.bind(&addr.into()) {
        println!(
            "[StudentAgent] Failed to bind to {}: {}. Trying random port...",
            addr_str, e
        );
        // Try random port
        let random_addr: std::net::SocketAddr = "0.0.0.0:0".parse().unwrap();
        socket.bind(&random_addr.into()).map_err(|e| {
            println!("[StudentAgent] Failed to bind to random port: {}", e);
            format!("Failed to bind to random port: {}", e)
        })?;
    }

    // Get the actual bound port
    let local_addr = socket
        .local_addr()
        .map_err(|e| format!("Failed to get local address: {}", e))?;
    // Convert socket2::SockAddr to std::net::SocketAddr
    let local_addr = local_addr
        .as_socket()
        .ok_or("Failed to get socket address")?;
    port = local_addr.port();

    // Update config with actual port
    if let Ok(mut config) = state.config.lock() {
        config.port = port;
        println!("[StudentAgent] Updated config with actual port: {}", port);
    }

    // Listen
    socket.listen(128).map_err(|e| {
        println!("[StudentAgent] Failed to listen: {}", e);
        format!("Failed to listen: {}", e)
    })?;

    // Convert to Tokio TcpListener
    let std_listener: std::net::TcpListener = socket.into();
    let listener = TcpListener::from_std(std_listener).map_err(|e| {
        println!("[StudentAgent] Failed to convert to Tokio listener: {}", e);
        format!("Failed to create async listener: {}", e)
    })?;

    println!("[StudentAgent] WebSocket server listening on port {}", port);

    log::info!("[StudentAgent] Listening on: {}", addr);
    state.set_status(AgentStatus::WaitingForTeacher);

    // Accept connections
    loop {
        let shutdown_rx = shutdown_tx.subscribe();

        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, addr)) => {
                        println!("[StudentAgent] Incoming connection from: {}", addr);
                        let state_clone = Arc::clone(&state);
                        tokio::spawn(handle_connection(
                            stream,
                            addr,
                            state_clone,
                            shutdown_rx,
                        ));
                    }
                    Err(e) => {
                        println!("[StudentAgent] Accept error: {}", e);
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                log::info!("[StudentAgent] Ctrl+C received, shutting down");
                break;
            }
        }

        // Check if we should stop
        if state.shutdown_tx.lock().unwrap().is_none() {
            break;
        }
    }

    state.set_status(AgentStatus::Stopped);
    Ok(())
}

/// Stop the student agent server
pub fn stop_agent(state: &AgentState) -> Result<(), String> {
    let mut tx = state
        .shutdown_tx
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;

    if let Some(sender) = tx.take() {
        let _ = sender.send(());
    }

    state.set_status(AgentStatus::Stopped);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_state() {
        let state = AgentState::new();
        assert_eq!(state.get_status(), AgentStatus::Stopped);

        state.set_status(AgentStatus::WaitingForTeacher);
        assert_eq!(state.get_status(), AgentStatus::WaitingForTeacher);
    }

    #[test]
    fn test_message_serialization() {
        let msg = StudentMessage::Welcome {
            student_name: "Test".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("welcome"));
        assert!(json.contains("Test"));
    }
}
