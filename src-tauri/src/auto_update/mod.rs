// Auto-Update System Module
// Implements a two-tier update architecture:
// - Tier A: Internet-based updates from public API for Teacher app
// - Tier B: LAN-based version synchronization for Students

mod types;
mod config;
mod api_client;
mod coordinator;
mod downloader;
mod verifier;
mod installer;
mod logger;
mod handshake;
mod lan_server;
mod student_coordinator;

// Re-export core types for external use
pub use types::{
    UpdateConfig,
    UpdateError,
    UpdateInfo,
    UpdateState,
};

// Re-export config functions
pub use config::{load_config, save_config, get_config_path};

// Re-export API client
pub use api_client::UpdateApiClient;

// Re-export coordinator
pub use coordinator::UpdateCoordinator;

// Re-export downloader
pub use downloader::{Downloader, DownloadProgress};

// Re-export verifier
pub use verifier::Verifier;

// Re-export installer
pub use installer::InstallerRunner;

// Re-export LAN server
pub use lan_server::LanDistributionServer;

// Re-export student coordinator
pub use student_coordinator::{StudentUpdateCoordinator, StudentUpdateState};
