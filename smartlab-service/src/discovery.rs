//! UDP Discovery Responder for SmartlabService
//!
//! Responds to teacher's LAN scan broadcasts on UDP port 3018,
//! so the teacher can discover this machine even before user login.
//!
//! Design: Retries binding INDEFINITELY with exponential backoff.
//! If socket dies, rebinds automatically. Never crashes the service.

use std::net::UdpSocket;
use std::time::Duration;

const DISCOVERY_PORT: u16 = 3018;

/// Wait until at least one non-loopback IPv4 address is available.
/// Retries indefinitely — needed at boot before DHCP assigns an IP.
fn wait_for_network() {
    use std::net::IpAddr;
    let mut attempt = 0u32;
    loop {
        let has_ip = if_addrs::get_if_addrs()
            .unwrap_or_default()
            .iter()
            .any(|iface| {
                !iface.is_loopback()
                    && matches!(iface.ip(), IpAddr::V4(_))
            });

        if has_ip {
            log::info!("[ServiceDiscovery] Network ready (attempt {})", attempt + 1);
            return;
        }

        attempt += 1;
        let delay = (attempt * 2).min(10) as u64;
        log::info!("[ServiceDiscovery] Waiting for network IP... (attempt {}, retry in {}s)", attempt, delay);
        std::thread::sleep(Duration::from_secs(delay));
    }
}

/// Run the discovery responder (blocking — call from a spawned thread).
/// This function retries FOREVER. It never returns under normal operation.
/// If the socket dies, it rebinds automatically.
pub fn run_discovery_responder() {
    let machine_name = std::env::var("COMPUTERNAME")
        .unwrap_or_else(|_| "Unknown".to_string());

    log::info!(
        "[ServiceDiscovery] Starting discovery responder as '{}' on UDP {}",
        machine_name, DISCOVERY_PORT
    );

    // Wait for a real IP before binding — at boot DHCP may not be done yet
    wait_for_network();

    loop {
        // Phase 1: Bind with exponential backoff (indefinite retry)
        let socket = match bind_udp_with_backoff() {
            Some(s) => s,
            None => {
                // This should never happen since we retry forever,
                // but just in case, sleep and retry the outer loop
                log::error!("[ServiceDiscovery] bind_udp_with_backoff returned None (unexpected)");
                std::thread::sleep(Duration::from_secs(10));
                continue;
            }
        };

        // Phase 2: Listen for discovery requests until socket fails
        log::info!("[ServiceDiscovery] Listening for broadcasts as '{}'", machine_name);
        listen_loop(&socket, &machine_name);

        // If we get here, the socket died — rebind after a short delay
        log::warn!("[ServiceDiscovery] Socket lost, rebinding in 2s...");
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// Bind UDP socket with exponential backoff. Retries FOREVER.
/// Backoff: 1s, 2s, 4s, 8s, 10s (capped), then 10s forever.
fn bind_udp_with_backoff() -> Option<UdpSocket> {
    let addr = format!("0.0.0.0:{}", DISCOVERY_PORT);
    let mut attempt: u32 = 0;
    let mut delay_secs: u64 = 1;
    const MAX_DELAY: u64 = 10;

    loop {
        attempt += 1;

        match socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        ) {
            Ok(s) => {
                // SO_REUSEADDR so student app can also bind this port
                s.set_reuse_address(true).ok();
                s.set_broadcast(true).ok();

                let bind_addr: std::net::SocketAddr = match addr.parse() {
                    Ok(a) => a,
                    Err(e) => {
                        log::error!("[ServiceDiscovery] Invalid address {}: {}", addr, e);
                        return None;
                    }
                };

                match s.bind(&bind_addr.into()) {
                    Ok(()) => {
                        log::info!(
                            "[ServiceDiscovery] Bound to UDP {} (attempt {})",
                            addr, attempt
                        );
                        let std_sock: UdpSocket = s.into();
                        std_sock.set_broadcast(true).ok();
                        std_sock.set_read_timeout(Some(Duration::from_secs(5))).ok();
                        return Some(std_sock);
                    }
                    Err(e) => {
                        log::warn!(
                            "[ServiceDiscovery] Bind failed UDP {} (attempt {}): {} — retry in {}s",
                            addr, attempt, e, delay_secs
                        );
                    }
                }
            }
            Err(e) => {
                log::warn!(
                    "[ServiceDiscovery] Socket create failed (attempt {}): {} — retry in {}s",
                    attempt, e, delay_secs
                );
            }
        }

        std::thread::sleep(Duration::from_secs(delay_secs));
        delay_secs = (delay_secs * 2).min(MAX_DELAY);
    }
}

/// Listen for discovery requests. Returns when the socket encounters a fatal error.
fn listen_loop(socket: &UdpSocket, machine_name: &str) {
    let mut buf = [0u8; 1024];
    let mut consecutive_errors: u32 = 0;

    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, addr)) => {
                consecutive_errors = 0;
                let msg = String::from_utf8_lossy(&buf[..size]);
                let msg = msg.trim();

                if msg == "DISCOVERY_REQUEST" || msg.starts_with("DISCOVERY_REQUEST") {
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
                consecutive_errors = 0;
            }
            Err(e) => {
                consecutive_errors += 1;
                log::warn!(
                    "[ServiceDiscovery] Socket error (#{} consecutive): {}",
                    consecutive_errors, e
                );

                // If too many consecutive errors, socket is probably dead
                if consecutive_errors > 20 {
                    log::error!("[ServiceDiscovery] Too many errors, rebinding...");
                    return;
                }

                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
}
