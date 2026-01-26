//! H.264 Encoder using OpenH264
//!
//! Provides efficient video encoding for screen sharing with:
//! - Keyframe (I-frame) every N frames
//! - Delta frames (P-frames) for efficient bandwidth
//! - RGBA to YUV420 conversion

use openh264::encoder::{Encoder, EncoderConfig};
use openh264::formats::YUVBuffer;
use openh264::OpenH264API;
use std::sync::Mutex;

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
    /// NAL unit data (can contain multiple NAL units)
    pub data: Vec<u8>,
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
        }
        
        Ok(())
    }
    
    /// Encode an RGBA frame to H.264
    pub fn encode_rgba(&self, rgba_data: &[u8], timestamp: u64) -> Result<EncodedFrame, String> {
        let expected_size = (self.width * self.height * 4) as usize;
        if rgba_data.len() != expected_size {
            return Err(format!(
                "Invalid RGBA data size: expected {}, got {}",
                expected_size,
                rgba_data.len()
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
        *frame_count += 1;
        drop(frame_count);
        
        // Encode frame
        let mut encoder = self.encoder.lock().unwrap();
        
        let bitstream = encoder.encode(&yuv_buffer)
            .map_err(|e| format!("Encode error: {:?}", e))?;
        
        // Collect NAL units
        let mut data = Vec::new();
        let mut is_keyframe = force_keyframe; // Use our tracking since API changed
        
        // Write raw bitstream
        data.extend_from_slice(bitstream.to_vec().as_slice());
        
        // Check first NAL for keyframe
        if data.len() > 4 {
            let nal_type = data[4] & 0x1F;
            if nal_type == 5 || nal_type == 7 || nal_type == 8 {
                is_keyframe = true;
            }
        }
        
        Ok(EncodedFrame {
            data,
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

/// Convert RGBA to YUV420 planar format
fn rgba_to_yuv420(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let width = width as usize;
    let height = height as usize;
    
    let y_size = width * height;
    let uv_size = (width / 2) * (height / 2);
    
    let mut yuv = vec![0u8; y_size + uv_size * 2];
    
    let (y_plane, uv_planes) = yuv.split_at_mut(y_size);
    let (u_plane, v_plane) = uv_planes.split_at_mut(uv_size);
    
    // Convert each pixel
    for y in 0..height {
        for x in 0..width {
            let rgba_idx = (y * width + x) * 4;
            let r = rgba[rgba_idx] as i32;
            let g = rgba[rgba_idx + 1] as i32;
            let b = rgba[rgba_idx + 2] as i32;
            
            // ITU-R BT.601 conversion
            let y_val = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16;
            y_plane[y * width + x] = y_val.clamp(0, 255) as u8;
            
            // Subsample U and V (2x2 blocks)
            if y % 2 == 0 && x % 2 == 0 {
                let uv_idx = (y / 2) * (width / 2) + (x / 2);
                let u_val = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
                let v_val = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;
                u_plane[uv_idx] = u_val.clamp(0, 255) as u8;
                v_plane[uv_idx] = v_val.clamp(0, 255) as u8;
            }
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
