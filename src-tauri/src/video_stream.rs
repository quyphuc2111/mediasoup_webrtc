//! Video Stream - TCP server/client for raw H.264 video streaming
//!
//! This module provides:
//! - TCP server on student side to stream H.264 frames
//! - TCP client on teacher side to receive H.264 frames
//! - Simple framing protocol: [4 bytes: frame_size (big-endian)][frame_data]
//!
//! Frame data format:
//! - [1 byte: is_keyframe (0=delta, 1=keyframe)]
//! - [8 bytes: timestamp (little-endian)]
//! - [4 bytes: width (little-endian)]
//! - [4 bytes: height (little-endian)]
//! - [2 bytes: sps_pps_length (little-endian)]
//! - [sps_pps_length bytes: AVCC description] (only for keyframes)
//! - [remaining: H.264 Annex-B data]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

/// Video frame for streaming
#[derive(Clone, Debug)]
pub struct VideoFrame {
    pub is_keyframe: bool,
    pub timestamp: u64,
    pub width: u32,
    pub height: u32,
    pub sps_pps: Option<Vec<u8>>,
    pub data: Vec<u8>, // H.264 Annex-B data
}

impl VideoFrame {
    /// Serialize frame to wire format
    pub fn to_bytes(&self) -> Vec<u8> {
        let sps_pps_len = self.sps_pps.as_ref().map(|d| d.len()).unwrap_or(0);
        let mut frame_data = Vec::with_capacity(19 + sps_pps_len + self.data.len());

        // Frame metadata
        frame_data.push(if self.is_keyframe { 1 } else { 0 });
        frame_data.extend_from_slice(&self.timestamp.to_le_bytes());
        frame_data.extend_from_slice(&self.width.to_le_bytes());
        frame_data.extend_from_slice(&self.height.to_le_bytes());
        frame_data.extend_from_slice(&(sps_pps_len as u16).to_le_bytes());

        // Optional SPS/PPS
        if let Some(ref desc) = self.sps_pps {
            frame_data.extend_from_slice(desc);
        }

        // H.264 data
        frame_data.extend_from_slice(&self.data);

        // Prepend frame size (4 bytes, big-endian)
        let mut result = Vec::with_capacity(4 + frame_data.len());
        result.extend_from_slice(&(frame_data.len() as u32).to_be_bytes());
        result.extend_from_slice(&frame_data);

        result
    }

    /// Deserialize frame from wire format (without the 4-byte size prefix)
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < 19 {
            return Err(format!("Frame data too short: {} bytes", data.len()));
        }

        let is_keyframe = data[0] == 1;
        let timestamp = u64::from_le_bytes(
            data[1..9]
                .try_into()
                .map_err(|_| "Failed to parse timestamp")?,
        );
        let width = u32::from_le_bytes(
            data[9..13]
                .try_into()
                .map_err(|_| "Failed to parse width")?,
        );
        let height = u32::from_le_bytes(
            data[13..17]
                .try_into()
                .map_err(|_| "Failed to parse height")?,
        );
        let sps_pps_len = u16::from_le_bytes(
            data[17..19]
                .try_into()
                .map_err(|_| "Failed to parse sps_pps_len")?,
        ) as usize;

        if data.len() < 19 + sps_pps_len {
            return Err(format!(
                "Frame data too short for SPS/PPS: expected {}, got {}",
                19 + sps_pps_len,
                data.len()
            ));
        }

        let sps_pps = if sps_pps_len > 0 {
            Some(data[19..19 + sps_pps_len].to_vec())
        } else {
            None
        };

        let h264_data = data[19 + sps_pps_len..].to_vec();

        Ok(VideoFrame {
            is_keyframe,
            timestamp,
            width,
            height,
            sps_pps,
            data: h264_data,
        })
    }
}

