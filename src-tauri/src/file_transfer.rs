//! File Transfer Module - Chunked TCP transfer like RustDesk/Veyon
//!
//! This module implements a dedicated TCP channel for file transfer:
//! 1. Teacher initiates transfer via WebSocket signaling
//! 2. Student opens a TCP listener for file reception
//! 3. Teacher connects and sends file in chunks with progress
//! 4. Both sides emit progress events to frontend

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Chunk size for file transfer (64KB - optimal for most networks)
pub const CHUNK_SIZE: usize = 64 * 1024;

/// File transfer port offset from main WebSocket port
pub const FILE_TRANSFER_PORT_OFFSET: u16 = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
}

/// File transfer job status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransferStatus {
    Pending,
    Connecting,
    Transferring,
    Completed,
    Failed { error: String },
    Cancelled,
}

/// File transfer job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferJob {
    pub id: String,
    pub file_name: String,
    pub file_size: u64,
    pub transferred: u64,
    pub status: TransferStatus,
    pub direction: TransferDirection,
    pub student_id: String,
    pub progress: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransferDirection {
    Send,    // Teacher -> Student
    Receive, // Student -> Teacher
}

/// Progress event emitted to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferProgress {
    pub job_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub transferred: u64,
    pub progress: f32,
    pub status: TransferStatus,
    pub student_id: String,
}

/// File transfer protocol messages (sent over TCP)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FileTransferMessage {
    /// Initial handshake from sender
    #[serde(rename = "init")]
    Init {
        job_id: String,
        file_name: String,
        file_size: u64,
    },
    /// Acknowledgment from receiver
    #[serde(rename = "ack")]
    Ack { job_id: String, ready: bool },
    /// File chunk
    #[serde(rename = "chunk")]
    Chunk {
        job_id: String,
        offset: u64,
        data: Vec<u8>,
    },
    /// Transfer complete
    #[serde(rename = "complete")]
    Complete { job_id: String },
    /// Error
    #[serde(rename = "error")]
    Error { job_id: String, message: String },
    /// Cancel transfer
    #[serde(rename = "cancel")]
    Cancel { job_id: String },
}

/// State for managing file transfers
pub struct FileTransferState {
    pub jobs: Mutex<HashMap<String, FileTransferJob>>,
    pub cancel_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub listener_port: Mutex<Option<u16>>,
}

impl Default for FileTransferState {
    fn default() -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
            cancel_flags: Mutex::new(HashMap::new()),
            listener_port: Mutex::new(None),
        }
    }
}

impl FileTransferState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_job(&self, job: FileTransferJob) {
        if let Ok(mut jobs) = self.jobs.lock() {
            let cancel_flag = Arc::new(AtomicBool::new(false));
            if let Ok(mut flags) = self.cancel_flags.lock() {
                flags.insert(job.id.clone(), cancel_flag);
            }
            jobs.insert(job.id.clone(), job);
        }
    }

    pub fn update_job(&self, job_id: &str, transferred: u64, status: TransferStatus) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(job) = jobs.get_mut(job_id) {
                job.transferred = transferred;
                job.status = status;
                job.progress = if job.file_size > 0 {
                    (transferred as f32 / job.file_size as f32) * 100.0
                } else {
                    0.0
                };
            }
        }
    }

    pub fn get_job(&self, job_id: &str) -> Option<FileTransferJob> {
        self.jobs.lock().ok()?.get(job_id).cloned()
    }

    pub fn remove_job(&self, job_id: &str) {
        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.remove(job_id);
        }
        if let Ok(mut flags) = self.cancel_flags.lock() {
            flags.remove(job_id);
        }
    }

    pub fn cancel_job(&self, job_id: &str) -> bool {
        if let Ok(flags) = self.cancel_flags.lock() {
            if let Some(flag) = flags.get(job_id) {
                flag.store(true, Ordering::Relaxed);
                return true;
            }
        }
        false
    }

    pub fn is_cancelled(&self, job_id: &str) -> bool {
        if let Ok(flags) = self.cancel_flags.lock() {
            if let Some(flag) = flags.get(job_id) {
                return flag.load(Ordering::Relaxed);
            }
        }
        false
    }
}

