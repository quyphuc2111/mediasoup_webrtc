//! Teacher Connector - Connect to student agents and view their screens
//!
//! This module implements the teacher-side WebSocket client that:
//! 1. Connects to student agent on their machine
//! 2. Authenticates using Ed25519 challenge-response
//! 3. Requests screen sharing from student

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::crypto;

/// Connection status for a single student
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Authenticating,
    Connected,
    Viewing,
    Error { message: String },
}

/// Information about a student connection
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct StudentConnection {
    pub id: String,
    pub ip: String,
    pub port: u16,
    pub name: Option<String>,
    pub status: ConnectionStatus,
}

/// Messages from student to teacher (same as in student_agent)
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum StudentMessage {
    #[serde(rename = "welcome")]
    Welcome {
        student_name: String,
        auth_mode: String,         // "Ed25519" or "Ldap"
        challenge: Option<String>, // Base64 encoded (Ed25519 only)
    },

    #[serde(rename = "auth_success")]
    AuthSuccess,

    #[serde(rename = "auth_failed")]
    AuthFailed { reason: String },

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

    #[serde(rename = "error")]
    Error { message: String },
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
    #[serde(rename = "auth_response")]
    AuthResponse { signature: String },

    #[serde(rename = "ldap_auth")]
    LdapAuth { username: String, password: String },

    #[serde(rename = "request_screen")]
    RequestScreen,

    #[serde(rename = "stop_screen")]
    StopScreen,

    #[serde(rename = "ping")]
    Ping,

    #[serde(rename = "mouse_input")]
    MouseInput { event: MouseInputEvent },

    #[serde(rename = "keyboard_input")]
    KeyboardInput { event: KeyboardInputEvent },
}

/// Command to send to a connection handler
#[derive(Debug)]
pub enum ConnectionCommand {
    RequestScreen,
    StopScreen,
    Disconnect,
    SendMouseInput(MouseInputEvent),
    SendKeyboardInput(KeyboardInputEvent),
}

/// Screen frame data
#[derive(Clone, Serialize, Debug)]
pub struct ScreenFrame {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>, // Base64 encoded (for JPEG fallback only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_binary: Option<Vec<u8>>, // Binary H.264 Annex-B data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sps_pps: Option<Vec<u8>>, // AVCC format description for WebCodecs
    pub timestamp: u64,
    pub width: u32,
    pub height: u32,
    pub is_keyframe: bool,
    pub codec: String, // "h264" or "jpeg"
}

/// State for managing all student connections
pub struct ConnectorState {
    pub connections: Mutex<HashMap<String, StudentConnection>>,
    pub command_senders: Mutex<HashMap<String, mpsc::Sender<ConnectionCommand>>>,
    pub screen_frames: Mutex<HashMap<String, ScreenFrame>>,
    pub screen_sizes: Mutex<HashMap<String, (u32, u32)>>,
    pub decoders: Mutex<HashMap<String, crate::h264_decoder::H264Decoder>>,
}

impl Default for ConnectorState {
    fn default() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            command_senders: Mutex::new(HashMap::new()),
            screen_frames: Mutex::new(HashMap::new()),
            screen_sizes: Mutex::new(HashMap::new()),
            decoders: Mutex::new(HashMap::new()),
        }
    }
}

