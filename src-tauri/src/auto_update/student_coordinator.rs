// Student Update Coordinator Module
// Manages LAN-based update downloads for Student apps
// Requirements: 8.1, 8.3, 8.4, 8.5

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

use crate::auto_update::{Downloader, DownloadProgress, UpdateError, Verifier};

/// Maximum number of retry attempts for LAN downloads
/// Requirements: 8.4 - Retry up to 3 times on failure
const MAX_LAN_RETRY_ATTEMPTS: u32 = 3;

/// Student update states
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum StudentUpdateState {
    /// No update in progress
    Idle,
    /// Update is required but not yet started
    UpdateRequired {
        current_version: String,
        required_version: String,
        update_url: Option<String>,
        sha256: Option<String>,
    },
    /// Downloading update from Teacher's LAN server
    Downloading {
        progress: f32,
        bytes_downloaded: u64,
        total_bytes: Option<u64>,
        retry_count: u32,
    },
    /// Verifying downloaded package
    Verifying,
    /// Ready to install
    ReadyToInstall {
        download_path: String,
    },
    /// Installing update
    Installing,
    /// Update completed, restarting
    Restarting,
    /// Update completed successfully
    Done,
    /// Update failed
    Failed {
        error: String,
        retry_count: u32,
        can_retry: bool,
    },
}

impl Default for StudentUpdateState {
    fn default() -> Self {
        StudentUpdateState::Idle
    }
}

/// Student update progress event emitted to frontend
#[derive(Debug, Clone, Serialize)]
pub struct StudentUpdateProgressEvent {
    pub state: StudentUpdateState,
    pub timestamp: u64,
}

/// Student Update Coordinator
/// Manages the update download process for Student apps from Teacher's LAN server
///
/// Requirements:
/// - 8.1: Request update package from Teacher's LAN_Distribution_Server
/// - 8.3: Verify SHA256 hash matches value from handshake
/// - 8.4: Retry up to 3 times on failure
/// - 8.5: Display retry button and error details after exhausting retries
pub struct StudentUpdateCoordinator {
    /// Current update state
    state: Mutex<StudentUpdateState>,
    /// Current app version
    current_version: String,
    /// Path to downloaded update file
    download_path: Mutex<Option<PathBuf>>,
    /// Number of retry attempts
    retry_count: Mutex<u32>,
    /// Update URL from handshake
    update_url: Mutex<Option<String>>,
    /// Expected SHA256 hash from handshake
    expected_hash: Mutex<Option<String>>,
    /// Required version from handshake
    required_version: Mutex<Option<String>>,
}

impl StudentUpdateCoordinator {
    /// Create a new StudentUpdateCoordinator
    ///
    /// # Arguments
    /// * `current_version` - Current application version
    pub fn new(current_version: String) -> Self {
        Self {
            state: Mutex::new(StudentUpdateState::Idle),
            current_version,
            download_path: Mutex::new(None),
            retry_count: Mutex::new(0),
            update_url: Mutex::new(None),
            expected_hash: Mutex::new(None),
            required_version: Mutex::new(None),
        }
    }

    /// Get the current update state
    pub fn get_state(&self) -> StudentUpdateState {
        self.state.lock().unwrap().clone()
    }

    /// Get the current app version
    pub fn get_current_version(&self) -> &str {
        &self.current_version
    }

    /// Get the current retry count
    pub fn get_retry_count(&self) -> u32 {
        *self.retry_count.lock().unwrap()
    }

    /// Get the download path if available
    pub fn get_download_path(&self) -> Option<PathBuf> {
        self.download_path.lock().unwrap().clone()
    }

    /// Get current timestamp in seconds since UNIX epoch
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Transition to a new state and emit event
    fn transition_state(&self, new_state: StudentUpdateState, app: Option<&AppHandle>) {
        {
            let mut state = self.state.lock().unwrap();
            *state = new_state.clone();
        }

        // Emit state change event to frontend
        if let Some(app_handle) = app {
            let event = StudentUpdateProgressEvent {
                state: new_state.clone(),
                timestamp: Self::current_timestamp(),
            };

            if let Err(e) = app_handle.emit("student-update-state-changed", &event) {
                log::warn!(
                    "[StudentUpdateCoordinator] Failed to emit state change event: {}",
                    e
                );
            }
        }

        log::info!(
            "[StudentUpdateCoordinator] State transition: {:?}",
            new_state
        );
    }

