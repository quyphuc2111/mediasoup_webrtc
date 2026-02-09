//! Teacher Connector - Connect to student agents and view their screens
//!
//! This module implements the teacher-side WebSocket client that:
//! 1. Connects to student agent on their machine
//! 2. Auto-connects without authentication
//! 3. Requests screen sharing from student

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::udp_frame_transport;

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

/// Update status for a student client
/// Requirements: 10.5
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum ClientUpdateState {
    /// Student is up to date
    UpToDate,
    /// Student needs to update
    UpdateRequired,
    /// Student is downloading update
    Downloading { progress: f32 },
    /// Student is verifying update
    Verifying,
    /// Student is installing update
    Installing,
    /// Update failed
    Failed { error: String },
}

/// Client update status for tracking student updates
/// Requirements: 10.5, 10.6
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ClientUpdateStatus {
    pub client_id: String,
    pub machine_name: Option<String>,
    pub ip: String,
    pub current_version: Option<String>,
    pub status: ClientUpdateState,
    pub progress: Option<f32>,
    pub last_updated: u64,
}

/// Information about a student connection
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct StudentConnection {
    pub id: String,
    pub ip: String,
    pub port: u16,
    pub name: Option<String>,
    pub status: ConnectionStatus,
    /// Student's current version (from handshake)
    /// Requirements: 10.5
    pub current_version: Option<String>,
    /// Machine name for identification
    pub machine_name: Option<String>,
    /// Update status for this student
    pub update_status: Option<ClientUpdateState>,
}

/// Messages from student to teacher (same as in student_agent)
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum StudentMessage {
    #[serde(rename = "welcome")]
    Welcome {
        student_name: String,
        /// Current version of the student app for version handshake
        /// Requirements: 5.1
        #[serde(skip_serializing_if = "Option::is_none")]
        current_version: Option<String>,
        /// Machine name for identification
        #[serde(skip_serializing_if = "Option::is_none")]
        machine_name: Option<String>,
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

    /// Screen status notification (e.g., login screen detected)
    #[serde(rename = "screen_status")]
    ScreenStatus {
        status: String,
        message: Option<String>,
    },

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

    /// Update status notification from student
    #[serde(rename = "update_status")]
    UpdateStatus {
        status: String, // "downloading", "verifying", "installing", "completed", "failed"
        progress: Option<f32>,
        error: Option<String>,
    },

    /// Acknowledgment of update_required broadcast
    /// Requirements: 14.4
    #[serde(rename = "update_acknowledged")]
    UpdateAcknowledged {
        version: String,
    },

    /// Student confirms UDP transport is active
    #[serde(rename = "udp_ready")]
    UdpReady,

    /// Student reports UDP failed, will use WebSocket
    #[serde(rename = "udp_fallback")]
    UdpFallback,
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
        file_data: String, // Base64 encoded
        file_size: u64,
    },

    #[serde(rename = "list_directory")]
    ListDirectory {
        path: String,
    },

    #[serde(rename = "shutdown")]
    Shutdown {
        delay_seconds: Option<u32>,
    },

    #[serde(rename = "restart")]
    Restart {
        delay_seconds: Option<u32>,
    },

    #[serde(rename = "lock_screen")]
    LockScreen,

    #[serde(rename = "logout")]
    Logout,

    /// Version handshake response to student
    /// Requirements: 5.2, 5.3
    #[serde(rename = "version_handshake_response")]
    VersionHandshakeResponse {
        required_version: String,
        mandatory_update: bool,
        update_url: Option<String>,
        sha256: Option<String>,
    },

    /// Broadcast update required notification to all students
    /// Requirements: 14.1, 14.2
    #[serde(rename = "update_required")]
    UpdateRequired {
        required_version: String,
        update_url: String,
        sha256: Option<String>,
    },

    /// Offer UDP port for frame delivery
    #[serde(rename = "udp_offer")]
    UdpOffer {
        udp_port: u16,
    },
}

/// Command to send to a connection handler
#[derive(Debug)]
pub enum ConnectionCommand {
    RequestScreen,
    StopScreen,
    Disconnect,
    SendMouseInput(MouseInputEvent),
    SendKeyboardInput(KeyboardInputEvent),
    SendTeacherMessage(TeacherMessage),
    SendFile {
        file_name: String,
        file_data: String,
        file_size: u64,
    },
    ListDirectory {
        path: String,
    },
    Shutdown {
        delay_seconds: Option<u32>,
    },
    Restart {
        delay_seconds: Option<u32>,
    },
    LockScreen,
    Logout,
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
    #[serde(default = "default_transport")]
    pub transport: String, // "udp" or "websocket"
}

fn default_transport() -> String {
    "websocket".to_string()
}

/// State for managing all student connections
pub struct ConnectorState {
    pub connections: Mutex<HashMap<String, StudentConnection>>,
    pub command_senders: Mutex<HashMap<String, mpsc::Sender<ConnectionCommand>>>,
    pub screen_frames: Mutex<HashMap<String, ScreenFrame>>,
    pub screen_sizes: Mutex<HashMap<String, (u32, u32)>>,
    pub decoders: Mutex<HashMap<String, crate::h264_decoder::H264Decoder>>,
    pub directory_responses: Mutex<HashMap<String, tokio::sync::oneshot::Sender<Result<Vec<crate::file_transfer::FileInfo>, String>>>>,
    /// Current teacher app version for version handshake
    pub current_version: Mutex<String>,
    /// LAN distribution server URL (if running)
    pub lan_update_url: Mutex<Option<String>>,
    /// SHA256 hash of the update package (if available)
    pub update_sha256: Mutex<Option<String>>,
    /// Track which students have acknowledged the update notification
    /// Requirements: 14.4
    pub update_acknowledgments: Mutex<HashMap<String, bool>>,
    /// Track transport protocol per connection ("udp" or "websocket")
    pub transport_protocols: Mutex<HashMap<String, String>>,
    /// UDP receiver stop flags per connection
    pub udp_stop_flags: Mutex<HashMap<String, Arc<std::sync::atomic::AtomicBool>>>,
}

