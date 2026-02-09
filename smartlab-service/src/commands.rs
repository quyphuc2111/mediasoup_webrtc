//! TCP Command Server
//!
//! Listens on a TCP port for JSON commands from the teacher app.
//! Runs at boot level — available even before user login.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

#[derive(Debug, Deserialize)]
#[serde(tag = "command")]
pub enum Command {
    /// Check if service is alive
    #[serde(rename = "ping")]
    Ping,

    /// Get machine status (is user logged in? which user?)
    #[serde(rename = "status")]
    Status,

    /// Login a user on this machine
    #[serde(rename = "login")]
    Login {
        username: String,
        password: String,
        domain: Option<String>,
    },

    /// Get VNC server status
    #[serde(rename = "vnc_status")]
    VncStatus,

    /// Start VNC server
    #[serde(rename = "vnc_start")]
    VncStart {
        password: Option<String>,
    },

    /// Stop VNC server
    #[serde(rename = "vnc_stop")]
    VncStop,

    /// Install VNC server
    #[serde(rename = "vnc_install")]
    VncInstall {
        installer_path: Option<String>,
        password: String,
    },
}

#[derive(Debug, Serialize)]
pub struct Response {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Run the TCP command server
/// Run the TCP command server with retry logic for binding
/// Run the TCP command server with retry logic for binding
pub async fn run_command_server(port: u16) {
    let addr = format!("0.0.0.0:{}", port);

    // Retry binding up to 30 times (total ~60s wait) to handle boot race conditions
    // where the service starts before the network stack is fully ready
    let max_retries = 30;
    let mut listener = None;

    for attempt in 1..=max_retries {
        match TcpListener::bind(&addr).await {
            Ok(l) => {
                log::info!("[CmdServer] Listening on {} (attempt {})", addr, attempt);
                listener = Some(l);
                break;
            }
            Err(e) => {
                log::warn!(
                    "[CmdServer] Failed to bind {} (attempt {}/{}): {}",
                    addr, attempt, max_retries, e
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        }
    }

    let listener = match listener {
        Some(l) => l,
        None => {
            log::error!("[CmdServer] Giving up binding to {} after {} attempts", addr, max_retries);
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                log::info!("[CmdServer] Connection from {}", peer);
                tokio::spawn(async move {
                    handle_connection(stream).await;
                });
            }
            Err(e) => {
                log::error!("[CmdServer] Accept error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }
}

async fn handle_connection(stream: tokio::net::TcpStream) {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // Connection closed
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                log::info!("[CmdServer] Received: {}", trimmed);

                let response = match serde_json::from_str::<Command>(trimmed) {
                    Ok(cmd) => process_command(cmd).await,
                    Err(e) => Response {
                        success: false,
                        message: format!("Invalid command: {}", e),
                        data: None,
                    },
                };

                let mut resp_json = serde_json::to_string(&response).unwrap_or_default();
                resp_json.push('\n');

                if writer.write_all(resp_json.as_bytes()).await.is_err() {
                    break;
                }
            }
            Err(e) => {
                log::error!("[CmdServer] Read error: {}", e);
                break;
            }
        }
    }
}

async fn process_command(cmd: Command) -> Response {
    match cmd {
        Command::Ping => Response {
            success: true,
            message: "pong".to_string(),
            data: None,
        },

        Command::Status => {
            let logged_in_user = get_logged_in_user();
            let machine_name = std::env::var("COMPUTERNAME").unwrap_or_default();

            Response {
                success: true,
                message: if logged_in_user.is_some() {
                    "user_logged_in".to_string()
                } else {
                    "no_user".to_string()
                },
                data: Some(serde_json::json!({
                    "machine_name": machine_name,
                    "logged_in_user": logged_in_user,
                })),
            }
        }

        Command::Login {
            username,
            password,
            domain,
        } => {
            log::info!("[CmdServer] Login request for user: {}", username);

            #[cfg(windows)]
            {
                match crate::logon::logon_user(&username, &password, domain.as_deref()) {
                    Ok(()) => {
                        log::info!("[CmdServer] Login successful for: {}", username);
                        Response {
                            success: true,
                            message: format!("User {} logged in successfully", username),
                            data: None,
                        }
                    }
                    Err(e) => {
                        log::error!("[CmdServer] Login failed for {}: {}", username, e);
                        Response {
                            success: false,
                            message: format!("Login failed: {}", e),
                            data: None,
                        }
                    }
                }
            }

            #[cfg(not(windows))]
            {
                let _ = (username, password, domain);
                Response {
                    success: false,
                    message: "Login only supported on Windows".to_string(),
                    data: None,
                }
            }
        }

        Command::VncStatus => {
            #[cfg(windows)]
            {
                let status = crate::vnc::get_vnc_status();
                Response {
                    success: true,
                    message: if status.running { "running" } else if status.installed { "stopped" } else { "not_installed" }.to_string(),
                    data: Some(serde_json::json!({
                        "installed": status.installed,
                        "running": status.running,
                        "port": status.port,
                    })),
                }
            }
            #[cfg(not(windows))]
            {
                Response {
                    success: false,
                    message: "VNC only supported on Windows".to_string(),
                    data: None,
                }
            }
        }

        Command::VncStart { password } => {
            log::info!("[CmdServer] VNC start request");
            #[cfg(windows)]
            {
                match crate::vnc::start_vnc(password.as_deref()) {
                    Ok(msg) => Response { success: true, message: msg, data: None },
                    Err(e) => Response { success: false, message: e, data: None },
                }
            }
            #[cfg(not(windows))]
            {
                let _ = password;
                Response { success: false, message: "VNC only supported on Windows".to_string(), data: None }
            }
        }

        Command::VncStop => {
            log::info!("[CmdServer] VNC stop request");
            #[cfg(windows)]
            {
                match crate::vnc::stop_vnc() {
                    Ok(msg) => Response { success: true, message: msg, data: None },
                    Err(e) => Response { success: false, message: e, data: None },
                }
            }
            #[cfg(not(windows))]
            {
                Response { success: false, message: "VNC only supported on Windows".to_string(), data: None }
            }
        }

        Command::VncInstall { installer_path, password } => {
            log::info!("[CmdServer] VNC install request");
            #[cfg(windows)]
            {
                match crate::vnc::install_vnc(installer_path.as_deref(), &password) {
                    Ok(msg) => Response { success: true, message: msg, data: None },
                    Err(e) => Response { success: false, message: e, data: None },
                }
            }
            #[cfg(not(windows))]
            {
                let _ = (installer_path, password);
                Response { success: false, message: "VNC only supported on Windows".to_string(), data: None }
            }
        }
    }
}

/// Get the currently logged-in user (if any)
fn get_logged_in_user() -> Option<String> {
    #[cfg(windows)]
    {
        // Try environment variable first
        if let Ok(user) = std::env::var("USERNAME") {
            // SYSTEM means no interactive user
            if user != "SYSTEM" && user != "SYSTEM$" {
                return Some(user);
            }
        }

        // Use WTSEnumerateSessions to check for active sessions
        crate::logon::get_active_session_user()
    }

    #[cfg(not(windows))]
    {
        std::env::var("USER").ok()
    }
}
