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
mod vnc;

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    #[cfg(windows)]
    {
        // When run with --install or --uninstall, manage the service
        let args: Vec<String> = std::env::args().collect();
        if args.len() > 1 {
            match args[1].as_str() {
                "--install" => {
                    if let Err(e) = service::install_service() {
                        eprintln!("Failed to install service: {}", e);
                        std::process::exit(1);
                    }
                    println!("Service installed successfully");
                    return;
                }
                "--uninstall" => {
                    if let Err(e) = service::uninstall_service() {
                        eprintln!("Failed to uninstall service: {}", e);
                        std::process::exit(1);
                    }
                    println!("Service uninstalled successfully");
                    return;
                }
                "--console" => {
                    // Run in console mode for debugging
                    log::info!("Running in console mode (not as service)");
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        commands::run_command_server(3019).await;
                    });
                    return;
                }
                _ => {}
            }
        }

        // Normal: run as Windows Service
        if let Err(e) = service::run_service() {
            log::error!("Service failed: {}", e);
        }
    }

    #[cfg(not(windows))]
    {
        eprintln!("This service only runs on Windows");
        std::process::exit(1);
    }
}