impl ConnectorState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_connection(&self, id: &str) -> Option<StudentConnection> {
        self.connections.lock().ok()?.get(id).cloned()
    }

    pub fn get_all_connections(&self) -> Vec<StudentConnection> {
        self.connections
            .lock()
            .map(|c| c.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn update_status(&self, id: &str, status: ConnectionStatus) {
        if let Ok(mut conns) = self.connections.lock() {
            if let Some(conn) = conns.get_mut(id) {
                conn.status = status;
            }
        }
    }

    pub fn update_name(&self, id: &str, name: String) {
        if let Ok(mut conns) = self.connections.lock() {
            if let Some(conn) = conns.get_mut(id) {
                conn.name = Some(name);
            }
        }
    }

    pub fn remove_connection(&self, id: &str) {
        if let Ok(mut conns) = self.connections.lock() {
            conns.remove(id);
        }
        if let Ok(mut senders) = self.command_senders.lock() {
            senders.remove(id);
        }
    }
}

/// Authentication credentials for teacher
#[derive(Clone)]
pub enum AuthCredentials {
    Ed25519, // Use Ed25519 keypair (default)
    Ldap { username: String, password: String },
}

/// Connect to a student agent
pub async fn connect_to_student(
    state: Arc<ConnectorState>,
    ip: String,
    port: u16,
) -> Result<String, String> {
    connect_to_student_with_auth(state, ip, port, AuthCredentials::Ed25519).await
}

/// Connect to a student agent with specific authentication
pub async fn connect_to_student_with_auth(
    state: Arc<ConnectorState>,
    ip: String,
    port: u16,
    credentials: AuthCredentials,
) -> Result<String, String> {
    // Generate connection ID
    let id = format!("{}:{}", ip, port);

    // Check if already connected
    if let Some(conn) = state.get_connection(&id) {
        if conn.status != ConnectionStatus::Disconnected
            && !matches!(conn.status, ConnectionStatus::Error { .. })
        {
            return Err("Already connected to this student".to_string());
        }
    }

    // Check if we have a keypair
    if !crypto::has_keypair() {
        return Err("No keypair found. Please generate one first.".to_string());
    }

    // Create connection entry
    let connection = StudentConnection {
        id: id.clone(),
        ip: ip.clone(),
        port,
        name: None,
        status: ConnectionStatus::Connecting,
    };

    // Store connection
    {
        let mut conns = state.connections.lock().map_err(|e| e.to_string())?;
        conns.insert(id.clone(), connection);
    }

    // Create command channel
    let (cmd_tx, cmd_rx) = mpsc::channel::<ConnectionCommand>(16);
    {
        let mut senders = state.command_senders.lock().map_err(|e| e.to_string())?;
        senders.insert(id.clone(), cmd_tx);
    }

    // Start connection handler
    let state_clone = Arc::clone(&state);
    let id_clone = id.clone();
    let ip_clone = ip.clone();
    let credentials_clone = credentials.clone();

    tokio::spawn(async move {
        if let Err(e) = handle_connection(
            state_clone,
            id_clone.clone(),
            ip_clone,
            port,
            cmd_rx,
            credentials_clone,
        )
        .await
        {
            log::error!("[TeacherConnector] Connection error: {}", e);
        }
    });

    Ok(id)
}

/// Public async handler that can be called from outside with its own command channel
pub async fn handle_connection_async(
    state: Arc<ConnectorState>,
    id: String,
    ip: String,
    port: u16,
) -> Result<(), String> {
    println!(
        "[TeacherConnector] handle_connection_async called for {}:{}",
        ip, port
    );

    // Create command channel internally
    let (cmd_tx, cmd_rx) = mpsc::channel::<ConnectionCommand>(16);
    {
        let mut senders = state.command_senders.lock().map_err(|e| e.to_string())?;
        senders.insert(id.clone(), cmd_tx);
    }

    let result = handle_connection(
        Arc::clone(&state),
        id.clone(),
        ip.clone(),
        port,
        cmd_rx,
        AuthCredentials::Ed25519,
    )
    .await;

    // Cleanup on error
    if result.is_err() {
        let err_msg = result.as_ref().err().unwrap().clone();
        println!(
            "[TeacherConnector] Connection error for {}:{}: {}",
            ip, port, err_msg
        );
        state.update_status(&id, ConnectionStatus::Error { message: err_msg });
    }

    result
}

/// Handle a connection to a student (internal)
async fn handle_connection(
    state: Arc<ConnectorState>,
    id: String,
    ip: String,
    port: u16,
    mut cmd_rx: mpsc::Receiver<ConnectionCommand>,
    credentials: AuthCredentials,
) -> Result<(), String> {
    let url = format!("ws://{}:{}", ip, port);
    println!(
        "[TeacherConnector] Attempting WebSocket connection to: {}",
        url
    );

    // Connect to student
    let (ws_stream, _) = connect_async(&url).await.map_err(|e| {
        println!("[TeacherConnector] WebSocket connect failed: {}", e);
        format!("Failed to connect: {}", e)
    })?;

    println!("[TeacherConnector] WebSocket connected to: {}", url);

    let (mut write, mut read) = ws_stream.split();

    state.update_status(&id, ConnectionStatus::Authenticating);

    // Wait for welcome message with auth mode info
    let welcome_msg = read
        .next()
        .await
        .ok_or("Connection closed")?
        .map_err(|e| format!("WebSocket error: {}", e))?;

    let welcome_text = match welcome_msg {
        Message::Text(text) => text,
        _ => return Err("Expected text message".to_string()),
    };

    let welcome: StudentMessage =
        serde_json::from_str(&welcome_text).map_err(|e| format!("Invalid welcome: {}", e))?;

    let (student_name, auth_mode, challenge_opt) = match welcome {
        StudentMessage::Welcome {
            student_name,
            auth_mode,
            challenge,
        } => (student_name, auth_mode, challenge),
        _ => return Err("Expected welcome message".to_string()),
    };

    state.update_name(&id, student_name);

    println!("[TeacherConnector] Student auth mode: {}", auth_mode);

    // Authenticate based on mode
    let auth_msg = if auth_mode == "Ed25519" {
        // Ed25519 authentication
        let challenge = challenge_opt.ok_or("No challenge provided for Ed25519 mode")?;

        let keypair =
            crypto::load_keypair().map_err(|e| format!("Failed to load keypair: {}", e))?;

        let challenge_bytes =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &challenge)
                .map_err(|e| format!("Invalid challenge: {}", e))?;

        let signature = crypto::sign_challenge(&keypair.private_key, &challenge_bytes)
            .map_err(|e| format!("Failed to sign: {}", e))?;

        TeacherMessage::AuthResponse { signature }
    } else if auth_mode == "Ldap" {
        // LDAP authentication - for now, return error asking for credentials
        // In a full implementation, this would come from UI
        return Err(
            "LDAP mode detected. Please use connect_to_student_with_ldap() \
             or implement UI for LDAP credentials."
                .to_string(),
        );
    } else {
        return Err(format!("Unknown auth mode: {}", auth_mode));
    };

    // Send auth message
    let auth_json =
        serde_json::to_string(&auth_msg).map_err(|e| format!("Serialize error: {}", e))?;

    write
        .send(Message::Text(auth_json))
        .await
        .map_err(|e| format!("Failed to send auth: {}", e))?;

    // Wait for auth result
    let auth_result = read
        .next()
        .await
        .ok_or("Connection closed during auth")?
        .map_err(|e| format!("WebSocket error: {}", e))?;

    let auth_text = match auth_result {
        Message::Text(text) => text,
        _ => return Err("Expected text message".to_string()),
    };

    let auth_response: StudentMessage =
        serde_json::from_str(&auth_text).map_err(|e| format!("Invalid auth response: {}", e))?;

    match auth_response {
        StudentMessage::AuthSuccess => {
            log::info!("[TeacherConnector] Authentication successful");
            state.update_status(&id, ConnectionStatus::Connected);
        }
        StudentMessage::AuthFailed { reason } => {
            state.update_status(
                &id,
                ConnectionStatus::Error {
                    message: reason.clone(),
                },
            );
            return Err(format!("Authentication failed: {}", reason));
        }
        _ => {
            return Err("Unexpected auth response".to_string());
        }
    }

    // Message handling loop
    loop {
        tokio::select! {
            // Handle incoming messages from student
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Err(e) = handle_student_message(&text, &state, &id).await {
                            log::error!("[TeacherConnector] Error: {}", e);
                        }
                    }
                    Some(Ok(Message::Binary(data))) => {
                        // H.264 frame format:
                        // [1 byte: frame_type]
                        // [8 bytes: timestamp]
                        // [4 bytes: width]
                        // [4 bytes: height]
                        // [2 bytes: description_length]
                        // [description_length bytes: AVCC description] (only for keyframes)
                        // [H.264 Annex-B data]
                        if data.len() > 19 {
                            let is_keyframe = data[0] == 1;
                            let timestamp = u64::from_le_bytes(data[1..9].try_into().unwrap_or([0u8; 8]));
                            let width = u32::from_le_bytes(data[9..13].try_into().unwrap_or([0u8; 4]));
                            let height = u32::from_le_bytes(data[13..17].try_into().unwrap_or([0u8; 4]));
                            let desc_len = u16::from_le_bytes(data[17..19].try_into().unwrap_or([0u8; 2])) as usize;

                            let desc_start = 19;
                            let desc_end = desc_start + desc_len;

                            if desc_end > data.len() {
                                log::warn!("[TeacherConnector] Invalid frame format: description length exceeds data");
                                continue;
                            }

                            // Extract H.264 Annex-B data
                            let h264_data = &data[desc_end..];

                            // Decode H.264 to JPEG using decoder
                            let jpeg_result = {
                                let mut decoders = match state.decoders.lock() {
                                    Ok(d) => d,
                                    Err(e) => {
                                        log::error!("[TeacherConnector] Failed to lock decoders: {}", e);
                                        continue;
                                    }
                                };

                                // Get or create decoder for this connection
                                let decoder = decoders.entry(id.clone()).or_insert_with(|| {
                                    log::info!("[TeacherConnector] H.264 decoder for connection: {}", id);
                                    crate::h264_decoder::H264Decoder::new().unwrap()
                                });

                                // Decode H.264 to JPEG
                                decoder.decode_to_jpeg(h264_data)
                            };

                            match jpeg_result {
                                Ok(Some(jpeg_base64)) => {
                                    // Successfully decoded to JPEG
                                    let frame = ScreenFrame {
                                        data: Some(jpeg_base64),  // JPEG base64
                                        data_binary: None,  // No raw H.264
                                        sps_pps: None,  // Not needed for JPEG
                                        timestamp,
                                        width,
                                        height,
                                        is_keyframe: true,  // JPEG frames are always complete
                                        codec: "jpeg".to_string(),
                                    };

                                    if let Ok(mut frames) = state.screen_frames.lock() {
                                        frames.insert(id.clone(), frame);
                                    }
                                }
                                Ok(None) => {
                                    // Frame not ready yet (waiting for keyframe or incomplete)
                                    log::debug!("[TeacherConnector] Frame not ready (waiting for keyframe)");
                                }
                                Err(e) => {
                                    log::error!("[TeacherConnector] H.264 decode error for {}: {}", id, e);
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        log::info!("[TeacherConnector] Connection closed by student");
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = write.send(Message::Pong(data)).await;
                    }
                    Some(Err(e)) => {
                        log::error!("[TeacherConnector] WebSocket error: {}", e);
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }

            // Handle commands from app
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(ConnectionCommand::RequestScreen) => {
                        let msg = TeacherMessage::RequestScreen;
                        let json = serde_json::to_string(&msg).unwrap();
                        let _ = write.send(Message::Text(json)).await;
                        state.update_status(&id, ConnectionStatus::Viewing);
                    }
                    Some(ConnectionCommand::StopScreen) => {
                        let msg = TeacherMessage::StopScreen;
                        let json = serde_json::to_string(&msg).unwrap();
                        let _ = write.send(Message::Text(json)).await;
                        state.update_status(&id, ConnectionStatus::Connected);
                    }
                    Some(ConnectionCommand::SendMouseInput(event)) => {
                        let msg = TeacherMessage::MouseInput { event };
                        let json = serde_json::to_string(&msg).unwrap();
                        let _ = write.send(Message::Text(json)).await;
                    }
                    Some(ConnectionCommand::SendKeyboardInput(event)) => {
                        let msg = TeacherMessage::KeyboardInput { event };
                        let json = serde_json::to_string(&msg).unwrap();
                        let _ = write.send(Message::Text(json)).await;
                    }
                    Some(ConnectionCommand::Disconnect) | None => {
                        log::info!("[TeacherConnector] Disconnect command received");
                        let _ = write.close().await;
                        break;
                    }
                }
            }
        }
    }

    // Cleanup
    state.update_status(&id, ConnectionStatus::Disconnected);
    log::info!("[TeacherConnector] Connection closed: {}", id);

    Ok(())
}

