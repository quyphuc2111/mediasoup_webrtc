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
pub use config::{load_config, save_config, ensure_config_exists, get_config_path};

// Re-export API client
pub use api_client::UpdateApiClient;

// Re-export coordinator
pub use coordinator::{
    UpdateCoordinator,
    PersistedUpdateState,
    PendingUpdate,
    StateChangeEvent,
    PostUpdateVerification,
};

// Re-export downloader
pub use downloader::{Downloader, DownloadProgress, ProgressCallback};

// Re-export verifier
pub use verifier::Verifier;

// Re-export installer
pub use installer::{InstallerRunner, InstallerType, InstallError};

// Re-export logger
pub use logger::{UpdateLogger, UpdateLogEntry, LogLevel};

// Re-export handshake
pub use handshake::{
    VersionHandshakeRequest,
    VersionHandshakeResponse,
    check_version_compatibility,
    check_version_compatibility_with_update,
    compare_versions,
    is_version_older,
    parse_semver,
};

// Re-export LAN server
pub use lan_server::{LanDistributionServer, ServerError};

// Re-export student coordinator
pub use student_coordinator::{
    StudentUpdateCoordinator,
    StudentUpdateState,
    StudentUpdateProgressEvent,
};
