// Core types and enums for the Auto-Update System
// Requirements: 12.1, 1.1

use serde::{Deserialize, Serialize};

/// Update states for the state machine
/// Requirements: 12.1 - THE Update_Coordinator SHALL implement states
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum UpdateState {
    /// No update operation in progress
    Idle,
    
    /// Checking for updates from the API
    Checking,
    
    /// A new update is available
    UpdateAvailable {
        version: String,
        release_notes: String,
    },
    
    /// Downloading the update package
    Downloading {
        progress: f32,
        bytes_downloaded: u64,
        total_bytes: u64,
    },
    
    /// Verifying the downloaded package (hash/signature)
    Verifying,
    
    /// Update is downloaded and verified, ready to install
    ReadyToInstall,
    
    /// Installing the update
    Installing,
    
    /// Restarting the application
    Restarting,
    
    /// Update completed successfully
    Done,
    
    /// Update failed with an error
    Failed {
        error: String,
        recoverable: bool,
    },
}

impl Default for UpdateState {
    fn default() -> Self {
        UpdateState::Idle
    }
}

/// Configuration for the update system
/// Requirements: 1.1, 1.2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    /// Base URL for the Update API
    pub api_base_url: String,
    
    /// Update channel (stable, beta, dev)
    pub channel: String,
    
    /// Check interval in seconds (default: 14400 = 4 hours)
    pub check_interval_secs: u64,
    
    /// Whether to auto-download updates
    pub auto_download: bool,
    
    /// LAN distribution server port
    pub lan_server_port: u16,
    
    /// Enable signature verification
    pub verify_signature: bool,
    
    /// Public key for signature verification (base64)
    pub signature_public_key: Option<String>,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            // Default to LAN IP - can be overridden by config file
            api_base_url: "http://192.168.1.36:3030/api".to_string(),
            channel: "stable".to_string(),
            check_interval_secs: 14400, // 4 hours
            auto_download: false,
            lan_server_port: 9280,
            verify_signature: false,
            signature_public_key: None,
        }
    }
}

/// Response from Update API
/// Requirements: 1.1 - THE Update_API SHALL return JSON containing version, published_at, download_url, sha256 hash, and release_notes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    /// Semantic version string (e.g., "1.2.0")
    pub version: String,
    
    /// ISO 8601 timestamp when the update was published
    pub published_at: String,
    
    /// URL to download the update package
    pub download_url: String,
    
    /// SHA256 hash of the update package
    pub sha256: String,
    
    /// Optional signature for the update package
    pub signature: Option<String>,
    
    /// Release notes in markdown format
    pub release_notes: String,
    
    /// Optional URL to full changelog
    pub changelog_url: Option<String>,
    
    /// Minimum app version required to apply this update
    pub min_app_version: Option<String>,
}

/// Error types for update operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "message")]
pub enum UpdateError {
    /// Network-related errors (connection, timeout, DNS)
    Network(String),
    
    /// API returned an error response
    ApiError {
        status_code: u16,
        message: String,
    },
    
    /// Failed to parse API response
    ParseError(String),
    
    /// Download failed
    DownloadFailed(String),
    
    /// Hash verification failed
    HashMismatch {
        expected: String,
        actual: String,
    },
    
    /// Signature verification failed
    SignatureInvalid(String),
    
    /// Installation failed
    InstallFailed(String),
    
    /// File system error
    FileSystem(String),
    
    /// Invalid state transition
    InvalidState {
        current: String,
        attempted: String,
    },
    
    /// Configuration error
    ConfigError(String),
    
    /// Version compatibility error
    VersionIncompatible {
        current: String,
        required: String,
    },
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateError::Network(msg) => write!(f, "Network error: {}", msg),
            UpdateError::ApiError { status_code, message } => {
                write!(f, "API error ({}): {}", status_code, message)
            }
            UpdateError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            UpdateError::DownloadFailed(msg) => write!(f, "Download failed: {}", msg),
            UpdateError::HashMismatch { expected, actual } => {
                write!(f, "Hash mismatch: expected {}, got {}", expected, actual)
            }
            UpdateError::SignatureInvalid(msg) => write!(f, "Invalid signature: {}", msg),
            UpdateError::InstallFailed(msg) => write!(f, "Installation failed: {}", msg),
            UpdateError::FileSystem(msg) => write!(f, "File system error: {}", msg),
            UpdateError::InvalidState { current, attempted } => {
                write!(f, "Invalid state transition from {} to {}", current, attempted)
            }
            UpdateError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            UpdateError::VersionIncompatible { current, required } => {
                write!(f, "Version {} incompatible, requires {}", current, required)
            }
        }
    }
}

impl std::error::Error for UpdateError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_state_default() {
        let state = UpdateState::default();
        assert_eq!(state, UpdateState::Idle);
    }

    #[test]
    fn test_update_config_default() {
        let config = UpdateConfig::default();
        assert_eq!(config.channel, "stable");
        assert_eq!(config.check_interval_secs, 14400);
        assert!(!config.auto_download);
        assert_eq!(config.lan_server_port, 9280);
        assert!(!config.verify_signature);
    }

    #[test]
    fn test_update_state_serialization() {
        let state = UpdateState::Downloading {
            progress: 50.0,
            bytes_downloaded: 1024,
            total_bytes: 2048,
        };
        
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: UpdateState = serde_json::from_str(&json).unwrap();
        
        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_update_info_serialization() {
        let info = UpdateInfo {
            version: "1.2.0".to_string(),
            published_at: "2024-01-15T10:30:00Z".to_string(),
            download_url: "https://example.com/update.msi".to_string(),
            sha256: "abc123".to_string(),
            signature: None,
            release_notes: "# What's New\n- Feature A".to_string(),
            changelog_url: None,
            min_app_version: Some("1.0.0".to_string()),
        };
        
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: UpdateInfo = serde_json::from_str(&json).unwrap();
        
        assert_eq!(info.version, deserialized.version);
        assert_eq!(info.sha256, deserialized.sha256);
    }

    #[test]
    fn test_update_error_display() {
        let error = UpdateError::HashMismatch {
            expected: "abc".to_string(),
            actual: "xyz".to_string(),
        };
        
        let display = format!("{}", error);
        assert!(display.contains("abc"));
        assert!(display.contains("xyz"));
    }

    #[test]
    fn test_update_error_serialization() {
        let error = UpdateError::ApiError {
            status_code: 404,
            message: "Not found".to_string(),
        };
        
        let json = serde_json::to_string(&error).unwrap();
        let deserialized: UpdateError = serde_json::from_str(&json).unwrap();
        
        assert_eq!(error, deserialized);
    }
}
