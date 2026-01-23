use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DiscoveredDevice {
    pub ip: String,
    pub name: String,
    pub port: u16,
    pub last_seen: u64,
}

pub fn get_local_network_range() -> Result<(Ipv4Addr, Ipv4Addr), String> {
    // Get local IP
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Failed to bind socket: {}", e))?;
    
    socket.connect("8.8.8.8:80")
        .map_err(|e| format!("Failed to connect: {}", e))?;
    
    let local_addr = socket.local_addr()
        .map_err(|e| format!("Failed to get local address: {}", e))?;
    
    if let IpAddr::V4(ipv4) = local_addr.ip() {
        let octets = ipv4.octets();
        // Assume /24 subnet (255.255.255.0)
        let network_start = Ipv4Addr::new(octets[0], octets[1], octets[2], 1);
        let network_end = Ipv4Addr::new(octets[0], octets[1], octets[2], 254);
        Ok((network_start, network_end))
    } else {
        Err("IPv6 not supported".to_string())
    }
}

pub fn discover_devices(port: u16, timeout_ms: u64) -> Result<Vec<DiscoveredDevice>, String> {
    // Note: get_local_network_range() is kept for future use (scanning specific IP ranges)
    let _network_range = get_local_network_range()?;
    let mut devices = Vec::new();
    
    // Create discovery socket
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Failed to bind discovery socket: {}", e))?;
    
    socket.set_read_timeout(Some(Duration::from_millis(timeout_ms)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;
    
    // Send discovery broadcast
    let discovery_msg = b"DISCOVERY_REQUEST";
    let broadcast_addr = format!("255.255.255.255:{}", port);
    
    socket.set_broadcast(true)
        .map_err(|e| format!("Failed to enable broadcast: {}", e))?;
    
    println!("[Discovery] Sending discovery broadcast to {} on port {}", broadcast_addr, port);
    socket.send_to(discovery_msg, &broadcast_addr)
        .map_err(|e| format!("Failed to send discovery: {}", e))?;
    
    println!("[Discovery] Discovery request sent, waiting for responses (timeout: {}ms)...", timeout_ms);
    
    // Listen for responses
    let mut buffer = [0u8; 1024];
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    
    while start_time.elapsed() < timeout {
        match socket.recv_from(&mut buffer) {
            Ok((size, addr)) => {
                let response = String::from_utf8_lossy(&buffer[..size]);
                println!("[Discovery] Received response from {}: {}", addr, response);
                
                if response.starts_with("DISCOVERY_RESPONSE:") {
                    // Parse response: "DISCOVERY_RESPONSE:name"
                    // Handle case where name might contain ':'
                    if let Some(name_start) = response.find(':') {
                        let name = response[name_start + 1..].trim().to_string();
                        
                        if !name.is_empty() {
                            // Check if device already exists (avoid duplicates)
                            let device_exists = devices.iter().any(|d: &DiscoveredDevice| d.ip == addr.ip().to_string());
                            
                            if !device_exists {
                                let device = DiscoveredDevice {
                                    ip: addr.ip().to_string(),
                                    name,
                                    port,
                                    last_seen: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs(),
                                };
                                println!("[Discovery] Found device: {} at {}", device.name, device.ip);
                                devices.push(device);
                            }
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                break;
            }
            Err(e) => {
                eprintln!("Error receiving discovery response: {}", e);
            }
        }
    }
    
    Ok(devices)
}

pub fn respond_to_discovery(name: &str, port: u16) -> Result<(), String> {
    // Try to bind to the port
    let socket = match UdpSocket::bind(format!("0.0.0.0:{}", port)) {
        Ok(s) => s,
        Err(e) => {
            return Err(format!("Failed to bind response socket on port {}: {}. Port may be in use.", port, e));
        }
    };
    
    socket.set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;
    
    socket.set_broadcast(true)
        .map_err(|e| format!("Failed to enable broadcast: {}", e))?;
    
    println!("[Discovery] Listener started on port {} for device: {}", port, name);
    
    let mut buffer = [0u8; 1024];
    let mut error_count = 0;
    const MAX_ERRORS: u32 = 10;
    
    loop {
        match socket.recv_from(&mut buffer) {
            Ok((size, addr)) => {
                error_count = 0; // Reset error count on success
                
                let request = String::from_utf8_lossy(&buffer[..size]);
                let request_trimmed = request.trim();
                
                println!("[Discovery] Received request from {}: '{}'", addr, request_trimmed);
                
                if request_trimmed == "DISCOVERY_REQUEST" {
                    let response = format!("DISCOVERY_RESPONSE:{}", name);
                    match socket.send_to(response.as_bytes(), addr) {
                        Ok(bytes_sent) => {
                            println!("[Discovery] ✅ Responded to discovery from {} (sent {} bytes, name: {})", addr, bytes_sent, name);
                        }
                        Err(e) => {
                            eprintln!("[Discovery] ❌ Failed to send response to {}: {}", addr, e);
                        }
                    }
                } else {
                    println!("[Discovery] ⚠️ Unknown request format: '{}'", request_trimmed);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // Continue listening - timeout is expected
                continue;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Non-blocking socket, continue
                continue;
            }
            Err(e) => {
                error_count += 1;
                eprintln!("[Discovery] Error receiving: {} (error count: {})", e, error_count);
                
                // If too many errors, return error
                if error_count >= MAX_ERRORS {
                    return Err(format!("Too many errors in discovery listener: {}", e));
                }
                
                // Small delay before retrying
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}
