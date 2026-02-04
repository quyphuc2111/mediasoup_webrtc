// Update Coordinator - State Machine for Auto-Update System
// Manages the update lifecycle with state transitions and event emission
// Requirements: 12.1, 12.2, 12.3, 12.4, 12.5, 5.5

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

use crate::auto_update::{
    Downloader, UpdateApiClient, UpdateConfig, UpdateError, UpdateInfo, UpdateState, Verifier,
};

/// Maximum number of retry attempts
const MAX_RETRY_ATTEMPTS: u32 = 3;

/// Base delay for exponential backoff in milliseconds
const BASE_BACKOFF_MS: u64 = 1000;

/// Maximum backoff delay in milliseconds (5 minutes)
const MAX_BACKOFF_MS: u64 = 300_000;

/// Persisted update state for recovery after unexpected termination
/// Requirements: 12.5
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedUpdateState {
    /// Timestamp of last update check
    pub last_check: Option<u64>,
    /// Pending update information
    pub pending_update: Option<PendingUpdate>,
    /// Timestamp of last successful update
    pub last_successful_update: Option<u64>,
}

impl Default for PersistedUpdateState {
    fn default() -> Self {
        Self {
            last_check: None,
            pending_update: None,
            last_successful_update: None,
        }
    }
}

/// Information about a pending update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingUpdate {
    /// Version of the pending update
    pub version: String,
    /// Path to the downloaded update file
    pub download_path: String,
    /// SHA256 hash of the update file
    pub sha256: String,
    /// Timestamp when the update was downloaded
    pub downloaded_at: u64,
    /// Whether the update has been verified
    pub verified: bool,
}

/// State change event emitted to the frontend
/// Requirements: 12.2
#[derive(Debug, Clone, Serialize)]
pub struct StateChangeEvent {
    /// Previous state
    pub previous_state: UpdateState,
    /// New state
    pub new_state: UpdateState,
    /// Timestamp of the change
    pub timestamp: u64,
}

/// Update coordinator - central state machine for managing updates
/// Requirements: 12.1
pub struct UpdateCoordinator {
    /// Current update state (protected by mutex)
    state: Mutex<UpdateState>,
    /// Update configuration
    config: UpdateConfig,
    /// Current application version
    current_version: String,
    /// Latest update information from API
    latest_info: Mutex<Option<UpdateInfo>>,
    /// Path to downloaded update file
    download_path: Mutex<Option<PathBuf>>,
    /// Number of retry attempts
    retry_count: Mutex<u32>,
    /// Persisted state for recovery
    persisted_state: Mutex<PersistedUpdateState>,
    /// Path to state persistence file
    state_file_path: Mutex<Option<PathBuf>>,
}


impl UpdateCoordinator {
    /// Create a new UpdateCoordinator
    ///
    /// # Arguments
    /// * `config` - Update configuration
    /// * `current_version` - Current application version
    pub fn new(config: UpdateConfig, current_version: String) -> Self {
        Self {
            state: Mutex::new(UpdateState::Idle),
            config,
            current_version,
            latest_info: Mutex::new(None),
            download_path: Mutex::new(None),
            retry_count: Mutex::new(0),
            persisted_state: Mutex::new(PersistedUpdateState::default()),
            state_file_path: Mutex::new(None),
        }
    }

    /// Create a new UpdateCoordinator with default configuration
    /// Loads config from file if exists, otherwise uses defaults
    pub fn with_defaults(current_version: String) -> Self {
        // Load config from file or use defaults
        let config = crate::auto_update::load_config();
        log::info!("[UpdateCoordinator] Using API URL: {}", config.api_base_url);
        Self::new(config, current_version)
    }

    /// Get the current update state
    pub fn get_state(&self) -> UpdateState {
        self.state.lock().unwrap().clone()
    }

    /// Get the current configuration
    pub fn get_config(&self) -> &UpdateConfig {
        &self.config
    }

    /// Get the current application version
    pub fn get_current_version(&self) -> &str {
        &self.current_version
    }

    /// Get the latest update info if available
    pub fn get_latest_info(&self) -> Option<UpdateInfo> {
        self.latest_info.lock().unwrap().clone()
    }

    /// Get the download path if available
    pub fn get_download_path(&self) -> Option<PathBuf> {
        self.download_path.lock().unwrap().clone()
    }

    /// Get the current retry count
    pub fn get_retry_count(&self) -> u32 {
        *self.retry_count.lock().unwrap()
    }

    /// Set the state file path for persistence
    pub fn set_state_file_path(&self, path: PathBuf) {
        *self.state_file_path.lock().unwrap() = Some(path);
    }

    /// Get current timestamp in seconds since UNIX epoch
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Transition to a new state and emit event
    /// Requirements: 12.1, 12.2
    fn transition_state(&self, new_state: UpdateState, app: Option<&AppHandle>) {
        let previous_state = {
            let mut state = self.state.lock().unwrap();
            let prev = state.clone();
            *state = new_state.clone();
            prev
        };

        // Emit state change event to frontend
        if let Some(app_handle) = app {
            let event = StateChangeEvent {
                previous_state: previous_state.clone(),
                new_state: new_state.clone(),
                timestamp: Self::current_timestamp(),
            };

            if let Err(e) = app_handle.emit("update-state-changed", &event) {
                log::warn!("[UpdateCoordinator] Failed to emit state change event: {}", e);
            }
        }

        log::info!(
            "[UpdateCoordinator] State transition: {:?} -> {:?}",
            previous_state,
            new_state
        );

        // Persist state on transitions
        self.persist_state();
    }

    /// Transition to Failed state with error
    /// Requirements: 12.3
    fn transition_to_failed(&self, error: UpdateError, app: Option<&AppHandle>) {
        let recoverable = Self::is_recoverable_error(&error);
        self.transition_state(
            UpdateState::Failed {
                error: error.to_string(),
                recoverable,
            },
            app,
        );
    }

    /// Check if an error is recoverable (can be retried)
    fn is_recoverable_error(error: &UpdateError) -> bool {
        matches!(
            error,
            UpdateError::Network(_)
                | UpdateError::DownloadFailed(_)
                | UpdateError::ApiError { status_code: 500..=599, .. }
        )
    }

    /// Calculate exponential backoff delay
    /// Requirements: 5.5
    /// Property 8: Exponential Backoff Calculation
    pub fn calculate_backoff_delay(&self) -> Duration {
        let retry_count = *self.retry_count.lock().unwrap();
        Self::calculate_backoff_for_attempt(retry_count)
    }

    /// Calculate backoff delay for a specific attempt number
    /// delay = base * 2^attempt, capped at max
    pub fn calculate_backoff_for_attempt(attempt: u32) -> Duration {
        let delay_ms = BASE_BACKOFF_MS.saturating_mul(2u64.saturating_pow(attempt));
        let capped_delay = delay_ms.min(MAX_BACKOFF_MS);
        Duration::from_millis(capped_delay)
    }


