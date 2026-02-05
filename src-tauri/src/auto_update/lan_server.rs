// LAN Distribution Server Module
// HTTP server for distributing updates to Students over LAN
// Requirements: 7.1, 7.2, 7.3, 7.4, 7.5

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::Mutex;
use tokio_util::io::ReaderStream;

use crate::auto_update::UpdateError;

/// Error types specific to LAN server operations
#[derive(Debug, Clone)]
pub enum ServerError {
    /// Server is already running
    AlreadyRunning,
    /// Server is not running
    NotRunning,
    /// Port is already in use
    PortInUse(u16),
    /// No package configured to serve
    NoPackage,
    /// File system error
    FileSystem(String),
    /// Server failed to start
    StartFailed(String),
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerError::AlreadyRunning => write!(f, "Server is already running"),
            ServerError::NotRunning => write!(f, "Server is not running"),
            ServerError::PortInUse(port) => write!(f, "Port {} is already in use", port),
            ServerError::NoPackage => write!(f, "No update package configured"),
            ServerError::FileSystem(msg) => write!(f, "File system error: {}", msg),
            ServerError::StartFailed(msg) => write!(f, "Server failed to start: {}", msg),
        }
    }
}

impl std::error::Error for ServerError {}

impl From<ServerError> for UpdateError {
    fn from(err: ServerError) -> Self {
        UpdateError::FileSystem(err.to_string())
    }
}

/// Shared state for the LAN distribution server
#[derive(Debug)]
struct ServerState {
    package_path: PathBuf,
    package_hash: String,
    package_size: u64,
    /// Original filename with extension for Content-Disposition
    filename: String,
}

/// LAN distribution server for serving update packages to Students
/// 
/// Requirements:
/// - 7.1: Serve update package over HTTP on configurable port
/// - 7.2: Support range requests for resume downloads
/// - 7.3: Include SHA256 hash in response headers
/// - 7.4: Handle concurrent connections efficiently
/// - 7.5: Stop server on app close and release port
pub struct LanDistributionServer {
    port: u16,
    package_path: Mutex<Option<PathBuf>>,
    package_hash: Mutex<Option<String>>,
    shutdown_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    is_running: Mutex<bool>,
    local_ip: Mutex<Option<String>>,
}

impl LanDistributionServer {
    /// Create a new LAN distribution server
    /// 
    /// # Arguments
    /// * `port` - Port to listen on (default: 9280)
    pub fn new(port: u16) -> Self {
        Self {
            port,
            package_path: Mutex::new(None),
            package_hash: Mutex::new(None),
            shutdown_tx: Mutex::new(None),
            is_running: Mutex::new(false),
            local_ip: Mutex::new(None),
        }
    }