    /// Set update required from version handshake response
    ///
    /// Requirements: 6.1, 6.4
    /// - Display "Update Required" screen
    /// - Automatically initiate update download
    ///
    /// # Arguments
    /// * `required_version` - Version required by Teacher
    /// * `update_url` - URL to download update from Teacher's LAN server
    /// * `sha256` - Expected SHA256 hash of the update package
    /// * `app` - Optional AppHandle for emitting events
    pub fn set_update_required(
        &self,
        required_version: String,
        update_url: Option<String>,
        sha256: Option<String>,
        app: Option<&AppHandle>,
    ) {
        // Store update info
        *self.update_url.lock().unwrap() = update_url.clone();
        *self.expected_hash.lock().unwrap() = sha256.clone();
        *self.required_version.lock().unwrap() = Some(required_version.clone());

        // Reset retry count
        *self.retry_count.lock().unwrap() = 0;

        self.transition_state(
            StudentUpdateState::UpdateRequired {
                current_version: self.current_version.clone(),
                required_version,
                update_url,
                sha256,
            },
            app,
        );
    }

    /// Download update from Teacher's LAN server
    ///
    /// Requirements: 8.1, 8.3
    /// - Request update package from Teacher's LAN_Distribution_Server
    /// - Verify SHA256 hash matches value from handshake
    ///
    /// # Arguments
    /// * `app` - AppHandle for emitting progress events
    ///
    /// # Returns
    /// * `Ok(PathBuf)` - Path to downloaded and verified update file
    /// * `Err(UpdateError)` - Error if download or verification fails
    pub async fn download_from_lan(&self, app: AppHandle) -> Result<PathBuf, UpdateError> {
        // Validate state
        let current_state = self.get_state();
        match &current_state {
            StudentUpdateState::UpdateRequired { .. }
            | StudentUpdateState::Failed { can_retry: true, .. } => {}
            _ => {
                return Err(UpdateError::InvalidState {
                    current: format!("{:?}", current_state),
                    attempted: "Downloading".to_string(),
                });
            }
        }

        // Get update URL
        let update_url = self
            .update_url
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| UpdateError::ConfigError("No update URL available".to_string()))?;

        // Get expected hash
        let expected_hash = self.expected_hash.lock().unwrap().clone();

        let retry_count = *self.retry_count.lock().unwrap();

        // Transition to Downloading state
        self.transition_state(
            StudentUpdateState::Downloading {
                progress: 0.0,
                bytes_downloaded: 0,
                total_bytes: None,
                retry_count,
            },
            Some(&app),
        );

        // Create temp directory for download
        let temp_dir = std::env::temp_dir().join("smartlab_student_updates");
        tokio::fs::create_dir_all(&temp_dir)
            .await
            .map_err(|e| UpdateError::FileSystem(format!("Failed to create temp dir: {}", e)))?;

        // Extract filename from URL or use default
        let filename = update_url
            .split('/')
            .last()
            .unwrap_or("student_update_package");
        let dest_path = temp_dir.join(filename);

        // Create downloader
        let downloader = Downloader::new();
        let app_clone = app.clone();

        // Progress callback
        let progress_callback = Box::new(move |progress: DownloadProgress| {
            // Emit progress event to frontend
            let _ = app_clone.emit("student-update-download-progress", &progress);
        });

        log::info!(
            "[StudentUpdateCoordinator] Starting LAN download from: {}",
            update_url
        );

