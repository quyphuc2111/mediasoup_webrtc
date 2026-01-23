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
    let (start, end) = get_local_network_range()?;
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
    
    socket.send_to(discovery_msg, &broadcast_addr)
        .map_err(|e| format!("Failed to send discovery: {}", e))?;
    
    // Listen for responses
    let mut buffer = [0u8; 1024];
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    
    while start_time.elapsed() < timeout {
        match socket.recv_from(&mut buffer) {
            Ok((size, addr)) => {
                let response = String::from_utf8_lossy(&buffer[..size]);
                if response.starts_with("DISCOVERY_RESPONSE:") {
                    let parts: Vec<&str> = response.split(':').collect();
                    if parts.len() >= 3 {
                        let name = parts[2].to_string();
                        let device = DiscoveredDevice {
                            ip: addr.ip().to_string(),
                            name,
                            port,
                            last_seen: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        };
                        devices.push(device);
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
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))
        .map_err(|e| format!("Failed to bind response socket: {}", e))?;
    
    socket.set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;
    
    let mut buffer = [0u8; 1024];
    
    loop {
        match socket.recv_from(&mut buffer) {
            Ok((size, addr)) => {
                let request = String::from_utf8_lossy(&buffer[..size]);
                if request == "DISCOVERY_REQUEST" {
                    let response = format!("DISCOVERY_RESPONSE:{}", name);
                    socket.send_to(response.as_bytes(), addr)
                        .map_err(|e| format!("Failed to send response: {}", e))?;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // Continue listening
                continue;
            }
            Err(e) => {
                return Err(format!("Error in discovery response: {}", e));
            }
        }
    }
}
