//! Student Auto-Connect - Automatically discover and connect to teacher
//!
//! This module allows students to automatically find and connect to the teacher
//! when they start their app, eliminating the need for manual connection from teacher side.

use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Auto-connect configuration
#[derive(Clone, Debug)]
pub struct AutoConnectConfig {
    /// Port to scan for teacher
    pub teacher_port: u16,
    /// How often to retry finding teacher (in seconds)
    pub retry_interval_secs: u64,
    /// Timeout for each discovery attempt (in milliseconds)
    pub discovery_timeout_ms: u64,
}

impl Default for AutoConnectConfig {
    fn default() -> Self {
        Self {
            teacher_port: 3017,
            retry_interval_secs: 10,
            discovery_timeout_ms: 3000,
        }
    }
}

/// Start auto-connect service that continuously tries to find and connect to teacher
pub async fn start_auto_connect(
    config: AutoConnectConfig,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    log::info!("[StudentAutoConnect] Starting auto-connect service...");
    log::info!("[StudentAutoConnect] Will scan for teacher on port {} every {}s", 
        config.teacher_port, config.retry_interval_secs);

    while !stop_flag.load(Ordering::Relaxed) {
        match discover_teacher(config.teacher_port, config.discovery_timeout_ms) {
            Ok(Some(teacher_ip)) => {
                log::info!("[StudentAutoConnect] Found teacher at: {}", teacher_ip);
                
                // Teacher will connect to us via our WebSocket server
                // We just need to announce our presence
                log::info!("[StudentAutoConnect] Teacher should connect to us automatically");
                
                // Wait longer before next scan since we found teacher
                sleep(Duration::from_secs(config.retry_interval_secs * 3)).await;
            }
            Ok(None) => {
                log::info!("[StudentAutoConnect] No teacher found, retrying in {}s...", 
                    config.retry_interval_secs);
                sleep(Duration::from_secs(config.retry_interval_secs)).await;
            }
            Err(e) => {
                log::error!("[StudentAutoConnect] Discovery error: {}, retrying in {}s...", 
                    e, config.retry_interval_secs);
                sleep(Duration::from_secs(config.retry_interval_secs)).await;
            }
        }
    }

    log::info!("[StudentAutoConnect] Auto-connect service stopped");
    Ok(())
}

/// Discover teacher on the local network
fn discover_teacher(port: u16, timeout_ms: u64) -> Result<Option<String>, String> {
    log::info!("[StudentAutoConnect] Scanning for teacher...");

    // Create discovery socket
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Failed to bind discovery socket: {}", e))?;

    socket
        .set_read_timeout(Some(Duration::from_millis(timeout_ms)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;

    socket
        .set_broadcast(true)
        .map_err(|e| format!("Failed to enable broadcast: {}", e))?;

    // Send discovery request looking for teacher
    let discovery_msg = b"STUDENT_LOOKING_FOR_TEACHER";
    
    // Try both global and local broadcast
    let global_broadcast = format!("255.255.255.255:{}", port);
    
    // Send multiple broadcasts
    for i in 0..3 {
        if let Err(e) = socket.send_to(discovery_msg, &global_broadcast) {
            log::warn!("[StudentAutoConnect] Failed to send broadcast #{}: {}", i + 1, e);
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    // Listen for teacher response
    let mut buffer = [0u8; 1024];
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_millis(timeout_ms);

    while start_time.elapsed() < timeout {
        match socket.recv_from(&mut buffer) {
            Ok((size, addr)) => {
                let response = String::from_utf8_lossy(&buffer[..size]);
                log::info!("[StudentAutoConnect] Received response from {}: {}", addr, response);

                if response.starts_with("TEACHER_HERE:") {
                    // Parse response: "TEACHER_HERE:teacher_name"
                    let teacher_ip = addr.ip().to_string();
                    log::info!("[StudentAutoConnect] ✅ Found teacher at: {}", teacher_ip);
                    return Ok(Some(teacher_ip));
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                break;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(e) => {
                log::warn!("[StudentAutoConnect] Error receiving: {}", e);
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    log::info!("[StudentAutoConnect] No teacher found in this scan");
    Ok(None)
}

/// Teacher-side: Respond to student discovery requests
pub fn respond_to_student_discovery(teacher_name: &str, port: u16) -> Result<(), String> {
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))
        .map_err(|e| format!("Failed to bind response socket on port {}: {}", port, e))?;

    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;

    socket
        .set_broadcast(true)
        .map_err(|e| format!("Failed to enable broadcast: {}", e))?;

    log::info!("[TeacherDiscovery] Listening for student discovery requests on port {}", port);

    let mut buffer = [0u8; 1024];

    loop {
        match socket.recv_from(&mut buffer) {
            Ok((size, addr)) => {
                let request = String::from_utf8_lossy(&buffer[..size]);
                let request_trimmed = request.trim();

                if request_trimmed == "STUDENT_LOOKING_FOR_TEACHER" {
                    let response = format!("TEACHER_HERE:{}", teacher_name);
                    match socket.send_to(response.as_bytes(), addr) {
                        Ok(_) => {
                            log::info!("[TeacherDiscovery] ✅ Responded to student at {}", addr);
                        }
                        Err(e) => {
                            log::error!("[TeacherDiscovery] Failed to respond to {}: {}", addr, e);
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                continue;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(ref e) if e.raw_os_error() == Some(35) => {
                continue;
            }
            Err(e) => {
                log::error!("[TeacherDiscovery] Error: {}", e);
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}
