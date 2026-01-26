//! H.264 Decoder using OpenH264
//!
//! Decodes H.264 frames to RGB and encodes to JPEG

use image::{codecs::jpeg::JpegEncoder, ImageBuffer, Rgb};
use openh264::decoder::{Decoder, DecoderConfig};
use openh264::formats::YUVSource;
use openh264::OpenH264API;
use std::io::Cursor;
use std::sync::Mutex;

pub struct H264Decoder {
    decoder: Mutex<Decoder>,
    width: u32,
    height: u32,
}

impl H264Decoder {
    /// Create a new H.264 decoder
    pub fn new() -> Result<Self, String> {
        let api = OpenH264API::from_source();
        let config = DecoderConfig::default();

        let decoder = Decoder::with_api_config(api, config)
            .map_err(|e| format!("Failed to create decoder: {:?}", e))?;

        Ok(Self {
            decoder: Mutex::new(decoder),
            width: 0,
            height: 0,
        })
    }

    /// Decode an H.264 frame (Annex-B format) to JPEG base64
    pub fn decode_to_jpeg(&mut self, h264_data: &[u8]) -> Result<Option<String>, String> {
        let mut decoder = self.decoder.lock().map_err(|e| e.to_string())?;

        // Decode the frame
        let yuv_opt = decoder
            .decode(h264_data)
            .map_err(|e| format!("Decode error: {:?}", e))?;

        // Check if we got a frame (might be None for incomplete frames)
        match yuv_opt {
            Some(yuv) => {
                // Get dimensions from internal format
                let (width, height) = yuv.dimensions();
                let width = width as u32;
                let height = height as u32;

                // Update dimensions
                self.width = width;
                self.height = height;

                // Convert YUV to RGB using openh264's built-in method
                // RGB8 requires width * height * 3 bytes
                let rgb_size = (width * height * 3) as usize;
                let mut rgb_data = vec![0u8; rgb_size];
                yuv.write_rgb8(&mut rgb_data);

                // Encode to JPEG
                let jpeg_data = rgb_to_jpeg(&rgb_data, width, height)?;

                // Encode to base64
                let base64_jpeg =
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &jpeg_data);

                Ok(Some(base64_jpeg))
            }
            None => {
                // No frame yet (need more data or waiting for keyframe)
                Ok(None)
            }
        }
    }

    /// Get current dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// Convert YUV420 planar to RGB24
fn yuv420_to_rgb(
    y_plane: &[u8],
    u_plane: &[u8],
    v_plane: &[u8],
    y_stride: usize,
    uv_stride: usize,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let width = width as usize;
    let height = height as usize;
    let mut rgb = vec![0u8; width * height * 3];

    for y in 0..height {
        for x in 0..width {
            // Get Y value
            let y_idx = y * y_stride + x;
            let y_val = if y_idx < y_plane.len() {
                y_plane[y_idx] as i32
            } else {
                16 // Default luma value
            };

            // Get U and V values (subsampled 4:2:0)
            let uv_y = y / 2;
            let uv_x = x / 2;
            let uv_idx = uv_y * uv_stride + uv_x;

            let u_val = if uv_idx < u_plane.len() {
                u_plane[uv_idx] as i32 - 128
            } else {
                0
            };

            let v_val = if uv_idx < v_plane.len() {
                v_plane[uv_idx] as i32 - 128
            } else {
                0
            };

            // Convert YUV to RGB using ITU-R BT.601
            let r = (y_val + ((1.402 * v_val as f32) as i32)).clamp(0, 255);
            let g = (y_val - ((0.344 * u_val as f32) as i32) - ((0.714 * v_val as f32) as i32))
                .clamp(0, 255);
            let b = (y_val + ((1.772 * u_val as f32) as i32)).clamp(0, 255);

            // Store RGB
            let rgb_idx = (y * width + x) * 3;
            rgb[rgb_idx] = r as u8;
            rgb[rgb_idx + 1] = g as u8;
            rgb[rgb_idx + 2] = b as u8;
        }
    }

    rgb
}

/// Encode RGB24 data to JPEG
fn rgb_to_jpeg(rgb_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    // Create image buffer from RGB data
    let img = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(width, height, rgb_data.to_vec())
        .ok_or("Failed to create image buffer")?;

    // Encode to JPEG with quality 85
    let mut jpeg_data = Cursor::new(Vec::new());
    let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_data, 85);

    encoder
        .encode(img.as_raw(), width, height, image::ExtendedColorType::Rgb8)
        .map_err(|e| format!("JPEG encode error: {}", e))?;

    Ok(jpeg_data.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_creation() {
        let decoder = H264Decoder::new();
        assert!(decoder.is_ok());
    }
}