    /// Start the LAN distribution server
    /// 
    /// Requirements: 7.1, 7.5
    /// - Serve update package over HTTP on configurable port
    /// - Handle port conflicts
    /// 
    /// # Arguments
    /// * `package_path` - Path to the update package file
    /// * `hash` - SHA256 hash of the package
    /// 
    /// # Returns
    /// * `Ok(())` - Server started successfully
    /// * `Err(ServerError)` - Failed to start server
    pub async fn start(&self, package_path: PathBuf, hash: String) -> Result<(), ServerError> {
        // Check if already running
        {
            let is_running = self.is_running.lock().await;
            if *is_running {
                return Err(ServerError::AlreadyRunning);
            }
        }

        // Verify the package file exists and get its size
        let metadata = tokio::fs::metadata(&package_path).await.map_err(|e| {
            ServerError::FileSystem(format!("Failed to read package metadata: {}", e))
        })?;
        let package_size = metadata.len();

        // Store package info
        {
            let mut path = self.package_path.lock().await;
            *path = Some(package_path.clone());
        }
        {
            let mut stored_hash = self.package_hash.lock().await;
            *stored_hash = Some(hash.clone());
        }

        // Extract original filename for Content-Disposition header
        let filename = package_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("update_package")
            .to_string();

        // Create shared state for handlers
        let state = Arc::new(ServerState {
            package_path,
            package_hash: hash,
            package_size,
            filename,
        });

        // Build the router
        let app = Router::new()
            .route("/update/package", get(serve_package))
            .route("/update/info", get(serve_info))
            .route("/health", get(health_check))
            .with_state(state);

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        {
            let mut tx = self.shutdown_tx.lock().await;
            *tx = Some(shutdown_tx);
        }

        // Try to bind to the port
        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                ServerError::PortInUse(self.port)
            } else {
                ServerError::StartFailed(format!("Failed to bind to port {}: {}", self.port, e))
            }
        })?;

        // Get local IP for download URL
        let local_ip = get_local_ip().unwrap_or_else(|| "127.0.0.1".to_string());
        {
            let mut ip = self.local_ip.lock().await;
            *ip = Some(local_ip);
        }

        // Mark as running
        {
            let mut is_running = self.is_running.lock().await;
            *is_running = true;
        }

        log::info!("[LanServer] Starting on port {}", self.port);

        // Spawn the server task
        let is_running_clone = Arc::new(Mutex::new(true));
        let is_running_for_task = is_running_clone.clone();
        
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                    log::info!("[LanServer] Shutdown signal received");
                })
                .await
                .ok();
            
            let mut running = is_running_for_task.lock().await;
            *running = false;
            log::info!("[LanServer] Server stopped");
        });

        Ok(())
    }

    /// Stop the LAN distribution server
    /// 
    /// Requirements: 7.5
    /// - Stop server on app close
    /// - Release the port
    pub async fn stop(&self) -> Result<(), ServerError> {
        let is_running = {
            let running = self.is_running.lock().await;
            *running
        };

        if !is_running {
            return Err(ServerError::NotRunning);
        }

        // Send shutdown signal
        let shutdown_tx = {
            let mut tx = self.shutdown_tx.lock().await;
            tx.take()
        };

        if let Some(tx) = shutdown_tx {
            let _ = tx.send(());
        }

        // Clear state
        {
            let mut path = self.package_path.lock().await;
            *path = None;
        }
        {
            let mut hash = self.package_hash.lock().await;
            *hash = None;
        }
        {
            let mut ip = self.local_ip.lock().await;
            *ip = None;
        }
        {
            let mut is_running = self.is_running.lock().await;
            *is_running = false;
        }

        log::info!("[LanServer] Stopped");
        Ok(())
    }

    /// Get the download URL for the update package
    /// 
    /// # Returns
    /// * `Some(String)` - URL if server is running
    /// * `None` - Server is not running
    pub async fn get_download_url(&self) -> Option<String> {
        let is_running = self.is_running.lock().await;
        if !*is_running {
            return None;
        }

        let ip = self.local_ip.lock().await;
        ip.as_ref().map(|ip| format!("http://{}:{}/update/package", ip, self.port))
    }

    /// Check if the server is running
    pub async fn is_running(&self) -> bool {
        let is_running = self.is_running.lock().await;
        *is_running
    }

    /// Get the configured port
    pub fn get_port(&self) -> u16 {
        self.port
    }

    /// Get the package hash if configured
    pub async fn get_package_hash(&self) -> Option<String> {
        let hash = self.package_hash.lock().await;
        hash.clone()
    }
}

/// Handler for serving the update package
/// 
/// Requirements: 7.1, 7.2, 7.3
/// - Serve update package over HTTP
/// - Support range requests for resume downloads
/// - Include SHA256 hash in response headers
async fn serve_package(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Response {
    // Open the file
    let file = match File::open(&state.package_path).await {
        Ok(f) => f,
        Err(e) => {
            log::error!("[LanServer] Failed to open package file: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to open file").into_response();
        }
    };

    let file_size = state.package_size;

    // Check for Range header
    if let Some(range_header) = headers.get(header::RANGE) {
        if let Ok(range_str) = range_header.to_str() {
            if let Some((start, end)) = parse_range_header(range_str, file_size) {
                return serve_partial_content(file, start, end, file_size, &state.package_hash).await;
            }
        }
        // Invalid range - return 416 Range Not Satisfiable
        return (
            StatusCode::RANGE_NOT_SATISFIABLE,
            [(header::CONTENT_RANGE, format!("bytes */{}", file_size))],
        ).into_response();
    }

    // Serve full file
    serve_full_content(file, file_size, &state.package_hash, &state.filename).await
}

/// Serve the full file content
/// 
/// Requirements: 7.1, 7.3
async fn serve_full_content(file: File, file_size: u64, hash: &str, filename: &str) -> Response {
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, file_size)
        .header(header::ACCEPT_RANGES, "bytes")
        .header("X-SHA256", hash)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(body)
        .unwrap()
}

