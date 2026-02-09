//! UDP Frame Transport for low-latency H.264 frame delivery
//!
//! Provides fragmentation/reassembly over UDP for H.264 frames.
//! Used as primary transport with WebSocket as fallback.
//!
//! Packet format (29-byte header):
//! [2 bytes: magic "SL"]
//! [4 bytes: frame_id (u32)]
//! [2 bytes: fragment_index (u16)]
//! [2 bytes: total_fragments (u16)]
//! [1 byte: flags - bit0: is_keyframe]
//! [8 bytes: timestamp (u64)]
//! [4 bytes: width (u32)]
//! [4 bytes: height (u32)]
//! [2 bytes: sps_pps_len (u16)] -- only meaningful in fragment 0
//! [payload bytes]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

const MAGIC: [u8; 2] = [b'S', b'L'];
const HEADER_SIZE: usize = 29;
const MAX_UDP_PAYLOAD: usize = 1400;
const MAX_FRAGMENT_PAYLOAD: usize = MAX_UDP_PAYLOAD - HEADER_SIZE;

/// Parsed frame header info shared across fragments
#[derive(Clone, Debug)]
pub struct FrameHeader {
    pub frame_id: u32,
    pub fragment_index: u16,
    pub total_fragments: u16,
    pub is_keyframe: bool,
    pub timestamp: u64,
    pub width: u32,
    pub height: u32,
    pub sps_pps_len: u16,
}

/// A fully reassembled UDP frame
#[derive(Clone, Debug)]
pub struct ReassembledFrame {
    pub is_keyframe: bool,
    pub timestamp: u64,
    pub width: u32,
    pub height: u32,
    pub sps_pps: Option<Vec<u8>>,
    pub h264_data: Vec<u8>,
}

/// Encode a header into bytes
fn encode_header(h: &FrameHeader) -> [u8; HEADER_SIZE] {
    let mut buf = [0u8; HEADER_SIZE];
    buf[0..2].copy_from_slice(&MAGIC);
    buf[2..6].copy_from_slice(&h.frame_id.to_le_bytes());
    buf[6..8].copy_from_slice(&h.fragment_index.to_le_bytes());
    buf[8..10].copy_from_slice(&h.total_fragments.to_le_bytes());
    buf[10] = if h.is_keyframe { 1 } else { 0 };
    buf[11..19].copy_from_slice(&h.timestamp.to_le_bytes());
    buf[19..23].copy_from_slice(&h.width.to_le_bytes());
    buf[23..27].copy_from_slice(&h.height.to_le_bytes());
    buf[27..29].copy_from_slice(&h.sps_pps_len.to_le_bytes());
    buf
}

/// Decode a header from bytes
fn decode_header(buf: &[u8]) -> Option<FrameHeader> {
    if buf.len() < HEADER_SIZE {
        return None;
    }
    if buf[0] != MAGIC[0] || buf[1] != MAGIC[1] {
        return None;
    }
    Some(FrameHeader {
        frame_id: u32::from_le_bytes(buf[2..6].try_into().ok()?),
        fragment_index: u16::from_le_bytes(buf[6..8].try_into().ok()?),
        total_fragments: u16::from_le_bytes(buf[8..10].try_into().ok()?),
        is_keyframe: buf[10] & 1 != 0,
        timestamp: u64::from_le_bytes(buf[11..19].try_into().ok()?),
        width: u32::from_le_bytes(buf[19..23].try_into().ok()?),
        height: u32::from_le_bytes(buf[23..27].try_into().ok()?),
        sps_pps_len: u16::from_le_bytes(buf[27..29].try_into().ok()?),
    })
}

// ─── SENDER (Student side) ───────────────────────────────────────────

/// Sends H.264 frames over UDP with fragmentation (sync version for capture thread).
/// `binary_frame` uses the same format as the WebSocket binary frame:
/// [1 byte type][8 bytes ts][4 bytes w][4 bytes h][2 bytes desc_len][desc][h264 data]
pub fn send_frame_udp_sync(
    socket: &std::net::UdpSocket,
    target: SocketAddr,
    frame_id: u32,
    binary_frame: &[u8],
) -> Result<(), String> {
    if binary_frame.len() < 19 {
        return Err("Frame too small".into());
    }

    // Parse the binary frame header (same format as WebSocket)
    let is_keyframe = binary_frame[0] == 1;
    let timestamp = u64::from_le_bytes(binary_frame[1..9].try_into().unwrap_or([0u8; 8]));
    let width = u32::from_le_bytes(binary_frame[9..13].try_into().unwrap_or([0u8; 4]));
    let height = u32::from_le_bytes(binary_frame[13..17].try_into().unwrap_or([0u8; 4]));
    let desc_len = u16::from_le_bytes(binary_frame[17..19].try_into().unwrap_or([0u8; 2]));

    // The payload is everything after the 19-byte WS header (desc + h264 data)
    let payload = &binary_frame[19..];
    let total_len = payload.len();
    let total_fragments = ((total_len + MAX_FRAGMENT_PAYLOAD - 1) / MAX_FRAGMENT_PAYLOAD).max(1) as u16;

    for i in 0..total_fragments {
        let start = (i as usize) * MAX_FRAGMENT_PAYLOAD;
        let end = ((i as usize + 1) * MAX_FRAGMENT_PAYLOAD).min(total_len);
        let chunk = &payload[start..end];

        let header = FrameHeader {
            frame_id,
            fragment_index: i,
            total_fragments,
            is_keyframe,
            timestamp,
            width,
            height,
            sps_pps_len: desc_len,
        };

        let hdr_bytes = encode_header(&header);
        let mut packet = Vec::with_capacity(HEADER_SIZE + chunk.len());
        packet.extend_from_slice(&hdr_bytes);
        packet.extend_from_slice(chunk);

        socket.send_to(&packet, target).map_err(|e| e.to_string())?;
    }

    Ok(())
}

