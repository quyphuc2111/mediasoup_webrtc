//! Student Agent - WebSocket server that allows teacher to connect and view screen
//! 
//! This module implements a mini WebSocket server on the student machine that:
//! 1. Listens for incoming connections from teacher
//! 2. Authenticates teacher using Ed25519 challenge-response
//! 3. Provides signaling for WebRTC screen sharing

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::crypto;

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
        challenge: String,  // Base64 encoded
    },
    
    #[serde(rename = "auth_success")]
    AuthSuccess,
    
    #[serde(rename = "auth_failed")]
    AuthFailed { reason: String },
    
    #[serde(rename = "screen_ready")]
    ScreenReady {
        // WebRTC signaling info will be added here
        offer_sdp: Option<String>,
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
        self.status.lock()
            .map(|s| s.clone())
            .unwrap_or(AgentStatus::Error { message: "Lock error".to_string() })
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
    
    // Accept WebSocket connection
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            log::error!("[StudentAgent] WebSocket handshake failed: {}", e);
            return;
        }
    };
    
    let (mut write, mut read) = ws_stream.split();
    
    // Generate challenge
    let challenge = crypto::generate_challenge();
    let challenge_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &challenge
    );
    
    // Store connection state
    {
        let mut conns = state.connections.lock().unwrap();
        conns.insert(addr, TeacherConnection {
            addr,
            authenticated: false,
            challenge: challenge.clone(),
        });
    }
    
    // Get student name
    let student_name = state.config.lock()
        .map(|c| c.student_name.clone())
        .unwrap_or_else(|_| "Student".to_string());
    
    // Send welcome with challenge
    let welcome = StudentMessage::Welcome {
        student_name,
        challenge: challenge_b64,
    };
    
    if let Err(e) = send_message(&mut write, &welcome).await {
        log::error!("[StudentAgent] Failed to send welcome: {}", e);
        return;
    }
    
    state.set_status(AgentStatus::Authenticating);
    
    // Message handling loop
    loop {
        tokio::select! {
            // Handle incoming messages
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Err(e) = handle_message(&text, addr, &state, &mut write).await {
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
            // Handle shutdown signal
            _ = shutdown_rx.recv() => {
                log::info!("[StudentAgent] Shutdown signal received");
                let _ = write.close().await;
                break;
            }
        }
    }
    
    // Cleanup connection
    {
        let mut conns = state.connections.lock().unwrap();
        conns.remove(&addr);
    }
    
    // Update status based on remaining connections
    let has_connections = state.connections.lock()
        .map(|c| !c.is_empty())
        .unwrap_or(false);
    
    if !has_connections {
        state.set_status(AgentStatus::WaitingForTeacher);
    }
    
    log::info!("[StudentAgent] Connection handler finished for: {}", addr);
}

/// Handle a single message from teacher
async fn handle_message<S>(
    text: &str,
    addr: SocketAddr,
    state: &Arc<AgentState>,
    write: &mut S,
) -> Result<(), String>
where
    S: SinkExt<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    let msg: TeacherMessage = serde_json::from_str(text)
        .map_err(|e| format!("Invalid message: {}", e))?;
    
    match msg {
        TeacherMessage::AuthResponse { signature } => {
            // Verify signature
            let (challenge, is_authenticated) = {
                let conns = state.connections.lock().unwrap();
                let conn = conns.get(&addr)
                    .ok_or("Connection not found")?;
                (conn.challenge.clone(), conn.authenticated)
            };
            
            if is_authenticated {
                return Ok(()); // Already authenticated
            }
            
            // Load teacher's public key and verify
            let public_key = crypto::load_teacher_public_key()
                .map_err(|e| format!("Failed to load teacher key: {}", e))?;
            
            let challenge_b64 = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &challenge
            );
            
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
                    teacher_name: "Teacher".to_string() 
                });
                
                let response = StudentMessage::AuthSuccess;
                send_message(write, &response).await?;
                
                log::info!("[StudentAgent] Teacher authenticated successfully");
            } else {
                let reason = result.error.unwrap_or_else(|| "Invalid signature".to_string());
                let response = StudentMessage::AuthFailed { reason: reason.clone() };
                send_message(write, &response).await?;
                
                return Err(format!("Authentication failed: {}", reason));
            }
        }
        
        TeacherMessage::RequestScreen => {
            // Check if authenticated
            let authenticated = {
                let conns = state.connections.lock().unwrap();
                conns.get(&addr).map(|c| c.authenticated).unwrap_or(false)
            };
            
            if !authenticated {
                let response = StudentMessage::Error { 
                    message: "Not authenticated".to_string() 
                };
                send_message(write, &response).await?;
                return Err("Not authenticated".to_string());
            }
            
            // TODO: Start screen capture and WebRTC
            // For now, just send ready signal
            let response = StudentMessage::ScreenReady {
                offer_sdp: None, // Will be populated with WebRTC offer
            };
            send_message(write, &response).await?;
            
            log::info!("[StudentAgent] Screen sharing requested");
        }
        
        TeacherMessage::StopScreen => {
            let response = StudentMessage::ScreenStopped;
            send_message(write, &response).await?;
            
            log::info!("[StudentAgent] Screen sharing stopped");
        }
        
        TeacherMessage::Ping => {
            let response = StudentMessage::Pong;
            send_message(write, &response).await?;
        }
    }
    
    Ok(())
}

/// Send a message to the WebSocket
async fn send_message<S>(write: &mut S, msg: &StudentMessage) -> Result<(), String>
where
    S: SinkExt<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    let json = serde_json::to_string(msg)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    
    write.send(Message::Text(json))
        .await
        .map_err(|e| format!("Failed to send: {}", e))
}

/// Start the student agent server
pub async fn start_agent(state: Arc<AgentState>) -> Result<(), String> {
    // Check if already running
    let current_status = state.get_status();
    if current_status != AgentStatus::Stopped {
        return Err("Agent already running".to_string());
    }
    
    // Check if teacher's public key exists
    if !crypto::has_teacher_public_key() {
        return Err("Teacher's public key not configured. Please import it first.".to_string());
    }
    
    state.set_status(AgentStatus::Starting);
    
    // Get configuration
    let port = state.config.lock()
        .map(|c| c.port)
        .unwrap_or(3017);
    
    // Create shutdown channel
    let (shutdown_tx, _) = broadcast::channel::<()>(1);
    {
        let mut tx = state.shutdown_tx.lock().unwrap();
        *tx = Some(shutdown_tx.clone());
    }
    
    // Bind to port
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await
        .map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;
    
    log::info!("[StudentAgent] Listening on: {}", addr);
    state.set_status(AgentStatus::WaitingForTeacher);
    
    // Accept connections
    loop {
        let shutdown_rx = shutdown_tx.subscribe();
        
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, addr)) => {
                        let state_clone = Arc::clone(&state);
                        tokio::spawn(handle_connection(
                            stream,
                            addr,
                            state_clone,
                            shutdown_rx,
                        ));
                    }
                    Err(e) => {
                        log::error!("[StudentAgent] Accept error: {}", e);
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
    let mut tx = state.shutdown_tx.lock()
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
            challenge: "abc123".to_string(),
        };
        
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("welcome"));
        assert!(json.contains("Test"));
    }
}
