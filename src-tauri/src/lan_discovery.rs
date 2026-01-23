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
    
    // Set socket to non-blocking for better control
    socket.set_nonblocking(false)
        .map_err(|e| format!("Failed to set socket mode: {}", e))?;
    
    socket.set_read_timeout(Some(Duration::from_millis(timeout_ms)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;
    
    socket.set_broadcast(true)
        .map_err(|e| format!("Failed to enable broadcast: {}", e))?;
    
    let discovery_msg = b"DISCOVERY_REQUEST";
    let broadcast_addr = format!("255.255.255.255:{}", port);
    
    // Send multiple discovery broadcasts to increase chance of discovery
    // Some devices might miss the first broadcast
    const BROADCAST_COUNT: usize = 3;
    const BROADCAST_INTERVAL_MS: u64 = 200;
    
    println!("[Discovery] Sending {} discovery broadcasts to {} on port {}", BROADCAST_COUNT, broadcast_addr, port);
    
    for i in 0..BROADCAST_COUNT {
        match socket.send_to(discovery_msg, &broadcast_addr) {
            Ok(_) => {
                if i == 0 {
                    println!("[Discovery] Discovery request sent, waiting for responses (timeout: {}ms)...", timeout_ms);
                }
            }
            Err(e) => {
                eprintln!("[Discovery] Failed to send broadcast #{}: {}", i + 1, e);
            }
        }
        
        // Wait a bit between broadcasts (except for the last one)
        if i < BROADCAST_COUNT - 1 {
            std::thread::sleep(Duration::from_millis(BROADCAST_INTERVAL_MS));
        }
    }
    
    // Listen for responses
    // Use a longer timeout to allow for delayed responses
    let extended_timeout = timeout_ms + (BROADCAST_COUNT as u64 * BROADCAST_INTERVAL_MS);
    socket.set_read_timeout(Some(Duration::from_millis(extended_timeout)))
        .map_err(|e| format!("Failed to set extended timeout: {}", e))?;
    
    let mut buffer = [0u8; 1024];
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_millis(extended_timeout);
    let mut last_response_time = start_time;
    const MAX_SILENCE_MS: u64 = 1000; // If no response for 1s, likely done
    
    println!("[Discovery] Listening for responses (extended timeout: {}ms)...", extended_timeout);
    
    while start_time.elapsed() < timeout {
        match socket.recv_from(&mut buffer) {
            Ok((size, addr)) => {
                last_response_time = std::time::Instant::now();
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
                                println!("[Discovery] ✅ Found device: {} at {} (total: {})", device.name, device.ip, devices.len() + 1);
                                devices.push(device);
                            } else {
                                println!("[Discovery] ⚠️ Device {} already in list, skipping duplicate", addr.ip());
                            }
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // Check if we've had silence for too long
                if last_response_time.elapsed().as_millis() as u64 > MAX_SILENCE_MS && !devices.is_empty() {
                    println!("[Discovery] No responses for {}ms, stopping early (found {} devices)", MAX_SILENCE_MS, devices.len());
                    break;
                }
                // Continue waiting if we haven't found any devices yet
                if devices.is_empty() {
                    continue;
                }
                break;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Non-blocking socket would block, but we're using blocking mode
                // This shouldn't happen, but handle it anyway
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(ref e) if e.raw_os_error() == Some(35) => {
                // macOS: Resource temporarily unavailable (EAGAIN)
                // This is normal for non-blocking sockets, but we're using blocking mode
                // Just continue waiting
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(e) => {
                // Only log non-expected errors
                let error_code = e.raw_os_error();
                if error_code != Some(35) && e.kind() != std::io::ErrorKind::WouldBlock {
                    eprintln!("[Discovery] Error receiving discovery response: {} (os error: {:?})", e, error_code);
                }
                // Continue waiting for more responses
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    
    println!("[Discovery] Discovery completed. Found {} device(s)", devices.len());
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
            Err(ref e) if e.raw_os_error() == Some(35) => {
                // macOS: Resource temporarily unavailable (EAGAIN)
                // This is normal for non-blocking sockets, just continue
                continue;
            }
            Err(e) => {
                error_count += 1;
                let error_code = e.raw_os_error();
                
                // Only log non-expected errors (not EAGAIN/EWOULDBLOCK)
                if error_code != Some(35) && e.kind() != std::io::ErrorKind::WouldBlock {
                    eprintln!("[Discovery] Error receiving: {} (os error: {:?}, error count: {})", e, error_code, error_count);
                }
                
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
