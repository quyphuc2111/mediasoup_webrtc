use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::time::Duration;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DiscoveredDevice {
    pub ip: String,
    pub name: String,
    pub port: u16,
    pub last_seen: u64,
}

pub fn get_local_network_range() -> Result<(Ipv4Addr, Ipv4Addr), String> {
    // Get local IP
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Failed to bind socket: {}", e))?;

    socket
        .connect("8.8.8.8:80")
        .map_err(|e| format!("Failed to connect: {}", e))?;

    let local_addr = socket
        .local_addr()
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
    let mut devices = Vec::new();

    // Create discovery socket
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Failed to bind discovery socket: {}", e))?;

    // Set socket to non-blocking for better control
    socket
        .set_nonblocking(false)
        .map_err(|e| format!("Failed to set socket mode: {}", e))?;

    socket
        .set_read_timeout(Some(Duration::from_millis(timeout_ms)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;

    socket
        .set_broadcast(true)
        .map_err(|e| format!("Failed to enable broadcast: {}", e))?;

    let discovery_msg = b"DISCOVERY_REQUEST";

    // 1. Global Broadcast Address
    let global_broadcast = format!("255.255.255.255:{}", port);

    // 2. Local Subnet Broadcast Address (Calculated)
    // This helps when OS points 255.255.255.255 to a wrong interface (e.g. VPN/VirtualBox)
    let local_broadcast = get_local_network_range().ok().map(|(start, _)| {
        let octets = start.octets();
        // Assume /24 for simplicity -> x.x.x.255
        format!("{}.{}.{}.255:{}", octets[0], octets[1], octets[2], port)
    });

    // Send multiple discovery broadcasts to increase chance of discovery
    const BROADCAST_COUNT: usize = 3;
    const BROADCAST_INTERVAL_MS: u64 = 200;

    println!(
        "[Discovery] Sending {} discovery broadcasts...",
        BROADCAST_COUNT
    );
    if let Some(ref local) = local_broadcast {
        println!(
            "[Discovery] Target 1: {} (Global), Target 2: {} (Local Subnet)",
            global_broadcast, local
        );
    } else {
        println!("[Discovery] Target: {} (Global)", global_broadcast);
    }

    for i in 0..BROADCAST_COUNT {
        // Send to Global Broadcast
        if let Err(e) = socket.send_to(discovery_msg, &global_broadcast) {
            eprintln!(
                "[Discovery] Failed to send global broadcast #{}: {}",
                i + 1,
                e
            );
        }

        // Send to Local Subnet Broadcast (if available)
        if let Some(ref local_addr) = local_broadcast {
            if let Err(e) = socket.send_to(discovery_msg, local_addr) {
                eprintln!(
                    "[Discovery] Failed to send local broadcast #{}: {}",
                    i + 1,
                    e
                );
            }
        }

        if i == 0 {
            println!(
                "[Discovery] Logic: Sent requests, waiting for responses (timeout: {}ms)...",
                timeout_ms
            );
        }

        // Wait a bit between broadcasts (except for the last one)
        if i < BROADCAST_COUNT - 1 {
            std::thread::sleep(Duration::from_millis(BROADCAST_INTERVAL_MS));
        }
    }

    // Listen for responses
    // Use a longer timeout to allow for delayed responses
    let extended_timeout = timeout_ms + (BROADCAST_COUNT as u64 * BROADCAST_INTERVAL_MS);
    socket
        .set_read_timeout(Some(Duration::from_millis(extended_timeout)))
        .map_err(|e| format!("Failed to set extended timeout: {}", e))?;

    let mut buffer = [0u8; 1024];
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_millis(extended_timeout);
    let mut last_response_time = start_time;
    const MAX_SILENCE_MS: u64 = 1000; // If no response for 1s, likely done

    println!(
        "[Discovery] Listening for responses (extended timeout: {}ms)...",
        extended_timeout
    );

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
                            let device_exists = devices
                                .iter()
                                .any(|d: &DiscoveredDevice| d.ip == addr.ip().to_string());

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
                                println!(
                                    "[Discovery] ✅ Found device: {} at {} (total: {})",
                                    device.name,
                                    device.ip,
                                    devices.len() + 1
                                );
                                devices.push(device);
                            }
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // Check if we've had silence for too long
                if last_response_time.elapsed().as_millis() as u64 > MAX_SILENCE_MS
                    && !devices.is_empty()
                {
                    println!(
                        "[Discovery] No responses for {}ms, stopping early (found {} devices)",
                        MAX_SILENCE_MS,
                        devices.len()
                    );
                    break;
                }
                // Continue waiting if we haven't found any devices yet
                if devices.is_empty() {
                    continue;
                }
                break;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(ref e) if e.raw_os_error() == Some(35) => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(e) => {
                // Only log non-expected errors
                let error_code = e.raw_os_error();
                if error_code != Some(35) && e.kind() != std::io::ErrorKind::WouldBlock {
                    eprintln!(
                        "[Discovery] Error receiving discovery response: {} (os error: {:?})",
                        e, error_code
                    );
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    println!(
        "[Discovery] Discovery completed. Found {} device(s)",
        devices.len()
    );
    Ok(devices)
}

pub fn respond_to_discovery(name: &str, port: u16) -> Result<(), String> {
    // Try to bind to the port
    let socket = match UdpSocket::bind(format!("0.0.0.0:{}", port)) {
        Ok(s) => s,
        Err(e) => {
            return Err(format!(
                "Failed to bind response socket on port {}: {}. Port may be in use.",
                port, e
            ));
        }
    };

    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;

    socket
        .set_broadcast(true)
        .map_err(|e| format!("Failed to enable broadcast: {}", e))?;

    println!(
        "[Discovery] Listener started on port {} for device: {}",
        port, name
    );

    let mut buffer = [0u8; 1024];
    let mut error_count = 0;
    const MAX_ERRORS: u32 = 10;

    loop {
        match socket.recv_from(&mut buffer) {
            Ok((size, addr)) => {
                error_count = 0; // Reset error count on success

                let request = String::from_utf8_lossy(&buffer[..size]);
                let request_trimmed = request.trim();

                println!(
                    "[Discovery] Received request from {}: '{}'",
                    addr, request_trimmed
                );

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
                    println!(
                        "[Discovery] ⚠️ Unknown request format: '{}'",
                        request_trimmed
                    );
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
                    eprintln!(
                        "[Discovery] Error receiving: {} (os error: {:?}, error count: {})",
                        e, error_code, error_count
                    );
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