/// Collect all files in a directory recursively
fn collect_files_in_directory(dir_path: &PathBuf, base_path: &PathBuf) -> Result<Vec<(PathBuf, String)>, String> {
    let mut files = Vec::new();
    
    let entries = std::fs::read_dir(dir_path)
        .map_err(|e| format!("Failed to read directory: {}", e))?;
    
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();
        
        if path.is_dir() {
            // Recursively collect files from subdirectory
            let sub_files = collect_files_in_directory(&path, base_path)?;
            files.extend(sub_files);
        } else {
            // Get relative path from base directory
            let relative_path = path.strip_prefix(base_path)
                .map_err(|_| "Failed to get relative path")?
                .to_string_lossy()
                .to_string();
            files.push((path, relative_path));
        }
    }
    
    Ok(files)
}

/// Send a file or folder to student via dedicated TCP connection
pub async fn send_file_chunked(
    state: Arc<FileTransferState>,
    app_handle: AppHandle,
    student_ip: String,
    student_port: u16,
    file_path: String,
    student_id: String,
) -> Result<String, String> {
    let path = PathBuf::from(&file_path);
    let metadata = std::fs::metadata(&path)
        .map_err(|e| format!("Failed to read file metadata: {}", e))?;
    
    // Check if it's a directory
    if metadata.is_dir() {
        return send_folder_chunked(state, app_handle, student_ip, student_port, file_path, student_id).await;
    }
    
    // Generate job ID
    let job_id = format!("send-{}-{}", student_id, chrono::Utc::now().timestamp_millis());
    
    let file_name = path
        .file_name()
        .ok_or("Invalid file path")?
        .to_string_lossy()
        .to_string();
    
    let file_size = metadata.len();

    // Create job
    let job = FileTransferJob {
        id: job_id.clone(),
        file_name: file_name.clone(),
        file_size,
        transferred: 0,
        status: TransferStatus::Pending,
        direction: TransferDirection::Send,
        student_id: student_id.clone(),
        progress: 0.0,
    };
    state.add_job(job);

    // Emit initial progress
    emit_progress(&app_handle, &state, &job_id);

    // Calculate file transfer port
    let transfer_port = student_port + FILE_TRANSFER_PORT_OFFSET;

    // Clone for async task
    let state_clone = Arc::clone(&state);
    let app_clone = app_handle.clone();
    let job_id_clone = job_id.clone();
    let file_path_clone = file_path.clone();

    // Spawn async task for non-blocking transfer
    tokio::spawn(async move {
        let result = send_file_task(
            state_clone.clone(),
            app_clone.clone(),
            student_ip,
            transfer_port,
            file_path_clone,
            job_id_clone.clone(),
            file_name,
            file_size,
        ).await;

        if let Err(e) = result {
            log::error!("[FileTransfer] Send failed: {}", e);
            state_clone.update_job(&job_id_clone, 0, TransferStatus::Failed { error: e });
            emit_progress(&app_clone, &state_clone, &job_id_clone);
        }
    });

    Ok(job_id)
}