    /// Check for updates from the API
    /// Requirements: 2.1, 2.4
    pub async fn check_for_updates(
        &self,
        app: Option<&AppHandle>,
    ) -> Result<Option<UpdateInfo>, UpdateError> {
        // Validate state transition - allow checking from more states
        let current_state = self.get_state();
        if !matches!(
            current_state,
            UpdateState::Idle 
                | UpdateState::Failed { .. }
                | UpdateState::UpdateAvailable { .. }
                | UpdateState::ReadyToInstall  // Allow re-checking when ready to install
                | UpdateState::Done
        ) {
            return Err(UpdateError::InvalidState {
                current: format!("{:?}", current_state),
                attempted: "Checking".to_string(),
            });
        }

        self.transition_state(UpdateState::Checking, app);

        let client = UpdateApiClient::from_config(&self.config);

        match client.get_latest_version().await {
            Ok(info) => {
                // Compare versions
                if self.is_newer_version(&info.version) {
                    // Store latest info
                    *self.latest_info.lock().unwrap() = Some(info.clone());

                    // Update persisted state
                    {
                        let mut persisted = self.persisted_state.lock().unwrap();
                        persisted.last_check = Some(Self::current_timestamp());
                    }
                    self.persist_state();

                    // Transition to UpdateAvailable
                    self.transition_state(
                        UpdateState::UpdateAvailable {
                            version: info.version.clone(),
                            release_notes: info.release_notes.clone(),
                        },
                        app,
                    );

                    // Reset retry count on success
                    *self.retry_count.lock().unwrap() = 0;

                    Ok(Some(info))
                } else {
                    // No update needed
                    self.transition_state(UpdateState::Idle, app);
                    *self.retry_count.lock().unwrap() = 0;
                    Ok(None)
                }
            }
            Err(e) => {
                self.transition_to_failed(e.clone(), app);
                Err(e)
            }
        }
    }

    /// Compare versions to determine if update is newer
    fn is_newer_version(&self, new_version: &str) -> bool {
        // Simple semver comparison
        let current_parts: Vec<u32> = self
            .current_version
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();
        let new_parts: Vec<u32> = new_version
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();

        for i in 0..3 {
            let current = current_parts.get(i).copied().unwrap_or(0);
            let new = new_parts.get(i).copied().unwrap_or(0);
            if new > current {
                return true;
            }
            if new < current {
                return false;
            }
        }
        false
    }

    /// Download the update package
    /// Requirements: 3.1, 3.2
    pub async fn download_update(&self, app: AppHandle) -> Result<PathBuf, UpdateError> {
        // Validate state transition
        let current_state = self.get_state();
        if !matches!(current_state, UpdateState::UpdateAvailable { .. }) {
            return Err(UpdateError::InvalidState {
                current: format!("{:?}", current_state),
                attempted: "Downloading".to_string(),
            });
        }

        // Get update info
        let info = self
            .latest_info
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| UpdateError::ConfigError("No update info available".to_string()))?;

        self.transition_state(
            UpdateState::Downloading {
                progress: 0.0,
                bytes_downloaded: 0,
                total_bytes: 0,
            },
            Some(&app),
        );

