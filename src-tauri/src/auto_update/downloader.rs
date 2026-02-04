// Downloader Module
// Handles file downloads with progress reporting and resume support
// Requirements: 3.1, 3.2, 15.4

use reqwest::Client;
use std::path::{Path, PathBuf};
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use futures_util::StreamExt;

use crate::auto_update::UpdateError;

/// Download progress information
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadProgress {
    /// Bytes downloaded so far
    pub bytes_downloaded: u64,
    /// Total bytes to download (if known)
    pub total_bytes: Option<u64>,
    /// Download progress as percentage (0.0 - 100.0)
    pub percentage: f32,
}

/// Progress callback type for download operations
pub type ProgressCallback = Box<dyn Fn(DownloadProgress) + Send + Sync>;

/// File downloader with progress reporting and resume support
/// 
/// Requirements:
/// - 3.1: Download update package to temporary directory
/// - 3.2: Emit progress events with bytes_downloaded and total_bytes
/// - 15.4: Resume from last position if supported
pub struct Downloader {
    http_client: Client,
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new()
    }
}

impl Downloader {
    /// Create a new Downloader instance
    pub fn new() -> Self {
        Self {
            http_client: Client::builder()
                .timeout(std::time::Duration::from_secs(300)) // 5 minute timeout
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    /// Create a Downloader with a custom HTTP client
    pub fn with_client(client: Client) -> Self {
        Self {
            http_client: client,
        }
    }

    /// Download a file to the specified destination
    /// 
    /// Requirements: 3.1, 3.2
    /// - Save to temporary directory
    /// - Emit progress events via callback
    /// 
    /// # Arguments
    /// * `url` - URL to download from
    /// * `dest_path` - Destination file path
    /// * `expected_size` - Optional expected file size for progress calculation
    /// * `progress_callback` - Optional callback for progress updates
    /// 
    /// # Returns
    /// * `Ok(PathBuf)` - Path to the downloaded file
    /// * `Err(UpdateError)` - Error if download fails
    pub async fn download(
        &self,
        url: &str,
        dest_path: &Path,
        expected_size: Option<u64>,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<PathBuf, UpdateError> {
        // Create parent directories if they don't exist
        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| UpdateError::FileSystem(format!("Failed to create directory: {}", e)))?;
        }

        // Start fresh download (no resume)
        let response = self.http_client
            .get(url)
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    UpdateError::Network(format!("Connection failed: {}", e))
                } else if e.is_timeout() {
                    UpdateError::Network(format!("Request timed out: {}", e))
                } else {
                    UpdateError::DownloadFailed(format!("Request failed: {}", e))
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(UpdateError::DownloadFailed(format!(
                "HTTP error {}: {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown error")
            )));
        }

        // Get content length from response or use expected size
        let total_bytes = response.content_length().or(expected_size);

        // Create destination file
        let mut file = File::create(dest_path)
            .await
            .map_err(|e| UpdateError::FileSystem(format!("Failed to create file: {}", e)))?;

        // Download with progress tracking
        let mut bytes_downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result
                .map_err(|e| UpdateError::DownloadFailed(format!("Failed to read chunk: {}", e)))?;

            file.write_all(&chunk)
                .await
                .map_err(|e| UpdateError::FileSystem(format!("Failed to write chunk: {}", e)))?;

            bytes_downloaded += chunk.len() as u64;