/// Internal task to send file
async fn send_file_task(
    state: Arc<FileTransferState>,
    app_handle: AppHandle,
    student_ip: String,
    transfer_port: u16,
    file_path: String,
    job_id: String,
    file_name: String,
    file_size: u64,
) -> Result<(), String> {
    // Update status to connecting
    state.update_job(&job_id, 0, TransferStatus::Connecting);
    emit_progress(&app_handle, &state, &job_id);

    // Connect to student's file transfer port
    let addr = format!("{}:{}", student_ip, transfer_port);
    log::info!("[FileTransfer] Connecting to {} for file transfer", addr);

    let mut stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| format!("Failed to connect to {}: {}", addr, e))?;

    log::info!("[FileTransfer] Connected to {}", addr);

    // Send init message
    let init_msg = FileTransferMessage::Init {
        job_id: job_id.clone(),
        file_name: file_name.clone(),
        file_size,
    };
    send_message(&mut stream, &init_msg).await?;

    // Wait for ack
    let ack = receive_message(&mut stream).await?;
    match ack {
        FileTransferMessage::Ack { ready: true, .. } => {
            log::info!("[FileTransfer] Receiver ready, starting transfer");
        }
        FileTransferMessage::Ack { ready: false, .. } => {
            return Err("Receiver not ready".to_string());
        }
        FileTransferMessage::Error { message, .. } => {
            return Err(format!("Receiver error: {}", message));
        }
        _ => {
            return Err("Unexpected response".to_string());
        }
    }

    // Update status to transferring
    state.update_job(&job_id, 0, TransferStatus::Transferring);
    emit_progress(&app_handle, &state, &job_id);

    // Open file and send chunks
    let mut file = tokio::fs::File::open(&file_path)
        .await
        .map_err(|e| format!("Failed to open file: {}", e))?;

    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut offset: u64 = 0;
    let mut last_progress_emit = std::time::Instant::now();

    loop {
        // Check for cancellation
        if state.is_cancelled(&job_id) {
            let cancel_msg = FileTransferMessage::Cancel { job_id: job_id.clone() };
            let _ = send_message(&mut stream, &cancel_msg).await;
            state.update_job(&job_id, offset, TransferStatus::Cancelled);
            emit_progress(&app_handle, &state, &job_id);
            return Ok(());
        }

        // Read chunk from file
        let bytes_read = file.read(&mut buffer)
            .await
            .map_err(|e| format!("Failed to read file: {}", e))?;

        if bytes_read == 0 {
            break; // EOF
        }

        // Send chunk
        let chunk_msg = FileTransferMessage::Chunk {
            job_id: job_id.clone(),
            offset,
            data: buffer[..bytes_read].to_vec(),
        };
        send_message(&mut stream, &chunk_msg).await?;

        offset += bytes_read as u64;

        // Update progress (throttle to every 100ms)
        if last_progress_emit.elapsed().as_millis() >= 100 {
            state.update_job(&job_id, offset, TransferStatus::Transferring);
            emit_progress(&app_handle, &state, &job_id);
            last_progress_emit = std::time::Instant::now();
        }
    }

    // Send complete message
    let complete_msg = FileTransferMessage::Complete { job_id: job_id.clone() };
    send_message(&mut stream, &complete_msg).await?;

    // Update final status
    state.update_job(&job_id, file_size, TransferStatus::Completed);
    emit_progress(&app_handle, &state, &job_id);

    log::info!("[FileTransfer] File transfer completed: {} ({} bytes)", file_name, file_size);

    Ok(())
}