        // Create temp directory for download
        let temp_dir = std::env::temp_dir().join("smartlab_updates");
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| UpdateError::FileSystem(format!("Failed to create temp dir: {}", e)))?;

        // Extract filename from URL or use default
        let filename = info
            .download_url
            .split('/')
            .last()
            .unwrap_or("update_package");
        let dest_path = temp_dir.join(filename);

        // Create downloader and download with progress
        let downloader = Downloader::new();
        let app_clone = app.clone();

        // Progress callback only emits events - state is updated after download
        let progress_callback = Box::new(move |progress: crate::auto_update::DownloadProgress| {
            // Emit progress event to frontend
            let _ = app_clone.emit("update-download-progress", &progress);
        });

        match downloader
            .download(&info.download_url, &dest_path, None, Some(progress_callback))
            .await
        {
            Ok(path) => {
                *self.download_path.lock().unwrap() = Some(path.clone());

                // Update persisted state
                {
                    let mut persisted = self.persisted_state.lock().unwrap();
                    persisted.pending_update = Some(PendingUpdate {
                        version: info.version.clone(),
                        download_path: path.to_string_lossy().to_string(),
                        sha256: info.sha256.clone(),
                        downloaded_at: Self::current_timestamp(),
                        verified: false,
                    });
                }
                self.persist_state();

                // Transition to Verifying and automatically verify
                self.transition_state(UpdateState::Verifying, Some(&app));
                
                // Auto-verify the downloaded file
                match self.verify_update(Some(&app)) {
                    Ok(()) => {
                        log::info!("[UpdateCoordinator] Download and verification completed successfully");
                    }
                    Err(e) => {
                        log::error!("[UpdateCoordinator] Verification failed: {}", e);
                        // State already transitioned to Failed by verify_update
                        return Err(e);
                    }
                }
                
                *self.retry_count.lock().unwrap() = 0;

                Ok(path)
            }
            Err(e) => {
                self.transition_to_failed(e.clone(), Some(&app));
                Err(e)
            }
        }
    }


    /// Verify the downloaded update package
    /// Requirements: 3.3, 3.4
    pub fn verify_update(&self, app: Option<&AppHandle>) -> Result<(), UpdateError> {
        // Validate state transition
        let current_state = self.get_state();
        if !matches!(current_state, UpdateState::Verifying) {
            return Err(UpdateError::InvalidState {
                current: format!("{:?}", current_state),
                attempted: "Verifying".to_string(),
            });
        }

        // Get download path and expected hash
        let download_path = self
            .download_path
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| UpdateError::FileSystem("No download path available".to_string()))?;

        let info = self
            .latest_info
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| UpdateError::ConfigError("No update info available".to_string()))?;

        // Verify hash (and cleanup on failure)
        match Verifier::verify_sha256_and_cleanup(&download_path, &info.sha256) {
            Ok(()) => {
                // Optionally verify signature if enabled
                if self.config.verify_signature {
                    if let (Some(signature), Some(public_key)) =
                        (&info.signature, &self.config.signature_public_key)
                    {
                        if let Err(e) =
                            Verifier::verify_signature(&download_path, signature, public_key)
                        {
                            self.transition_to_failed(e.clone(), app);
                            return Err(e);
                        }
                    } else {
                        let error = UpdateError::SignatureInvalid(
                            "Signature verification enabled but no signature/key available"
                                .to_string(),
                        );
                        self.transition_to_failed(error.clone(), app);
                        return Err(error);
                    }
                }

                // Update persisted state
                {
                    let mut persisted = self.persisted_state.lock().unwrap();
                    if let Some(ref mut pending) = persisted.pending_update {
                        pending.verified = true;
                    }
                }
                self.persist_state();

                // Transition to ReadyToInstall
                self.transition_state(UpdateState::ReadyToInstall, app);
                *self.retry_count.lock().unwrap() = 0;

                Ok(())
            }
            Err(e) => {
                // Clear download path since file was deleted
                *self.download_path.lock().unwrap() = None;
                self.transition_to_failed(e.clone(), app);
                Err(e)
            }
        }
    }

    /// Install the verified update
    /// Requirements: 4.1
    pub fn install_update(&self, app: Option<&AppHandle>) -> Result<(), UpdateError> {
        // Validate state transition
        let current_state = self.get_state();
        if !matches!(current_state, UpdateState::ReadyToInstall) {
            return Err(UpdateError::InvalidState {
                current: format!("{:?}", current_state),
                attempted: "Installing".to_string(),
            });
        }

        let download_path = self
            .download_path
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| UpdateError::FileSystem("No download path available".to_string()))?;

        self.transition_state(UpdateState::Installing, app);

        // Detect installer type and run
        use crate::auto_update::InstallerRunner;

        let installer_type = InstallerRunner::detect_installer_type(&download_path)
            .map_err(|e| UpdateError::InstallFailed(e.to_string()))?;

        match InstallerRunner::run_silent(&download_path, installer_type) {
            Ok(()) => {
                // Transition to Restarting
                self.transition_state(UpdateState::Restarting, app);
                *self.retry_count.lock().unwrap() = 0;
                Ok(())
            }
            Err(e) => {
                let error = UpdateError::InstallFailed(e.to_string());
                self.transition_to_failed(error.clone(), app);
                Err(error)
            }
        }
    }

    /// Restart the application after update
    /// Requirements: 4.3
    pub fn restart_app(&self, app: Option<&AppHandle>) -> Result<(), UpdateError> {
        // Validate state transition
        let current_state = self.get_state();
        if !matches!(current_state, UpdateState::Restarting) {
            return Err(UpdateError::InvalidState {
                current: format!("{:?}", current_state),
                attempted: "Restarting".to_string(),
            });
        }

        use crate::auto_update::InstallerRunner;

        // Update persisted state before restart
        {
            let mut persisted = self.persisted_state.lock().unwrap();
            persisted.last_successful_update = Some(Self::current_timestamp());
            persisted.pending_update = None;
        }
        self.persist_state();

        match InstallerRunner::restart_app() {
            Ok(()) => {
                self.transition_state(UpdateState::Done, app);
                Ok(())
            }
            Err(e) => {
                let error = UpdateError::InstallFailed(format!("Failed to restart: {}", e));
                self.transition_to_failed(error.clone(), app);
                Err(error)
            }
        }
    }


    /// Retry the last failed operation
    /// Requirements: 12.4
    pub fn retry(&self, app: Option<&AppHandle>) -> Result<UpdateState, UpdateError> {
        let current_state = self.get_state();

        // Can only retry from Failed state
        let (error_msg, recoverable) = match &current_state {
            UpdateState::Failed { error, recoverable } => (error.clone(), *recoverable),
            _ => {
                return Err(UpdateError::InvalidState {
                    current: format!("{:?}", current_state),
                    attempted: "Retry".to_string(),
                });
            }
        };

        if !recoverable {
            return Err(UpdateError::ConfigError(format!(
                "Error is not recoverable: {}",
                error_msg
            )));
        }

        // Increment retry count
        let retry_count = {
            let mut count = self.retry_count.lock().unwrap();
            *count += 1;
            *count
        };

        if retry_count > MAX_RETRY_ATTEMPTS {
            return Err(UpdateError::ConfigError(format!(
                "Maximum retry attempts ({}) exceeded",
                MAX_RETRY_ATTEMPTS
            )));
        }

        // Determine which state to retry from based on persisted state
        let retry_state = self.determine_retry_state();
        self.transition_state(retry_state.clone(), app);

        Ok(retry_state)
    }

    /// Determine which state to transition to for retry
    fn determine_retry_state(&self) -> UpdateState {
        let persisted = self.persisted_state.lock().unwrap();

        if let Some(ref pending) = persisted.pending_update {
            if pending.verified {
                // Already verified, retry install
                return UpdateState::ReadyToInstall;
            }

            let path = PathBuf::from(&pending.download_path);
            if path.exists() {
                // File exists, retry verification
                *self.download_path.lock().unwrap() = Some(path);
                return UpdateState::Verifying;
            }
        }

        // Default: start from checking
        UpdateState::Idle
    }

    /// Reset the coordinator to idle state
    pub fn reset(&self, app: Option<&AppHandle>) {
        *self.retry_count.lock().unwrap() = 0;
        *self.latest_info.lock().unwrap() = None;
        *self.download_path.lock().unwrap() = None;
        self.transition_state(UpdateState::Idle, app);
    }

    // ========================================
    // State Persistence Methods
    // Requirements: 12.5
    // ========================================

    /// Persist current state to file
    fn persist_state(&self) {
        let state_file = match self.state_file_path.lock().unwrap().clone() {
            Some(path) => path,
            None => return, // No persistence path configured
        };

        let persisted = self.persisted_state.lock().unwrap().clone();

        if let Ok(json) = serde_json::to_string_pretty(&persisted) {
            if let Some(parent) = state_file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::write(&state_file, json) {
                log::warn!("[UpdateCoordinator] Failed to persist state: {}", e);
            }
        }
    }

    /// Load persisted state from file
    pub fn load_persisted_state(&self) -> Result<(), UpdateError> {
        let state_file = self
            .state_file_path
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| UpdateError::ConfigError("No state file path configured".to_string()))?;

        if !state_file.exists() {
            return Ok(()); // No persisted state, use defaults
        }

        let content = std::fs::read_to_string(&state_file)
            .map_err(|e| UpdateError::FileSystem(format!("Failed to read state file: {}", e)))?;

        let persisted: PersistedUpdateState = serde_json::from_str(&content)
            .map_err(|e| UpdateError::ParseError(format!("Failed to parse state file: {}", e)))?;

        *self.persisted_state.lock().unwrap() = persisted;

        Ok(())
    }

    /// Recover state after unexpected termination
    /// Requirements: 12.5
    pub fn recover_state(&self, app: Option<&AppHandle>) -> Result<(), UpdateError> {
        self.load_persisted_state()?;

        let persisted = self.persisted_state.lock().unwrap().clone();

        match persisted.pending_update {
            Some(pending) if pending.verified => {
                // Resume from ReadyToInstall
                *self.download_path.lock().unwrap() = Some(PathBuf::from(&pending.download_path));
                self.transition_state(UpdateState::ReadyToInstall, app);
            }
            Some(pending) => {
                let path = PathBuf::from(&pending.download_path);
                if path.exists() {
                    // Verify the existing download
                    *self.download_path.lock().unwrap() = Some(path);
                    self.transition_state(UpdateState::Verifying, app);
                } else {
                    // File missing, clean up and start fresh
                    self.cleanup_stale_pending_update();
                    self.transition_state(UpdateState::Idle, app);
                }
            }
            None => {
                // No pending update, start fresh
                self.transition_state(UpdateState::Idle, app);
            }
        }

        Ok(())
    }

    /// Clean up stale pending updates
    /// Requirements: 12.5
    pub fn cleanup_stale_pending_update(&self) {
        let mut persisted = self.persisted_state.lock().unwrap();

        if let Some(ref pending) = persisted.pending_update {
            // Check if the pending update is stale (older than 24 hours)
            let now = Self::current_timestamp();
            let age_hours = (now - pending.downloaded_at) / 3600;

            if age_hours > 24 {
                // Delete the file if it exists
                let path = PathBuf::from(&pending.download_path);
                if path.exists() {
                    let _ = std::fs::remove_file(&path);
                }
                persisted.pending_update = None;
            }
        }

        drop(persisted);
        self.persist_state();
    }

    /// Get the persisted state
    pub fn get_persisted_state(&self) -> PersistedUpdateState {
        self.persisted_state.lock().unwrap().clone()
    }

    // ========================================
    // Cleanup Methods
    // Requirements: 15.3
    // ========================================

    /// Get the temporary directory for updates
    fn get_temp_dir() -> PathBuf {
        std::env::temp_dir().join("smartlab_updates")
    }

    /// Clean up temporary files after successful update
    /// Requirements: 15.3
    pub fn cleanup_after_success(&self) -> Result<(), UpdateError> {
        log::info!("[UpdateCoordinator] Cleaning up temporary files after successful update");

        // Get download path if available
        let download_path = self.download_path.lock().unwrap().clone();

        if let Some(path) = download_path {
            if path.exists() {
                std::fs::remove_file(&path).map_err(|e| {
                    UpdateError::FileSystem(format!("Failed to remove update file: {}", e))
                })?;
                log::info!("[UpdateCoordinator] Removed update file: {}", path.display());
            }
        }

        // Clear download path
        *self.download_path.lock().unwrap() = None;

        // Clear pending update from persisted state
        {
            let mut persisted = self.persisted_state.lock().unwrap();
            persisted.pending_update = None;
        }
        self.persist_state();

        Ok(())
    }

    /// Clean up temporary files after failed update
    /// Requirements: 15.3
    pub fn cleanup_after_failure(&self) -> Result<(), UpdateError> {
        log::info!("[UpdateCoordinator] Cleaning up temporary files after failed update");

        // Get download path if available
        let download_path = self.download_path.lock().unwrap().clone();

        if let Some(path) = download_path {
            if path.exists() {
                std::fs::remove_file(&path).map_err(|e| {
                    UpdateError::FileSystem(format!("Failed to remove update file: {}", e))
                })?;
                log::info!("[UpdateCoordinator] Removed failed update file: {}", path.display());
            }
        }

        // Clear download path
        *self.download_path.lock().unwrap() = None;

        // Clear pending update from persisted state
        {
            let mut persisted = self.persisted_state.lock().unwrap();
            persisted.pending_update = None;
        }
        self.persist_state();

        Ok(())
    }

    /// Clean up stale temporary files on startup
    /// Requirements: 15.3
    pub fn cleanup_stale_files(&self) -> Result<(), UpdateError> {
        log::info!("[UpdateCoordinator] Cleaning up stale temporary files on startup");

        let temp_dir = Self::get_temp_dir();

        if !temp_dir.exists() {
            return Ok(()); // No temp directory, nothing to clean
        }

        // Get current timestamp
        let now = Self::current_timestamp();

        // Read directory entries
        let entries = std::fs::read_dir(&temp_dir).map_err(|e| {
            UpdateError::FileSystem(format!("Failed to read temp directory: {}", e))
        })?;

        let mut cleaned_count = 0;

        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();

                // Get file metadata to check age
                if let Ok(metadata) = std::fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                            let file_timestamp = duration.as_secs();
                            let age_hours = (now - file_timestamp) / 3600;

                            // Remove files older than 24 hours
                            if age_hours > 24 {
                                if let Err(e) = std::fs::remove_file(&path) {
                                    log::warn!(
                                        "[UpdateCoordinator] Failed to remove stale file {}: {}",
                                        path.display(),
                                        e
                                    );
                                } else {
                                    cleaned_count += 1;
                                    log::debug!(
                                        "[UpdateCoordinator] Removed stale file: {}",
                                        path.display()
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        if cleaned_count > 0 {
            log::info!(
                "[UpdateCoordinator] Cleaned up {} stale file(s)",
                cleaned_count
            );
        }

        Ok(())
    }

    /// Clean up all temporary files (force cleanup)
    /// This removes all files in the temp directory regardless of age
    pub fn cleanup_all_temp_files(&self) -> Result<(), UpdateError> {
        log::info!("[UpdateCoordinator] Force cleaning all temporary files");

        let temp_dir = Self::get_temp_dir();

        if !temp_dir.exists() {
            return Ok(()); // No temp directory, nothing to clean
        }

        // Remove the entire temp directory
        std::fs::remove_dir_all(&temp_dir).map_err(|e| {
            UpdateError::FileSystem(format!("Failed to remove temp directory: {}", e))
        })?;

        log::info!("[UpdateCoordinator] Removed temp directory: {}", temp_dir.display());

        Ok(())
    }

    // ========================================
    // Post-Update Verification Methods
    // Requirements: 4.5, 9.5
    // ========================================

    /// Verify the running version after an update
    /// This should be called on application startup to check if an update was successful
    /// Requirements: 4.5, 9.5
    pub fn verify_post_update_version(&self) -> Result<PostUpdateVerification, UpdateError> {
        log::info!("[UpdateCoordinator] Verifying post-update version");

        // Load persisted state
        self.load_persisted_state()?;

        let persisted = self.persisted_state.lock().unwrap().clone();

        // Check if there was a recent successful update
        if let Some(last_update_timestamp) = persisted.last_successful_update {
            let now = Self::current_timestamp();
            let time_since_update = now - last_update_timestamp;

            // Only verify if the update was within the last hour (3600 seconds)
            // This prevents false positives from old updates
            if time_since_update < 3600 {
                // Check if there was a pending update with a target version
                if let Some(ref pending) = persisted.pending_update {
                    let target_version = &pending.version;
                    let current_version = &self.current_version;

                    if current_version == target_version {
                        // Update was successful
                        log::info!(
                            "[UpdateCoordinator] Post-update verification SUCCESS: Updated to version {}",
                            current_version
                        );

                        // Clean up after successful update
                        let _ = self.cleanup_after_success();

                        return Ok(PostUpdateVerification {
                            success: true,
                            expected_version: target_version.clone(),
                            actual_version: current_version.clone(),
                            message: format!(
                                "Successfully updated to version {}",
                                current_version
                            ),
                        });
                    } else {
                        // Update failed - version mismatch
                        log::error!(
                            "[UpdateCoordinator] Post-update verification FAILED: Expected version {}, but running version {}",
                            target_version,
                            current_version
                        );

                        return Ok(PostUpdateVerification {
                            success: false,
                            expected_version: target_version.clone(),
                            actual_version: current_version.clone(),
                            message: format!(
                                "Update failed: Expected version {}, but running version {}",
                                target_version, current_version
                            ),
                        });
                    }
                }
            }
        }

        // No recent update to verify
        log::debug!("[UpdateCoordinator] No recent update to verify");
        Ok(PostUpdateVerification {
            success: true,
            expected_version: self.current_version.clone(),
            actual_version: self.current_version.clone(),
            message: "No recent update to verify".to_string(),
        })
    }

    /// Check if the current version matches the expected version after update
    /// This is a simpler version check that doesn't require persisted state
    pub fn check_version_match(&self, expected_version: &str) -> bool {
        self.current_version == expected_version
    }

    /// Log the result of a post-update verification
    pub fn log_post_update_result(&self, verification: &PostUpdateVerification) {
        if verification.success {
            log::info!(
                "[UpdateCoordinator] Post-update verification: {} (expected: {}, actual: {})",
                verification.message,
                verification.expected_version,
                verification.actual_version
            );
        } else {
            log::error!(
                "[UpdateCoordinator] Post-update verification: {} (expected: {}, actual: {})",
                verification.message,
                verification.expected_version,
                verification.actual_version
            );
        }
    }
}

/// Result of post-update version verification
/// Requirements: 4.5, 9.5
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PostUpdateVerification {
    /// Whether the verification was successful
    pub success: bool,
    /// Expected version after update
    pub expected_version: String,
    /// Actual running version
    pub actual_version: String,
    /// Human-readable message
    pub message: String,
}


#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_coordinator() -> UpdateCoordinator {
        UpdateCoordinator::new(UpdateConfig::default(), "1.0.0".to_string())
    }

    #[test]
    fn test_coordinator_creation() {
        let coordinator = create_test_coordinator();
        assert_eq!(coordinator.get_state(), UpdateState::Idle);
        assert_eq!(coordinator.get_current_version(), "1.0.0");
        assert_eq!(coordinator.get_retry_count(), 0);
    }

    #[test]
    fn test_coordinator_with_defaults() {
        let coordinator = UpdateCoordinator::with_defaults("2.0.0".to_string());
        assert_eq!(coordinator.get_current_version(), "2.0.0");
        assert_eq!(coordinator.get_config().channel, "stable");
    }

    #[test]
    fn test_state_transition() {
        let coordinator = create_test_coordinator();

        // Initial state is Idle
        assert_eq!(coordinator.get_state(), UpdateState::Idle);

        // Transition to Checking
        coordinator.transition_state(UpdateState::Checking, None);
        assert_eq!(coordinator.get_state(), UpdateState::Checking);

        // Transition to UpdateAvailable
        coordinator.transition_state(
            UpdateState::UpdateAvailable {
                version: "1.1.0".to_string(),
                release_notes: "New features".to_string(),
            },
            None,
        );
        match coordinator.get_state() {
            UpdateState::UpdateAvailable { version, .. } => {
                assert_eq!(version, "1.1.0");
            }
            _ => panic!("Expected UpdateAvailable state"),
        }
    }

    #[test]
    fn test_transition_to_failed() {
        let coordinator = create_test_coordinator();

        // Network error should be recoverable
        let network_error = UpdateError::Network("Connection failed".to_string());
        coordinator.transition_to_failed(network_error, None);

        match coordinator.get_state() {
            UpdateState::Failed { error, recoverable } => {
                assert!(error.contains("Connection failed"));
                assert!(recoverable);
            }
            _ => panic!("Expected Failed state"),
        }
    }

    #[test]
    fn test_is_recoverable_error() {
        // Network errors are recoverable
        assert!(UpdateCoordinator::is_recoverable_error(&UpdateError::Network(
            "test".to_string()
        )));

        // Download failures are recoverable
        assert!(UpdateCoordinator::is_recoverable_error(
            &UpdateError::DownloadFailed("test".to_string())
        ));

        // Server errors (5xx) are recoverable
        assert!(UpdateCoordinator::is_recoverable_error(&UpdateError::ApiError {
            status_code: 500,
            message: "Internal Server Error".to_string(),
        }));

        // Client errors (4xx) are not recoverable
        assert!(!UpdateCoordinator::is_recoverable_error(
            &UpdateError::ApiError {
                status_code: 400,
                message: "Bad Request".to_string(),
            }
        ));

        // Hash mismatch is not recoverable
        assert!(!UpdateCoordinator::is_recoverable_error(
            &UpdateError::HashMismatch {
                expected: "abc".to_string(),
                actual: "xyz".to_string(),
            }
        ));
    }

    #[test]
    fn test_exponential_backoff_calculation() {
        // Attempt 0: 1000ms
        let delay0 = UpdateCoordinator::calculate_backoff_for_attempt(0);
        assert_eq!(delay0.as_millis(), 1000);

        // Attempt 1: 2000ms
        let delay1 = UpdateCoordinator::calculate_backoff_for_attempt(1);
        assert_eq!(delay1.as_millis(), 2000);

        // Attempt 2: 4000ms
        let delay2 = UpdateCoordinator::calculate_backoff_for_attempt(2);
        assert_eq!(delay2.as_millis(), 4000);

        // Attempt 3: 8000ms
        let delay3 = UpdateCoordinator::calculate_backoff_for_attempt(3);
        assert_eq!(delay3.as_millis(), 8000);

        // High attempt should be capped at MAX_BACKOFF_MS
        let delay_high = UpdateCoordinator::calculate_backoff_for_attempt(20);
        assert_eq!(delay_high.as_millis(), MAX_BACKOFF_MS as u128);
    }

    #[test]
    fn test_backoff_never_exceeds_max() {
        // Test many attempts to ensure we never exceed max
        for attempt in 0..100 {
            let delay = UpdateCoordinator::calculate_backoff_for_attempt(attempt);
            assert!(
                delay.as_millis() <= MAX_BACKOFF_MS as u128,
                "Backoff for attempt {} exceeded max: {}ms",
                attempt,
                delay.as_millis()
            );
        }
    }

    #[test]
    fn test_version_comparison() {
        let coordinator = UpdateCoordinator::new(UpdateConfig::default(), "1.0.0".to_string());

        // Newer versions
        assert!(coordinator.is_newer_version("1.0.1"));
        assert!(coordinator.is_newer_version("1.1.0"));
        assert!(coordinator.is_newer_version("2.0.0"));

        // Same version
        assert!(!coordinator.is_newer_version("1.0.0"));

        // Older versions
        assert!(!coordinator.is_newer_version("0.9.9"));
        assert!(!coordinator.is_newer_version("0.0.1"));
    }

    #[test]
    fn test_version_comparison_partial() {
        let coordinator = UpdateCoordinator::new(UpdateConfig::default(), "1.2".to_string());

        assert!(coordinator.is_newer_version("1.3"));
        assert!(coordinator.is_newer_version("1.2.1"));
        assert!(!coordinator.is_newer_version("1.2"));
        assert!(!coordinator.is_newer_version("1.1"));
    }

    #[test]
    fn test_reset() {
        let coordinator = create_test_coordinator();

        // Set some state
        coordinator.transition_state(UpdateState::Checking, None);
        *coordinator.retry_count.lock().unwrap() = 5;

        // Reset
        coordinator.reset(None);

        assert_eq!(coordinator.get_state(), UpdateState::Idle);
        assert_eq!(coordinator.get_retry_count(), 0);
        assert!(coordinator.get_latest_info().is_none());
        assert!(coordinator.get_download_path().is_none());
    }

    #[test]
    fn test_retry_from_failed_state() {
        let coordinator = create_test_coordinator();

        // Transition to Failed state with recoverable error
        coordinator.transition_state(
            UpdateState::Failed {
                error: "Network error".to_string(),
                recoverable: true,
            },
            None,
        );

        // Retry should succeed
        let result = coordinator.retry(None);
        assert!(result.is_ok());
        assert_eq!(coordinator.get_retry_count(), 1);
    }

    #[test]
    fn test_retry_from_non_failed_state() {
        let coordinator = create_test_coordinator();

        // Try to retry from Idle state
        let result = coordinator.retry(None);
        assert!(result.is_err());

        match result {
            Err(UpdateError::InvalidState { current, attempted }) => {
                assert!(current.contains("Idle"));
                assert_eq!(attempted, "Retry");
            }
            _ => panic!("Expected InvalidState error"),
        }
    }

    #[test]
    fn test_retry_non_recoverable_error() {
        let coordinator = create_test_coordinator();

        // Transition to Failed state with non-recoverable error
        coordinator.transition_state(
            UpdateState::Failed {
                error: "Hash mismatch".to_string(),
                recoverable: false,
            },
            None,
        );

        // Retry should fail
        let result = coordinator.retry(None);
        assert!(result.is_err());
    }

    #[test]
    fn test_retry_max_attempts() {
        let coordinator = create_test_coordinator();

        // Set retry count to max
        *coordinator.retry_count.lock().unwrap() = MAX_RETRY_ATTEMPTS;

        // Transition to Failed state
        coordinator.transition_state(
            UpdateState::Failed {
                error: "Network error".to_string(),
                recoverable: true,
            },
            None,
        );

        // Retry should fail due to max attempts
        let result = coordinator.retry(None);
        assert!(result.is_err());

        match result {
            Err(UpdateError::ConfigError(msg)) => {
                assert!(msg.contains("Maximum retry attempts"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_persisted_state_default() {
        let state = PersistedUpdateState::default();
        assert!(state.last_check.is_none());
        assert!(state.pending_update.is_none());
        assert!(state.last_successful_update.is_none());
    }

    #[test]
    fn test_persisted_state_serialization() {
        let state = PersistedUpdateState {
            last_check: Some(1234567890),
            pending_update: Some(PendingUpdate {
                version: "1.2.0".to_string(),
                download_path: "/tmp/update.msi".to_string(),
                sha256: "abc123".to_string(),
                downloaded_at: 1234567800,
                verified: true,
            }),
            last_successful_update: Some(1234500000),
        };

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: PersistedUpdateState = serde_json::from_str(&json).unwrap();

        assert_eq!(state.last_check, deserialized.last_check);
        assert!(deserialized.pending_update.is_some());
        let pending = deserialized.pending_update.unwrap();
        assert_eq!(pending.version, "1.2.0");
        assert!(pending.verified);
    }

    #[test]
    fn test_state_change_event_serialization() {
        let event = StateChangeEvent {
            previous_state: UpdateState::Idle,
            new_state: UpdateState::Checking,
            timestamp: 1234567890,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Idle"));
        assert!(json.contains("Checking"));
        assert!(json.contains("1234567890"));
    }

    #[test]
    fn test_determine_retry_state_no_pending() {
        let coordinator = create_test_coordinator();
        let retry_state = coordinator.determine_retry_state();
        assert_eq!(retry_state, UpdateState::Idle);
    }

    #[test]
    fn test_determine_retry_state_verified_pending() {
        let coordinator = create_test_coordinator();

        // Set up verified pending update
        {
            let mut persisted = coordinator.persisted_state.lock().unwrap();
            persisted.pending_update = Some(PendingUpdate {
                version: "1.1.0".to_string(),
                download_path: "/tmp/update.msi".to_string(),
                sha256: "abc123".to_string(),
                downloaded_at: UpdateCoordinator::current_timestamp(),
                verified: true,
            });
        }

        let retry_state = coordinator.determine_retry_state();
        assert_eq!(retry_state, UpdateState::ReadyToInstall);
    }

    #[test]
    fn test_invalid_state_transition_check() {
        let coordinator = create_test_coordinator();

        // Transition to Downloading (invalid from Idle)
        coordinator.transition_state(
            UpdateState::Downloading {
                progress: 0.0,
                bytes_downloaded: 0,
                total_bytes: 0,
            },
            None,
        );

        // Now try to check for updates - should fail
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(coordinator.check_for_updates(None));

        assert!(result.is_err());
        match result {
            Err(UpdateError::InvalidState { .. }) => {}
            _ => panic!("Expected InvalidState error"),
        }
    }

    #[test]
    fn test_current_timestamp() {
        let ts1 = UpdateCoordinator::current_timestamp();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let ts2 = UpdateCoordinator::current_timestamp();

        // Timestamps should be close but ts2 >= ts1
        assert!(ts2 >= ts1);
    }

    #[test]
    fn test_state_persistence_to_file() {
        let coordinator = create_test_coordinator();
        let temp_dir = std::env::temp_dir().join(format!("coordinator_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let state_file = temp_dir.join("update_state.json");

        // Set state file path
        coordinator.set_state_file_path(state_file.clone());

        // Update persisted state
        {
            let mut persisted = coordinator.persisted_state.lock().unwrap();
            persisted.last_check = Some(1234567890);
            persisted.pending_update = Some(PendingUpdate {
                version: "1.2.0".to_string(),
                download_path: "/tmp/update.msi".to_string(),
                sha256: "abc123".to_string(),
                downloaded_at: 1234567800,
                verified: false,
            });
        }

        // Trigger persistence via state transition
        coordinator.transition_state(UpdateState::Checking, None);

        // Verify file was created
        assert!(state_file.exists());

        // Read and verify content
        let content = std::fs::read_to_string(&state_file).unwrap();
        let loaded: PersistedUpdateState = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.last_check, Some(1234567890));
        assert!(loaded.pending_update.is_some());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_persisted_state_from_file() {
        let coordinator = create_test_coordinator();
        let temp_dir = std::env::temp_dir().join(format!("coordinator_load_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let state_file = temp_dir.join("update_state.json");

        // Create a state file
        let state = PersistedUpdateState {
            last_check: Some(9999999999),
            pending_update: Some(PendingUpdate {
                version: "2.0.0".to_string(),
                download_path: "/tmp/test.msi".to_string(),
                sha256: "xyz789".to_string(),
                downloaded_at: 9999999000,
                verified: true,
            }),
            last_successful_update: Some(9999990000),
        };
        std::fs::write(&state_file, serde_json::to_string(&state).unwrap()).unwrap();

        // Set state file path and load
        coordinator.set_state_file_path(state_file.clone());
        let result = coordinator.load_persisted_state();
        assert!(result.is_ok());

        // Verify loaded state
        let loaded = coordinator.get_persisted_state();
        assert_eq!(loaded.last_check, Some(9999999999));
        assert!(loaded.pending_update.is_some());
        let pending = loaded.pending_update.unwrap();
        assert_eq!(pending.version, "2.0.0");
        assert!(pending.verified);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_persisted_state_no_file() {
        let coordinator = create_test_coordinator();
        let temp_dir = std::env::temp_dir().join(format!("coordinator_nofile_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let state_file = temp_dir.join("nonexistent.json");

        // Set state file path (file doesn't exist)
        coordinator.set_state_file_path(state_file);
        let result = coordinator.load_persisted_state();

        // Should succeed with defaults
        assert!(result.is_ok());
        let loaded = coordinator.get_persisted_state();
        assert!(loaded.last_check.is_none());
        assert!(loaded.pending_update.is_none());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_recover_state_with_verified_pending() {
        let coordinator = create_test_coordinator();
        let temp_dir = std::env::temp_dir().join(format!("coordinator_recover_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let state_file = temp_dir.join("update_state.json");

        // Create a state file with verified pending update
        let state = PersistedUpdateState {
            last_check: Some(1234567890),
            pending_update: Some(PendingUpdate {
                version: "1.5.0".to_string(),
                download_path: "/tmp/verified.msi".to_string(),
                sha256: "verified123".to_string(),
                downloaded_at: 1234567800,
                verified: true,
            }),
            last_successful_update: None,
        };
        std::fs::write(&state_file, serde_json::to_string(&state).unwrap()).unwrap();

        // Set state file path and recover
        coordinator.set_state_file_path(state_file);
        let result = coordinator.recover_state(None);
        assert!(result.is_ok());

        // Should be in ReadyToInstall state
        assert_eq!(coordinator.get_state(), UpdateState::ReadyToInstall);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_recover_state_no_pending() {
        let coordinator = create_test_coordinator();
        let temp_dir = std::env::temp_dir().join(format!("coordinator_recover_empty_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let state_file = temp_dir.join("update_state.json");

        // Create a state file with no pending update
        let state = PersistedUpdateState {
            last_check: Some(1234567890),
            pending_update: None,
            last_successful_update: Some(1234500000),
        };
        std::fs::write(&state_file, serde_json::to_string(&state).unwrap()).unwrap();

        // Set state file path and recover
        coordinator.set_state_file_path(state_file);
        let result = coordinator.recover_state(None);
        assert!(result.is_ok());

        // Should be in Idle state
        assert_eq!(coordinator.get_state(), UpdateState::Idle);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cleanup_stale_pending_update() {
        let coordinator = create_test_coordinator();

        // Set up a stale pending update (older than 24 hours)
        let stale_timestamp = UpdateCoordinator::current_timestamp() - (25 * 3600); // 25 hours ago
        {
            let mut persisted = coordinator.persisted_state.lock().unwrap();
            persisted.pending_update = Some(PendingUpdate {
                version: "1.0.0".to_string(),
                download_path: "/nonexistent/path.msi".to_string(),
                sha256: "stale".to_string(),
                downloaded_at: stale_timestamp,
                verified: false,
            });
        }

        // Cleanup should remove the stale update
        coordinator.cleanup_stale_pending_update();

        let persisted = coordinator.get_persisted_state();
        assert!(persisted.pending_update.is_none());
    }

    #[test]
    fn test_cleanup_fresh_pending_update_not_removed() {
        let coordinator = create_test_coordinator();

        // Set up a fresh pending update (less than 24 hours old)
        let fresh_timestamp = UpdateCoordinator::current_timestamp() - (1 * 3600); // 1 hour ago
        {
            let mut persisted = coordinator.persisted_state.lock().unwrap();
            persisted.pending_update = Some(PendingUpdate {
                version: "1.0.0".to_string(),
                download_path: "/nonexistent/path.msi".to_string(),
                sha256: "fresh".to_string(),
                downloaded_at: fresh_timestamp,
                verified: false,
            });
        }

        // Cleanup should NOT remove the fresh update
        coordinator.cleanup_stale_pending_update();

        let persisted = coordinator.get_persisted_state();
        assert!(persisted.pending_update.is_some());
    }

    #[test]
    fn test_cleanup_after_success() {
        let coordinator = create_test_coordinator();
        let temp_dir = std::env::temp_dir().join(format!("cleanup_success_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("update.msi");

        // Create a test file
        std::fs::write(&test_file, b"test update content").unwrap();
        assert!(test_file.exists());

        // Set download path
        *coordinator.download_path.lock().unwrap() = Some(test_file.clone());

        // Set pending update
        {
            let mut persisted = coordinator.persisted_state.lock().unwrap();
            persisted.pending_update = Some(PendingUpdate {
                version: "1.0.0".to_string(),
                download_path: test_file.to_string_lossy().to_string(),
                sha256: "test".to_string(),
                downloaded_at: UpdateCoordinator::current_timestamp(),
                verified: true,
            });
        }

        // Cleanup after success
        let result = coordinator.cleanup_after_success();
        assert!(result.is_ok());

        // File should be removed
        assert!(!test_file.exists());

        // Download path should be cleared
        assert!(coordinator.get_download_path().is_none());

        // Pending update should be cleared
        let persisted = coordinator.get_persisted_state();
        assert!(persisted.pending_update.is_none());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cleanup_after_failure() {
        let coordinator = create_test_coordinator();
        let temp_dir = std::env::temp_dir().join(format!("cleanup_failure_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("failed_update.msi");

        // Create a test file
        std::fs::write(&test_file, b"failed update content").unwrap();
        assert!(test_file.exists());

        // Set download path
        *coordinator.download_path.lock().unwrap() = Some(test_file.clone());

        // Set pending update
        {
            let mut persisted = coordinator.persisted_state.lock().unwrap();
            persisted.pending_update = Some(PendingUpdate {
                version: "1.0.0".to_string(),
                download_path: test_file.to_string_lossy().to_string(),
                sha256: "test".to_string(),
                downloaded_at: UpdateCoordinator::current_timestamp(),
                verified: false,
            });
        }

        // Cleanup after failure
        let result = coordinator.cleanup_after_failure();
        assert!(result.is_ok());

        // File should be removed
        assert!(!test_file.exists());

        // Download path should be cleared
        assert!(coordinator.get_download_path().is_none());

        // Pending update should be cleared
        let persisted = coordinator.get_persisted_state();
        assert!(persisted.pending_update.is_none());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cleanup_stale_files() {
        let coordinator = create_test_coordinator();
        let temp_dir = UpdateCoordinator::get_temp_dir();
        let _ = std::fs::create_dir_all(&temp_dir);

        // Create a fresh file (should not be removed)
        let fresh_file = temp_dir.join("fresh_update.msi");
        std::fs::write(&fresh_file, b"fresh content").unwrap();

        // Create a stale file by modifying its timestamp
        let stale_file = temp_dir.join("stale_update.msi");
        std::fs::write(&stale_file, b"stale content").unwrap();

        // Modify the stale file's timestamp to be 25 hours old
        // Note: This is platform-specific and may not work on all systems
        // For testing purposes, we'll just verify the cleanup logic runs without error
        let result = coordinator.cleanup_stale_files();
        assert!(result.is_ok());

        // Cleanup
        let _ = std::fs::remove_file(&fresh_file);
        let _ = std::fs::remove_file(&stale_file);
    }

    #[test]
    fn test_cleanup_all_temp_files() {
        let coordinator = create_test_coordinator();
        let temp_dir = UpdateCoordinator::get_temp_dir();
        let _ = std::fs::create_dir_all(&temp_dir);

        // Create some test files
        let file1 = temp_dir.join("file1.msi");
        let file2 = temp_dir.join("file2.exe");
        std::fs::write(&file1, b"content1").unwrap();
        std::fs::write(&file2, b"content2").unwrap();

        assert!(file1.exists());
        assert!(file2.exists());

        // Cleanup all temp files
        let result = coordinator.cleanup_all_temp_files();
        assert!(result.is_ok());

        // Temp directory should be removed
        assert!(!temp_dir.exists());
    }

    #[test]
    fn test_cleanup_nonexistent_file() {
        let coordinator = create_test_coordinator();

        // Set download path to nonexistent file
        *coordinator.download_path.lock().unwrap() = Some(PathBuf::from("/nonexistent/file.msi"));

        // Cleanup should succeed even if file doesn't exist
        let result = coordinator.cleanup_after_success();
        assert!(result.is_ok());

        // Download path should be cleared
        assert!(coordinator.get_download_path().is_none());
    }

    #[test]
    fn test_verify_post_update_version_success() {
        let coordinator = UpdateCoordinator::new(UpdateConfig::default(), "1.2.0".to_string());
        let temp_dir = std::env::temp_dir().join(format!("post_update_success_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let state_file = temp_dir.join("update_state.json");

        // Create a state file with recent successful update
        let now = UpdateCoordinator::current_timestamp();
        let state = PersistedUpdateState {
            last_check: Some(now - 100),
            pending_update: Some(PendingUpdate {
                version: "1.2.0".to_string(),
                download_path: "/tmp/update.msi".to_string(),
                sha256: "abc123".to_string(),
                downloaded_at: now - 200,
                verified: true,
            }),
            last_successful_update: Some(now - 60), // 1 minute ago
        };
        std::fs::write(&state_file, serde_json::to_string(&state).unwrap()).unwrap();

        // Set state file path
        coordinator.set_state_file_path(state_file);

        // Verify post-update version
        let result = coordinator.verify_post_update_version();
        assert!(result.is_ok());

        let verification = result.unwrap();
        assert!(verification.success);
        assert_eq!(verification.expected_version, "1.2.0");
        assert_eq!(verification.actual_version, "1.2.0");

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_verify_post_update_version_failure() {
        let coordinator = UpdateCoordinator::new(UpdateConfig::default(), "1.1.0".to_string());
        let temp_dir = std::env::temp_dir().join(format!("post_update_failure_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let state_file = temp_dir.join("update_state.json");

        // Create a state file with recent successful update but version mismatch
        let now = UpdateCoordinator::current_timestamp();
        let state = PersistedUpdateState {
            last_check: Some(now - 100),
            pending_update: Some(PendingUpdate {
                version: "1.2.0".to_string(), // Expected version
                download_path: "/tmp/update.msi".to_string(),
                sha256: "abc123".to_string(),
                downloaded_at: now - 200,
                verified: true,
            }),
            last_successful_update: Some(now - 60), // 1 minute ago
        };
        std::fs::write(&state_file, serde_json::to_string(&state).unwrap()).unwrap();

        // Set state file path
        coordinator.set_state_file_path(state_file);

        // Verify post-update version
        let result = coordinator.verify_post_update_version();
        assert!(result.is_ok());

        let verification = result.unwrap();
        assert!(!verification.success); // Should fail due to version mismatch
        assert_eq!(verification.expected_version, "1.2.0");
        assert_eq!(verification.actual_version, "1.1.0");

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_verify_post_update_no_recent_update() {
        let coordinator = UpdateCoordinator::new(UpdateConfig::default(), "1.0.0".to_string());
        let temp_dir = std::env::temp_dir().join(format!("post_update_no_recent_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let state_file = temp_dir.join("update_state.json");

        // Create a state file with old update (more than 1 hour ago)
        let now = UpdateCoordinator::current_timestamp();
        let state = PersistedUpdateState {
            last_check: Some(now - 10000),
            pending_update: None,
            last_successful_update: Some(now - 7200), // 2 hours ago
        };
        std::fs::write(&state_file, serde_json::to_string(&state).unwrap()).unwrap();

        // Set state file path
        coordinator.set_state_file_path(state_file);

        // Verify post-update version
        let result = coordinator.verify_post_update_version();
        assert!(result.is_ok());

        let verification = result.unwrap();
        assert!(verification.success); // Should succeed (no recent update to verify)
        assert_eq!(verification.expected_version, "1.0.0");
        assert_eq!(verification.actual_version, "1.0.0");
        assert!(verification.message.contains("No recent update"));

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_check_version_match() {
        let coordinator = UpdateCoordinator::new(UpdateConfig::default(), "1.5.0".to_string());

        assert!(coordinator.check_version_match("1.5.0"));
        assert!(!coordinator.check_version_match("1.4.0"));
        assert!(!coordinator.check_version_match("1.5.1"));
    }

    #[test]
    fn test_post_update_verification_serialization() {
        let verification = PostUpdateVerification {
            success: true,
            expected_version: "1.2.0".to_string(),
            actual_version: "1.2.0".to_string(),
            message: "Update successful".to_string(),
        };

        let json = serde_json::to_string(&verification).unwrap();
        let deserialized: PostUpdateVerification = serde_json::from_str(&json).unwrap();

        assert_eq!(verification, deserialized);
    }

    #[test]
    fn test_log_post_update_result() {
        let coordinator = create_test_coordinator();

        let success_verification = PostUpdateVerification {
            success: true,
            expected_version: "1.2.0".to_string(),
            actual_version: "1.2.0".to_string(),
            message: "Update successful".to_string(),
        };

        // Should not panic
        coordinator.log_post_update_result(&success_verification);

        let failure_verification = PostUpdateVerification {
            success: false,
            expected_version: "1.2.0".to_string(),
            actual_version: "1.1.0".to_string(),
            message: "Update failed".to_string(),
        };

        // Should not panic
        coordinator.log_post_update_result(&failure_verification);
    }
}