/// Start TCP video server (student side)
/// Returns the port number it's listening on
pub async fn start_video_server(
    port: u16,
    mut frame_rx: mpsc::Receiver<VideoFrame>,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind video server to {}: {}", addr, e))?;

    crate::log_debug(
        "info",
        &format!("[VideoStream] TCP video server listening on port {}", port),
    );

    // Create broadcast channel for distributing frames to multiple clients
    let (broadcast_tx, _) = tokio::sync::broadcast::channel::<VideoFrame>(64);
    let broadcast_tx_clone = broadcast_tx.clone();

    // Spawn task to receive frames from mpsc and broadcast to all clients
    let stop_flag_broadcaster = Arc::clone(&stop_flag);
    tokio::spawn(async move {
        while !stop_flag_broadcaster.load(Ordering::Relaxed) {
            match tokio::time::timeout(tokio::time::Duration::from_millis(100), frame_rx.recv())
                .await
            {
                Ok(Some(frame)) => {
                    // Broadcast to all connected clients
                    let _ = broadcast_tx_clone.send(frame);
                }
                Ok(None) => {
                    crate::log_debug("info", "[VideoStream] Frame channel closed");
                    break;
                }
                Err(_) => {
                    // Timeout, continue
                    continue;
                }
            }
        }
    });

    // Accept connections in a loop
    let stop_flag_acceptor = Arc::clone(&stop_flag);
    tokio::spawn(async move {
        while !stop_flag_acceptor.load(Ordering::Relaxed) {
            // Accept connection
            let accept_result =
                tokio::time::timeout(tokio::time::Duration::from_millis(100), listener.accept())
                    .await;

            match accept_result {
                Ok(Ok((stream, addr))) => {
                    crate::log_debug(
                        "info",
                        &format!("[VideoStream] Video client connected: {}", addr),
                    );

                    // Subscribe to broadcast channel
                    let client_rx = broadcast_tx.subscribe();
                    let stop_flag_client = Arc::clone(&stop_flag_acceptor);

                    tokio::spawn(async move {
                        if let Err(e) =
                            handle_video_client(stream, client_rx, stop_flag_client).await
                        {
                            crate::log_debug(
                                "error",
                                &format!("[VideoStream] Client error {}: {}", addr, e),
                            );
                        }
                    });
                }
                Ok(Err(e)) => {
                    crate::log_debug("error", &format!("[VideoStream] Accept error: {}", e));
                    break;
                }
                Err(_) => {
                    // Timeout, continue loop
                    continue;
                }
            }
        }

        crate::log_debug("info", "[VideoStream] TCP video server stopped");
    });

    Ok(())
}

/// Handle a single video client connection
async fn handle_video_client(
    mut stream: TcpStream,
    mut frame_rx: tokio::sync::broadcast::Receiver<VideoFrame>,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    let mut frame_count: u64 = 0;

    while !stop_flag.load(Ordering::Relaxed) {
        // Receive frame from broadcast channel with timeout
        let frame_opt =
            tokio::time::timeout(tokio::time::Duration::from_millis(100), frame_rx.recv()).await;

        match frame_opt {
            Ok(Ok(frame)) => {
                // Serialize and send frame
                let frame_bytes = frame.to_bytes();

                if let Err(e) = stream.write_all(&frame_bytes).await {
                    return Err(format!("Failed to send frame: {}", e));
                }

                frame_count += 1;
                if frame_count == 1 || frame_count % 30 == 0 {
                    crate::log_debug(
                        "info",
                        &format!(
                            "[VideoStream] Sent {} frames, last: {} bytes, keyframe={}",
                            frame_count,
                            frame_bytes.len(),
                            frame.is_keyframe
                        ),
                    );
                }
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                // Client is too slow, skip lagged frames
                crate::log_debug("warn", "[VideoStream] Client lagging, skipping frames");
                continue;
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                // Channel closed
                crate::log_debug("info", "[VideoStream] Broadcast channel closed");
                break;
            }
            Err(_) => {
                // Timeout, continue
                continue;
            }
        }
    }

    Ok(())
}

/// Connect to video server and receive frames (teacher side)
pub async fn connect_video_client(
    ip: String,
    port: u16,
    frame_tx: mpsc::Sender<VideoFrame>,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    let addr = format!("{}:{}", ip, port);
    let mut stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| format!("Failed to connect to video server {}: {}", addr, e))?;

    crate::log_debug(
        "info",
        &format!("[VideoStream] Connected to video server: {}", addr),
    );

    let mut frame_count: u64 = 0;

    while !stop_flag.load(Ordering::Relaxed) {
        // Read frame size (4 bytes, big-endian)
        let mut size_buf = [0u8; 4];
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(5),
            stream.read_exact(&mut size_buf),
        )
        .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                return Err(format!("Failed to read frame size: {}", e));
            }
            Err(_) => {
                // Timeout - check stop flag
                continue;
            }
        }

        let frame_size = u32::from_be_bytes(size_buf) as usize;

        if frame_size == 0 || frame_size > 10_000_000 {
            return Err(format!("Invalid frame size: {}", frame_size));
        }

        // Read frame data
        let mut frame_data = vec![0u8; frame_size];
        stream
            .read_exact(&mut frame_data)
            .await
            .map_err(|e| format!("Failed to read frame data: {}", e))?;

        // Deserialize frame
        match VideoFrame::from_bytes(&frame_data) {
            Ok(frame) => {
                frame_count += 1;
                if frame_count == 1 || frame_count % 30 == 0 {
                    crate::log_debug(
                        "info",
                        &format!(
                            "[VideoStream] Received {} frames, last: {} bytes, keyframe={}",
                            frame_count, frame_size, frame.is_keyframe
                        ),
                    );
                }

                // Send to channel
                if frame_tx.send(frame).await.is_err() {
                    crate::log_debug("info", "[VideoStream] Frame receiver dropped");
                    break;
                }
            }
            Err(e) => {
                crate::log_debug(
                    "error",
                    &format!("[VideoStream] Failed to parse frame: {}", e),
                );
                continue;
            }
        }
    }

    crate::log_debug("info", "[VideoStream] Video client disconnected");
    Ok(())
}