/// Send a folder to student (sends all files with relative paths)
async fn send_folder_chunked(
    state: Arc<FileTransferState>,
    app_handle: AppHandle,
    student_ip: String,
    student_port: u16,
    folder_path: String,
    student_id: String,
) -> Result<String, String> {
    let path = PathBuf::from(&folder_path);
    let folder_name = path
        .file_name()
        .ok_or("Invalid folder path")?
        .to_string_lossy()
        .to_string();

    // Collect all files in the folder
    let files = collect_files_in_directory(&path, &path)?;
    
    if files.is_empty() {
        return Err("Folder is empty".to_string());
    }

    // Calculate total size
    let total_size: u64 = files.iter()
        .map(|(p, _)| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .sum();

    // Generate job ID for the folder transfer
    let job_id = format!("folder-{}-{}", student_id, chrono::Utc::now().timestamp_millis());

    // Create job
    let job = FileTransferJob {
        id: job_id.clone(),
        file_name: format!("📁 {} ({} files)", folder_name, files.len()),
        file_size: total_size,
        transferred: 0,
        status: TransferStatus::Pending,
        direction: TransferDirection::Send,
        student_id: student_id.clone(),
        progress: 0.0,
    };
    state.add_job(job);
    emit_progress(&app_handle, &state, &job_id);

    // Calculate file transfer port
    let transfer_port = student_port + FILE_TRANSFER_PORT_OFFSET;

    // Clone for async task
    let state_clone = Arc::clone(&state);
    let app_clone = app_handle.clone();
    let job_id_clone = job_id.clone();

    // Spawn async task for non-blocking transfer
    tokio::spawn(async move {
        let result = send_folder_task(
            state_clone.clone(),
            app_clone.clone(),
            student_ip,
            transfer_port,
            folder_name,
            files,
            total_size,
            job_id_clone.clone(),
        ).await;

        if let Err(e) = result {
            log::error!("[FileTransfer] Folder send failed: {}", e);
            state_clone.update_job(&job_id_clone, 0, TransferStatus::Failed { error: e });
            emit_progress(&app_clone, &state_clone, &job_id_clone);
        }
    });

    Ok(job_id)
}

/// Internal task to send folder
async fn send_folder_task(
    state: Arc<FileTransferState>,
    app_handle: AppHandle,
    student_ip: String,
    transfer_port: u16,
    folder_name: String,
    files: Vec<(PathBuf, String)>,
    total_size: u64,
    job_id: String,
) -> Result<(), String> {
    // Update status to connecting
    state.update_job(&job_id, 0, TransferStatus::Connecting);
    emit_progress(&app_handle, &state, &job_id);

    // Connect to student's file transfer port
    let addr = format!("{}:{}", student_ip, transfer_port);
    log::info!("[FileTransfer] Connecting to {} for folder transfer", addr);

    let mut stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| format!("Failed to connect to {}: {}", addr, e))?;

    log::info!("[FileTransfer] Connected to {}", addr);

    let mut total_transferred: u64 = 0;
    let mut last_progress_emit = std::time::Instant::now();

    // Send each file
    for (file_path, relative_path) in &files {
        // Check for cancellation
        if state.is_cancelled(&job_id) {
            let cancel_msg = FileTransferMessage::Cancel { job_id: job_id.clone() };
            let _ = send_message(&mut stream, &cancel_msg).await;
            state.update_job(&job_id, total_transferred, TransferStatus::Cancelled);
            emit_progress(&app_handle, &state, &job_id);
            return Ok(());
        }

        let file_metadata = std::fs::metadata(file_path)
            .map_err(|e| format!("Failed to read file metadata: {}", e))?;
        let file_size = file_metadata.len();

        // Construct destination path: folder_name/relative_path
        let dest_path = format!("{}/{}", folder_name, relative_path);

        // Send init message for this file
        let init_msg = FileTransferMessage::Init {
            job_id: job_id.clone(),
            file_name: dest_path.clone(),
            file_size,
        };
        send_message(&mut stream, &init_msg).await?;

        // Wait for ack
        let ack = receive_message(&mut stream).await?;
        match ack {
            FileTransferMessage::Ack { ready: true, .. } => {}
            FileTransferMessage::Ack { ready: false, .. } => {
                return Err(format!("Receiver not ready for file: {}", dest_path));
            }
            FileTransferMessage::Error { message, .. } => {
                return Err(format!("Receiver error for {}: {}", dest_path, message));
            }
            _ => {
                return Err("Unexpected response".to_string());
            }
        }

        // Update status to transferring
        state.update_job(&job_id, total_transferred, TransferStatus::Transferring);
        emit_progress(&app_handle, &state, &job_id);

        // Open file and send chunks
        let mut file = tokio::fs::File::open(file_path)
            .await
            .map_err(|e| format!("Failed to open file {}: {}", file_path.display(), e))?;

        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut file_offset: u64 = 0;

        loop {
            // Check for cancellation
            if state.is_cancelled(&job_id) {
                let cancel_msg = FileTransferMessage::Cancel { job_id: job_id.clone() };
                let _ = send_message(&mut stream, &cancel_msg).await;
                state.update_job(&job_id, total_transferred, TransferStatus::Cancelled);
                emit_progress(&app_handle, &state, &job_id);
                return Ok(());
            }

            // Read chunk from file
            let bytes_read = file.read(&mut buffer)
                .await
                .map_err(|e| format!("Failed to read file: {}", e))?;

            if bytes_read == 0 {
                break; // EOF
            }

            // Send chunk
            let chunk_msg = FileTransferMessage::Chunk {
                job_id: job_id.clone(),
                offset: file_offset,
                data: buffer[..bytes_read].to_vec(),
            };
            send_message(&mut stream, &chunk_msg).await?;

            file_offset += bytes_read as u64;
            total_transferred += bytes_read as u64;

            // Update progress (throttle to every 100ms)
            if last_progress_emit.elapsed().as_millis() >= 100 {
                state.update_job(&job_id, total_transferred, TransferStatus::Transferring);
                emit_progress(&app_handle, &state, &job_id);
                last_progress_emit = std::time::Instant::now();
            }
        }

        // Send complete message for this file
        let complete_msg = FileTransferMessage::Complete { job_id: job_id.clone() };
        send_message(&mut stream, &complete_msg).await?;

        log::info!("[FileTransfer] File in folder completed: {}", dest_path);
    }

    // Update final status
    state.update_job(&job_id, total_size, TransferStatus::Completed);
    emit_progress(&app_handle, &state, &job_id);

    log::info!("[FileTransfer] Folder transfer completed: {} ({} files, {} bytes)", 
        folder_name, files.len(), total_size);

    Ok(())
}

