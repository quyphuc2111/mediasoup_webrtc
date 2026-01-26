//! H.264 Encoder using OpenH264
//!
//! Provides efficient video encoding for screen sharing with:
//! - Keyframe (I-frame) every N frames
//! - Delta frames (P-frames) for efficient bandwidth
//! - RGBA to YUV420 conversion
//! - Handles stride/padding from screen capture

use openh264::encoder::{Encoder, EncoderConfig};
use openh264::formats::YUVBuffer;
use openh264::OpenH264API;
use std::sync::Mutex;
use rayon::prelude::*;

/// H.264 encoder state
pub struct H264Encoder {
    encoder: Mutex<Encoder>,
    width: u32,
    height: u32,
    frame_count: Mutex<u64>,
    keyframe_interval: u64,
}

/// Encoded frame data
#[derive(Clone)]
pub struct EncodedFrame {
    /// NAL unit data in Annex-B format (for transmission)
    pub data: Vec<u8>,
    /// AVCC format codec description (SPS/PPS) for WebCodecs
    /// Only present for keyframes
    pub sps_pps: Option<Vec<u8>>,
    /// Frame timestamp in milliseconds
    pub timestamp: u64,
    /// Whether this is a keyframe (I-frame)
    pub is_keyframe: bool,
    /// Frame width
    pub width: u32,
    /// Frame height  
    pub height: u32,
}

impl H264Encoder {
    /// Create a new H.264 encoder
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        // Ensure dimensions are even (required for YUV420)
        let width = width & !1;
        let height = height & !1;
        
        let api = OpenH264API::from_source();
        let config = EncoderConfig::new()
            .max_frame_rate(30.0)
            .set_bitrate_bps(2_000_000) // 2 Mbps
            .enable_skip_frame(false);
        
        let encoder = Encoder::with_api_config(api, config)
            .map_err(|e| format!("Failed to create encoder: {:?}", e))?;
        
