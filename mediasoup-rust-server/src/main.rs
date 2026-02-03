mod config;
mod manager;
mod messages;
mod room;
mod signaling;

use config::Config;
use manager::MediasoupManager;
use signaling::SignalingServer;
use std::sync::Arc;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    println!("{}", "=".repeat(50));
    println!("Screen Sharing SFU Server (Mediasoup Rust)");
    println!("{}", "=".repeat(50));

    let config = Config::default();
    let local_ip = config::get_local_ip();
    let listen_port = config.listen_port;

    println!("Local IP: {}", local_ip);
    println!("WebSocket Port: {}", listen_port);
    println!("Max Clients: {}", config.max_clients_per_room);
    println!("{}", "=".repeat(50));

    // Initialize Mediasoup
    let manager = Arc::new(MediasoupManager::new(config).await?);

    // Start signaling server
    let signaling = SignalingServer::new(manager.clone());

    println!("\nServer ready!");
    println!("Students can connect to: ws://{}:{}", local_ip, listen_port);

    // Handle shutdown signals
    let shutdown = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C handler");
        println!("\nShutting down...");
    };

    tokio::select! {
        result = signaling.run(listen_port) => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = shutdown => {}
    }

    Ok(())
}
