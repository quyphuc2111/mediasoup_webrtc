use std::collections::HashMap;
use std::net::{UdpSocket, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioPacket {
    pub sequence: u32,
    pub timestamp: u64,
    pub data: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UdpAudioConfig {
    pub port: u16,
    pub sample_rate: u32,
    pub channels: u8,
    pub bitrate: u32,
}

pub struct UdpAudioServer {
    socket: Arc<UdpSocket>,
    clients: Arc<Mutex<HashMap<SocketAddr, Instant>>>,
    config: UdpAudioConfig,
    sequence: Arc<Mutex<u32>>,
}

impl UdpAudioServer {
    pub fn new(config: UdpAudioConfig) -> Result<Self, String> {
        let addr = format!("0.0.0.0:{}", config.port);
        let socket = UdpSocket::bind(&addr)
            .map_err(|e| format!("Failed to bind UDP socket: {}", e))?;
        
        socket.set_nonblocking(true)
            .map_err(|e| format!("Failed to set non-blocking: {}", e))?;

        Ok(Self {
            socket: Arc::new(socket),
            clients: Arc::new(Mutex::new(HashMap::new())),
            config,
            sequence: Arc::new(Mutex::new(0)),
        })
    }

    pub fn broadcast_audio(&self, audio_data: &[u8]) -> Result<usize, String> {
        let mut seq_guard = self.sequence.lock().map_err(|e| e.to_string())?;
        let sequence = *seq_guard;
        *seq_guard = sequence.wrapping_add(1);

        let packet = AudioPacket {
            sequence,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            data: audio_data.to_vec(),
        };

        let packet_data = bincode::serialize(&packet)
            .map_err(|e| format!("Failed to serialize packet: {}", e))?;

        let mut clients_guard = self.clients.lock().map_err(|e| e.to_string())?;
        let mut sent_count = 0;
        let timeout = Duration::from_secs(5);
        let now = Instant::now();

        // Remove stale clients
        clients_guard.retain(|_, last_seen| now.duration_since(*last_seen) < timeout);

        for (addr, _) in clients_guard.iter() {
            if let Err(e) = self.socket.send_to(&packet_data, addr) {
                eprintln!("Failed to send to {}: {}", addr, e);
            } else {
                sent_count += 1;
            }
        }

        Ok(sent_count)
    }

    pub fn register_client(&self, addr: SocketAddr) {
        let mut clients_guard = self.clients.lock().unwrap();
        clients_guard.insert(addr, Instant::now());
    }

    pub fn get_client_count(&self) -> usize {
        let clients_guard = self.clients.lock().unwrap();
        clients_guard.len()
    }
}

pub struct UdpAudioClient {
    socket: Arc<UdpSocket>,
    server_addr: SocketAddr,
    last_sequence: Arc<Mutex<Option<u32>>>,
    config: UdpAudioConfig,
}

impl UdpAudioClient {
    pub fn new(server_ip: &str, config: UdpAudioConfig) -> Result<Self, String> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("Failed to bind UDP socket: {}", e))?;
        
        socket.set_nonblocking(true)
            .map_err(|e| format!("Failed to set non-blocking: {}", e))?;

        let server_addr = format!("{}:{}", server_ip, config.port)
            .parse::<SocketAddr>()
            .map_err(|e| format!("Invalid server address: {}", e))?;

        Ok(Self {
            socket: Arc::new(socket),
            server_addr,
            last_sequence: Arc::new(Mutex::new(None)),
            config,
        })
    }

    pub fn receive_audio(&self, _buffer: &mut [u8]) -> Result<Option<Vec<u8>>, String> {
        let mut recv_buffer = vec![0u8; 65507]; // Max UDP packet size
        
        match self.socket.recv_from(&mut recv_buffer) {
            Ok((size, _)) => {
                recv_buffer.truncate(size);
                
                let packet: AudioPacket = bincode::deserialize(&recv_buffer)
                    .map_err(|e| format!("Failed to deserialize packet: {}", e))?;

                // Check for out-of-order packets
                let mut last_seq_guard = self.last_sequence.lock().map_err(|e| e.to_string())?;
                if let Some(last_seq) = *last_seq_guard {
                    if packet.sequence <= last_seq {
                        // Out-of-order or duplicate packet, skip
                        return Ok(None);
                    }
                }
                *last_seq_guard = Some(packet.sequence);

                Ok(Some(packet.data))
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                Ok(None)
            }
            Err(e) => Err(format!("Failed to receive: {}", e)),
        }
    }

    pub fn send_heartbeat(&self) -> Result<(), String> {
        let heartbeat = b"HEARTBEAT";
        self.socket.send_to(heartbeat, self.server_addr)
            .map_err(|e| format!("Failed to send heartbeat: {}", e))?;
        Ok(())
    }
}
