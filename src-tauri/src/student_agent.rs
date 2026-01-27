//! Student Agent - WebSocket server that allows teacher to connect and view screen
//!
//! This module implements a mini WebSocket server on the student machine that:
//! 1. Listens for incoming connections from teacher
//! 2. Authenticates teacher using Ed25519 challenge-response
//! 3. Captures and streams screen to teacher

use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::crypto;
use crate::h264_encoder::H264Encoder;
use crate::screen_capture;
use crate::video_stream::{start_video_server, VideoFrame};

/// TCP port for video streaming (separate from WebSocket control port)
const VIDEO_STREAM_PORT: u16 = 3018;

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

/// Messages from teacher to student
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum TeacherMessage {
    #[serde(rename = "auth_response")]
    AuthResponse { signature: String },

    #[serde(rename = "request_screen")]
    RequestScreen,

    #[serde(rename = "stop_screen")]
    StopScreen,

    #[serde(rename = "ping")]
    Ping,
}

/// Messages from student to teacher
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum StudentMessage {
    #[serde(rename = "welcome")]
    Welcome {
        student_name: String,
        auth_mode: String,         // "Ed25519"
        challenge: Option<String>, // Base64 encoded
    },

    #[serde(rename = "auth_success")]
    AuthSuccess,

    #[serde(rename = "auth_failed")]
    AuthFailed { reason: String },

    #[serde(rename = "screen_ready")]
    ScreenReady {
        width: u32,
        height: u32,
        video_port: u16, // TCP port for video streaming
    },

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

    #[serde(rename = "error")]
    Error { message: String },
}

/// Connection state for a single teacher connection
struct TeacherConnection {
    addr: SocketAddr,
    authenticated: bool,
    challenge: Vec<u8>,
    screen_sharing: bool,
    stop_capture: Option<Arc<AtomicBool>>,
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

/// Type alias for WebSocket writer
type WsWriter = SplitSink<WebSocketStream<TcpStream>, Message>;

/// Shared WebSocket writer wrapped in Arc<Mutex>
type SharedWsWriter = Arc<tokio::sync::Mutex<WsWriter>>;

/// Handle a single WebSocket connection from teacher
async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    state: Arc<AgentState>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    log::info!("[StudentAgent] New connection from: {}", addr);

