//! Document Distribution Module
//! 
//! Provides HTTP file server for teachers to distribute documents to students.
//! Teachers can upload files, students can browse and download via HTTP.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;

/// Default port for document server
pub const DOCUMENT_SERVER_PORT: u16 = 8765;

/// Document metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub mime_type: String,
    pub uploaded_at: u64,
    pub description: Option<String>,
    pub category: Option<String>,
}

/// Document server state
pub struct DocumentServerState {
    pub documents: Mutex<HashMap<String, Document>>,
    pub storage_path: Mutex<Option<PathBuf>>,
    pub is_running: Mutex<bool>,
    pub server_port: Mutex<u16>,
}

impl Default for DocumentServerState {
    fn default() -> Self {
        Self {
            documents: Mutex::new(HashMap::new()),
            storage_path: Mutex::new(None),
            is_running: Mutex::new(false),
            server_port: Mutex::new(DOCUMENT_SERVER_PORT),
        }
    }
}

impl DocumentServerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_document(&self, doc: Document) {
        if let Ok(mut docs) = self.documents.lock() {
            docs.insert(doc.id.clone(), doc);
        }
    }

    pub fn remove_document(&self, id: &str) -> Option<Document> {
        if let Ok(mut docs) = self.documents.lock() {
            return docs.remove(id);
        }
        None
    }

    pub fn get_document(&self, id: &str) -> Option<Document> {
        if let Ok(docs) = self.documents.lock() {
            return docs.get(id).cloned();
        }
        None
    }

    pub fn list_documents(&self) -> Vec<Document> {
        if let Ok(docs) = self.documents.lock() {
            return docs.values().cloned().collect();
        }
        Vec::new()
    }
}

/// Get storage directory for documents
fn get_storage_dir() -> Result<PathBuf, String> {
    let data_dir = dirs::data_local_dir()
        .ok_or_else(|| "Failed to get data directory".to_string())?;
    let storage_dir = data_dir.join("smartlab").join("documents");
    
    if !storage_dir.exists() {
        std::fs::create_dir_all(&storage_dir)
            .map_err(|e| format!("Failed to create storage directory: {}", e))?;
    }
    
    Ok(storage_dir)
}

/// Get MIME type from file extension
fn get_mime_type(filename: &str) -> String {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "pdf" => "application/pdf",
        "doc" | "docx" => "application/msword",
        "xls" | "xlsx" => "application/vnd.ms-excel",
        "ppt" | "pptx" => "application/vnd.ms-powerpoint",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "zip" => "application/zip",
        "rar" => "application/x-rar-compressed",
        "7z" => "application/x-7z-compressed",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "avi" => "video/x-msvideo",
        "mkv" => "video/x-matroska",
        _ => "application/octet-stream",
    }.to_string()
}

/// Save uploaded document to storage
pub async fn save_document(
    state: Arc<DocumentServerState>,
    name: String,
    data: Vec<u8>,
    description: Option<String>,
    category: Option<String>,
) -> Result<Document, String> {
    let storage_dir = get_storage_dir()?;
    
    // Generate unique ID
    let id = format!("doc-{}", chrono::Utc::now().timestamp_millis());
    
    // Save file
    let file_path = storage_dir.join(&id);
    let mut file = tokio::fs::File::create(&file_path)
        .await
        .map_err(|e| format!("Failed to create file: {}", e))?;
    
    file.write_all(&data)
        .await
        .map_err(|e| format!("Failed to write file: {}", e))?;
    
    // Create document metadata
    let doc = Document {
        id: id.clone(),
        name: name.clone(),
        size: data.len() as u64,
        mime_type: get_mime_type(&name),
        uploaded_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        description,
        category,
    };
    
    // Store metadata
    state.add_document(doc.clone());
    
    // Save metadata to disk
    save_metadata(&state)?;
    
    Ok(doc)
}