        Ok(Self {
            encoder: Mutex::new(encoder),
            width,
            height,
            frame_count: Mutex::new(0),
            keyframe_interval: 30, // Keyframe every 30 frames (~1 second at 30fps)
        })
    }
    
    /// Update encoder dimensions if screen size changed
    pub fn update_dimensions(&mut self, width: u32, height: u32) -> Result<(), String> {
        let width = width & !1;
        let height = height & !1;
        
        if self.width != width || self.height != height {
            // Recreate encoder for new dimensions
            let api = OpenH264API::from_source();
            let config = EncoderConfig::new()
                .max_frame_rate(30.0)
                .set_bitrate_bps(2_000_000)
                .enable_skip_frame(false);
            
            let encoder = Encoder::with_api_config(api, config)
                .map_err(|e| format!("Failed to recreate encoder: {:?}", e))?;
            
            *self.encoder.lock().unwrap() = encoder;
            self.width = width;
            self.height = height;
            *self.frame_count.lock().unwrap() = 0;
            crate::log_debug("info", &format!("[H264Encoder] Updated dimensions to {}x{}", width, height));
        }
        
        Ok(())
    }
    
    /// Extract SPS and PPS from Annex-B format and convert to AVCC
    /// Returns AVCC format description for WebCodecs
    fn extract_sps_pps_to_avcc(annex_b_data: &[u8]) -> Option<Vec<u8>> {
        // Annex-B format uses start codes: 0x00 0x00 0x00 0x01 or 0x00 0x00 0x01
        let mut sps: Option<Vec<u8>> = None;
        let mut pps: Option<Vec<u8>> = None;
        let mut i = 0;
        
        while i < annex_b_data.len().saturating_sub(4) {
            // Check for 4-byte start code: 0x00 0x00 0x00 0x01
            if annex_b_data[i] == 0x00 
                && annex_b_data[i + 1] == 0x00 
                && annex_b_data[i + 2] == 0x00 
                && annex_b_data[i + 3] == 0x01 {
                
                if i + 4 < annex_b_data.len() {
                    let nal_header = annex_b_data[i + 4];
                    let nal_type = nal_header & 0x1F;
                    
                    // Find next start code
                    let mut next_start = i + 5;
                    while next_start < annex_b_data.len().saturating_sub(4) {
                        if (annex_b_data[next_start] == 0x00 
                            && annex_b_data[next_start + 1] == 0x00 
                            && annex_b_data[next_start + 2] == 0x00 
                            && annex_b_data[next_start + 3] == 0x01)
                            || (annex_b_data[next_start] == 0x00 
                                && annex_b_data[next_start + 1] == 0x00 
                                && annex_b_data[next_start + 2] == 0x01) {
                            break;
                        }
                        next_start += 1;
                    }
                    
                    // Extract NAL unit (without start code and header)
                    let nal_data = &annex_b_data[i + 5..next_start];
                    
                    match nal_type {
                        7 => { // SPS
                            sps = Some(nal_data.to_vec());
                        }
                        8 => { // PPS
                            pps = Some(nal_data.to_vec());
                        }
                        _ => {}
                    }
                    
                    i = next_start;
                    continue;
                }
            }
            
            // Check for 3-byte start code: 0x00 0x00 0x01
            if i < annex_b_data.len().saturating_sub(3)
                && annex_b_data[i] == 0x00 
                && annex_b_data[i + 1] == 0x00 
                && annex_b_data[i + 2] == 0x01 {
                
                if i + 3 < annex_b_data.len() {
                    let nal_header = annex_b_data[i + 3];
                    let nal_type = nal_header & 0x1F;
                    
                    // Find next start code
                    let mut next_start = i + 4;
                    while next_start < annex_b_data.len().saturating_sub(3) {
                        if (annex_b_data[next_start] == 0x00 
                            && annex_b_data[next_start + 1] == 0x00 
                            && annex_b_data[next_start + 2] == 0x00 
                            && annex_b_data[next_start + 3] == 0x01)
                            || (annex_b_data[next_start] == 0x00 
                                && annex_b_data[next_start + 1] == 0x00 
                                && annex_b_data[next_start + 2] == 0x01) {
                            break;
                        }
                        next_start += 1;
                    }
                    
                    // Extract NAL unit (without start code and header)
                    let nal_data = &annex_b_data[i + 4..next_start];
                    
                    match nal_type {
                        7 => { // SPS
                            sps = Some(nal_data.to_vec());
                        }
                        8 => { // PPS
                            pps = Some(nal_data.to_vec());
                        }
                        _ => {}
                    }
                    
                    i = next_start;
                    continue;
                }
            }
            
            i += 1;
        }
        
        // Convert to AVCC format if we have both SPS and PPS
        let has_sps = sps.is_some();
        let has_pps = pps.is_some();
        
        if let (Some(sps_data), Some(pps_data)) = (sps, pps) {
            if sps_data.len() < 4 || pps_data.is_empty() {
                crate::log_debug("warn", &format!("[H264Encoder] SPS/PPS too short: SPS={} bytes, PPS={} bytes", sps_data.len(), pps_data.len()));
                return None;
            }
            
            // AVCC format:
            // [1 byte: configurationVersion = 1]
            // [1 byte: AVCProfileIndication] (from SPS byte 1)
            // [1 byte: profile_compatibility] (from SPS byte 2)
            // [1 byte: AVCLevelIndication] (from SPS byte 3)
            // [1 byte: lengthSizeMinusOne (0xFC = 4 bytes) | numOfSequenceParameterSets (0x01 = 1)]
            // [2 bytes: SPS length (big-endian)]
            // [SPS data]
            // [1 byte: numOfPictureParameterSets (0x01 = 1)]
            // [2 bytes: PPS length (big-endian)]
            // [PPS data]
            
            let mut avcc = Vec::with_capacity(8 + sps_data.len() + pps_data.len());
            
            // Configuration version
            avcc.push(1);
            
            // Profile and level from SPS
            avcc.push(sps_data[0]); // AVCProfileIndication
            avcc.push(sps_data[1]); // profile_compatibility
            avcc.push(sps_data[2]); // AVCLevelIndication
            
            // lengthSizeMinusOne (4 bytes = 0xFC) | numOfSequenceParameterSets (1 = 0x01)
            avcc.push(0xFC | 0x01);
            
            // SPS length (big-endian, 2 bytes)
            avcc.push((sps_data.len() >> 8) as u8);
            avcc.push(sps_data.len() as u8);
            
            // SPS data
            avcc.extend_from_slice(&sps_data);
            
            // numOfPictureParameterSets
            avcc.push(0x01);
            
            // PPS length (big-endian, 2 bytes)
            avcc.push((pps_data.len() >> 8) as u8);
            avcc.push(pps_data.len() as u8);
            
            // PPS data
            avcc.extend_from_slice(&pps_data);
            
            crate::log_debug("info", &format!("[H264Encoder] Extracted SPS/PPS: SPS={} bytes, PPS={} bytes, AVCC={} bytes", 
                sps_data.len(), pps_data.len(), avcc.len()));
            
            return Some(avcc);
        }
        
        crate::log_debug("warn", &format!("[H264Encoder] SPS/PPS not found: SPS={}, PPS={}", has_sps, has_pps));
        None
    }
    
    /// Encode an RGBA frame to H.264
    /// Accepts RGBA data and reported width/height (may differ from actual data size due to stride/padding)
    /// Automatically calculates actual dimensions from data size and updates encoder
    pub fn encode_rgba_with_size(&mut self, rgba_data: &[u8], reported_width: u32, reported_height: u32, timestamp: u64) -> Result<EncodedFrame, String> {
        // Calculate actual dimensions from data size (handles stride/padding)
        let actual_pixels = rgba_data.len() / 4;
        let reported_width_usize = reported_width as usize;
        
        // Calculate actual height from data size
        let calculated_height = actual_pixels / reported_width_usize;
        
        // Use reported width, but calculated height (to handle stride/padding)
        let actual_width = reported_width;
        let actual_height = calculated_height as u32;
        
        // Ensure dimensions are even (required for YUV420)
        let encoder_width = actual_width & !1;
        let encoder_height = actual_height & !1;
        
        // Update encoder dimensions if they changed
        if encoder_width != self.width || encoder_height != self.height {
            crate::log_debug("info", &format!("[H264Encoder] Updating dimensions: {}x{} -> {}x{} (reported: {}x{}, data: {} bytes, calculated height: {})", 
                self.width, self.height, encoder_width, encoder_height, 
                reported_width, reported_height, rgba_data.len(), calculated_height));
            self.update_dimensions(encoder_width, encoder_height)?;
        }
        
        // Crop data to match encoder dimensions (handle stride/padding)
        let expected_size = (self.width * self.height * 4) as usize;
        let rgba_data = if rgba_data.len() != expected_size {
            // Extract only the pixels we need (crop to encoder dimensions)
            let mut adjusted = Vec::with_capacity(expected_size);
            let src_stride = reported_width_usize * 4;
            let dst_stride = self.width as usize * 4;
            let rows_to_copy = (self.height as usize).min(calculated_height);
            
            for y in 0..rows_to_copy {
                let src_start = y * src_stride;
                let src_end = src_start + dst_stride.min(src_stride);
                if src_end <= rgba_data.len() {
                    adjusted.extend_from_slice(&rgba_data[src_start..src_end]);
                    // Pad row if source is narrower
                    if dst_stride > src_stride {
                        adjusted.extend(vec![0u8; dst_stride - src_stride]);
                    }
                }
            }
            // Pad remaining rows if needed
            while adjusted.len() < expected_size {
                adjusted.extend(vec![0u8; dst_stride]);
            }
            adjusted.truncate(expected_size);
            adjusted
        } else {
            rgba_data.to_vec()
        };
        
        self.encode_rgba(&rgba_data, timestamp)
    }
    
    /// Encode an RGBA frame to H.264
    pub fn encode_rgba(&self, rgba_data: &[u8], timestamp: u64) -> Result<EncodedFrame, String> {
        let expected_size = (self.width * self.height * 4) as usize;
        
        if rgba_data.len() != expected_size {
            return Err(format!(
                "Invalid RGBA data size: expected {}, got {} (width={}, height={})",
                expected_size,
                rgba_data.len(),
                self.width,
                self.height
            ));
        }
        
        // Convert RGBA to YUV420
        let yuv_data = rgba_to_yuv420(rgba_data, self.width, self.height);
        
        // Create YUV buffer
        let yuv_buffer = YUVBuffer::from_vec(
            yuv_data,
            self.width as usize,
            self.height as usize,
        );
        
        // Check if we need a keyframe
        let mut frame_count = self.frame_count.lock().unwrap();
        let force_keyframe = *frame_count == 0 || *frame_count % self.keyframe_interval == 0;
        let current_frame = *frame_count;
        *frame_count += 1;
        drop(frame_count);
        
        // Encode frame
        let mut encoder = self.encoder.lock().unwrap();
        
        // Force keyframe if needed
        if force_keyframe {
            encoder.force_intra_frame();
            if current_frame == 0 {
                crate::log_debug("info", "[H264Encoder] Forcing keyframe for first frame");
            } else {
                crate::log_debug("info", &format!("[H264Encoder] Forcing keyframe at frame {}", current_frame));
            }
        }
        
        let bitstream = encoder.encode(&yuv_buffer)
            .map_err(|e| format!("Encode error: {:?}", e))?;
        
        // Collect NAL units
        let mut data = Vec::new();
        
        // Write raw bitstream (Annex-B format)
        let bitstream_data = bitstream.to_vec();
        data.extend_from_slice(&bitstream_data);
        
        if current_frame == 0 || force_keyframe {
            crate::log_debug("info", &format!("[H264Encoder] Frame {}: bitstream size={} bytes", current_frame, data.len()));
            if data.len() > 0 && data.len() <= 100 {
                let preview: Vec<String> = data.iter().take(20).map(|b| format!("{:02x}", b)).collect();
                crate::log_debug("info", &format!("[H264Encoder] First 20 bytes: {}", preview.join(" ")));
            }
        }
        
        // Scan for NAL units to detect keyframe and extract SPS/PPS
        let mut is_keyframe = false;
        let mut has_sps = false;
        let mut has_pps = false;
        let mut has_idr = false;
        
        // Scan through data to find NAL units
        let mut i = 0;
        while i < data.len().saturating_sub(4) {
            // Check for 4-byte start code: 0x00 0x00 0x00 0x01
            if data[i] == 0x00 
                && data[i + 1] == 0x00 
                && data[i + 2] == 0x00 
                && data[i + 3] == 0x01 {
                
                if i + 4 < data.len() {
                    let nal_type = data[i + 4] & 0x1F;
                    match nal_type {
                        5 => has_idr = true,  // IDR frame (keyframe)
                        7 => has_sps = true,  // SPS
                        8 => has_pps = true,  // PPS
                        _ => {}
                    }
                }
            }
            // Check for 3-byte start code: 0x00 0x00 0x01
            else if i < data.len().saturating_sub(3)
                && data[i] == 0x00 
                && data[i + 1] == 0x00 
                && data[i + 2] == 0x01 {
                
                if i + 3 < data.len() {
                    let nal_type = data[i + 3] & 0x1F;
                    match nal_type {
                        5 => has_idr = true,  // IDR frame (keyframe)
                        7 => has_sps = true,  // SPS
                        8 => has_pps = true,  // PPS
                        _ => {}
                    }
                }
            }
            i += 1;
        }
        
        // Keyframe if we have IDR or if forced and we have SPS/PPS
        is_keyframe = has_idr || (force_keyframe && (has_sps || has_pps));
        
        if force_keyframe && !is_keyframe {
            crate::log_debug("warn", &format!("[H264Encoder] Warning: Keyframe forced but not detected (has_idr={}, has_sps={}, has_pps={})", 
                has_idr, has_sps, has_pps));
        }
        
        // Extract SPS/PPS for keyframes
        let mut sps_pps = None;
        if is_keyframe {
            sps_pps = Self::extract_sps_pps_to_avcc(&data);
            if sps_pps.is_none() && force_keyframe {
                // Log warning if we expected SPS/PPS but didn't find them
                crate::log_debug("warn", "[H264Encoder] Warning: Keyframe detected but SPS/PPS extraction failed");
            } else if sps_pps.is_some() {
                crate::log_debug("info", "[H264Encoder] Successfully extracted SPS/PPS for keyframe");
            }
        }
        
        Ok(EncodedFrame {
            data,
            sps_pps,
            timestamp,
            is_keyframe,
            width: self.width,
            height: self.height,
        })
    }
    
    /// Force next frame to be a keyframe
    pub fn request_keyframe(&self) {
        *self.frame_count.lock().unwrap() = 0;
    }
    
    /// Get current dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// Convert RGBA to YUV420 planar format (optimized with parallel processing)
