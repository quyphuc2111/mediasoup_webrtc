use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferRequest {
    pub student_id: String,
    pub file_path: String,
    pub destination: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferProgress {
    pub student_id: String,
    pub file_name: String,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub percentage: f32,
}

/// List files and folders in a directory
pub fn list_directory(path: &str) -> Result<Vec<FileInfo>, String> {
    let path_buf = PathBuf::from(path);
    
    if !path_buf.exists() {
        return Err(format!("Path does not exist: {}", path));
    }
    
    if !path_buf.is_dir() {
        return Err(format!("Path is not a directory: {}", path));
    }
    
    let mut files = Vec::new();
    
    let entries = fs::read_dir(&path_buf)
        .map_err(|e| format!("Failed to read directory: {}", e))?;
    
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let metadata = entry.metadata()
            .map_err(|e| format!("Failed to read metadata: {}", e))?;
        
        let name = entry.file_name()
            .to_string_lossy()
            .to_string();
        
        let path = entry.path()
            .to_string_lossy()
            .to_string();
        
        let modified = metadata.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        
        files.push(FileInfo {
            name,
            path,
            is_dir: metadata.is_dir(),
            size: metadata.len(),
            modified,
        });
    }
    
    // Sort: directories first, then by name
    files.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
    
    Ok(files)
}

/// Get home directory path
pub fn get_home_directory() -> Result<String, String> {
    dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())
        .map(|p| p.to_string_lossy().to_string())
}

/// Get desktop directory path
pub fn get_desktop_directory() -> Result<String, String> {
    dirs::desktop_dir()
        .ok_or_else(|| "Failed to get desktop directory".to_string())
        .map(|p| p.to_string_lossy().to_string())
}

/// Get documents directory path
pub fn get_documents_directory() -> Result<String, String> {
    dirs::document_dir()
        .ok_or_else(|| "Failed to get documents directory".to_string())
        .map(|p| p.to_string_lossy().to_string())
}

/// Read file as base64 (for transfer)
pub fn read_file_as_base64(path: &str) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|e| format!("Failed to open file: {}", e))?;
    
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    
    Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &buffer))
}

/// Write file from base64 (for transfer)
pub fn write_file_from_base64(path: &str, data: &str) -> Result<(), String> {
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;
    
    // Create parent directories if they don't exist
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories: {}", e))?;
    }
    
    let mut file = fs::File::create(path)
        .map_err(|e| format!("Failed to create file: {}", e))?;
    
    file.write_all(&bytes)
        .map_err(|e| format!("Failed to write file: {}", e))?;
    
    Ok(())
}

/// Get file info
pub fn get_file_info(path: &str) -> Result<FileInfo, String> {
    let path_buf = PathBuf::from(path);
    
    if !path_buf.exists() {
        return Err(format!("Path does not exist: {}", path));
    }
    
    let metadata = fs::metadata(&path_buf)
        .map_err(|e| format!("Failed to read metadata: {}", e))?;
    
    let name = path_buf.file_name()
        .ok_or_else(|| "Failed to get file name".to_string())?
        .to_string_lossy()
        .to_string();
    
    let modified = metadata.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    
    Ok(FileInfo {
        name,
        path: path.to_string(),
        is_dir: metadata.is_dir(),
        size: metadata.len(),
        modified,
    })
}