/// Delete a document
pub async fn delete_document(
    state: Arc<DocumentServerState>,
    id: &str,
) -> Result<(), String> {
    let storage_dir = get_storage_dir()?;
    let file_path = storage_dir.join(id);
    
    // Remove file
    if file_path.exists() {
        tokio::fs::remove_file(&file_path)
            .await
            .map_err(|e| format!("Failed to delete file: {}", e))?;
    }
    
    // Remove from state
    state.remove_document(id);
    
    // Update metadata
    save_metadata(&state)?;
    
    Ok(())
}

/// Save metadata to disk
fn save_metadata(state: &DocumentServerState) -> Result<(), String> {
    let storage_dir = get_storage_dir()?;
    let metadata_path = storage_dir.join("metadata.json");
    
    let docs = state.list_documents();
    let json = serde_json::to_string_pretty(&docs)
        .map_err(|e| format!("Failed to serialize metadata: {}", e))?;
    
    std::fs::write(&metadata_path, json)
        .map_err(|e| format!("Failed to write metadata: {}", e))?;
    
    Ok(())
}

/// Load metadata from disk
pub fn load_metadata(state: &DocumentServerState) -> Result<(), String> {
    let storage_dir = get_storage_dir()?;
    let metadata_path = storage_dir.join("metadata.json");
    
    if !metadata_path.exists() {
        return Ok(());
    }
    
    let json = std::fs::read_to_string(&metadata_path)
        .map_err(|e| format!("Failed to read metadata: {}", e))?;
    
    let docs: Vec<Document> = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse metadata: {}", e))?;
    
    if let Ok(mut doc_map) = state.documents.lock() {
        for doc in docs {
            doc_map.insert(doc.id.clone(), doc);
        }
    }
    
    Ok(())
}

/// Start HTTP server for document distribution
pub async fn start_document_server(
    state: Arc<DocumentServerState>,
    port: u16,
) -> Result<(), String> {
    use tokio::net::TcpListener;

    // Check if already running
    {
        let is_running = state.is_running.lock().map_err(|e| e.to_string())?;
        if *is_running {
            return Err("Document server already running".to_string());
        }
    }
    
    // Load existing metadata
    let _ = load_metadata(&state);
    
    // Store storage path
    {
        let storage_dir = get_storage_dir()?;
        let mut storage_path = state.storage_path.lock().map_err(|e| e.to_string())?;
        *storage_path = Some(storage_dir);
    }
    
    // Bind to port
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;
    
    log::info!("[DocumentServer] HTTP server listening on port {}", port);
    
    // Mark as running
    {
        let mut is_running = state.is_running.lock().map_err(|e| e.to_string())?;
        *is_running = true;
        let mut server_port = state.server_port.lock().map_err(|e| e.to_string())?;
        *server_port = port;
    }
    
    // Accept connections
    loop {
        match listener.accept().await {
            Ok((mut stream, addr)) => {
                log::debug!("[DocumentServer] Connection from {}", addr);
                let state_clone = Arc::clone(&state);
                
                tokio::spawn(async move {
                    if let Err(e) = handle_http_request(state_clone, &mut stream).await {
                        log::error!("[DocumentServer] Request error: {}", e);
                    }
                });
            }
            Err(e) => {
                log::error!("[DocumentServer] Accept error: {}", e);
            }
        }
    }
}

/// Handle HTTP request
async fn handle_http_request(
    state: Arc<DocumentServerState>,
    stream: &mut tokio::net::TcpStream,
) -> Result<(), String> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);
    
    // Read request line
    let mut request_line = String::new();
    buf_reader.read_line(&mut request_line)
        .await
        .map_err(|e| format!("Failed to read request: {}", e))?;
    
    let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
    if parts.len() < 2 {
        let response = build_error_response(400, "Bad Request");
        writer.write_all(response.as_bytes()).await.map_err(|e| e.to_string())?;
        return Ok(());
    }
    
    let method = parts[0];
    let path = parts[1];
    
    // Read headers (skip them for now)
    loop {
        let mut line = String::new();
        buf_reader.read_line(&mut line)
            .await
            .map_err(|e| format!("Failed to read header: {}", e))?;
        
        if line.trim().is_empty() {
            break;
        }
    }
    
    log::info!("[DocumentServer] {} {}", method, path);
    
    // Route request
    let response = match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => {
            build_document_list_html(&state)
        }
        ("GET", "/api/documents") => {
            build_document_list_json(&state)
        }
        ("GET", p) if p.starts_with("/download/") => {
            let doc_id = &p[10..]; // Remove "/download/"
            build_document_download(&state, doc_id).await
        }
        ("OPTIONS", _) => {
            build_cors_preflight()
        }
        _ => {
            build_error_response(404, "Not Found")
        }
    };
    
    writer.write_all(response.as_bytes()).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Build CORS preflight response