/// Serve partial content for range requests
/// 
/// Requirements: 7.2
async fn serve_partial_content(
    mut file: File,
    start: u64,
    end: u64,
    total_size: u64,
    hash: &str,
) -> Response {
    // Seek to start position
    if let Err(e) = file.seek(std::io::SeekFrom::Start(start)).await {
        log::error!("[LanServer] Failed to seek in file: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to seek in file").into_response();
    }

    let content_length = end - start + 1;
    
    // Create a limited reader for the range
    let limited_file = file.take(content_length);
    let stream = ReaderStream::new(limited_file);
    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, content_length)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(
            header::CONTENT_RANGE,
            format!("bytes {}-{}/{}", start, end, total_size),
        )
        .header("X-SHA256", hash)
        .body(body)
        .unwrap()
}

/// Parse HTTP Range header
/// 
/// Supports formats:
/// - bytes=0-499 (first 500 bytes)
/// - bytes=500-999 (second 500 bytes)
/// - bytes=-500 (last 500 bytes)
/// - bytes=500- (from byte 500 to end)
/// 
/// # Returns
/// * `Some((start, end))` - Valid range (inclusive)
/// * `None` - Invalid range
fn parse_range_header(range: &str, file_size: u64) -> Option<(u64, u64)> {
    let range = range.strip_prefix("bytes=")?;
    
    if let Some(suffix_len) = range.strip_prefix('-') {
        // Suffix range: -500 means last 500 bytes
        let len: u64 = suffix_len.parse().ok()?;
        if len == 0 || len > file_size {
            return None;
        }
        let start = file_size - len;
        return Some((start, file_size - 1));
    }

    let parts: Vec<&str> = range.split('-').collect();
    if parts.len() != 2 {
        return None;
    }

    let start: u64 = parts[0].parse().ok()?;
    
    let end = if parts[1].is_empty() {
        // Open-ended range: 500- means from 500 to end
        file_size - 1
    } else {
        parts[1].parse().ok()?
    };

    // Validate range
    if start > end || start >= file_size {
        return None;
    }

    // Clamp end to file size
    let end = end.min(file_size - 1);

    Some((start, end))
}

/// Handler for serving update info
async fn serve_info(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let info = serde_json::json!({
        "sha256": state.package_hash,
        "size": state.package_size,
    });

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&info).unwrap(),
    )
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

