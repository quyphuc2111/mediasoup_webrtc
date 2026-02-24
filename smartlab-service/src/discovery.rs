//! UDP Discovery Responder for SmartlabService
//!
//! Responds to teacher's LAN scan broadcasts on UDP port 3018,
//! so the teacher can discover this machine even before user login.
//! Uses the same protocol as the student agent's discovery responder.

use std::net::UdpSocket;
use std::time::Duration;

const DISCOVERY_PORT: u16 = 3018;

/// Run the discovery responder (blocking — call from a spawned thread)
pub fn run_discovery_responder() {
    let machine_name = std::env::var("COMPUTERNAME")
        .unwrap_or_else(|_| "Unknown".to_string());

    // Retry binding with backoff (network may not be ready at boot)
    // Use SO_REUSEADDR so student app can also bind this port when it starts
    let socket = {
        let addr = format!("0.0.0.0:{}", DISCOVERY_PORT);
        let mut sock = None;
        for attempt in 1..=30 {
            match socket2::Socket::new(
                socket2::Domain::IPV4,
                socket2::Type::DGRAM,
                Some(socket2::Protocol::UDP),
            ) {
                Ok(s) => {
                    s.set_reuse_address(true).ok();
                    s.set_broadcast(true).ok();
                    let bind_addr: std::net::SocketAddr = addr.parse().unwrap();
                    match s.bind(&bind_addr.into()) {
                        Ok(()) => {
                            log::info!(
                                "[ServiceDiscovery] Bound to UDP {} (attempt {})",
                                addr, attempt
                            );
                            let std_sock: UdpSocket = s.into();
                            sock = Some(std_sock);
                            break;
                        }
                        Err(e) => {
                            log::warn!(
                                "[ServiceDiscovery] Failed to bind UDP {} (attempt {}/30): {}",
                                addr, attempt, e
                            );
                        }
                    }
                }
                Err(e) => {
                    log::warn!(
                        "[ServiceDiscovery] Failed to create socket (attempt {}/30): {}",
                        attempt, e
                    );
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        match sock {
            Some(s) => s,
            None => {
                log::info!("[ServiceDiscovery] Could not bind UDP {} — student app may be handling discovery", addr);
                return;
            }
        }
    };

    socket.set_broadcast(true).ok();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .ok();

    log::info!(
        "[ServiceDiscovery] Listening for discovery broadcasts as '{}'",
        machine_name
    );

    let mut buf = [0u8; 1024];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, addr)) => {
                let msg = String::from_utf8_lossy(&buf[..size]);
                let msg = msg.trim();

                if msg == "DISCOVERY_REQUEST" || msg.starts_with("DISCOVERY_REQUEST") {
                    // Respond with same format as student agent
                    let response = format!("STUDENT_HERE:{}", machine_name);
                    if let Err(e) = socket.send_to(response.as_bytes(), addr) {
                        log::warn!("[ServiceDiscovery] Failed to respond to {}: {}", addr, e);
                    } else {
                        log::info!("[ServiceDiscovery] Responded to scan from {}", addr);
                    }
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                // Normal timeout, continue listening
            }
            Err(e) => {
                log::warn!("[ServiceDiscovery] Socket error: {}", e);
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
}