// ─── RECEIVER (Teacher side) ─────────────────────────────────────────

/// Fragment reassembly buffer for one frame
struct FrameBuffer {
    header: FrameHeader,
    fragments: Vec<Option<Vec<u8>>>,
    received_count: u16,
}

/// Starts a UDP receiver that reassembles fragments and emits complete frames.
/// Returns the local port the socket is bound to.
pub async fn start_udp_receiver(
    frame_tx: mpsc::Sender<ReassembledFrame>,
    stop_flag: Arc<AtomicBool>,
) -> Result<u16, String> {
    // Bind to any available port
    let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| format!("UDP bind failed: {}", e))?;
    let local_port = socket.local_addr().map_err(|e| e.to_string())?.port();

    log::info!("[UdpReceiver] Listening on port {}", local_port);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let mut frame_buffers: HashMap<u32, FrameBuffer> = HashMap::new();
        // Track last completed frame_id to discard stale fragments
        let mut last_completed_frame_id: u32 = 0;

        loop {
            if stop_flag.load(Ordering::Relaxed) {
                log::info!("[UdpReceiver] Stop flag set, exiting");
                break;
            }

            let recv_result = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                socket.recv_from(&mut buf),
            ).await;

            let (len, _addr) = match recv_result {
                Ok(Ok((len, addr))) => (len, addr),
                Ok(Err(e)) => {
                    log::warn!("[UdpReceiver] recv error: {}", e);
                    continue;
                }
                Err(_) => continue, // timeout, check stop flag
            };

            if len < HEADER_SIZE {
                continue;
            }

            let header = match decode_header(&buf[..len]) {
                Some(h) => h,
                None => continue,
            };

            let payload = buf[HEADER_SIZE..len].to_vec();

            // Discard fragments from old frames
            if header.frame_id < last_completed_frame_id.saturating_sub(5) {
                continue;
            }

            let fb = frame_buffers.entry(header.frame_id).or_insert_with(|| {
                FrameBuffer {
                    header: header.clone(),
                    fragments: vec![None; header.total_fragments as usize],
                    received_count: 0,
                }
            });

            let idx = header.fragment_index as usize;
            if idx < fb.fragments.len() && fb.fragments[idx].is_none() {
                fb.fragments[idx] = Some(payload);
                fb.received_count += 1;
            }

            // Check if frame is complete
            if fb.received_count == fb.header.total_fragments {
                let fh = fb.header.clone();
                let mut full_payload = Vec::new();
                for frag in &fb.fragments {
                    if let Some(data) = frag {
                        full_payload.extend_from_slice(data);
                    }
                }

                // Parse sps_pps and h264_data from the reassembled payload
                let desc_len = fh.sps_pps_len as usize;
                let (sps_pps, h264_data) = if desc_len > 0 && full_payload.len() >= desc_len {
                    let desc = full_payload[..desc_len].to_vec();
                    let data = full_payload[desc_len..].to_vec();
                    (Some(desc), data)
                } else {
                    (None, full_payload)
                };

                let frame = ReassembledFrame {
                    is_keyframe: fh.is_keyframe,
                    timestamp: fh.timestamp,
                    width: fh.width,
                    height: fh.height,
                    sps_pps,
                    h264_data,
                };

                if fh.frame_id > last_completed_frame_id {
                    last_completed_frame_id = fh.frame_id;
                }

                let _ = frame_tx.try_send(frame);

                // Cleanup old buffers
                let cutoff = last_completed_frame_id.saturating_sub(10);
                frame_buffers.retain(|id, _| *id > cutoff);
            }
        }

        log::info!("[UdpReceiver] Receiver task ended");
    });

    Ok(local_port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_encode_decode() {
        let header = FrameHeader {
            frame_id: 42,
            fragment_index: 1,
            total_fragments: 3,
            is_keyframe: true,
            timestamp: 12345678,
            width: 1920,
            height: 1080,
            sps_pps_len: 32,
        };
        let encoded = encode_header(&header);
        let decoded = decode_header(&encoded).unwrap();
        assert_eq!(decoded.frame_id, 42);
        assert_eq!(decoded.fragment_index, 1);
        assert_eq!(decoded.total_fragments, 3);
        assert!(decoded.is_keyframe);
        assert_eq!(decoded.timestamp, 12345678);
        assert_eq!(decoded.width, 1920);
        assert_eq!(decoded.height, 1080);
        assert_eq!(decoded.sps_pps_len, 32);
    }

    #[test]
    fn test_invalid_magic() {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0] = b'X';
        buf[1] = b'Y';
        assert!(decode_header(&buf).is_none());
    }
}