/// Start file transfer listener on student side
pub async fn start_file_receiver(
    state: Arc<FileTransferState>,
    app_handle: AppHandle,
    base_port: u16,
) -> Result<u16, String> {
    let transfer_port = base_port + FILE_TRANSFER_PORT_OFFSET;

    // Check if already listening
    if let Ok(port) = state.listener_port.lock() {
        if port.is_some() {
            return Ok(transfer_port);
        }
    }

    // Bind to file transfer port
    let addr = format!("0.0.0.0:{}", transfer_port);
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind file transfer port {}: {}", transfer_port, e))?;

    log::info!("[FileTransfer] File receiver listening on port {}", transfer_port);

    // Store port
    if let Ok(mut port) = state.listener_port.lock() {
        *port = Some(transfer_port);
    }

    // Spawn listener task
    let state_clone = Arc::clone(&state);
    let app_clone = app_handle.clone();

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    log::info!("[FileTransfer] Incoming file transfer from {}", addr);
                    let state_inner = Arc::clone(&state_clone);
                    let app_inner = app_clone.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = handle_incoming_transfer(state_inner, app_inner, stream).await {
                            log::error!("[FileTransfer] Transfer error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    log::error!("[FileTransfer] Accept error: {}", e);
                }
            }
        }
    });

    Ok(transfer_port)
}

