use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::time::Duration;

/// Discovery port — shared between teacher scan and student responder
pub const DISCOVERY_PORT: u16 = 3018;

/// Student agent WebSocket port
pub const STUDENT_AGENT_PORT: u16 = 3017;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DiscoveredDevice {
    pub ip: String,
    pub name: String,
    pub port: u16,
    pub last_seen: u64,
}

fn get_local_ip() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) => Some(ip),
        _ => None,
    }
}

/// Get all broadcast addresses to try (subnet + global)
fn get_broadcast_addresses(port: u16) -> Vec<String> {
    let mut addrs = vec![format!("255.255.255.255:{}", port)];
    if let Some(ip) = get_local_ip() {
        let o = ip.octets();
        addrs.push(format!("{}.{}.{}.255:{}", o[0], o[1], o[2], port));
    }
    addrs
}


// ============================================================
// Teacher side: Scan for students
// ============================================================

/// Discover student devices on the LAN.
/// Sends UDP broadcast and also tries direct TCP probe on known port.
pub fn discover_devices(port: u16, timeout_ms: u64) -> Result<Vec<DiscoveredDevice>, String> {
    let mut devices = Vec::new();

    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Failed to bind: {}", e))?;
    socket.set_broadcast(true).ok();
    socket.set_read_timeout(Some(Duration::from_millis(timeout_ms))).ok();

    let discovery_msg = b"DISCOVERY_REQUEST";
    let broadcasts = get_broadcast_addresses(port);

    // Send 5 broadcasts across all addresses for reliability
    for round in 0..5 {
        for addr in &broadcasts {
            let _ = socket.send_to(discovery_msg, addr);
        }
        if round < 4 {
            std::thread::sleep(Duration::from_millis(150));
        }
    }

    log::info!("[Discovery] Sent broadcasts to {:?}, listening...", broadcasts);

    // Listen for responses
    let mut buffer = [0u8; 1024];
    let start = std::time::Instant::now();
    let deadline = Duration::from_millis(timeout_ms + 800);

    while start.elapsed() < deadline {
        match socket.recv_from(&mut buffer) {
            Ok((size, addr)) => {
                let response = String::from_utf8_lossy(&buffer[..size]);
                let response = response.trim();

                // Accept both old and new response formats
                let name = if response.starts_with("DISCOVERY_RESPONSE:") {
                    response.strip_prefix("DISCOVERY_RESPONSE:").unwrap_or("").to_string()
                } else if response.starts_with("STUDENT_HERE:") {
                    response.strip_prefix("STUDENT_HERE:").unwrap_or("").to_string()
                } else {
                    continue;
                };

                if name.is_empty() { continue; }

                let ip = addr.ip().to_string();
                if !devices.iter().any(|d: &DiscoveredDevice| d.ip == ip) {
                    log::info!("[Discovery] Found: {} at {}", name, ip);
                    devices.push(DiscoveredDevice {
                        ip,
                        name,
                        port: STUDENT_AGENT_PORT,
                        last_seen: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap().as_secs(),
                    });
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut
                || e.kind() == std::io::ErrorKind::WouldBlock
                || e.raw_os_error() == Some(35) =>
            {
                if !devices.is_empty() { break; }
                continue;
            }
            Err(_) => {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    log::info!("[Discovery] Found {} device(s)", devices.len());
    Ok(devices)
}


// ============================================================
// Student side: Respond to teacher's discovery scan
// ============================================================

/// Run on the student machine. Listens for discovery broadcasts from teacher
/// and responds with the student's name. This is what makes the student
/// visible to the teacher's "Scan" button.
///
/// Also periodically announces itself so the teacher's auto-connect can find it.
pub fn run_student_discovery_responder(
    student_name: &str,
    port: u16,
) -> Result<(), String> {
    // Use SO_REUSEADDR so multiple processes can share the port
    let socket = {
        let sock = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        ).map_err(|e| format!("Failed to create socket: {}", e))?;

        sock.set_reuse_address(true).ok();
        #[cfg(unix)]
        {
            // SO_REUSEPORT on Unix
            let _ = sock.set_reuse_address(true);
        }
        sock.set_broadcast(true).ok();
        sock.set_read_timeout(Some(Duration::from_secs(2))).ok();

        let addr: std::net::SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
        sock.bind(&addr.into())
            .map_err(|e| format!("Failed to bind port {}: {}", port, e))?;

        let std_socket: UdpSocket = sock.into();
        std_socket
    };

    log::info!("[StudentDiscovery] Listening on port {} as '{}'", port, student_name);

    let mut buffer = [0u8; 1024];
    let mut announce_timer = std::time::Instant::now();
    let announce_interval = Duration::from_secs(5);

    loop {
        // Periodically announce ourselves (so teacher auto-connect picks us up)
        if announce_timer.elapsed() >= announce_interval {
            let announce = format!("STUDENT_HERE:{}", student_name);
            for addr in &get_broadcast_addresses(port) {
                let _ = socket.send_to(announce.as_bytes(), addr);
            }
            announce_timer = std::time::Instant::now();
        }

        match socket.recv_from(&mut buffer) {
            Ok((size, addr)) => {
                let request = String::from_utf8_lossy(&buffer[..size]);
                let request = request.trim();

                match request {
                    // Teacher's scan button
                    "DISCOVERY_REQUEST" => {
                        let response = format!("STUDENT_HERE:{}", student_name);
                        if let Err(e) = socket.send_to(response.as_bytes(), addr) {
                            log::warn!("[StudentDiscovery] Failed to respond to {}: {}", addr, e);
                        } else {
                            log::info!("[StudentDiscovery] Responded to scan from {}", addr);
                        }
                    }
                    // Legacy: old teacher format
                    _ if request.starts_with("DISCOVERY_REQUEST") => {
                        let response = format!("DISCOVERY_RESPONSE:{}", student_name);
                        let _ = socket.send_to(response.as_bytes(), addr);
                    }
                    _ => {} // Ignore other messages (including our own broadcasts)
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut
                || e.kind() == std::io::ErrorKind::WouldBlock
                || e.raw_os_error() == Some(35) =>
            {
                continue;
            }
            Err(e) => {
                log::warn!("[StudentDiscovery] Error: {}", e);
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

// ============================================================
// Teacher side: Listen for student announcements + auto-connect
// ============================================================

/// Run on the teacher machine. Listens for student announcements
/// and auto-connects to new students.
pub fn run_teacher_auto_connect<F>(
    teacher_name: &str,
    port: u16,
    on_student_found: F,
) -> Result<(), String>
where
    F: Fn(String, u16) + Send + 'static,
{
    let socket = {
        let sock = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        ).map_err(|e| format!("Failed to create socket: {}", e))?;

        sock.set_reuse_address(true).ok();
        #[cfg(unix)]
        {
            let _ = sock.set_reuse_address(true);
        }
        sock.set_broadcast(true).ok();
        sock.set_read_timeout(Some(Duration::from_secs(2))).ok();

        let addr: std::net::SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
        sock.bind(&addr.into())
            .map_err(|e| format!("Failed to bind port {}: {}", port, e))?;

        let std_socket: UdpSocket = sock.into();
        std_socket
    };

    log::info!("[TeacherAutoConnect] Listening on port {} for students", port);

    let mut buffer = [0u8; 1024];
    let mut known_students = std::collections::HashSet::new();

    // Also periodically send discovery requests (active scanning)
    let mut scan_timer = std::time::Instant::now();
    let scan_interval = Duration::from_secs(8);

    loop {
        // Periodically broadcast discovery request
        if scan_timer.elapsed() >= scan_interval {
            let discovery_msg = b"DISCOVERY_REQUEST";
            for addr in &get_broadcast_addresses(port) {
                let _ = socket.send_to(discovery_msg, addr);
            }
            scan_timer = std::time::Instant::now();
        }

        match socket.recv_from(&mut buffer) {
            Ok((size, addr)) => {
                let msg = String::from_utf8_lossy(&buffer[..size]);
                let msg = msg.trim();

                // Student announcing itself or responding to our scan
                let student_name = if msg.starts_with("STUDENT_HERE:") {
                    msg.strip_prefix("STUDENT_HERE:").unwrap_or("")
                } else if msg.starts_with("DISCOVERY_RESPONSE:") {
                    msg.strip_prefix("DISCOVERY_RESPONSE:").unwrap_or("")
                } else if msg == "STUDENT_LOOKING_FOR_TEACHER" {
                    // Legacy student auto-connect — respond and connect
                    let response = format!("TEACHER_HERE:{}", teacher_name);
                    let _ = socket.send_to(response.as_bytes(), addr);
                    "Student" // Use generic name, will get real name on WebSocket connect
                } else {
                    continue;
                };

                if student_name.is_empty() { continue; }

                let student_ip = addr.ip().to_string();

                // Skip our own broadcasts
                if let Some(local_ip) = get_local_ip() {
                    if student_ip == local_ip.to_string() { continue; }
                }

                if !known_students.contains(&student_ip) {
                    known_students.insert(student_ip.clone());
                    log::info!("[TeacherAutoConnect] New student: {} at {}", student_name, student_ip);
                    on_student_found(student_ip, STUDENT_AGENT_PORT);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut
                || e.kind() == std::io::ErrorKind::WouldBlock
                || e.raw_os_error() == Some(35) =>
            {
                continue;
            }
            Err(e) => {
                log::warn!("[TeacherAutoConnect] Error: {}", e);
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Legacy: respond_to_discovery (kept for backward compatibility)
pub fn respond_to_discovery(name: &str, port: u16) -> Result<(), String> {
    run_student_discovery_responder(name, port)
}
