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

                // Use DecodedYUV's built-in write_rgb8 which handles YUV to RGB conversion correctly
                let rgb_size = (width * height * 3) as usize;
                let mut rgb_data = vec![0u8; rgb_size];
                yuv.write_rgb8(&mut rgb_data);

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
