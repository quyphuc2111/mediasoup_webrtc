//! SmartlabStudent Windows Service
//!
//! Runs at boot level (before user login), listens on TCP port 3019 for
//! teacher commands. Supports:
//! - Remote login (LogonUser + create user session)
//! - Status check (is user logged in?)
//! - Heartbeat (service alive?)
//!
//! This is a pure Rust Windows Service — no GUI, no Tauri.

#[cfg(windows)]
mod service;
#[cfg(windows)]
mod commands;
#[cfg(windows)]
mod logon;
#[cfg(windows)]
mod discovery;

use log::LevelFilter;
use std::io::Write;
use std::sync::Mutex;

/// Simple file logger that writes to C:\ProgramData\Smartlab\logs\service.log
/// Falls back to stderr if file cannot be opened.
struct FileLogger {
    file: Mutex<Option<std::fs::File>>,
}

impl FileLogger {
    fn new() -> Self {
        let file = Self::open_log_file();
        FileLogger {
            file: Mutex::new(file),
        }
    }

    fn open_log_file() -> Option<std::fs::File> {
        #[cfg(windows)]
        {
            let log_dir = r"C:\ProgramData\Smartlab\logs";
            if std::fs::create_dir_all(log_dir).is_err() {
                eprintln!("[SmartlabService] Cannot create log dir: {}", log_dir);
                return None;
            }
            let log_path = format!(r"{}\service.log", log_dir);
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .ok()
        }
        #[cfg(not(windows))]
        {
            None
        }
    }
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= LevelFilter::Info
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let msg = format!("{} [{}] {}\n", now, record.level(), record.args());

        // Write to file
        if let Ok(mut guard) = self.file.lock() {
            if let Some(ref mut f) = *guard {
                let _ = f.write_all(msg.as_bytes());
                let _ = f.flush();
            }
        }
        // Also write to stderr (useful for --console mode)
        eprint!("{}", msg);
    }

    fn flush(&self) {
        if let Ok(mut guard) = self.file.lock() {
            if let Some(ref mut f) = *guard {
                let _ = f.flush();
            }
        }
    }
}

fn init_logging() {
    let logger = Box::new(FileLogger::new());
    if log::set_boxed_logger(logger).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}

fn main() {
    init_logging();
    log::info!("========================================");
    log::info!("[Main] Process started, PID={}", std::process::id());
    log::info!("[Main] Exe: {:?}", std::env::current_exe().unwrap_or_default());

    #[cfg(windows)]
    {
        let args: Vec<String> = std::env::args().collect();
        log::info!("[Main] Args: {:?}", args);

        if args.len() > 1 {
            match args[1].as_str() {
                "--install" => {
                    log::info!("[Main] Mode: install");
                    if let Err(e) = service::install_service() {
                        log::error!("[Main] Install failed: {}", e);
                        eprintln!("Failed to install service: {}", e);
                        std::process::exit(1);
                    }
                    println!("Service installed successfully");
                    return;
                }
                "--uninstall" => {
                    log::info!("[Main] Mode: uninstall");
                    if let Err(e) = service::uninstall_service() {
                        log::error!("[Main] Uninstall failed: {}", e);
                        eprintln!("Failed to uninstall service: {}", e);
                        std::process::exit(1);
                    }
                    println!("Service uninstalled successfully");
                    return;
                }
                "--console" => {
                    log::info!("[Main] Mode: console (debug)");
                    // Start discovery responder in background
                    std::thread::spawn(|| {
                        discovery::run_discovery_responder();
                    });
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        commands::run_command_server(3019).await;
                    });
                    return;
                }
                _ => {
                    log::info!("[Main] Unknown arg: {}, running as service", args[1]);
                }
            }
        }

        // Normal: run as Windows Service
        log::info!("[Main] Mode: Windows Service (SCM dispatch)");
        if let Err(e) = service::run_service() {
            log::error!("[Main] Service dispatch failed: {}", e);
        }
    }

    #[cfg(not(windows))]
    {
        eprintln!("This service only runs on Windows");
        std::process::exit(1);
    }
}