            // Emit progress callback
            if let Some(ref callback) = progress_callback {
                let percentage = match total_bytes {
                    Some(total) if total > 0 => (bytes_downloaded as f32 / total as f32) * 100.0,
                    _ => 0.0,
                };

                callback(DownloadProgress {
                    bytes_downloaded,
                    total_bytes,
                    percentage,
                });
            }
        }

        // Ensure all data is flushed to disk
        file.flush()
            .await
            .map_err(|e| UpdateError::FileSystem(format!("Failed to flush file: {}", e)))?;

        Ok(dest_path.to_path_buf())
    }

    /// Download a file with resume support
    /// 
    /// Requirements: 15.4
    /// - Check for existing partial file
    /// - Use HTTP Range header for resume
    /// - Verify resumed download integrity
    /// 
    /// # Arguments
    /// * `url` - URL to download from
    /// * `dest_path` - Destination file path
    /// * `progress_callback` - Optional callback for progress updates
    /// 
    /// # Returns
    /// * `Ok(PathBuf)` - Path to the downloaded file
    /// * `Err(UpdateError)` - Error if download fails
    pub async fn download_with_resume(
        &self,
        url: &str,
        dest_path: &Path,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<PathBuf, UpdateError> {
        // Create parent directories if they don't exist
        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| UpdateError::FileSystem(format!("Failed to create directory: {}", e)))?;
        }

        // Check for existing partial file
        let existing_size = self.get_existing_file_size(dest_path).await;

        // First, make a HEAD request to get the total size and check range support
        let head_response = self.http_client
            .head(url)
            .send()
            .await
            .map_err(|e| UpdateError::Network(format!("HEAD request failed: {}", e)))?;

        let total_bytes = head_response.content_length();
        let accepts_ranges = head_response
            .headers()
            .get("accept-ranges")
            .map(|v| v.to_str().unwrap_or("") == "bytes")
            .unwrap_or(false);

        // If we have a partial file and server supports ranges, try to resume
        if existing_size > 0 && accepts_ranges {
            // Check if the partial file is smaller than total
            if let Some(total) = total_bytes {
                if existing_size >= total {
                    // File is already complete
                    if let Some(ref callback) = progress_callback {
                        callback(DownloadProgress {
                            bytes_downloaded: total,
                            total_bytes: Some(total),
                            percentage: 100.0,
                        });
                    }
                    return Ok(dest_path.to_path_buf());
                }

                // Resume download from where we left off
                return self.resume_download(
                    url,
                    dest_path,
                    existing_size,
                    total,
                    progress_callback,
                ).await;
            }
        }

        // No resume possible, start fresh download
        self.download(url, dest_path, total_bytes, progress_callback).await
    }

    /// Resume a partial download using HTTP Range header
    async fn resume_download(
        &self,
        url: &str,
        dest_path: &Path,
        start_byte: u64,
        total_bytes: u64,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<PathBuf, UpdateError> {
        log::info!(
            "[Downloader] Resuming download from byte {} of {}",
            start_byte,
            total_bytes
        );

        // Make range request
        let response = self.http_client
            .get(url)
            .header("Range", format!("bytes={}-", start_byte))
            .send()
            .await
            .map_err(|e| UpdateError::Network(format!("Range request failed: {}", e)))?;

        let status = response.status();
        
        // Check for 206 Partial Content or 200 OK (some servers ignore Range)
        if status.as_u16() == 200 {
            // Server doesn't support range, start fresh
            log::warn!("[Downloader] Server returned 200 instead of 206, starting fresh download");
            return self.download(url, dest_path, Some(total_bytes), progress_callback).await;
        }

        if status.as_u16() != 206 {
            return Err(UpdateError::DownloadFailed(format!(
                "Range request failed with status {}",
                status.as_u16()
            )));
        }

        // Open file for appending
        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(dest_path)
            .await
            .map_err(|e| UpdateError::FileSystem(format!("Failed to open file for append: {}", e)))?;

        // Download remaining bytes with progress tracking
        let mut bytes_downloaded = start_byte;
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result
                .map_err(|e| UpdateError::DownloadFailed(format!("Failed to read chunk: {}", e)))?;

            file.write_all(&chunk)
                .await
                .map_err(|e| UpdateError::FileSystem(format!("Failed to write chunk: {}", e)))?;

            bytes_downloaded += chunk.len() as u64;

            // Emit progress callback
            if let Some(ref callback) = progress_callback {
                let percentage = (bytes_downloaded as f32 / total_bytes as f32) * 100.0;

                callback(DownloadProgress {
                    bytes_downloaded,
                    total_bytes: Some(total_bytes),
                    percentage,
                });
            }
        }

        // Ensure all data is flushed to disk
        file.flush()
            .await
            .map_err(|e| UpdateError::FileSystem(format!("Failed to flush file: {}", e)))?;

        // Verify final size
        let final_size = self.get_existing_file_size(dest_path).await;
        if final_size != total_bytes {
            return Err(UpdateError::DownloadFailed(format!(
                "Downloaded file size mismatch: expected {}, got {}",
                total_bytes, final_size
            )));
        }

        Ok(dest_path.to_path_buf())
    }

    /// Get the size of an existing file, or 0 if it doesn't exist
    async fn get_existing_file_size(&self, path: &Path) -> u64 {
        match tokio::fs::metadata(path).await {
            Ok(metadata) => metadata.len(),
            Err(_) => 0,
        }
    }

    /// Get a reference to the HTTP client
    pub fn client(&self) -> &Client {
        &self.http_client
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_downloader_creation() {
        let downloader = Downloader::new();
        // Just verify the client is created
        let _ = downloader.client();
    }

    #[test]
    fn test_downloader_default() {
        let downloader = Downloader::default();
        let _ = downloader.client();
    }

    #[test]
    fn test_downloader_with_custom_client() {
        let custom_client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap();
        let downloader = Downloader::with_client(custom_client);
        let _ = downloader.client();
    }

    #[test]
    fn test_download_progress_struct() {
        let progress = DownloadProgress {
            bytes_downloaded: 500,
            total_bytes: Some(1000),
            percentage: 50.0,
        };
        
        assert_eq!(progress.bytes_downloaded, 500);
        assert_eq!(progress.total_bytes, Some(1000));
        assert!((progress.percentage - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_download_progress_clone() {
        let progress = DownloadProgress {
            bytes_downloaded: 500,
            total_bytes: Some(1000),
            percentage: 50.0,
        };
        
        let cloned = progress.clone();
        assert_eq!(cloned.bytes_downloaded, progress.bytes_downloaded);
        assert_eq!(cloned.total_bytes, progress.total_bytes);
    }

    #[tokio::test]
    async fn test_get_existing_file_size_nonexistent() {
        let downloader = Downloader::new();
        let temp_dir = std::env::temp_dir().join(format!("downloader_test_{}", std::process::id()));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;
        let nonexistent_path = temp_dir.join("nonexistent.txt");
        
        let size = downloader.get_existing_file_size(&nonexistent_path).await;
        assert_eq!(size, 0);
        
        // Cleanup
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_get_existing_file_size_existing() {
        let downloader = Downloader::new();
        let temp_dir = std::env::temp_dir().join(format!("downloader_test_existing_{}", std::process::id()));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;
        let file_path = temp_dir.join("test.txt");
        
        // Create a file with known content
        tokio::fs::write(&file_path, b"Hello, World!").await.unwrap();
        
        let size = downloader.get_existing_file_size(&file_path).await;
        assert_eq!(size, 13); // "Hello, World!" is 13 bytes
        
        // Cleanup
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_download_creates_parent_directories() {
        let downloader = Downloader::new();
        let temp_dir = std::env::temp_dir().join(format!("downloader_test_dirs_{}", std::process::id()));
        let nested_path = temp_dir.join("a").join("b").join("c").join("test.txt");
        
        // This should fail because the URL is invalid, but it should create directories first
        let result = downloader.download(
            "http://invalid.local/test.txt",
            &nested_path,
            None,
            None,
        ).await;
        
        // The download will fail, but parent directories should exist
        assert!(result.is_err());
        assert!(nested_path.parent().unwrap().exists());
        
        // Cleanup
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_download_with_resume_creates_parent_directories() {
        let downloader = Downloader::new();
        let temp_dir = std::env::temp_dir().join(format!("downloader_resume_dirs_{}", std::process::id()));
        let nested_path = temp_dir.join("x").join("y").join("z").join("resume_test.txt");
        
        // This should fail because the URL is invalid, but it should create directories first
        let result = downloader.download_with_resume(
            "http://invalid.local/test.txt",
            &nested_path,
            None,
        ).await;
        
        // The download will fail, but parent directories should exist
        assert!(result.is_err());
        assert!(nested_path.parent().unwrap().exists());
        
        // Cleanup
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_progress_callback_is_called() {
        let progress_count = Arc::new(AtomicU64::new(0));
        let progress_count_clone = Arc::clone(&progress_count);
        
        let callback: ProgressCallback = Box::new(move |_progress| {
            progress_count_clone.fetch_add(1, Ordering::SeqCst);
        });
        
        // Simulate calling the callback
        callback(DownloadProgress {
            bytes_downloaded: 100,
            total_bytes: Some(1000),
            percentage: 10.0,
        });
        
        assert_eq!(progress_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_progress_callback_tracks_bytes() {
        let last_bytes = Arc::new(AtomicU64::new(0));
        let last_bytes_clone = Arc::clone(&last_bytes);
        
        let callback: ProgressCallback = Box::new(move |progress| {
            last_bytes_clone.store(progress.bytes_downloaded, Ordering::SeqCst);
        });
        
        // Simulate multiple progress updates
        callback(DownloadProgress {
            bytes_downloaded: 100,
            total_bytes: Some(1000),
            percentage: 10.0,
        });
        assert_eq!(last_bytes.load(Ordering::SeqCst), 100);
        
        callback(DownloadProgress {
            bytes_downloaded: 500,
            total_bytes: Some(1000),
            percentage: 50.0,
        });
        assert_eq!(last_bytes.load(Ordering::SeqCst), 500);
        
        callback(DownloadProgress {
            bytes_downloaded: 1000,
            total_bytes: Some(1000),
            percentage: 100.0,
        });
        assert_eq!(last_bytes.load(Ordering::SeqCst), 1000);
    }

    #[test]
    fn test_download_progress_percentage_calculation() {
        // Test percentage calculation logic
        let bytes_downloaded: u64 = 500;
        let total_bytes: u64 = 1000;
        let percentage = (bytes_downloaded as f32 / total_bytes as f32) * 100.0;
        
        assert!((percentage - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_download_progress_percentage_zero_total() {
        // When total is 0, percentage should be 0
        let bytes_downloaded: u64 = 500;
        let total_bytes: u64 = 0;
        let percentage = if total_bytes > 0 {
            (bytes_downloaded as f32 / total_bytes as f32) * 100.0
        } else {
            0.0
        };
        
        assert!((percentage - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_download_progress_percentage_at_boundaries() {
        // Test at 0%
        let percentage_0 = (0_u64 as f32 / 1000_u64 as f32) * 100.0;
        assert!((percentage_0 - 0.0).abs() < f32::EPSILON);
        
        // Test at 100%
        let percentage_100 = (1000_u64 as f32 / 1000_u64 as f32) * 100.0;
        assert!((percentage_100 - 100.0).abs() < f32::EPSILON);
        
        // Test at 25%
        let percentage_25 = (250_u64 as f32 / 1000_u64 as f32) * 100.0;
        assert!((percentage_25 - 25.0).abs() < f32::EPSILON);
        
        // Test at 75%
        let percentage_75 = (750_u64 as f32 / 1000_u64 as f32) * 100.0;
        assert!((percentage_75 - 75.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_range_header_format() {
        // Test that the Range header format is correct
        let start_byte: u64 = 1024;
        let range_header = format!("bytes={}-", start_byte);
        assert_eq!(range_header, "bytes=1024-");
        
        let start_byte_large: u64 = 1_000_000;
        let range_header_large = format!("bytes={}-", start_byte_large);
        assert_eq!(range_header_large, "bytes=1000000-");
    }

    #[tokio::test]
    async fn test_download_network_error() {
        let downloader = Downloader::new();
        let temp_dir = std::env::temp_dir().join(format!("downloader_net_err_{}", std::process::id()));
        let file_path = temp_dir.join("test.txt");
        
        let result = downloader.download(
            "http://192.0.2.1/nonexistent.txt", // TEST-NET-1, should fail
            &file_path,
            None,
            None,
        ).await;
        
        assert!(result.is_err());
        match result {
            Err(UpdateError::Network(_)) | Err(UpdateError::DownloadFailed(_)) => {
                // Expected error types
            }
            Err(e) => panic!("Unexpected error type: {:?}", e),
            Ok(_) => panic!("Expected error but got success"),
        }
        
        // Cleanup
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