fn build_cors_preflight() -> String {
    "HTTP/1.1 204 No Content\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
        Access-Control-Allow-Headers: Content-Type\r\n\
        Access-Control-Max-Age: 86400\r\n\
        \r\n".to_string()
}

/// Build error response
fn build_error_response(status: u16, message: &str) -> String {
    let body = format!(r#"<!DOCTYPE html>
<html><head><title>{} {}</title></head>
<body><h1>{} {}</h1></body></html>"#, status, message, status, message);
    
    format!(
        "HTTP/1.1 {} {}\r\n\
        Content-Type: text/html; charset=utf-8\r\n\
        Content-Length: {}\r\n\
        Access-Control-Allow-Origin: *\r\n\
        \r\n{}",
        status, message, body.len(), body
    )
}

/// Build document list as HTML page
fn build_document_list_html(state: &DocumentServerState) -> String {
    let docs = state.list_documents();
    
    let mut doc_rows = String::new();
    for doc in &docs {
        let size_str = format_file_size(doc.size);
        let date_str = format_timestamp(doc.uploaded_at);
        doc_rows.push_str(&format!(
            r#"<tr>
                <td><a href="/download/{}" class="file-link">{}</a></td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
            </tr>"#,
            doc.id, doc.name, size_str, doc.mime_type, date_str
        ));
    }
    
    if docs.is_empty() {
        doc_rows = r#"<tr><td colspan="4" style="text-align:center;color:#666;">Chưa có tài liệu nào</td></tr>"#.to_string();
    }
    
    let html = format!(r#"<!DOCTYPE html>
<html lang="vi">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>📚 Kho Tài Liệu - SmartLab</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            padding: 20px;
        }}
        .container {{
            max-width: 1000px;
            margin: 0 auto;
            background: white;
            border-radius: 16px;
            box-shadow: 0 20px 60px rgba(0,0,0,0.3);
            overflow: hidden;
        }}
        .header {{
            background: linear-gradient(135deg, #4f46e5 0%, #7c3aed 100%);
            color: white;
            padding: 30px;
            text-align: center;
        }}
        .header h1 {{ font-size: 28px; margin-bottom: 8px; }}
        .header p {{ opacity: 0.9; font-size: 14px; }}
        .content {{ padding: 30px; }}
        table {{ width: 100%; border-collapse: collapse; }}
        th, td {{ padding: 14px 16px; text-align: left; border-bottom: 1px solid #e5e7eb; }}
        th {{ background: #f9fafb; font-weight: 600; color: #374151; font-size: 13px; text-transform: uppercase; }}
        tr:hover {{ background: #f3f4f6; }}
        .file-link {{ color: #4f46e5; text-decoration: none; font-weight: 500; }}
        .file-link:hover {{ text-decoration: underline; }}
        .file-link::before {{ content: '📄 '; }}
        .stats {{ display: flex; gap: 20px; margin-bottom: 20px; }}
        .stat-card {{ background: #f3f4f6; padding: 16px 24px; border-radius: 12px; flex: 1; }}
        .stat-card h3 {{ font-size: 24px; color: #4f46e5; }}
        .stat-card p {{ font-size: 13px; color: #6b7280; }}
        .footer {{ text-align: center; padding: 20px; background: #f9fafb; color: #6b7280; font-size: 13px; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>📚 Kho Tài Liệu</h1>
            <p>Tải tài liệu học tập từ giáo viên</p>
        </div>
        <div class="content">
            <div class="stats">
                <div class="stat-card">
                    <h3>{}</h3>
                    <p>Tổng số tài liệu</p>
                </div>
                <div class="stat-card">
                    <h3>{}</h3>
                    <p>Tổng dung lượng</p>
                </div>
            </div>
            <table>
                <thead>
                    <tr>
                        <th>Tên tài liệu</th>
                        <th>Kích thước</th>
                        <th>Loại file</th>
                        <th>Ngày tải lên</th>
                    </tr>
                </thead>
                <tbody>
                    {}
                </tbody>
            </table>
        </div>
        <div class="footer">
            SmartLab - Hệ thống quản lý phòng máy thông minh
        </div>
    </div>
</body>
</html>"#,
        docs.len(),
        format_file_size(docs.iter().map(|d| d.size).sum()),
        doc_rows
    );
    
    format!(
        "HTTP/1.1 200 OK\r\n\
        Content-Type: text/html; charset=utf-8\r\n\
        Content-Length: {}\r\n\
        Access-Control-Allow-Origin: *\r\n\
        \r\n{}",
        html.len(), html
    )
}

/// Build document list as JSON
fn build_document_list_json(state: &DocumentServerState) -> String {
    let docs = state.list_documents();
    let json = serde_json::to_string(&docs).unwrap_or_else(|_| "[]".to_string());
    
    format!(
        "HTTP/1.1 200 OK\r\n\
        Content-Type: application/json\r\n\
        Content-Length: {}\r\n\
        Access-Control-Allow-Origin: *\r\n\
        \r\n{}",
        json.len(), json
    )
}

/// Build document download response
async fn build_document_download(state: &DocumentServerState, doc_id: &str) -> String {
    // Get document metadata
    let doc = match state.get_document(doc_id) {
        Some(d) => d,
        None => return build_error_response(404, "Document not found"),
    };
    
    // Get file path
    let storage_dir = match get_storage_dir() {
        Ok(d) => d,
        Err(_) => return build_error_response(500, "Storage error"),
    };
    let file_path = storage_dir.join(doc_id);
    
    if !file_path.exists() {
        return build_error_response(404, "File not found");
    }
    
    // Read file
    let data = match tokio::fs::read(&file_path).await {
        Ok(d) => d,
        Err(_) => return build_error_response(500, "Failed to read file"),
    };
    
    // URL encode filename for Content-Disposition
    let encoded_name = urlencoding_encode(&doc.name);
    
    // Build response with file
    let headers = format!(
        "HTTP/1.1 200 OK\r\n\
        Content-Type: {}\r\n\
        Content-Length: {}\r\n\
        Content-Disposition: attachment; filename=\"{}\"; filename*=UTF-8''{}\r\n\
        Access-Control-Allow-Origin: *\r\n\
        \r\n",
        doc.mime_type, data.len(), doc.name, encoded_name
    );
    
    // Combine headers and data
    let mut response = headers.into_bytes();
    response.extend(data);
    
    // Convert to string (this is a hack, but works for binary data)
    unsafe { String::from_utf8_unchecked(response) }
}

/// Simple URL encoding for filenames
fn urlencoding_encode(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                result.push(c);
            }
            _ => {
                for byte in c.to_string().as_bytes() {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    result
}

/// Format file size for display
fn format_file_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    
    if size >= GB {
        format!("{:.1} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.1} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.1} KB", size as f64 / KB as f64)
    } else {
        format!("{} B", size)
    }
}

/// Format timestamp for display
fn format_timestamp(ts: u64) -> String {
    use chrono::{TimeZone, Utc};
    let dt = Utc.timestamp_opt(ts as i64, 0).single();
    match dt {
        Some(d) => d.format("%d/%m/%Y %H:%M").to_string(),
        None => "N/A".to_string(),
    }
}