/// Handle a message from student
async fn handle_student_message(
    text: &str,
    state: &Arc<ConnectorState>,
    id: &str,
) -> Result<(), String> {
    let msg: StudentMessage =
        serde_json::from_str(text).map_err(|e| format!("Invalid message: {}", e))?;

    match msg {
        StudentMessage::ScreenReady { width, height } => {
            crate::log_debug(
                "info",
                &format!(
                    "[TeacherConnector] Screen ready from {} ({}x{})",
                    id, width, height
                ),
            );

            // Store screen size
            if let Ok(mut sizes) = state.screen_sizes.lock() {
                sizes.insert(id.to_string(), (width, height));
            }

            state.update_status(id, ConnectionStatus::Viewing);
        }
        StudentMessage::ScreenFrame { data, timestamp } => {
            // Legacy JPEG frame handling (for compatibility)
            let (width, height) = state
                .screen_sizes
                .lock()
                .ok()
                .and_then(|sizes| sizes.get(id).copied())
                .unwrap_or((960, 540));

            let frame = ScreenFrame {
                data: Some(data), // JPEG uses base64 string
                data_binary: None,
                sps_pps: None,
                timestamp,
                width,
                height,
                is_keyframe: true, // JPEG frames are always complete
                codec: "jpeg".to_string(),
            };

            if let Ok(mut frames) = state.screen_frames.lock() {
                frames.insert(id.to_string(), frame);
            }
        }
        StudentMessage::ScreenStopped => {
            println!("[TeacherConnector] Screen stopped from {}", id);

            // Clear stored frame
            if let Ok(mut frames) = state.screen_frames.lock() {
                frames.remove(id);
            }

            state.update_status(id, ConnectionStatus::Connected);
        }
        StudentMessage::Error { message } => {
            println!("[TeacherConnector] Error from student: {}", message);
        }
        StudentMessage::Pong => {
            // Keep-alive response
        }
        _ => {
            println!("[TeacherConnector] Unexpected message: {:?}", msg);
        }
    }

    Ok(())
}

