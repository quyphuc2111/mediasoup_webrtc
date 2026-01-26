//! Screen capture module for student agent
//! 
//! Captures screen frames and encodes them as JPEG for transmission

use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use image::{ImageBuffer, Rgba, ImageEncoder};
use image::codecs::jpeg::JpegEncoder;
use xcap::Monitor;

/// Capture settings
pub struct CaptureSettings {
    /// JPEG quality (1-100)
    pub quality: u8,
    /// Target width (0 = original)
    pub target_width: u32,
    /// Target height (0 = original)
    pub target_height: u32,
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            quality: 50, // Lower quality for faster transmission
            target_width: 0,
            target_height: 0,
        }
    }
}

/// Capture a single frame from the primary monitor
pub fn capture_frame(settings: &CaptureSettings) -> Result<Vec<u8>, String> {
    // Get primary monitor
    let monitors = Monitor::all()
        .map_err(|e| format!("Failed to get monitors: {}", e))?;
    
    let monitor = monitors.first()
        .ok_or_else(|| "No monitors found".to_string())?;
    
    // Capture screen
    let image = monitor.capture_image()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;
    
    // Convert to RGBA
    let width = image.width();
    let height = image.height();
    let raw_pixels = image.into_raw();
    
    // xcap returns BGRA, convert to RGBA
    let rgba_pixels: Vec<u8> = raw_pixels
        .chunks(4)
        .flat_map(|chunk| {
            if chunk.len() >= 4 {
                [chunk[2], chunk[1], chunk[0], chunk[3]] // BGRA -> RGBA
            } else {
                [0, 0, 0, 255]
            }
        })
        .collect();
    
    let img_buffer: ImageBuffer<Rgba<u8>, Vec<u8>> = 
        ImageBuffer::from_raw(width, height, rgba_pixels)
            .ok_or_else(|| "Failed to create image buffer".to_string())?;
    
    // Resize if needed
    let final_image = if settings.target_width > 0 && settings.target_height > 0 {
        image::imageops::resize(
            &img_buffer,
            settings.target_width,
            settings.target_height,
            image::imageops::FilterType::Triangle,
        )
    } else if settings.target_width > 0 {
        // Maintain aspect ratio
        let ratio = settings.target_width as f32 / width as f32;
        let new_height = (height as f32 * ratio) as u32;
        image::imageops::resize(
            &img_buffer,
            settings.target_width,
            new_height,
            image::imageops::FilterType::Triangle,
        )
    } else {
        img_buffer
    };
    
    // Encode as JPEG
    let mut buffer = Cursor::new(Vec::new());
    let encoder = JpegEncoder::new_with_quality(&mut buffer, settings.quality);
    
    // Convert RGBA to RGB for JPEG
    let rgb_image: ImageBuffer<image::Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(
        final_image.width(),
        final_image.height(),
        |x, y| {
            let pixel = final_image.get_pixel(x, y);
            image::Rgb([pixel[0], pixel[1], pixel[2]])
        }
    );
    
    encoder.write_image(
        &rgb_image,
        rgb_image.width(),
        rgb_image.height(),
        image::ExtendedColorType::Rgb8,
    ).map_err(|e| format!("Failed to encode JPEG: {}", e))?;
    
    Ok(buffer.into_inner())
}

/// Capture frames continuously and send via callback
pub async fn capture_loop<F>(
    settings: CaptureSettings,
    stop_flag: Arc<AtomicBool>,
    interval_ms: u64,
    mut on_frame: F,
) where
    F: FnMut(Vec<u8>) + Send,
{
    println!("[ScreenCapture] Starting capture loop with {}ms interval", interval_ms);
    
    while !stop_flag.load(Ordering::Relaxed) {
        match capture_frame(&settings) {
            Ok(frame_data) => {
                on_frame(frame_data);
            }
            Err(e) => {
                println!("[ScreenCapture] Capture error: {}", e);
            }
        }
        
        tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)).await;
    }
    
    println!("[ScreenCapture] Capture loop stopped");
}

/// Get screen resolution
pub fn get_screen_resolution() -> Result<(u32, u32), String> {
    let monitors = Monitor::all()
        .map_err(|e| format!("Failed to get monitors: {}", e))?;
    
    let monitor = monitors.first()
        .ok_or_else(|| "No monitors found".to_string())?;
    
    Ok((monitor.width(), monitor.height()))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_capture_frame() {
        let settings = CaptureSettings::default();
        let result = capture_frame(&settings);
        // This may fail in CI/headless environments
        if result.is_ok() {
            let data = result.unwrap();
            assert!(!data.is_empty());
            // Check JPEG magic bytes
            assert_eq!(data[0], 0xFF);
            assert_eq!(data[1], 0xD8);
        }
    }
}