        // Download the update package
        match downloader
            .download(&update_url, &dest_path, None, Some(progress_callback))
            .await
        {
            Ok(path) => {
                log::info!(
                    "[StudentUpdateCoordinator] Download completed: {:?}",
                    path
                );

                // Store download path
                *self.download_path.lock().unwrap() = Some(path.clone());

                // Transition to Verifying state
                self.transition_state(StudentUpdateState::Verifying, Some(&app));

                // Verify hash if provided
                // Requirements: 8.3 - Verify SHA256 hash matches value from handshake
                if let Some(hash) = expected_hash {
                    log::info!(
                        "[StudentUpdateCoordinator] Verifying hash: {}",
                        hash
                    );

                    match Verifier::verify_sha256_and_cleanup(&path, &hash) {
                        Ok(()) => {
                            log::info!("[StudentUpdateCoordinator] Hash verification passed");
                        }
                        Err(e) => {
                            log::error!(
                                "[StudentUpdateCoordinator] Hash verification failed: {}",
                                e
                            );
                            *self.download_path.lock().unwrap() = None;
                            return Err(e);
                        }
                    }
                } else {
                    log::warn!(
                        "[StudentUpdateCoordinator] No hash provided, skipping verification"
                    );
                }

                // Transition to ReadyToInstall
                self.transition_state(
                    StudentUpdateState::ReadyToInstall {
                        download_path: path.to_string_lossy().to_string(),
                    },
                    Some(&app),
                );

                // Reset retry count on success
                *self.retry_count.lock().unwrap() = 0;

                Ok(path)
            }
            Err(e) => {
                log::error!("[StudentUpdateCoordinator] Download failed: {}", e);
                Err(e)
            }
        }
    }


    /// Download update with retry logic
    ///
    /// Requirements: 8.4, 8.5
    /// - Retry up to 3 times on failure
    /// - Track retry count
    /// - Report error after exhausting retries
    ///
    /// # Arguments
    /// * `app` - AppHandle for emitting progress events
    ///
    /// # Returns
    /// * `Ok(PathBuf)` - Path to downloaded and verified update file
    /// * `Err(UpdateError)` - Error if all retries exhausted
    pub async fn download_with_retry(&self, app: AppHandle) -> Result<PathBuf, UpdateError> {
        loop {
            let retry_count = *self.retry_count.lock().unwrap();

            match self.download_from_lan(app.clone()).await {
                Ok(path) => return Ok(path),
                Err(e) => {
                    let new_retry_count = retry_count + 1;
                    *self.retry_count.lock().unwrap() = new_retry_count;

                    log::warn!(
                        "[StudentUpdateCoordinator] Download attempt {} failed: {}",
                        new_retry_count,
                        e
                    );

                    // Check if we've exhausted retries
                    // Requirements: 8.4 - Retry up to 3 times on failure
                    if new_retry_count >= MAX_LAN_RETRY_ATTEMPTS {
                        // Requirements: 8.5 - Display retry button and error details
                        self.transition_state(
                            StudentUpdateState::Failed {
                                error: e.to_string(),
                                retry_count: new_retry_count,
                                can_retry: true, // Allow manual retry
                            },
                            Some(&app),
                        );

                        log::error!(
                            "[StudentUpdateCoordinator] All {} retry attempts exhausted",
                            MAX_LAN_RETRY_ATTEMPTS
                        );

                        return Err(UpdateError::DownloadFailed(format!(
                            "Download failed after {} attempts: {}",
                            MAX_LAN_RETRY_ATTEMPTS, e
                        )));
                    }

                    // Update state to show retry in progress
                    self.transition_state(
                        StudentUpdateState::Downloading {
                            progress: 0.0,
                            bytes_downloaded: 0,
                            total_bytes: None,
                            retry_count: new_retry_count,
                        },
                        Some(&app),
                    );

                    // Brief delay before retry
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }

    /// Manually retry download after failure
    ///
    /// Requirements: 8.5 - Display retry button
    ///
    /// # Arguments
    /// * `app` - AppHandle for emitting progress events
    ///
    /// # Returns
    /// * `Ok(PathBuf)` - Path to downloaded and verified update file
    /// * `Err(UpdateError)` - Error if retry fails
    pub async fn retry_download(&self, app: AppHandle) -> Result<PathBuf, UpdateError> {
        let current_state = self.get_state();

        // Can only retry from Failed state with can_retry=true
        match &current_state {
            StudentUpdateState::Failed { can_retry: true, .. } => {}
            _ => {
                return Err(UpdateError::InvalidState {
                    current: format!("{:?}", current_state),
                    attempted: "Retry".to_string(),
                });
            }
        }

        // Reset retry count for manual retry
        *self.retry_count.lock().unwrap() = 0;

        // Restore UpdateRequired state to allow download
        let required_version = self.required_version.lock().unwrap().clone();
        let update_url = self.update_url.lock().unwrap().clone();
        let sha256 = self.expected_hash.lock().unwrap().clone();

        self.transition_state(
            StudentUpdateState::UpdateRequired {
                current_version: self.current_version.clone(),
                required_version: required_version.unwrap_or_default(),
                update_url,
                sha256,
            },
            Some(&app),
        );

        // Start download with retry
        self.download_with_retry(app).await
    }

    /// Reset the coordinator to idle state
    pub fn reset(&self, app: Option<&AppHandle>) {
        *self.retry_count.lock().unwrap() = 0;
        *self.download_path.lock().unwrap() = None;
        *self.update_url.lock().unwrap() = None;
        *self.expected_hash.lock().unwrap() = None;
        *self.required_version.lock().unwrap() = None;
        self.transition_state(StudentUpdateState::Idle, app);
    }

    /// Check if update is required
    pub fn is_update_required(&self) -> bool {
        matches!(
            self.get_state(),
            StudentUpdateState::UpdateRequired { .. }
                | StudentUpdateState::Downloading { .. }
                | StudentUpdateState::Verifying
                | StudentUpdateState::ReadyToInstall { .. }
                | StudentUpdateState::Installing
        )
    }

    /// Transition to Installing state
    pub fn start_install(&self, app: Option<&AppHandle>) -> Result<PathBuf, UpdateError> {
        let current_state = self.get_state();

        match &current_state {
            StudentUpdateState::ReadyToInstall { .. } => {}
            _ => {
                return Err(UpdateError::InvalidState {
                    current: format!("{:?}", current_state),
                    attempted: "Installing".to_string(),
                });
            }
        }

        let download_path = self
            .download_path
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| UpdateError::FileSystem("No download path available".to_string()))?;

        self.transition_state(StudentUpdateState::Installing, app);

        Ok(download_path)
    }

    /// Transition to Restarting state
    pub fn start_restart(&self, app: Option<&AppHandle>) -> Result<(), UpdateError> {
        let current_state = self.get_state();

        if !matches!(current_state, StudentUpdateState::Installing) {
            return Err(UpdateError::InvalidState {
                current: format!("{:?}", current_state),
                attempted: "Restarting".to_string(),
            });
        }

        self.transition_state(StudentUpdateState::Restarting, app);
        Ok(())
    }

    /// Mark update as complete
    pub fn complete(&self, app: Option<&AppHandle>) {
        self.transition_state(StudentUpdateState::Done, app);
    }

    /// Transition to Failed state (for installation errors)
    pub fn transition_to_failed(&self, error: String, app: Option<&AppHandle>) {
        let retry_count = *self.retry_count.lock().unwrap();
        self.transition_state(
            StudentUpdateState::Failed {
                error,
                retry_count,
                can_retry: true,
            },
            app,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_coordinator() -> StudentUpdateCoordinator {
        StudentUpdateCoordinator::new("1.0.0".to_string())
    }

    #[test]
    fn test_coordinator_creation() {
        let coordinator = create_test_coordinator();
        assert_eq!(coordinator.get_state(), StudentUpdateState::Idle);
        assert_eq!(coordinator.get_current_version(), "1.0.0");
        assert_eq!(coordinator.get_retry_count(), 0);
    }

    #[test]
    fn test_set_update_required() {
        let coordinator = create_test_coordinator();

        coordinator.set_update_required(
            "1.1.0".to_string(),
            Some("http://192.168.1.1:9280/update/package".to_string()),
            Some("abc123".to_string()),
            None,
        );

        match coordinator.get_state() {
            StudentUpdateState::UpdateRequired {
                current_version,
                required_version,
                update_url,
                sha256,
            } => {
                assert_eq!(current_version, "1.0.0");
                assert_eq!(required_version, "1.1.0");
                assert_eq!(
                    update_url,
                    Some("http://192.168.1.1:9280/update/package".to_string())
                );
                assert_eq!(sha256, Some("abc123".to_string()));
            }
            _ => panic!("Expected UpdateRequired state"),
        }
    }

    #[test]
    fn test_is_update_required() {
        let coordinator = create_test_coordinator();

        // Initially not required
        assert!(!coordinator.is_update_required());

        // Set update required
        coordinator.set_update_required(
            "1.1.0".to_string(),
            Some("http://localhost:9280/update".to_string()),
            None,
            None,
        );

        assert!(coordinator.is_update_required());
    }

    #[test]
    fn test_reset() {
        let coordinator = create_test_coordinator();

        // Set some state
        coordinator.set_update_required(
            "1.1.0".to_string(),
            Some("http://localhost:9280/update".to_string()),
            Some("hash123".to_string()),
            None,
        );
        *coordinator.retry_count.lock().unwrap() = 2;

        // Reset
        coordinator.reset(None);

        assert_eq!(coordinator.get_state(), StudentUpdateState::Idle);
        assert_eq!(coordinator.get_retry_count(), 0);
        assert!(coordinator.get_download_path().is_none());
    }

    #[test]
    fn test_state_transitions() {
        let coordinator = create_test_coordinator();

        // Idle -> UpdateRequired
        coordinator.set_update_required("1.1.0".to_string(), None, None, None);
        assert!(matches!(
            coordinator.get_state(),
            StudentUpdateState::UpdateRequired { .. }
        ));

        // Manual transition to Downloading
        coordinator.transition_state(
            StudentUpdateState::Downloading {
                progress: 50.0,
                bytes_downloaded: 500,
                total_bytes: Some(1000),
                retry_count: 0,
            },
            None,
        );
        match coordinator.get_state() {
            StudentUpdateState::Downloading {
                progress,
                bytes_downloaded,
                total_bytes,
                retry_count,
            } => {
                assert!((progress - 50.0).abs() < f32::EPSILON);
                assert_eq!(bytes_downloaded, 500);
                assert_eq!(total_bytes, Some(1000));
                assert_eq!(retry_count, 0);
            }
            _ => panic!("Expected Downloading state"),
        }

        // Downloading -> Verifying
        coordinator.transition_state(StudentUpdateState::Verifying, None);
        assert_eq!(coordinator.get_state(), StudentUpdateState::Verifying);

        // Verifying -> ReadyToInstall
        coordinator.transition_state(
            StudentUpdateState::ReadyToInstall {
                download_path: "/tmp/update.exe".to_string(),
            },
            None,
        );
        match coordinator.get_state() {
            StudentUpdateState::ReadyToInstall { download_path } => {
                assert_eq!(download_path, "/tmp/update.exe");
            }
            _ => panic!("Expected ReadyToInstall state"),
        }
    }

    #[test]
    fn test_failed_state_with_retry() {
        let coordinator = create_test_coordinator();

        coordinator.transition_state(
            StudentUpdateState::Failed {
                error: "Network error".to_string(),
                retry_count: 2,
                can_retry: true,
            },
            None,
        );

        match coordinator.get_state() {
            StudentUpdateState::Failed {
                error,
                retry_count,
                can_retry,
            } => {
                assert_eq!(error, "Network error");
                assert_eq!(retry_count, 2);
                assert!(can_retry);
            }
            _ => panic!("Expected Failed state"),
        }
    }

    #[test]
    fn test_start_install_invalid_state() {
        let coordinator = create_test_coordinator();

        // Try to install from Idle state
        let result = coordinator.start_install(None);
        assert!(result.is_err());

        match result {
            Err(UpdateError::InvalidState { current, attempted }) => {
                assert!(current.contains("Idle"));
                assert_eq!(attempted, "Installing");
            }
            _ => panic!("Expected InvalidState error"),
        }
    }

    #[test]
    fn test_start_install_valid_state() {
        let coordinator = create_test_coordinator();

        // Set up ReadyToInstall state with download path
        *coordinator.download_path.lock().unwrap() = Some(PathBuf::from("/tmp/update.exe"));
        coordinator.transition_state(
            StudentUpdateState::ReadyToInstall {
                download_path: "/tmp/update.exe".to_string(),
            },
            None,
        );

        let result = coordinator.start_install(None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("/tmp/update.exe"));
        assert_eq!(coordinator.get_state(), StudentUpdateState::Installing);
    }

    #[test]
    fn test_start_restart_invalid_state() {
        let coordinator = create_test_coordinator();

        // Try to restart from Idle state
        let result = coordinator.start_restart(None);
        assert!(result.is_err());
    }

    #[test]
    fn test_start_restart_valid_state() {
        let coordinator = create_test_coordinator();

        // Set up Installing state
        coordinator.transition_state(StudentUpdateState::Installing, None);

        let result = coordinator.start_restart(None);
        assert!(result.is_ok());
        assert_eq!(coordinator.get_state(), StudentUpdateState::Restarting);
    }

    #[test]
    fn test_complete() {
        let coordinator = create_test_coordinator();

        coordinator.complete(None);
        assert_eq!(coordinator.get_state(), StudentUpdateState::Done);
    }

    #[test]
    fn test_state_serialization() {
        let state = StudentUpdateState::Downloading {
            progress: 75.5,
            bytes_downloaded: 7550,
            total_bytes: Some(10000),
            retry_count: 1,
        };

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: StudentUpdateState = serde_json::from_str(&json).unwrap();

        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_progress_event_serialization() {
        let event = StudentUpdateProgressEvent {
            state: StudentUpdateState::Verifying,
            timestamp: 1234567890,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Verifying"));
        assert!(json.contains("1234567890"));
    }

    #[test]
    fn test_max_retry_constant() {
        assert_eq!(MAX_LAN_RETRY_ATTEMPTS, 3);
    }

    #[test]
    fn test_retry_count_tracking() {
        let coordinator = create_test_coordinator();

        // Initial retry count is 0
        assert_eq!(coordinator.get_retry_count(), 0);

        // Manually increment retry count
        *coordinator.retry_count.lock().unwrap() = 1;
        assert_eq!(coordinator.get_retry_count(), 1);

        *coordinator.retry_count.lock().unwrap() = 2;
        assert_eq!(coordinator.get_retry_count(), 2);

        *coordinator.retry_count.lock().unwrap() = 3;
        assert_eq!(coordinator.get_retry_count(), 3);
    }

    #[test]
    fn test_failed_state_after_max_retries() {
        let coordinator = create_test_coordinator();

        // Simulate exhausted retries
        coordinator.transition_state(
            StudentUpdateState::Failed {
                error: "Download failed after 3 attempts".to_string(),
                retry_count: 3,
                can_retry: true,
            },
            None,
        );

        match coordinator.get_state() {
            StudentUpdateState::Failed {
                error,
                retry_count,
                can_retry,
            } => {
                assert!(error.contains("3 attempts"));
                assert_eq!(retry_count, 3);
                // can_retry should be true to allow manual retry
                assert!(can_retry);
            }
            _ => panic!("Expected Failed state"),
        }
    }

    #[test]
    fn test_downloading_state_includes_retry_count() {
        let coordinator = create_test_coordinator();

        // First attempt (retry_count = 0)
        coordinator.transition_state(
            StudentUpdateState::Downloading {
                progress: 0.0,
                bytes_downloaded: 0,
                total_bytes: Some(1000),
                retry_count: 0,
            },
            None,
        );

        match coordinator.get_state() {
            StudentUpdateState::Downloading { retry_count, .. } => {
                assert_eq!(retry_count, 0);
            }
            _ => panic!("Expected Downloading state"),
        }

        // Second attempt (retry_count = 1)
        coordinator.transition_state(
            StudentUpdateState::Downloading {
                progress: 0.0,
                bytes_downloaded: 0,
                total_bytes: Some(1000),
                retry_count: 1,
            },
            None,
        );

        match coordinator.get_state() {
            StudentUpdateState::Downloading { retry_count, .. } => {
                assert_eq!(retry_count, 1);
            }
            _ => panic!("Expected Downloading state"),
        }

        // Third attempt (retry_count = 2)
        coordinator.transition_state(
            StudentUpdateState::Downloading {
                progress: 0.0,
                bytes_downloaded: 0,
                total_bytes: Some(1000),
                retry_count: 2,
            },
            None,
        );

        match coordinator.get_state() {
            StudentUpdateState::Downloading { retry_count, .. } => {
                assert_eq!(retry_count, 2);
            }
            _ => panic!("Expected Downloading state"),
        }
    }

    #[test]
    fn test_set_update_required_resets_retry_count() {
        let coordinator = create_test_coordinator();

        // Set retry count to non-zero
        *coordinator.retry_count.lock().unwrap() = 2;
        assert_eq!(coordinator.get_retry_count(), 2);

        // Set update required should reset retry count
        coordinator.set_update_required(
            "1.1.0".to_string(),
            Some("http://localhost:9280/update".to_string()),
            None,
            None,
        );

        assert_eq!(coordinator.get_retry_count(), 0);
    }

    #[test]
    fn test_failed_state_serialization_with_retry_info() {
        let state = StudentUpdateState::Failed {
            error: "Connection timeout".to_string(),
            retry_count: 2,
            can_retry: true,
        };

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: StudentUpdateState = serde_json::from_str(&json).unwrap();

        match deserialized {
            StudentUpdateState::Failed {
                error,
                retry_count,
                can_retry,
            } => {
                assert_eq!(error, "Connection timeout");
                assert_eq!(retry_count, 2);
                assert!(can_retry);
            }
            _ => panic!("Expected Failed state"),
        }
    }

    #[test]
    fn test_update_required_state_stores_url_and_hash() {
        let coordinator = create_test_coordinator();

        coordinator.set_update_required(
            "1.2.0".to_string(),
            Some("http://192.168.1.100:9280/update/package".to_string()),
            Some("sha256hash123".to_string()),
            None,
        );

        // Verify internal state stores the URL and hash
        assert_eq!(
            *coordinator.update_url.lock().unwrap(),
            Some("http://192.168.1.100:9280/update/package".to_string())
        );
        assert_eq!(
            *coordinator.expected_hash.lock().unwrap(),
            Some("sha256hash123".to_string())
        );
        assert_eq!(
            *coordinator.required_version.lock().unwrap(),
            Some("1.2.0".to_string())
        );
    }
}