/// Get the latest screen frame for a connection
pub fn get_screen_frame(state: &ConnectorState, id: &str) -> Option<ScreenFrame> {
    state
        .screen_frames
        .lock()
        .ok()
        .and_then(|frames| frames.get(id).cloned())
}

/// Disconnect from a student
pub fn disconnect_student(state: &ConnectorState, id: &str) -> Result<(), String> {
    let senders = state.command_senders.lock().map_err(|e| e.to_string())?;

    if let Some(sender) = senders.get(id) {
        let _ = sender.try_send(ConnectionCommand::Disconnect);
    }

    Ok(())
}

/// Request screen from a student
pub fn request_screen(state: &ConnectorState, id: &str) -> Result<(), String> {
    let senders = state.command_senders.lock().map_err(|e| e.to_string())?;

    if let Some(sender) = senders.get(id) {
        sender
            .try_send(ConnectionCommand::RequestScreen)
            .map_err(|e| format!("Failed to send command: {}", e))?;
    } else {
        return Err("Connection not found".to_string());
    }

    Ok(())
}

/// Stop screen viewing
pub fn stop_screen(state: &ConnectorState, id: &str) -> Result<(), String> {
    let senders = state.command_senders.lock().map_err(|e| e.to_string())?;

    if let Some(sender) = senders.get(id) {
        sender
            .try_send(ConnectionCommand::StopScreen)
            .map_err(|e| format!("Failed to send command: {}", e))?;
    } else {
        return Err("Connection not found".to_string());
    }

    Ok(())
}