impl Default for ConnectorState {
    fn default() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            command_senders: Mutex::new(HashMap::new()),
            screen_frames: Mutex::new(HashMap::new()),
            screen_sizes: Mutex::new(HashMap::new()),
            decoders: Mutex::new(HashMap::new()),
            directory_responses: Mutex::new(HashMap::new()),
            current_version: Mutex::new(env!("CARGO_PKG_VERSION").to_string()),
            lan_update_url: Mutex::new(None),
            update_sha256: Mutex::new(None),
            update_acknowledgments: Mutex::new(HashMap::new()),
            transport_protocols: Mutex::new(HashMap::new()),
            udp_stop_flags: Mutex::new(HashMap::new()),
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

    /// Update student version info from handshake
    /// Requirements: 10.5
    pub fn update_student_version(&self, id: &str, version: String, machine_name: Option<String>) {
        if let Ok(mut conns) = self.connections.lock() {
            if let Some(conn) = conns.get_mut(id) {
                conn.current_version = Some(version);
                conn.machine_name = machine_name;
            }
        }
    }

    /// Update student's update status
    /// Requirements: 10.5
    pub fn update_student_update_status(&self, id: &str, update_status: ClientUpdateState) {
        if let Ok(mut conns) = self.connections.lock() {
            if let Some(conn) = conns.get_mut(id) {
                conn.update_status = Some(update_status);
            }
        }
    }

    /// Get the current teacher version
    pub fn get_current_version(&self) -> String {
        self.current_version
            .lock()
            .map(|v| v.clone())
            .unwrap_or_else(|_| "0.0.0".to_string())
    }

    /// Get the LAN update URL if available
    pub fn get_lan_update_url(&self) -> Option<String> {
        self.lan_update_url.lock().ok()?.clone()
    }

    /// Get the update SHA256 hash if available
    pub fn get_update_sha256(&self) -> Option<String> {
        self.update_sha256.lock().ok()?.clone()
    }

    /// Set the LAN distribution info
    pub fn set_lan_distribution(&self, url: Option<String>, sha256: Option<String>) {
        if let Ok(mut u) = self.lan_update_url.lock() {
            *u = url;
        }
        if let Ok(mut h) = self.update_sha256.lock() {
            *h = sha256;
        }
    }

    /// Check if all connected students are up to date
    /// Requirements: 14.5
    pub fn all_students_up_to_date(&self) -> bool {
        if let Ok(conns) = self.connections.lock() {
            conns.values().all(|conn| {
                matches!(conn.update_status, Some(ClientUpdateState::UpToDate) | None)
            })
        } else {
            false
        }
    }

    /// Record that a student has acknowledged the update notification
    /// Requirements: 14.4
    pub fn record_acknowledgment(&self, id: &str, version: &str) {
        if let Ok(mut acks) = self.update_acknowledgments.lock() {
            acks.insert(id.to_string(), true);
            log::info!(
                "[TeacherConnector] Student {} acknowledged update to version {}",
                id,
                version
            );
        }
    }

    /// Clear all acknowledgments (called when a new broadcast is sent)
    /// Requirements: 14.4
    pub fn clear_acknowledgments(&self) {
        if let Ok(mut acks) = self.update_acknowledgments.lock() {
            acks.clear();
        }
    }

    /// Check if all connected students have acknowledged the update
    /// Requirements: 14.4
    pub fn all_students_acknowledged(&self) -> bool {
        let conns = match self.connections.lock() {
            Ok(c) => c,
            Err(_) => return false,
        };
        let acks = match self.update_acknowledgments.lock() {
            Ok(a) => a,
            Err(_) => return false,
        };

        // Check that all connected students have acknowledged
        conns.keys().all(|id| acks.get(id).copied().unwrap_or(false))
    }

    /// Get the list of students who have not yet acknowledged
    /// Requirements: 14.4
    pub fn get_pending_acknowledgments(&self) -> Vec<String> {
        let conns = match self.connections.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let acks = match self.update_acknowledgments.lock() {
            Ok(a) => a,
            Err(_) => return Vec::new(),
        };

        conns
            .keys()
            .filter(|id| !acks.get(*id).copied().unwrap_or(false))
            .cloned()
            .collect()
    }

    /// Get update status for all connected students
    /// Requirements: 10.5, 10.6
    pub fn get_all_client_update_status(&self) -> Vec<ClientUpdateStatus> {
        let conns = match self.connections.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let current_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        conns
            .values()
            .map(|conn| {
                let status = conn.update_status.clone().unwrap_or(ClientUpdateState::UpToDate);
                let progress = match &status {
                    ClientUpdateState::Downloading { progress } => Some(*progress),
                    _ => None,
                };

                ClientUpdateStatus {
                    client_id: conn.id.clone(),
                    machine_name: conn.machine_name.clone(),
                    ip: conn.ip.clone(),
                    current_version: conn.current_version.clone(),
                    status,
                    progress,
                    last_updated: current_timestamp,
                }
            })
            .collect()
    }