/// Get the local IP address
fn get_local_ip() -> Option<String> {
    // Try to get the local IP by connecting to a public address
    // This doesn't actually send any data, just determines the local interface
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let local_addr = socket.local_addr().ok()?;
    Some(local_addr.ip().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range_header_full_range() {
        assert_eq!(parse_range_header("bytes=0-499", 1000), Some((0, 499)));
        assert_eq!(parse_range_header("bytes=500-999", 1000), Some((500, 999)));
    }

    #[test]
    fn test_parse_range_header_open_end() {
        assert_eq!(parse_range_header("bytes=500-", 1000), Some((500, 999)));
        assert_eq!(parse_range_header("bytes=0-", 1000), Some((0, 999)));
    }

    #[test]
    fn test_parse_range_header_suffix() {
        assert_eq!(parse_range_header("bytes=-500", 1000), Some((500, 999)));
        assert_eq!(parse_range_header("bytes=-100", 1000), Some((900, 999)));
    }

    #[test]
    fn test_parse_range_header_clamp_end() {
        // End beyond file size should be clamped
        assert_eq!(parse_range_header("bytes=0-2000", 1000), Some((0, 999)));
    }

    #[test]
    fn test_parse_range_header_invalid() {
        // Invalid formats
        assert_eq!(parse_range_header("bytes=", 1000), None);
        assert_eq!(parse_range_header("bytes=abc-def", 1000), None);
        assert_eq!(parse_range_header("invalid", 1000), None);
        
        // Start beyond file size
        assert_eq!(parse_range_header("bytes=1000-", 1000), None);
        assert_eq!(parse_range_header("bytes=2000-3000", 1000), None);
        
        // Start > end
        assert_eq!(parse_range_header("bytes=500-100", 1000), None);
        
        // Zero suffix length
        assert_eq!(parse_range_header("bytes=-0", 1000), None);
        
        // Suffix larger than file
        assert_eq!(parse_range_header("bytes=-2000", 1000), None);
    }

    #[test]
    fn test_server_error_display() {
        assert_eq!(
            ServerError::AlreadyRunning.to_string(),
            "Server is already running"
        );
        assert_eq!(
            ServerError::PortInUse(8080).to_string(),
            "Port 8080 is already in use"
        );
        assert_eq!(
            ServerError::NoPackage.to_string(),
            "No update package configured"
        );
    }

    #[test]
    fn test_lan_server_new() {
        let server = LanDistributionServer::new(9280);
        assert_eq!(server.get_port(), 9280);
    }

    #[tokio::test]
    async fn test_lan_server_not_running_initially() {
        let server = LanDistributionServer::new(9280);
        assert!(!server.is_running().await);
        assert!(server.get_download_url().await.is_none());
    }

    #[tokio::test]
    async fn test_lan_server_stop_when_not_running() {
        let server = LanDistributionServer::new(9280);
        let result = server.stop().await;
        assert!(matches!(result, Err(ServerError::NotRunning)));
    }

    #[tokio::test]
    async fn test_lan_server_start_with_nonexistent_file() {
        let server = LanDistributionServer::new(19280);
        let result = server.start(
            PathBuf::from("/nonexistent/file.exe"),
            "abc123".to_string(),
        ).await;
        assert!(matches!(result, Err(ServerError::FileSystem(_))));
        assert!(!server.is_running().await);
    }

    #[tokio::test]
    async fn test_lan_server_lifecycle() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temp file to serve
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"test update package content").unwrap();
        temp_file.flush().unwrap();
        let file_path = temp_file.path().to_path_buf();

        // Use a unique port to avoid conflicts
        let server = LanDistributionServer::new(19281);
        
        // Start the server
        let result = server.start(file_path, "testhash123".to_string()).await;
        assert!(result.is_ok(), "Failed to start server: {:?}", result);
        
        // Verify server is running
        assert!(server.is_running().await);
        
        // Verify download URL is available
        let url = server.get_download_url().await;
        assert!(url.is_some());
        let url = url.unwrap();
        assert!(url.contains(":19281/update/package"));
        
        // Verify hash is stored
        let hash = server.get_package_hash().await;
        assert_eq!(hash, Some("testhash123".to_string()));
        
        // Stop the server
        let result = server.stop().await;
        assert!(result.is_ok());
        
        // Verify server is stopped
        assert!(!server.is_running().await);
        assert!(server.get_download_url().await.is_none());
        assert!(server.get_package_hash().await.is_none());
    }

    #[tokio::test]
    async fn test_lan_server_double_start() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temp file to serve
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"test content").unwrap();
        temp_file.flush().unwrap();
        let file_path = temp_file.path().to_path_buf();

        let server = LanDistributionServer::new(19282);
        
        // Start the server
        let result = server.start(file_path.clone(), "hash1".to_string()).await;
        assert!(result.is_ok());
        
        // Try to start again - should fail
        let result = server.start(file_path, "hash2".to_string()).await;
        assert!(matches!(result, Err(ServerError::AlreadyRunning)));
        
        // Clean up
        let _ = server.stop().await;
    }
}