fn rgba_to_yuv420(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let width = width as usize;
    let height = height as usize;
    
    let y_size = width * height;
    let uv_size = (width / 2) * (height / 2);
    
    let mut yuv = vec![0u8; y_size + uv_size * 2];
    
    let (y_plane, uv_planes) = yuv.split_at_mut(y_size);
    let (u_plane, v_plane) = uv_planes.split_at_mut(uv_size);
    
    // Process Y plane in parallel (process rows in parallel)
    y_plane.par_chunks_mut(width).enumerate().for_each(|(y, y_row)| {
        for x in 0..width {
            let rgba_idx = (y * width + x) * 4;
            let r = rgba[rgba_idx] as i32;
            let g = rgba[rgba_idx + 1] as i32;
            let b = rgba[rgba_idx + 2] as i32;
            
            // ITU-R BT.601 conversion
            let y_val = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16;
            y_row[x] = y_val.clamp(0, 255) as u8;
        }
    });
    
    // Process UV planes (sequential is fine since it's smaller than Y plane)
    for uv_y in 0..(height / 2) {
        for uv_x in 0..(width / 2) {
            let src_y = uv_y * 2;
            let src_x = uv_x * 2;
            
            // Average 2x2 block for U and V
            let mut r_sum = 0i32;
            let mut g_sum = 0i32;
            let mut b_sum = 0i32;
            
            for dy in 0..2 {
                for dx in 0..2 {
                    let rgba_idx = ((src_y + dy) * width + (src_x + dx)) * 4;
                    r_sum += rgba[rgba_idx] as i32;
                    g_sum += rgba[rgba_idx + 1] as i32;
                    b_sum += rgba[rgba_idx + 2] as i32;
                }
            }
            
            let r = r_sum / 4;
            let g = g_sum / 4;
            let b = b_sum / 4;
            
            let uv_idx = uv_y * (width / 2) + uv_x;
            let u_val = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
            let v_val = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;
            u_plane[uv_idx] = u_val.clamp(0, 255) as u8;
            v_plane[uv_idx] = v_val.clamp(0, 255) as u8;
        }
    }
    
    yuv
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encoder_creation() {
        let encoder = H264Encoder::new(640, 480);
        assert!(encoder.is_ok());
    }
    
    #[test]
    fn test_rgba_to_yuv() {
        let rgba = vec![128u8; 640 * 480 * 4];
        let yuv = rgba_to_yuv420(&rgba, 640, 480);
        assert_eq!(yuv.len(), 640 * 480 * 3 / 2);
    }
}