    pub fn remove_connection(&self, id: &str) {
        if let Ok(mut conns) = self.connections.lock() {
            conns.remove(id);
        }
        if let Ok(mut senders) = self.command_senders.lock() {
            senders.remove(id);
        }
        if let Ok(mut protos) = self.transport_protocols.lock() {
            protos.remove(id);
        }
        // Stop UDP receiver if running
        if let Ok(mut flags) = self.udp_stop_flags.lock() {
            if let Some(flag) = flags.remove(id) {
                flag.store(true, Ordering::Relaxed);
            }
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

    // Create connection entry
    let connection = StudentConnection {
        id: id.clone(),
        ip: ip.clone(),
        port,
        name: None,
        status: ConnectionStatus::Connecting,
        current_version: None,
        machine_name: None,
        update_status: None,
    };

    // Store connection
    {
        let mut conns = state.connections.lock().map_err(|e| e.to_string())?;
        conns.insert(id.clone(), connection);
    }

    // Create command channel with larger buffer for mouse events (100 events)
    let (cmd_tx, cmd_rx) = mpsc::channel::<ConnectionCommand>(100);
    {
        let mut senders = state.command_senders.lock().map_err(|e| e.to_string())?;
        senders.insert(id.clone(), cmd_tx);
    }

    // Start connection handler
    let state_clone = Arc::clone(&state);
    let id_clone = id.clone();
    let ip_clone = ip.clone();

    tokio::spawn(async move {
        if let Err(e) = handle_connection(
            state_clone,
            id_clone.clone(),
            ip_clone,
            port,
            cmd_rx,
            None, // No app handle initially
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
    app_handle: AppHandle,
) -> Result<(), String> {
    println!(
        "[TeacherConnector] handle_connection_async called for {}:{}",
        ip, port
    );

    // Create command channel internally with larger buffer for mouse events (100 events)
    let (cmd_tx, cmd_rx) = mpsc::channel::<ConnectionCommand>(100);
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
        Some(app_handle),
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
    app_handle: Option<AppHandle>,
) -> Result<(), String> {
    let url = format!("ws://{}:{}", ip, port);
    println!(
        "[TeacherConnector] Attempting WebSocket connection to: {}",
        url
    );

    // WebSocket config with increased message size limit (100MB)
    let ws_config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
        max_message_size: Some(100 * 1024 * 1024), // 100MB
        max_frame_size: Some(16 * 1024 * 1024),    // 16MB per frame
        ..Default::default()
    };

    // Connect to student with config
    let (ws_stream, _) = tokio_tungstenite::connect_async_with_config(&url, Some(ws_config), false)
        .await
        .map_err(|e| {
            println!("[TeacherConnector] WebSocket connect failed: {}", e);
            format!("Failed to connect: {}", e)
        })?;

    println!("[TeacherConnector] WebSocket connected to: {}", url);

    let (mut write, mut read) = ws_stream.split();

    // Wait for welcome message (no auth required)
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

    let (student_name, student_version, machine_name) = match welcome {
        StudentMessage::Welcome { student_name, current_version, machine_name } => {
            (student_name, current_version, machine_name)
        }
        _ => return Err("Expected welcome message".to_string()),
    };

    state.update_name(&id, student_name.clone());
    
    // Update student version info if provided
    // Requirements: 10.5 - Track student versions in connection state
    if let Some(ref version) = student_version {
        state.update_student_version(&id, version.clone(), machine_name.clone());
        log::info!(
            "[TeacherConnector] Student {} version: {}, machine: {:?}",
            student_name,
            version,
            machine_name
        );
    }

    // Perform version handshake
    // Requirements: 5.2, 5.3 - Check student version and send required_version
    let teacher_version = state.get_current_version();
    let mandatory_update = student_version
        .as_ref()
        .map(|v| v != &teacher_version)
        .unwrap_or(false);

    // Determine update status
    let update_status = if mandatory_update {
        ClientUpdateState::UpdateRequired
    } else {
        ClientUpdateState::UpToDate
    };
    state.update_student_update_status(&id, update_status);

    // Send version handshake response
    let handshake_response = TeacherMessage::VersionHandshakeResponse {
        required_version: teacher_version.clone(),
        mandatory_update,
        update_url: state.get_lan_update_url(),
        sha256: state.get_update_sha256(),
    };

    let handshake_json = serde_json::to_string(&handshake_response)
        .map_err(|e| format!("Failed to serialize handshake: {}", e))?;
    write
        .send(Message::Text(handshake_json))
        .await
        .map_err(|e| format!("Failed to send handshake: {}", e))?;

    log::info!(
        "[TeacherConnector] Sent version handshake to {}: required={}, mandatory={}",
        student_name,
        teacher_version,
        mandatory_update
    );

    state.update_status(&id, ConnectionStatus::Connected);

    println!("[TeacherConnector] Connected to student: {}", student_name);

    // Wait for screen_ready message (auto-started by student)
    let screen_ready_msg = read
        .next()
        .await
        .ok_or("Connection closed")?
        .map_err(|e| format!("WebSocket error: {}", e))?;

    let screen_ready_text = match screen_ready_msg {
        Message::Text(text) => text,
        _ => return Err("Expected text message".to_string()),
    };

    let screen_ready: StudentMessage =
        serde_json::from_str(&screen_ready_text).map_err(|e| format!("Invalid screen_ready: {}", e))?;

    match screen_ready {
        StudentMessage::ScreenReady { width, height } => {
            println!("[TeacherConnector] Screen ready: {}x{}", width, height);
            state.update_status(&id, ConnectionStatus::Viewing);
            
            // Store screen size
            if let Ok(mut sizes) = state.screen_sizes.lock() {
                sizes.insert(id.clone(), (width, height));
            }
        }
        _ => {
            println!("[TeacherConnector] Unexpected message, expected screen_ready");
        }
    }

    // --- Start UDP receiver and offer UDP transport ---
    let udp_stop = Arc::new(AtomicBool::new(false));
    {
        if let Ok(mut flags) = state.udp_stop_flags.lock() {
            flags.insert(id.clone(), Arc::clone(&udp_stop));
        }
    }
    // Default to websocket
    if let Ok(mut protos) = state.transport_protocols.lock() {
        protos.insert(id.clone(), "websocket".to_string());
    }

    let (udp_frame_tx, mut udp_frame_rx) = mpsc::channel::<udp_frame_transport::ReassembledFrame>(16);
    let udp_port_result = udp_frame_transport::start_udp_receiver(udp_frame_tx, Arc::clone(&udp_stop)).await;

    if let Ok(udp_port) = udp_port_result {
        log::info!("[TeacherConnector] UDP receiver started on port {}, sending offer to student", udp_port);
        let offer = TeacherMessage::UdpOffer { udp_port };
        let offer_json = serde_json::to_string(&offer).unwrap();
        let _ = write.send(Message::Text(offer_json)).await;
    } else {
        log::warn!("[TeacherConnector] Failed to start UDP receiver, using WebSocket only");
    }

    // Message handling loop
    loop {
        tokio::select! {
            // Handle UDP frames (primary transport)
            Some(udp_frame) = udp_frame_rx.recv() => {
                // Mark transport as UDP
                if let Ok(mut protos) = state.transport_protocols.lock() {
                    protos.insert(id.clone(), "udp".to_string());
                }

                let frame = ScreenFrame {
                    data: None,
                    data_binary: Some(udp_frame.h264_data),
                    sps_pps: udp_frame.sps_pps,
                    timestamp: udp_frame.timestamp,
                    width: udp_frame.width,
                    height: udp_frame.height,
                    is_keyframe: udp_frame.is_keyframe,
                    codec: "h264".to_string(),
                    transport: "udp".to_string(),
                };

                if let Ok(mut frames) = state.screen_frames.lock() {
                    frames.insert(id.clone(), frame.clone());
                }

                if let Some(ref app) = app_handle {
                    let _ = app.emit("screen-frame", (id.clone(), frame));
                }
            }
            // Handle incoming messages from student
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Err(e) = handle_student_message(&text, &state, &id, &app_handle).await {
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

                            // Extract SPS/PPS (AVCC description)
                            let sps_pps = if desc_len > 0 {
                                Some(data[desc_start..desc_end].to_vec())
                            } else {
                                None
                            };

                            // Extract H.264 Annex-B data
                            let h264_data = data[desc_end..].to_vec();

                            // Create the ScreenFrame with raw binary data - NO CPU-HEAVY DECODING HERE
                            let frame = ScreenFrame {
                                data: None, // No JPEG base64 string
                                data_binary: Some(h264_data),
                                sps_pps,
                                timestamp,
                                width,
                                height,
                                is_keyframe,
                                codec: "h264".to_string(),
                                transport: "websocket".to_string(),
                            };

                            // Update state
                            if let Ok(mut frames) = state.screen_frames.lock() {
                                frames.insert(id.clone(), frame.clone());
                            }

                            // Emit event to frontend for real-time push
                            if let Some(ref app) = app_handle {
                                let _ = app.emit("screen-frame", (id.clone(), frame));
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

            // Handle commands from app with batching for mouse move events
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
                    Some(ConnectionCommand::SendTeacherMessage(msg)) => {
                        let json = serde_json::to_string(&msg).unwrap();
                        let _ = write.send(Message::Text(json)).await;
                    }
                    Some(ConnectionCommand::SendMouseInput(event)) => {
                        // Batch mouse move events for better performance
                        // Collect up to 10 move events or wait max 16ms
                        const BATCH_TIMEOUT_MS: u64 = 16; // ~60fps
                        const MAX_BATCH_SIZE: usize = 10;

                        // Only batch move events, send others immediately
                        if event.event_type == "move" {
                            let mut events = vec![event];
                            let start = std::time::Instant::now();
                            let mut pending_non_move: Option<MouseInputEvent> = None;
                            let mut pending_other_cmd: Option<ConnectionCommand> = None;

                            while events.len() < MAX_BATCH_SIZE && start.elapsed().as_millis() < BATCH_TIMEOUT_MS as u128 {
                                let remaining_time = BATCH_TIMEOUT_MS.saturating_sub(start.elapsed().as_millis() as u64);
                                match tokio::time::timeout(
                                    std::time::Duration::from_millis(remaining_time.max(1)),
                                    cmd_rx.recv()
                                ).await {
                                    Ok(Some(ConnectionCommand::SendMouseInput(next_event))) => {
                                        if next_event.event_type == "move" {
                                            events.push(next_event);
                                        } else {
                                            // Non-move event, save it and break
                                            pending_non_move = Some(next_event);
                                            break;
                                        }
                                    }
                                    Ok(Some(other_cmd)) => {
                                        // Other command, save it and break
                                        pending_other_cmd = Some(other_cmd);
                                        break;
                                    }
                                    Ok(None) => break,
                                    Err(_) => break,
                                }
                            }

                            // Send batched events
                            if events.len() > 1 {
                                let msg = TeacherMessage::MouseInputBatch { events };
                                let json = serde_json::to_string(&msg).unwrap();
                                let _ = write.send(Message::Text(json)).await;
                            } else if !events.is_empty() {
                                let msg = TeacherMessage::MouseInput { event: events.remove(0) };
                                let json = serde_json::to_string(&msg).unwrap();
                                let _ = write.send(Message::Text(json)).await;
                            }

                            // Handle pending non-move event
                            if let Some(non_move_event) = pending_non_move {
                                let msg = TeacherMessage::MouseInput { event: non_move_event };
                                let json = serde_json::to_string(&msg).unwrap();
                                let _ = write.send(Message::Text(json)).await;
                            }

                            // Handle pending other command
                            if let Some(other_cmd) = pending_other_cmd {
                                match other_cmd {
                                    ConnectionCommand::RequestScreen => {
                                        let msg = TeacherMessage::RequestScreen;
                                        let json = serde_json::to_string(&msg).unwrap();
                                        let _ = write.send(Message::Text(json)).await;
                                        state.update_status(&id, ConnectionStatus::Viewing);
                                    }
                                    ConnectionCommand::StopScreen => {
                                        let msg = TeacherMessage::StopScreen;
                                        let json = serde_json::to_string(&msg).unwrap();
                                        let _ = write.send(Message::Text(json)).await;
                                        state.update_status(&id, ConnectionStatus::Connected);
                                    }
                                    ConnectionCommand::SendTeacherMessage(msg) => {
                                        let json = serde_json::to_string(&msg).unwrap();
                                        let _ = write.send(Message::Text(json)).await;
                                    }
                                    ConnectionCommand::SendKeyboardInput(event) => {
                                        let msg = TeacherMessage::KeyboardInput { event };
                                        let json = serde_json::to_string(&msg).unwrap();
                                        let _ = write.send(Message::Text(json)).await;
                                    }
                                    ConnectionCommand::SendFile { file_name, file_data, file_size } => {
                                        log::info!("[TeacherConnector] Sending file: {} ({} bytes)", file_name, file_size);
                                        let msg = TeacherMessage::SendFile {
                                            file_name,
                                            file_data,
                                            file_size,
                                        };
                                        let json = serde_json::to_string(&msg).unwrap();
                                        let _ = write.send(Message::Text(json)).await;
                                    }
                                    ConnectionCommand::ListDirectory { path } => {
                                        log::info!("[TeacherConnector] Requesting directory listing: {}", path);
                                        let msg = TeacherMessage::ListDirectory { path };
                                        let json = serde_json::to_string(&msg).unwrap();
                                        let _ = write.send(Message::Text(json)).await;
                                    }
                                    ConnectionCommand::Disconnect => {
                                        log::info!("[TeacherConnector] Disconnect command received");
                                        let _ = write.close().await;
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        } else {
                            // Non-move event, send immediately
                            let msg = TeacherMessage::MouseInput { event };
                            let json = serde_json::to_string(&msg).unwrap();
                            let _ = write.send(Message::Text(json)).await;
                        }
                    }
                    Some(ConnectionCommand::SendKeyboardInput(event)) => {
                        let msg = TeacherMessage::KeyboardInput { event };
                        let json = serde_json::to_string(&msg).unwrap();
                        let _ = write.send(Message::Text(json)).await;
                    }
                    Some(ConnectionCommand::SendFile { file_name, file_data, file_size }) => {
                        log::info!("[TeacherConnector] Sending file: {} ({} bytes)", file_name, file_size);
                        let msg = TeacherMessage::SendFile {
                            file_name,
                            file_data,
                            file_size,
                        };
                        let json = serde_json::to_string(&msg).unwrap();
                        let _ = write.send(Message::Text(json)).await;
                    }
                    Some(ConnectionCommand::ListDirectory { path }) => {
                        log::info!("[TeacherConnector] Requesting directory listing: {}", path);
                        let msg = TeacherMessage::ListDirectory { path };
                        let json = serde_json::to_string(&msg).unwrap();
                        let _ = write.send(Message::Text(json)).await;
                    }
                    Some(ConnectionCommand::Shutdown { delay_seconds }) => {
                        log::info!("[TeacherConnector] Sending shutdown command");
                        let msg = TeacherMessage::Shutdown { delay_seconds };
                        let json = serde_json::to_string(&msg).unwrap();
                        let _ = write.send(Message::Text(json)).await;
                    }
                    Some(ConnectionCommand::Restart { delay_seconds }) => {
                        log::info!("[TeacherConnector] Sending restart command");
                        let msg = TeacherMessage::Restart { delay_seconds };
                        let json = serde_json::to_string(&msg).unwrap();
                        let _ = write.send(Message::Text(json)).await;
                    }
                    Some(ConnectionCommand::LockScreen) => {
                        log::info!("[TeacherConnector] Sending lock screen command");
                        let msg = TeacherMessage::LockScreen;
                        let json = serde_json::to_string(&msg).unwrap();
                        let _ = write.send(Message::Text(json)).await;
                    }
                    Some(ConnectionCommand::Logout) => {
                        log::info!("[TeacherConnector] Sending logout command");
                        let msg = TeacherMessage::Logout;
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
    udp_stop.store(true, Ordering::Relaxed);
    if let Ok(mut flags) = state.udp_stop_flags.lock() {
        flags.remove(&id);
    }
    if let Ok(mut protos) = state.transport_protocols.lock() {
        protos.remove(&id);
    }
    state.update_status(&id, ConnectionStatus::Disconnected);
    log::info!("[TeacherConnector] Connection closed: {}", id);

    Ok(())
}

/// Handle a message from student
async fn handle_student_message(
    text: &str,
    state: &Arc<ConnectorState>,
    id: &str,
    app_handle: &Option<AppHandle>,
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
                transport: "websocket".to_string(),
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
        StudentMessage::ScreenStatus { status, message } => {
            log::info!("[TeacherConnector] Screen status from {}: {} - {:?}", id, status, message);
            
            // Emit event to frontend so UI can show appropriate state
            if let Some(ref app) = app_handle {
                let _ = app.emit("student-screen-status", serde_json::json!({
                    "studentId": id,
                    "status": status,
                    "message": message,
                }));
            }
        }
        StudentMessage::Error { message } => {
            println!("[TeacherConnector] Error from student: {}", message);
        }
        StudentMessage::Pong => {
            // Keep-alive response
        }
        StudentMessage::DirectoryListing { path: _, files } => {
            log::info!("[TeacherConnector] Received directory listing with {} files", files.len());
            // Send response to waiting request
            if let Ok(mut responses) = state.directory_responses.lock() {
                if let Some(sender) = responses.remove(id) {
                    let _ = sender.send(Ok(files));
                }
            }
        }
        StudentMessage::FileReceived { file_name, success, message } => {
            log::info!("[TeacherConnector] File received response: {} - {} - {}", file_name, success, message);
        }
        StudentMessage::UpdateStatus { status, progress, error } => {
            // Handle update status from student
            // Requirements: 10.5, 10.6 - Track and display student update status
            log::info!(
                "[TeacherConnector] Update status from {}: status={}, progress={:?}, error={:?}",
                id,
                status,
                progress,
                error
            );

            let update_state = match status.as_str() {
                "update_required" => ClientUpdateState::UpdateRequired,
                "downloading" => ClientUpdateState::Downloading {
                    progress: progress.unwrap_or(0.0),
                },
                "verifying" => ClientUpdateState::Verifying,
                "installing" => ClientUpdateState::Installing,
                "completed" => ClientUpdateState::UpToDate,
                "failed" => ClientUpdateState::Failed {
                    error: error.unwrap_or_else(|| "Unknown error".to_string()),
                },
                _ => ClientUpdateState::UpdateRequired,
            };

            state.update_student_update_status(id, update_state);
        }
        StudentMessage::Welcome { .. } => {
            // Welcome message is handled during connection setup
            log::debug!("[TeacherConnector] Received welcome message (already processed)");
        }
        StudentMessage::UpdateAcknowledged { version } => {
            // Handle update acknowledgment from student
            // Requirements: 14.4 - Track which Students have acknowledged the update notification
            log::info!(
                "[TeacherConnector] Student {} acknowledged update to version {}",
                id,
                version
            );
            state.record_acknowledgment(id, &version);
        }
        StudentMessage::UdpReady => {
            log::info!("[TeacherConnector] Student {} confirmed UDP transport", id);
            if let Ok(mut protos) = state.transport_protocols.lock() {
                protos.insert(id.to_string(), "udp".to_string());
            }
        }
        StudentMessage::UdpFallback => {
            log::info!("[TeacherConnector] Student {} falling back to WebSocket transport", id);
            if let Ok(mut protos) = state.transport_protocols.lock() {
                protos.insert(id.to_string(), "websocket".to_string());
            }
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
pub fn send_mouse_input(
    state: &ConnectorState,
    id: &str,
    event: MouseInputEvent,
) -> Result<(), String> {
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
pub fn send_keyboard_input(
    state: &ConnectorState,
    id: &str,
    event: KeyboardInputEvent,
) -> Result<(), String> {
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

/// Request a keyframe from student
pub fn request_keyframe(state: &ConnectorState, id: &str) -> Result<(), String> {
    let senders = state.command_senders.lock().map_err(|e| e.to_string())?;

    if let Some(sender) = senders.get(id) {
        sender
            .try_send(ConnectionCommand::SendTeacherMessage(
                TeacherMessage::RequestKeyframe,
            ))
            .map_err(|e| format!("Failed to request keyframe: {}", e))?;
    } else {
        return Err("Connection not found".to_string());
    }

    Ok(())
}

/// Send file to student
pub fn send_file(
    state: &ConnectorState,
    id: &str,
    file_name: String,
    file_data: String,
    file_size: u64,
) -> Result<(), String> {
    let senders = state.command_senders.lock().map_err(|e| e.to_string())?;

    if let Some(sender) = senders.get(id) {
        sender
            .try_send(ConnectionCommand::SendFile {
                file_name,
                file_data,
                file_size,
            })
            .map_err(|e| format!("Failed to send file: {}", e))?;
    } else {
        return Err("Connection not found".to_string());
    }

    Ok(())
}

/// Request directory listing from student
pub async fn list_student_directory(
    state: &ConnectorState,
    id: &str,
    path: String,
) -> Result<Vec<crate::file_transfer::FileInfo>, String> {
    // Create oneshot channel for response
    let (tx, rx) = tokio::sync::oneshot::channel();

    // Store the sender
    {
        let mut responses = state.directory_responses.lock().map_err(|e| e.to_string())?;
        responses.insert(id.to_string(), tx);
    }

    // Send the request
    {
        let senders = state.command_senders.lock().map_err(|e| e.to_string())?;
        if let Some(sender) = senders.get(id) {
            sender
                .try_send(ConnectionCommand::ListDirectory { path })
                .map_err(|e| format!("Failed to send command: {}", e))?;
        } else {
            // Remove the pending response
            if let Ok(mut responses) = state.directory_responses.lock() {
                responses.remove(id);
            }
            return Err("Connection not found".to_string());
        }
    }

    // Wait for response with timeout
    match tokio::time::timeout(std::time::Duration::from_secs(10), rx).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("Response channel closed".to_string()),
        Err(_) => {
            // Remove the pending response on timeout
            if let Ok(mut responses) = state.directory_responses.lock() {
                responses.remove(id);
            }
            Err("Request timed out".to_string())
        }
    }
}

/// Send shutdown command to student
pub fn send_shutdown(state: &ConnectorState, id: &str, delay_seconds: Option<u32>) -> Result<(), String> {
    let senders = state.command_senders.lock().map_err(|e| e.to_string())?;

    if let Some(sender) = senders.get(id) {
        sender
            .try_send(ConnectionCommand::SendTeacherMessage(
                TeacherMessage::Shutdown { delay_seconds },
            ))
            .map_err(|e| format!("Failed to send shutdown command: {}", e))?;
    } else {
        return Err("Connection not found".to_string());
    }

    Ok(())
}

/// Send restart command to student
pub fn send_restart(state: &ConnectorState, id: &str, delay_seconds: Option<u32>) -> Result<(), String> {
    let senders = state.command_senders.lock().map_err(|e| e.to_string())?;

    if let Some(sender) = senders.get(id) {
        sender
            .try_send(ConnectionCommand::SendTeacherMessage(
                TeacherMessage::Restart { delay_seconds },
            ))
            .map_err(|e| format!("Failed to send restart command: {}", e))?;
    } else {
        return Err("Connection not found".to_string());
    }

    Ok(())
}

/// Send lock screen command to student
pub fn send_lock_screen(state: &ConnectorState, id: &str) -> Result<(), String> {
    let senders = state.command_senders.lock().map_err(|e| e.to_string())?;

    if let Some(sender) = senders.get(id) {
        sender
            .try_send(ConnectionCommand::SendTeacherMessage(
                TeacherMessage::LockScreen,
            ))
            .map_err(|e| format!("Failed to send lock screen command: {}", e))?;
    } else {
        return Err("Connection not found".to_string());
    }

    Ok(())
}

/// Send logout command to student
pub fn send_logout(state: &ConnectorState, id: &str) -> Result<(), String> {
    let senders = state.command_senders.lock().map_err(|e| e.to_string())?;

    if let Some(sender) = senders.get(id) {
        sender
            .try_send(ConnectionCommand::SendTeacherMessage(
                TeacherMessage::Logout,
            ))
            .map_err(|e| format!("Failed to send logout command: {}", e))?;
    } else {
        return Err("Connection not found".to_string());
    }

    Ok(())
}

/// Broadcast result for tracking which students received the message
/// Requirements: 14.1, 14.4
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct BroadcastResult {
    /// Total number of connected students
    pub total_students: usize,
    /// Number of students who received the message
    pub sent_count: usize,
    /// IDs of students who failed to receive the message
    pub failed_ids: Vec<String>,
}

/// Broadcast update_required message to all connected students
/// Requirements: 14.1, 14.2, 14.4
///
/// This function sends an update_required message to all connected students,
/// including the required_version and download endpoint URL.
/// It also clears previous acknowledgments and tracks which students received the message.
pub fn broadcast_update_required(
    state: &ConnectorState,
    required_version: String,
    update_url: String,
    sha256: Option<String>,
) -> Result<BroadcastResult, String> {
    // Clear previous acknowledgments before broadcasting
    state.clear_acknowledgments();

    let senders = state.command_senders.lock().map_err(|e| e.to_string())?;
    let connections = state.connections.lock().map_err(|e| e.to_string())?;

    let total_students = connections.len();
    let mut sent_count = 0;
    let mut failed_ids = Vec::new();

    // Create the broadcast message
    let msg = TeacherMessage::UpdateRequired {
        required_version: required_version.clone(),
        update_url: update_url.clone(),
        sha256: sha256.clone(),
    };

    log::info!(
        "[TeacherConnector] Broadcasting update_required to {} students: version={}, url={}",
        total_students,
        required_version,
        update_url
    );

    // Send to all connected students
    for (id, _conn) in connections.iter() {
        if let Some(sender) = senders.get(id) {
            match sender.try_send(ConnectionCommand::SendTeacherMessage(msg.clone())) {
                Ok(_) => {
                    sent_count += 1;
                    log::debug!("[TeacherConnector] Sent update_required to {}", id);
                }
                Err(e) => {
                    log::warn!(
                        "[TeacherConnector] Failed to send update_required to {}: {}",
                        id,
                        e
                    );
                    failed_ids.push(id.clone());
                }
            }
        } else {
            log::warn!(
                "[TeacherConnector] No command sender found for student {}",
                id
            );
            failed_ids.push(id.clone());
        }
    }

    // Update the LAN distribution info in state
    state.set_lan_distribution(Some(update_url), sha256);

    // Update all students' update status to UpdateRequired
    drop(connections); // Release the lock before calling update_student_update_status
    drop(senders);

    if let Ok(conns) = state.connections.lock() {
        for id in conns.keys() {
            // We need to drop the lock before calling update_student_update_status
            let id_clone = id.clone();
            drop(conns);
            state.update_student_update_status(&id_clone, ClientUpdateState::UpdateRequired);
            // Re-acquire the lock for the next iteration
            break; // We'll handle this differently
        }
    }

    // Update all students' update status
    let student_ids: Vec<String> = state
        .connections
        .lock()
        .map(|c| c.keys().cloned().collect())
        .unwrap_or_default();

    for id in student_ids {
        state.update_student_update_status(&id, ClientUpdateState::UpdateRequired);
    }

    log::info!(
        "[TeacherConnector] Broadcast complete: {}/{} students notified",
        sent_count,
        total_students
    );

    Ok(BroadcastResult {
        total_students,
        sent_count,
        failed_ids,
    })
}

/// Send update_required to a specific student
/// Requirements: 14.1, 14.2
///
/// This is useful for sending the update notification to a newly connected student
/// after the initial broadcast has already been sent.
pub fn send_update_required(
    state: &ConnectorState,
    id: &str,
    required_version: String,
    update_url: String,
    sha256: Option<String>,
) -> Result<(), String> {
    let senders = state.command_senders.lock().map_err(|e| e.to_string())?;

    if let Some(sender) = senders.get(id) {
        let msg = TeacherMessage::UpdateRequired {
            required_version: required_version.clone(),
            update_url,
            sha256,
        };

        sender
            .try_send(ConnectionCommand::SendTeacherMessage(msg))
            .map_err(|e| format!("Failed to send update_required: {}", e))?;

        log::info!(
            "[TeacherConnector] Sent update_required to {}: version={}",
            id,
            required_version
        );

        // Update the student's update status
        drop(senders);
        state.update_student_update_status(id, ClientUpdateState::UpdateRequired);
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
            current_version: Some("1.0.0".to_string()),
            machine_name: Some("TestPC".to_string()),
            update_status: None,
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
        let msg = TeacherMessage::Shutdown {
            delay_seconds: Some(5),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("shutdown"));
    }

    #[test]
    fn test_version_handshake_response() {
        let msg = TeacherMessage::VersionHandshakeResponse {
            required_version: "1.1.0".to_string(),
            mandatory_update: true,
            update_url: Some("http://localhost:9280/update".to_string()),
            sha256: Some("abc123".to_string()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("version_handshake_response"));
        assert!(json.contains("1.1.0"));
    }

    #[test]
    fn test_student_version_tracking() {
        let state = ConnectorState::new();

        let conn = StudentConnection {
            id: "test".to_string(),
            ip: "192.168.1.1".to_string(),
            port: 3017,
            name: Some("Student1".to_string()),
            status: ConnectionStatus::Connected,
            current_version: None,
            machine_name: None,
            update_status: None,
        };

        state
            .connections
            .lock()
            .unwrap()
            .insert("test".to_string(), conn);

        // Update version info
        state.update_student_version("test", "1.0.0".to_string(), Some("Lab-PC-01".to_string()));

        let retrieved = state.get_connection("test").unwrap();
        assert_eq!(retrieved.current_version, Some("1.0.0".to_string()));
        assert_eq!(retrieved.machine_name, Some("Lab-PC-01".to_string()));
    }

    #[test]
    fn test_update_status_tracking() {
        let state = ConnectorState::new();

        let conn = StudentConnection {
            id: "test".to_string(),
            ip: "192.168.1.1".to_string(),
            port: 3017,
            name: Some("Student1".to_string()),
            status: ConnectionStatus::Connected,
            current_version: Some("1.0.0".to_string()),
            machine_name: None,
            update_status: None,
        };

        state
            .connections
            .lock()
            .unwrap()
            .insert("test".to_string(), conn);

        // Update status
        state.update_student_update_status("test", ClientUpdateState::Downloading { progress: 50.0 });

        let retrieved = state.get_connection("test").unwrap();
        assert!(matches!(
            retrieved.update_status,
            Some(ClientUpdateState::Downloading { progress: _ })
        ));
    }

    #[test]
    fn test_all_students_up_to_date() {
        let state = ConnectorState::new();

        // No connections - should return true
        assert!(state.all_students_up_to_date());

        // Add a student that's up to date
        let conn = StudentConnection {
            id: "test".to_string(),
            ip: "192.168.1.1".to_string(),
            port: 3017,
            name: Some("Student1".to_string()),
            status: ConnectionStatus::Connected,
            current_version: Some("1.0.0".to_string()),
            machine_name: None,
            update_status: Some(ClientUpdateState::UpToDate),
        };

        state
            .connections
            .lock()
            .unwrap()
            .insert("test".to_string(), conn);

        assert!(state.all_students_up_to_date());

        // Change to update required
        state.update_student_update_status("test", ClientUpdateState::UpdateRequired);
        assert!(!state.all_students_up_to_date());
    }

    #[test]
    fn test_update_required_message_serialization() {
        let msg = TeacherMessage::UpdateRequired {
            required_version: "1.2.0".to_string(),
            update_url: "http://192.168.1.100:9280/update".to_string(),
            sha256: Some("abc123def456".to_string()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("update_required"));
        assert!(json.contains("1.2.0"));
        assert!(json.contains("http://192.168.1.100:9280/update"));
        assert!(json.contains("abc123def456"));
    }

    #[test]
    fn test_update_acknowledged_message_deserialization() {
        let json = r#"{"type":"update_acknowledged","version":"1.2.0"}"#;
        let msg: StudentMessage = serde_json::from_str(json).unwrap();

        match msg {
            StudentMessage::UpdateAcknowledged { version } => {
                assert_eq!(version, "1.2.0");
            }
            _ => panic!("Expected UpdateAcknowledged message"),
        }
    }

    #[test]
    fn test_acknowledgment_tracking() {
        let state = ConnectorState::new();

        // Add two students
        let conn1 = StudentConnection {
            id: "student1".to_string(),
            ip: "192.168.1.1".to_string(),
            port: 3017,
            name: Some("Student1".to_string()),
            status: ConnectionStatus::Connected,
            current_version: Some("1.0.0".to_string()),
            machine_name: None,
            update_status: Some(ClientUpdateState::UpdateRequired),
        };

        let conn2 = StudentConnection {
            id: "student2".to_string(),
            ip: "192.168.1.2".to_string(),
            port: 3017,
            name: Some("Student2".to_string()),
            status: ConnectionStatus::Connected,
            current_version: Some("1.0.0".to_string()),
            machine_name: None,
            update_status: Some(ClientUpdateState::UpdateRequired),
        };

        state
            .connections
            .lock()
            .unwrap()
            .insert("student1".to_string(), conn1);
        state
            .connections
            .lock()
            .unwrap()
            .insert("student2".to_string(), conn2);

        // Initially no acknowledgments
        assert!(!state.all_students_acknowledged());
        assert_eq!(state.get_pending_acknowledgments().len(), 2);

        // First student acknowledges
        state.record_acknowledgment("student1", "1.2.0");
        assert!(!state.all_students_acknowledged());
        assert_eq!(state.get_pending_acknowledgments().len(), 1);
        assert!(state.get_pending_acknowledgments().contains(&"student2".to_string()));

        // Second student acknowledges
        state.record_acknowledgment("student2", "1.2.0");
        assert!(state.all_students_acknowledged());
        assert_eq!(state.get_pending_acknowledgments().len(), 0);

        // Clear acknowledgments
        state.clear_acknowledgments();
        assert!(!state.all_students_acknowledged());
        assert_eq!(state.get_pending_acknowledgments().len(), 2);
    }

    #[test]
    fn test_broadcast_result_serialization() {
        let result = BroadcastResult {
            total_students: 5,
            sent_count: 4,
            failed_ids: vec!["student3".to_string()],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("total_students"));
        assert!(json.contains("5"));
        assert!(json.contains("sent_count"));
        assert!(json.contains("4"));
        assert!(json.contains("student3"));
    }
}

// --- Binary Protocol Helpers ---

fn create_mouse_packet(event: &MouseInputEvent) -> Vec<u8> {
    let mut buf = Vec::with_capacity(19);
    buf.push(1); // Type=Mouse

    let type_byte = match event.event_type.as_str() {
        "move" => 0,
        "click" => 1,
        "down" => 2,
        "up" => 3,
        "scroll" => 4,
        _ => 0,
    };
    buf.push(type_byte);

    let btn_byte = match event.button {
        Some(MouseButton::Left) => 0,
        Some(MouseButton::Right) => 1,
        Some(MouseButton::Middle) => 2,
        _ => 0,
    };
    buf.push(btn_byte);

    buf.extend_from_slice(&(event.x as f32).to_le_bytes());
    buf.extend_from_slice(&(event.y as f32).to_le_bytes());

    let dx = event.delta_x.unwrap_or(0.0) as f32;
    let dy = event.delta_y.unwrap_or(0.0) as f32;
    buf.extend_from_slice(&dx.to_le_bytes());
    buf.extend_from_slice(&dy.to_le_bytes());

    buf
}

fn create_keyboard_packet(event: &KeyboardInputEvent) -> Vec<u8> {
    let mut buf = Vec::with_capacity(64);
    buf.push(2); // Type=Keyboard

    let type_byte = match event.event_type.as_str() {
        "keydown" => 0,
        "keyup" => 1,
        _ => 0,
    };
    buf.push(type_byte);

    let mut mods = 0u8;
    if event.modifiers.ctrl {
        mods |= 1;
    }
    if event.modifiers.alt {
        mods |= 2;
    }
    if event.modifiers.shift {
        mods |= 4;
    }
    if event.modifiers.meta {
        mods |= 8;
    }
    buf.push(mods);

    let code_bytes = event.code.as_bytes();
    buf.push(code_bytes.len() as u8);
    buf.extend_from_slice(code_bytes);

    let key_bytes = event.key.as_bytes();
    buf.push(key_bytes.len() as u8);
    buf.extend_from_slice(key_bytes);

    buf
}