    // Accept WebSocket connection
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            log::error!("[StudentAgent] WebSocket handshake failed: {}", e);
            return;
        }
    };

    let (write, mut read) = ws_stream.split();

    // Wrap writer in Arc<Mutex> so capture loop can send frames directly
    let shared_writer: SharedWsWriter = Arc::new(tokio::sync::Mutex::new(write));

    // Generate challenge for Ed25519 authentication
    let challenge = crypto::generate_challenge();
    let challenge_b64 = Some(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &challenge,
    ));

    // Store connection state
    {
        let mut conns = state.connections.lock().unwrap();
        conns.insert(
            addr,
            TeacherConnection {
                addr,
                authenticated: false,
                challenge: challenge.clone(),
                screen_sharing: false,
                stop_capture: None,
            },
        );
    }

    // Get student name
    let student_name = state
        .config
        .lock()
        .map(|c| c.student_name.clone())
        .unwrap_or_else(|_| "Student".to_string());

    // Send welcome with Ed25519 auth mode
    let welcome = StudentMessage::Welcome {
        student_name,
        auth_mode: "Ed25519".to_string(),
        challenge: challenge_b64,
    };

    {
        let mut writer = shared_writer.lock().await;
        if let Err(e) = send_message(&mut *writer, &welcome).await {
            log::error!("[StudentAgent] Failed to send welcome: {}", e);
            return;
        }
    }

    state.set_status(AgentStatus::Authenticating);

    crate::log_debug(
        "info",
        "[StudentAgent] Starting message loop (frames sent directly from capture)",
    );

    // Message handling loop - frames are sent directly from capture loop
    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let mut writer = shared_writer.lock().await;
                        if let Err(e) = handle_message_with_shared_writer(&text, addr, &state, &mut *writer, Arc::clone(&shared_writer)).await {
                            log::error!("[StudentAgent] Error handling message: {}", e);
                            let error_msg = StudentMessage::Error { message: e };
                            let _ = send_message(&mut *writer, &error_msg).await;
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        log::info!("[StudentAgent] Connection closed by teacher: {}", addr);
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let mut writer = shared_writer.lock().await;
                        let _ = writer.send(Message::Pong(data)).await;
                    }
                    Some(Err(e)) => {
                        log::error!("[StudentAgent] WebSocket error: {}", e);
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }

            // Handle shutdown signal
            _ = shutdown_rx.recv() => {
                log::info!("[StudentAgent] Shutdown signal received");
                break;
            }
        }
    }

    // Close WebSocket
    {
        let mut writer = shared_writer.lock().await;
        let _ = writer.close().await;
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

/// Handle a single message from teacher with shared WebSocket writer
async fn handle_message_with_shared_writer<S>(
    text: &str,
    addr: SocketAddr,
    state: &Arc<AgentState>,
    write: &mut S,
    shared_writer: SharedWsWriter,
) -> Result<(), String>
where
    S: SinkExt<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    let msg: TeacherMessage =
        serde_json::from_str(text).map_err(|e| format!("Invalid message: {}", e))?;

    match msg {
        TeacherMessage::AuthResponse { signature } => {
            // Ed25519 authentication
            let (challenge, is_authenticated) = {
                let conns = state.connections.lock().unwrap();
                let conn = conns.get(&addr).ok_or("Connection not found")?;
                (conn.challenge.clone(), conn.authenticated)
            };

            if is_authenticated {
                return Ok(()); // Already authenticated
            }

            // Load teacher's public key and verify
            let public_key = crypto::load_teacher_public_key()
                .map_err(|e| format!("Failed to load teacher key: {}", e))?;

            let result = crypto::verify_signature(&public_key, &challenge, &signature);

            if result.valid {
                // Mark as authenticated
                {
                    let mut conns = state.connections.lock().unwrap();
                    if let Some(conn) = conns.get_mut(&addr) {
                        conn.authenticated = true;
                    }
                }

                state.set_status(AgentStatus::Connected {
                    teacher_name: "Teacher".to_string(),
                });

                let response = StudentMessage::AuthSuccess;
                send_message(write, &response).await?;

                crate::log_debug(
                    "info",
                    "[StudentAgent] Teacher authenticated successfully (Ed25519)",
                );

                // AUTO-START SCREEN CAPTURE after authentication
                // Use TCP streaming (NEW: separate from WebSocket)
                start_screen_capture_tcp(addr, state)?;

                // Get screen resolution
                let (width, height) =
                    screen_capture::get_screen_resolution().unwrap_or((1920, 1080));

                let ready_response = StudentMessage::ScreenReady {
                    width,
                    height,
                    video_port: VIDEO_STREAM_PORT,
                };
                send_message(write, &ready_response).await?;
            } else {
                let reason = result
                    .error
                    .unwrap_or_else(|| "Invalid signature".to_string());
                let response = StudentMessage::AuthFailed {
                    reason: reason.clone(),
                };
                send_message(write, &response).await?;

                return Err(format!("Authentication failed: {}", reason));
            }
        }

        TeacherMessage::RequestScreen => {
            // Check if authenticated
            let (authenticated, already_sharing) = {
                let conns = state.connections.lock().unwrap();
                conns
                    .get(&addr)
                    .map(|c| (c.authenticated, c.screen_sharing))
                    .unwrap_or((false, false))
            };

            if !authenticated {
                let response = StudentMessage::Error {
                    message: "Not authenticated".to_string(),
                };
                send_message(write, &response).await?;
                return Err("Not authenticated".to_string());
            }

            if already_sharing {
                println!("[StudentAgent] Screen already being shared");
                return Ok(());
            }

            // Start screen capture
            start_screen_capture_tcp(addr, state)?;

            // Get screen resolution
            let (width, height) = screen_capture::get_screen_resolution().unwrap_or((1920, 1080));

            let response = StudentMessage::ScreenReady {
                width,
                height,
                video_port: VIDEO_STREAM_PORT,
            };
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
    }

    Ok(())
}

/// Start screen capture with DIRECT WebSocket sending (no channel)
fn start_screen_capture_direct(
    addr: SocketAddr,
    state: &Arc<AgentState>,
    shared_writer: SharedWsWriter,
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

    // Create stop flag
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = Arc::clone(&stop_flag);

    // Update connection state
    {
        let mut conns = state.connections.lock().unwrap();
        if let Some(conn) = conns.get_mut(&addr) {
            conn.screen_sharing = true;
            conn.stop_capture = Some(stop_flag);
        }
    }

    // Start capture in background with DIRECT WebSocket sending
    tokio::spawn(async move {
        crate::log_debug(
            "info",
            "[ScreenCapture] Starting H.264 capture loop (direct WebSocket)",
        );

        // Get initial screen resolution
        let (init_width, init_height) =
            screen_capture::get_screen_resolution().unwrap_or((1920, 1080));

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
        let mut ws_send_count: u64 = 0;
        let target_frame_time = std::time::Duration::from_millis(50); // ~20 FPS to reduce load

        while !stop_flag_clone.load(Ordering::Relaxed) {
            let frame_start = std::time::Instant::now();

            // Capture frame
            let capture_result = {
                let monitor = match screen_capture::get_primary_monitor() {
                    Ok(m) => Some(m),
                    Err(e) => {
                        crate::log_debug(
                            "error",
                            &format!("[ScreenCapture] Failed to get monitor: {}", e),
                        );
                        None
                    }
                };

                if let Some(monitor) = monitor {
                    screen_capture::capture_raw_frame(&monitor).ok()
                } else {
                    None
                }
            };

            match capture_result {
                Some(raw_frame) => {
                    let timestamp = start_time.elapsed().as_millis() as u64;

                    // Encode to H.264
                    match encoder.encode_rgba_with_size(
                        &raw_frame.rgba_data,
                        raw_frame.width,
                        raw_frame.height,
                        timestamp,
                    ) {
                        Ok(encoded) => {
                            // Create binary frame
                            let desc_len = encoded.sps_pps.as_ref().map(|d| d.len()).unwrap_or(0);
                            let mut binary_frame =
                                Vec::with_capacity(19 + desc_len + encoded.data.len());

                            binary_frame.push(if encoded.is_keyframe { 1 } else { 0 });
                            binary_frame.extend_from_slice(&timestamp.to_le_bytes());
                            binary_frame.extend_from_slice(&encoded.width.to_le_bytes());
                            binary_frame.extend_from_slice(&encoded.height.to_le_bytes());
                            binary_frame.extend_from_slice(&(desc_len as u16).to_le_bytes());

                            if let Some(ref desc) = encoded.sps_pps {
                                binary_frame.extend_from_slice(desc);
                            }
                            binary_frame.extend_from_slice(&encoded.data);

                            let frame_len = binary_frame.len();

                            // DIRECTLY send via WebSocket (acquire lock, send, release)
                            {
                                let mut writer = shared_writer.lock().await;
                                match writer.send(Message::Binary(binary_frame)).await {
                                    Ok(_) => {
                                        ws_send_count += 1;
                                        if ws_send_count == 1 || ws_send_count % 30 == 0 {
                                            crate::log_debug("info", &format!(
                                                "[ScreenCapture] Sent {} frames via WebSocket, last size={} bytes",
                                                ws_send_count, frame_len
                                            ));
                                        }
                                    }
                                    Err(e) => {
                                        crate::log_debug(
                                            "error",
                                            &format!(
                                                "[ScreenCapture] WebSocket send failed: {}",
                                                e
                                            ),
                                        );
                                        break; // Exit loop on WebSocket error
                                    }
                                }
                            }

                            frame_count += 1;
                            if frame_count % 30 == 0 {
                                let elapsed = start_time.elapsed().as_secs_f32();
                                let fps = frame_count as f32 / elapsed;
                                crate::log_debug(
                                    "info",
                                    &format!(
                                        "[ScreenCapture] {} frames encoded, {:.1} FPS, keyframe={}",
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
                None => {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
            }

            // Adaptive sleep
            let elapsed = frame_start.elapsed();
            if elapsed < target_frame_time {
                tokio::time::sleep(target_frame_time - elapsed).await;
            }
        }

        crate::log_debug("info", "[ScreenCapture] H.264 capture loop ended");
    });

    Ok(())
}

/// Start screen capture with TCP video streaming (NEW: separate from WebSocket control)
fn start_screen_capture_tcp(addr: SocketAddr, state: &Arc<AgentState>) -> Result<(), String> {
    // Check if already sharing
    {
        let conns = state.connections.lock().unwrap();
        if let Some(conn) = conns.get(&addr) {
            if conn.screen_sharing {
                return Ok(()); // Already sharing
            }
        }
    }

    // Create stop flag
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = Arc::clone(&stop_flag);
    let stop_flag_server = Arc::clone(&stop_flag);

    // Update connection state
    {
        let mut conns = state.connections.lock().unwrap();
        if let Some(conn) = conns.get_mut(&addr) {
            conn.screen_sharing = true;
            conn.stop_capture = Some(stop_flag);
        }
    }

    // Create channel for video frames
    let (frame_tx, frame_rx) = tokio::sync::mpsc::channel::<VideoFrame>(64);

    // Start TCP video server
    tokio::spawn(async move {
        if let Err(e) = start_video_server(VIDEO_STREAM_PORT, frame_rx, stop_flag_server).await {
            crate::log_debug(
                "error",
                &format!("[VideoStream] Failed to start TCP video server: {}", e),
            );
        }
    });

    // Wait a bit for server to start
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Start H.264 encoder and capture loop
    tokio::spawn(async move {
        crate::log_debug(
            "info",
            "[ScreenCapture] Starting H.264 capture loop (TCP streaming)",
        );

        // Get initial screen resolution
        let (init_width, init_height) =
            screen_capture::get_screen_resolution().unwrap_or((1920, 1080));

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
        let target_frame_time = std::time::Duration::from_millis(50); // ~20 FPS

        while !stop_flag_clone.load(Ordering::Relaxed) {
            let frame_start = std::time::Instant::now();

            // Capture frame
            let capture_result = {
                let monitor = match screen_capture::get_primary_monitor() {
                    Ok(m) => Some(m),
                    Err(e) => {
                        crate::log_debug(
                            "error",
                            &format!("[ScreenCapture] Failed to get monitor: {}", e),
                        );
                        None
                    }
                };

                if let Some(monitor) = monitor {
                    screen_capture::capture_raw_frame(&monitor).ok()
                } else {
                    None
                }
            };

            match capture_result {
                Some(raw_frame) => {
                    let timestamp = start_time.elapsed().as_millis() as u64;

                    // Encode to H.264
                    match encoder.encode_rgba_with_size(
                        &raw_frame.rgba_data,
                        raw_frame.width,
                        raw_frame.height,
                        timestamp,
                    ) {
                        Ok(encoded) => {
                            // Create VideoFrame for TCP streaming
                            let video_frame = VideoFrame {
                                is_keyframe: encoded.is_keyframe,
                                timestamp: encoded.timestamp,
                                width: encoded.width,
                                height: encoded.height,
                                sps_pps: encoded.sps_pps,
                                data: encoded.data,
                            };

                            // Send to TCP stream channel
                            if frame_tx.send(video_frame).await.is_err() {
                                crate::log_debug(
                                    "info",
                                    "[ScreenCapture] Video stream channel closed",
                                );
                                break;
                            }

                            frame_count += 1;
                            if frame_count % 30 == 0 {
                                let elapsed = start_time.elapsed().as_secs_f32();
                                let fps = frame_count as f32 / elapsed;
                                crate::log_debug(
                                    "info",
                                    &format!(
                                        "[ScreenCapture] {} frames encoded, {:.1} FPS, keyframe={}",
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
                None => {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
            }

            // Adaptive sleep
            let elapsed = frame_start.elapsed();
            if elapsed < target_frame_time {
                tokio::time::sleep(target_frame_time - elapsed).await;
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

    // Check Ed25519 authentication requirements: require teacher's public key
    if !crypto::has_teacher_public_key() {
        println!("[StudentAgent] ERROR: Teacher's public key not configured!");
        state.set_status(AgentStatus::Error {
            message: "Chưa import khóa giáo viên".to_string(),
        });
        return Err("Teacher's public key not configured. Please import it first.".to_string());
    }
    println!("[StudentAgent] Ed25519 mode - Teacher's public key found, starting...");
    state.set_status(AgentStatus::Starting);

    // Get configuration
    let port = state.config.lock().map(|c| c.port).unwrap_or(3017);

    // Create shutdown channel
    let (shutdown_tx, _) = broadcast::channel::<()>(1);
    {
        let mut tx = state.shutdown_tx.lock().unwrap();
        *tx = Some(shutdown_tx.clone());
    }

    // Bind to port with SO_REUSEADDR
    let addr_str = format!("0.0.0.0:{}", port);
    println!("[StudentAgent] Binding WebSocket server to: {}", addr_str);

    // Create socket with SO_REUSEADDR to allow port reuse
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).map_err(|e| {
        println!("[StudentAgent] Failed to create socket: {}", e);
        format!("Failed to create socket: {}", e)
    })?;

    // Set SO_REUSEADDR to allow port reuse after crash/restart
    socket.set_reuse_address(true).map_err(|e| {
        println!("[StudentAgent] Failed to set SO_REUSEADDR: {}", e);
        format!("Failed to set SO_REUSEADDR: {}", e)
    })?;

    // Set non-blocking mode
    socket.set_nonblocking(true).map_err(|e| {
        println!("[StudentAgent] Failed to set non-blocking: {}", e);
        format!("Failed to set non-blocking: {}", e)
    })?;

    // Parse and bind address
    let addr: std::net::SocketAddr = addr_str
        .parse()
        .map_err(|e| format!("Invalid address: {}", e))?;

    socket.bind(&addr.into()).map_err(|e| {
        println!("[StudentAgent] Failed to bind: {}", e);
        format!("Failed to bind to {}: {}", addr_str, e)
    })?;

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
            auth_mode: "Ed25519".to_string(),
            challenge: Some("abc123".to_string()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("welcome"));
        assert!(json.contains("Test"));
    }
}