/// Send mouse input to student
pub fn send_mouse_input(state: &ConnectorState, id: &str, event: MouseInputEvent) -> Result<(), String> {
    let senders = state.command_senders.lock().map_err(|e| e.to_string())?;

    if let Some(sender) = senders.get(id) {
        sender
            .try_send(ConnectionCommand::SendMouseInput(event))
            .map_err(|e| format!("Failed to send mouse input: {}", e))?;
    } else {
        return Err("Connection not found".to_string());
    }

    Ok(())
}

/// Send keyboard input to student
pub fn send_keyboard_input(state: &ConnectorState, id: &str, event: KeyboardInputEvent) -> Result<(), String> {
    let senders = state.command_senders.lock().map_err(|e| e.to_string())?;

    if let Some(sender) = senders.get(id) {
        sender
            .try_send(ConnectionCommand::SendKeyboardInput(event))
            .map_err(|e| format!("Failed to send keyboard input: {}", e))?;
    } else {
        return Err("Connection not found".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connector_state() {
        let state = ConnectorState::new();

        let conn = StudentConnection {
            id: "test".to_string(),
            ip: "192.168.1.1".to_string(),
            port: 3017,
            name: None,
            status: ConnectionStatus::Disconnected,
        };

        state
            .connections
            .lock()
            .unwrap()
            .insert("test".to_string(), conn);

        let retrieved = state.get_connection("test");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().ip, "192.168.1.1");
    }

    #[test]
    fn test_message_serialization() {
        let msg = TeacherMessage::AuthResponse {
            signature: "abc123".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("auth_response"));
    }
}
