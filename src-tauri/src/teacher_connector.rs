//! Teacher Connector - Connect to student agents and view their screens
//!
//! This module implements the teacher-side WebSocket client that:
//! 1. Connects to student agent on their machine
//! 2. Authenticates using Ed25519 challenge-response
//! 3. Requests screen sharing from student

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use futures_util::{SinkExt, StreamExt};
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
        challenge: String,
    },

    #[serde(rename = "auth_success")]
    AuthSuccess,

    #[serde(rename = "auth_failed")]
    AuthFailed { reason: String },

    #[serde(rename = "screen_ready")]
    ScreenReady { offer_sdp: Option<String> },

    #[serde(rename = "screen_stopped")]
    ScreenStopped,

    #[serde(rename = "pong")]
    Pong,

    #[serde(rename = "error")]
    Error { message: String },
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

/// Command to send to a connection handler
#[derive(Debug)]
pub enum ConnectionCommand {
    RequestScreen,
    StopScreen,
    Disconnect,
}

/// State for managing all student connections
pub struct ConnectorState {
    pub connections: Mutex<HashMap<String, StudentConnection>>,
    pub command_senders: Mutex<HashMap<String, mpsc::Sender<ConnectionCommand>>>,
}

impl Default for ConnectorState {
    fn default() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            command_senders: Mutex::new(HashMap::new()),
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

/// Connect to a student agent
pub async fn connect_to_student(
    state: Arc<ConnectorState>,
    ip: String,
    port: u16,
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

    tokio::spawn(async move {
        if let Err(e) = handle_connection(state_clone, id_clone.clone(), ip_clone, port, cmd_rx).await
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
    println!("[TeacherConnector] handle_connection_async called for {}:{}", ip, port);
    
    // Create command channel internally
    let (cmd_tx, cmd_rx) = mpsc::channel::<ConnectionCommand>(16);
    {
        let mut senders = state.command_senders.lock().map_err(|e| e.to_string())?;
        senders.insert(id.clone(), cmd_tx);
    }
    
    let result = handle_connection(Arc::clone(&state), id.clone(), ip.clone(), port, cmd_rx).await;
    
    // Cleanup on error
    if result.is_err() {
        let err_msg = result.as_ref().err().unwrap().clone();
        println!("[TeacherConnector] Connection error for {}:{}: {}", ip, port, err_msg);
        state.update_status(&id, ConnectionStatus::Error { 
            message: err_msg 
        });
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
) -> Result<(), String> {
    let url = format!("ws://{}:{}", ip, port);
    println!("[TeacherConnector] Attempting WebSocket connection to: {}", url);

    // Connect to student
    let (ws_stream, _) = connect_async(&url)
        .await
        .map_err(|e| {
            println!("[TeacherConnector] WebSocket connect failed: {}", e);
            format!("Failed to connect: {}", e)
        })?;
    
    println!("[TeacherConnector] WebSocket connected to: {}", url);

    let (mut write, mut read) = ws_stream.split();

    state.update_status(&id, ConnectionStatus::Authenticating);

    // Wait for welcome message with challenge
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

    let (student_name, challenge) = match welcome {
        StudentMessage::Welcome {
            student_name,
            challenge,
        } => (student_name, challenge),
        _ => return Err("Expected welcome message".to_string()),
    };

    state.update_name(&id, student_name);

    // Sign the challenge
    let keypair = crypto::load_keypair().map_err(|e| format!("Failed to load keypair: {}", e))?;

    let challenge_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &challenge,
    )
    .map_err(|e| format!("Invalid challenge: {}", e))?;

    let signature = crypto::sign_challenge(&keypair.private_key, &challenge_bytes)
        .map_err(|e| format!("Failed to sign: {}", e))?;

    // Send auth response
    let auth_msg = TeacherMessage::AuthResponse { signature };
    let auth_json = serde_json::to_string(&auth_msg).map_err(|e| format!("Serialize error: {}", e))?;

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
            state.update_status(&id, ConnectionStatus::Error { message: reason.clone() });
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
        StudentMessage::ScreenReady { offer_sdp } => {
            log::info!("[TeacherConnector] Screen ready from {}", id);
            // TODO: Handle WebRTC offer
            if let Some(_sdp) = offer_sdp {
                // Process SDP offer
            }
            state.update_status(id, ConnectionStatus::Viewing);
        }
        StudentMessage::ScreenStopped => {
            log::info!("[TeacherConnector] Screen stopped from {}", id);
            state.update_status(id, ConnectionStatus::Connected);
        }
        StudentMessage::Error { message } => {
            log::error!("[TeacherConnector] Error from student: {}", message);
        }
        StudentMessage::Pong => {
            // Keep-alive response
        }
        _ => {
            log::warn!("[TeacherConnector] Unexpected message: {:?}", msg);
        }
    }

    Ok(())
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

        state.connections.lock().unwrap().insert("test".to_string(), conn);

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