/// Handle incoming file transfer on student side (supports both single file and folder)
async fn handle_incoming_transfer(
    state: Arc<FileTransferState>,
    app_handle: AppHandle,
    mut stream: TcpStream,
) -> Result<(), String> {
    // Get Downloads directory
    let downloads_dir = dirs::download_dir()
        .ok_or_else(|| "Failed to get Downloads directory".to_string())?;

    let mut total_received: u64 = 0;
    let mut total_size: u64 = 0;
    let mut file_count = 0;
    let mut job_id: Option<String> = None;
    let mut is_folder_transfer = false;
    let mut last_progress_emit = std::time::Instant::now();

    loop {
        // Receive message (Init for new file, or end of connection)
        let msg = match receive_message(&mut stream).await {
            Ok(m) => m,
            Err(e) => {
                if e.contains("Failed to read length") {
                    // Connection closed normally after folder transfer
                    break;
                }
                return Err(e);
            }
        };

        match msg {
            FileTransferMessage::Init { job_id: jid, file_name, file_size } => {
                log::info!("[FileTransfer] Receiving file: {} ({} bytes)", file_name, file_size);

                // Check if this is a folder transfer (path contains /)
                is_folder_transfer = file_name.contains('/') || file_name.contains('\\');
                
                // First file - create job
                if job_id.is_none() {
                    job_id = Some(jid.clone());
                    
                    let display_name = if is_folder_transfer {
                        // Extract folder name from path
                        let folder_name = file_name.split('/').next()
                            .or_else(|| file_name.split('\\').next())
                            .unwrap_or(&file_name);
                        format!("📁 {}", folder_name)
                    } else {
                        file_name.clone()
                    };

                    let job = FileTransferJob {
                        id: jid.clone(),
                        file_name: display_name,
                        file_size,
                        transferred: 0,
                        status: TransferStatus::Pending,
                        direction: TransferDirection::Receive,
                        student_id: "local".to_string(),
                        progress: 0.0,
                    };
                    state.add_job(job);
                }

                // For folder transfer, accumulate total size
                if is_folder_transfer {
                    total_size += file_size;
                    // Update job with new total size
                    if let Some(ref jid) = job_id {
                        if let Ok(mut jobs) = state.jobs.lock() {
                            if let Some(job) = jobs.get_mut(jid) {
                                job.file_size = total_size;
                            }
                        }
                    }
                } else {
                    total_size = file_size;
                }

                emit_progress(&app_handle, &state, job_id.as_ref().unwrap());

                // Create file path (handle subdirectories for folder transfer)
                let file_path = if is_folder_transfer {
                    let relative_path = downloads_dir.join(&file_name);
                    // Create parent directories
                    if let Some(parent) = relative_path.parent() {
                        tokio::fs::create_dir_all(parent)
                            .await
                            .map_err(|e| format!("Failed to create directories: {}", e))?;
                    }
                    relative_path
                } else {
                    // Single file - handle duplicates
                    let mut file_path = downloads_dir.join(&file_name);
                    let mut counter = 1;
                    while file_path.exists() {
                        let stem = PathBuf::from(&file_name)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("file")
                            .to_string();
                        let ext = PathBuf::from(&file_name)
                            .extension()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_string();

                        let new_name = if ext.is_empty() {
                            format!("{} ({})", stem, counter)
                        } else {
                            format!("{} ({}).{}", stem, counter, ext)
                        };
                        file_path = downloads_dir.join(new_name);
                        counter += 1;
                    }
                    file_path
                };

                // Send ack
                let ack_msg = FileTransferMessage::Ack {
                    job_id: job_id.clone().unwrap(),
                    ready: true,
                };
                send_message(&mut stream, &ack_msg).await?;

                // Update status
                if let Some(ref jid) = job_id {
                    state.update_job(jid, total_received, TransferStatus::Transferring);
                    emit_progress(&app_handle, &state, jid);
                }

                // Create file
                let mut file = tokio::fs::File::create(&file_path)
                    .await
                    .map_err(|e| format!("Failed to create file: {}", e))?;

                // Receive chunks for this file
                loop {
                    let chunk_msg = receive_message(&mut stream).await?;

                    match chunk_msg {
                        FileTransferMessage::Chunk { data, .. } => {
                            file.write_all(&data)
                                .await
                                .map_err(|e| format!("Failed to write chunk: {}", e))?;

                            total_received += data.len() as u64;

                            // Update progress (throttle to every 100ms)
                            if last_progress_emit.elapsed().as_millis() >= 100 {
                                if let Some(ref jid) = job_id {
                                    state.update_job(jid, total_received, TransferStatus::Transferring);
                                    emit_progress(&app_handle, &state, jid);
                                }
                                last_progress_emit = std::time::Instant::now();
                            }
                        }
                        FileTransferMessage::Complete { .. } => {
                            file_count += 1;
                            log::info!("[FileTransfer] File {} complete: {}", file_count, file_path.display());
                            
                            // For single file, we're done
                            if !is_folder_transfer {
                                if let Some(ref jid) = job_id {
                                    state.update_job(jid, total_size, TransferStatus::Completed);
                                    emit_progress(&app_handle, &state, jid);
                                }
                                return Ok(());
                            }
                            // For folder, break inner loop to receive next file
                            break;
                        }
                        FileTransferMessage::Cancel { .. } => {
                            log::info!("[FileTransfer] Transfer cancelled");
                            if let Some(ref jid) = job_id {
                                state.update_job(jid, total_received, TransferStatus::Cancelled);
                                emit_progress(&app_handle, &state, jid);
                            }
                            let _ = tokio::fs::remove_file(&file_path).await;
                            return Ok(());
                        }
                        FileTransferMessage::Error { message, .. } => {
                            log::error!("[FileTransfer] Transfer error: {}", message);
                            if let Some(ref jid) = job_id {
                                state.update_job(jid, total_received, TransferStatus::Failed { error: message });
                                emit_progress(&app_handle, &state, jid);
                            }
                            let _ = tokio::fs::remove_file(&file_path).await;
                            return Ok(());
                        }
                        _ => {
                            log::warn!("[FileTransfer] Unexpected message during file transfer");
                        }
                    }
                }
            }
            FileTransferMessage::Cancel { .. } => {
                log::info!("[FileTransfer] Transfer cancelled");
                if let Some(ref jid) = job_id {
                    state.update_job(jid, total_received, TransferStatus::Cancelled);
                    emit_progress(&app_handle, &state, jid);
                }
                return Ok(());
            }
            _ => {
                // End of folder transfer or unexpected message
                break;
            }
        }
    }

    // Folder transfer complete
    if is_folder_transfer && file_count > 0 {
        if let Some(ref jid) = job_id {
            // Update display name with file count
            if let Ok(mut jobs) = state.jobs.lock() {
                if let Some(job) = jobs.get_mut(jid) {
                    let folder_name = job.file_name.replace("📁 ", "");
                    job.file_name = format!("📁 {} ({} files)", folder_name, file_count);
                }
            }
            state.update_job(jid, total_size, TransferStatus::Completed);
            emit_progress(&app_handle, &state, jid);
        }
        log::info!("[FileTransfer] Folder transfer complete: {} files, {} bytes", file_count, total_size);
    }

    Ok(())
}

/// Send a message over TCP
async fn send_message(stream: &mut TcpStream, msg: &FileTransferMessage) -> Result<(), String> {
    let json = serde_json::to_vec(msg)
        .map_err(|e| format!("Failed to serialize message: {}", e))?;

    // Send length prefix (4 bytes, little endian)
    let len = json.len() as u32;
    stream.write_all(&len.to_le_bytes())
        .await
        .map_err(|e| format!("Failed to send length: {}", e))?;

    // Send message
    stream.write_all(&json)
        .await
        .map_err(|e| format!("Failed to send message: {}", e))?;

    Ok(())
}

/// Receive a message over TCP
async fn receive_message(stream: &mut TcpStream) -> Result<FileTransferMessage, String> {
    // Read length prefix
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)
        .await
        .map_err(|e| format!("Failed to read length: {}", e))?;

    let len = u32::from_le_bytes(len_buf) as usize;

    // Sanity check
    if len > 10 * 1024 * 1024 {
        return Err("Message too large".to_string());
    }

    // Read message
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)
        .await
        .map_err(|e| format!("Failed to read message: {}", e))?;

    serde_json::from_slice(&buf)
        .map_err(|e| format!("Failed to parse message: {}", e))
}

/// Emit progress event to frontend
fn emit_progress(app_handle: &AppHandle, state: &FileTransferState, job_id: &str) {
    if let Some(job) = state.get_job(job_id) {
        let progress = FileTransferProgress {
            job_id: job.id,
            file_name: job.file_name,
            file_size: job.file_size,
            transferred: job.transferred,
            progress: job.progress,
            status: job.status,
            student_id: job.student_id,
        };
        let _ = app_handle.emit("file-transfer-progress", progress);
    }
}

// ============ Legacy functions for compatibility ============

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

    let entries = std::fs::read_dir(&path_buf)
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to read metadata: {}", e))?;

        let name = entry.file_name().to_string_lossy().to_string();

        let path = entry.path().to_string_lossy().to_string();

        let modified = metadata
            .modified()
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
    files.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
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

/// Read file as base64 (for small files / legacy)
pub fn read_file_as_base64(path: &str) -> Result<String, String> {
    use std::io::Read;
    
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &buffer,
    ))
}

/// Write file from base64 (for small files / legacy)
pub fn write_file_from_base64(path: &str, data: &str) -> Result<(), String> {
    use std::io::Write;
    use std::path::Path;
    
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    // Create parent directories if they don't exist
    if let Some(parent) = Path::new(path).parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories: {}", e))?;
    }

    let mut file =
        std::fs::File::create(path).map_err(|e| format!("Failed to create file: {}", e))?;

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

    let metadata =
        std::fs::metadata(&path_buf).map_err(|e| format!("Failed to read metadata: {}", e))?;

    let name = path_buf
        .file_name()
        .ok_or_else(|| "Failed to get file name".to_string())?
        .to_string_lossy()
        .to_string();

    let modified = metadata
        .modified()
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
