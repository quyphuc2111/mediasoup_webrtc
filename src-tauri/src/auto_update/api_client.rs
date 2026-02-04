// Update API Client
// Handles HTTP communication with the public Update API
// Requirements: 1.1, 1.2, 1.4, 10.2

use reqwest::Client;
use crate::auto_update::{UpdateError, UpdateInfo, UpdateConfig};

/// HTTP client for communicating with the public Update API
pub struct UpdateApiClient {
    /// Base URL for the Update API
    base_url: String,
    /// HTTP client instance
    http_client: Client,
    /// Update channel (stable, beta, dev)
    channel: String,
    /// Application type (teacher, student)
    app_type: String,
    /// Operating system identifier
    os: String,
    /// Architecture identifier
    arch: String,
}

impl UpdateApiClient {
    /// Create a new Update API client
    /// 
    /// # Arguments
    /// * `base_url` - Base URL for the Update API (e.g., "https://updates.smartlab.example.com")
    /// * `channel` - Update channel (stable, beta, dev)
    /// * `app_type` - Application type (teacher, student)
    pub fn new(base_url: &str, channel: &str, app_type: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http_client: Client::new(),
            channel: channel.to_string(),
            app_type: app_type.to_string(),
            os: Self::detect_os(),
            arch: Self::detect_arch(),
        }
    }

    /// Create a new client from UpdateConfig
    pub fn from_config(config: &UpdateConfig) -> Self {
        Self::new(&config.api_base_url, &config.channel, &config.app_type)
    }

    /// Detect the current operating system
    fn detect_os() -> String {
        if cfg!(target_os = "windows") {
            "windows".to_string()
        } else if cfg!(target_os = "macos") {
            "macos".to_string()
        } else if cfg!(target_os = "linux") {
            "linux".to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// Detect the current architecture
    fn detect_arch() -> String {
        if cfg!(target_arch = "x86_64") {
            "x64".to_string()
        } else if cfg!(target_arch = "aarch64") {
            "arm64".to_string()
        } else if cfg!(target_arch = "x86") {
            "x86".to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// Get the latest version information from the Update API
    /// 
    /// Requirements: 1.1, 1.2
    /// - Returns JSON containing version, published_at, download_url, sha256 hash, and release_notes
    /// - Specifies channel, os, and arch parameters
    /// 
    /// # Returns
    /// * `Ok(UpdateInfo)` - Latest version information
    /// * `Err(UpdateError)` - Error if request fails or response is invalid
    pub async fn get_latest_version(&self) -> Result<UpdateInfo, UpdateError> {
        // URL format: {base_url}/updates/latest?app_type=...&channel=...&os=...&arch=...
        // If base_url already contains /api, use /updates/latest
        // Otherwise, use /api/updates/latest for backward compatibility
        let url = if self.base_url.ends_with("/api") {
            format!(
                "{}/updates/latest?app_type={}&channel={}&os={}&arch={}",
                self.base_url, self.app_type, self.channel, self.os, self.arch
            )
        } else {
            format!(
                "{}/api/updates/latest?app_type={}&channel={}&os={}&arch={}",
                self.base_url, self.app_type, self.channel, self.os, self.arch
            )
        };

        let response = self.http_client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    UpdateError::Network(format!("Connection failed: {}", e))
                } else if e.is_timeout() {
                    UpdateError::Network(format!("Request timed out: {}", e))
                } else {
                    UpdateError::Network(format!("Request failed: {}", e))
                }
            })?;

        let status = response.status();
        
        // Handle error responses (Requirements: 1.4)
        if !status.is_success() {
            let status_code = status.as_u16();
            let error_body = response.text().await.unwrap_or_default();
            
            return Err(UpdateError::ApiError {
                status_code,
                message: match status_code {
                    400 => format!("Invalid request parameters: {}", error_body),
                    404 => "No update available for this configuration".to_string(),
                    500 => format!("Server error: {}", error_body),
                    _ => format!("HTTP {}: {}", status_code, error_body),
                },
            });
        }

        // Parse JSON response into UpdateInfo
        let update_info: UpdateInfo = response
            .json()
            .await
            .map_err(|e| UpdateError::ParseError(format!("Failed to parse response: {}", e)))?;

        // Validate required fields
        Self::validate_update_info(&update_info)?;

        Ok(update_info)
    }

    /// Validate that UpdateInfo contains all required fields
    fn validate_update_info(info: &UpdateInfo) -> Result<(), UpdateError> {
        if info.version.is_empty() {
            return Err(UpdateError::ParseError("Missing required field: version".to_string()));
        }
        if info.published_at.is_empty() {
            return Err(UpdateError::ParseError("Missing required field: published_at".to_string()));
        }
        if info.download_url.is_empty() {
            return Err(UpdateError::ParseError("Missing required field: download_url".to_string()));
        }
        if info.sha256.is_empty() {
            return Err(UpdateError::ParseError("Missing required field: sha256".to_string()));
        }
        Ok(())
    }

    /// Get changelog between two versions
    /// 
    /// Requirements: 10.2
    /// - Returns markdown string containing changelog
    /// 
    /// # Arguments
    /// * `from` - Starting version (e.g., "1.0.0")
    /// * `to` - Ending version (e.g., "1.2.0")
    /// 
    /// # Returns
    /// * `Ok(String)` - Markdown changelog
    /// * `Err(UpdateError)` - Error if request fails
    pub async fn get_changelog(&self, from: &str, to: &str) -> Result<String, UpdateError> {
        let url = if self.base_url.ends_with("/api") {
            format!(
                "{}/updates/changelog?from={}&to={}&channel={}",
                self.base_url, from, to, self.channel
            )
        } else {
            format!(
                "{}/api/updates/changelog?from={}&to={}&channel={}",
                self.base_url, from, to, self.channel
            )
        };

        let response = self.http_client
            .get(&url)
            .header("Accept", "text/markdown")
            .send()
            .await
            .map_err(|e| UpdateError::Network(format!("Failed to fetch changelog: {}", e)))?;

        let status = response.status();
        
        if !status.is_success() {
            let status_code = status.as_u16();
            let error_body = response.text().await.unwrap_or_default();
            
            return Err(UpdateError::ApiError {
                status_code,
                message: format!("Failed to get changelog: {}", error_body),
            });
        }

        response
            .text()
            .await
            .map_err(|e| UpdateError::ParseError(format!("Failed to read changelog: {}", e)))
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Get the channel
    pub fn channel(&self) -> &str {
        &self.channel
    }

    /// Get the OS
    pub fn os(&self) -> &str {
        &self.os
    }

    /// Get the architecture
    pub fn arch(&self) -> &str {
        &self.arch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_client() {
        let client = UpdateApiClient::new("http://localhost:3030/api", "stable", "teacher");
        assert_eq!(client.base_url(), "http://localhost:3030/api");
        assert_eq!(client.channel(), "stable");
    }

    #[test]
    fn test_new_client_trims_trailing_slash() {
        let client = UpdateApiClient::new("http://localhost:3030/api/", "beta", "student");
        assert_eq!(client.base_url(), "http://localhost:3030/api");
    }

    #[test]
    fn test_from_config() {
        let config = UpdateConfig {
            api_base_url: "http://localhost:3030/api".to_string(),
            channel: "dev".to_string(),
            app_type: "teacher".to_string(),
            ..Default::default()
        };
        let client = UpdateApiClient::from_config(&config);
        assert_eq!(client.base_url(), "http://localhost:3030/api");
        assert_eq!(client.channel(), "dev");
    }

    #[test]
    fn test_detect_os() {
        let os = UpdateApiClient::detect_os();
        #[cfg(target_os = "windows")]
        assert_eq!(os, "windows");
        #[cfg(target_os = "macos")]
        assert_eq!(os, "macos");
        #[cfg(target_os = "linux")]
        assert_eq!(os, "linux");
    }

    #[test]
    fn test_detect_arch() {
        let arch = UpdateApiClient::detect_arch();
        #[cfg(target_arch = "x86_64")]
        assert_eq!(arch, "x64");
        #[cfg(target_arch = "aarch64")]
        assert_eq!(arch, "arm64");
    }

    #[test]
    fn test_validate_update_info_valid() {
        let info = UpdateInfo {
            version: "1.0.0".to_string(),
            published_at: "2024-01-15T10:30:00Z".to_string(),
            download_url: "https://example.com/update.msi".to_string(),
            sha256: "abc123def456".to_string(),
            signature: None,
            release_notes: "# Release Notes".to_string(),
            changelog_url: None,
            min_app_version: None,
        };
        assert!(UpdateApiClient::validate_update_info(&info).is_ok());
    }

    #[test]
    fn test_validate_update_info_missing_version() {
        let info = UpdateInfo {
            version: "".to_string(),
            published_at: "2024-01-15T10:30:00Z".to_string(),
            download_url: "https://example.com/update.msi".to_string(),
            sha256: "abc123def456".to_string(),
            signature: None,
            release_notes: "# Release Notes".to_string(),
            changelog_url: None,
            min_app_version: None,
        };
        let result = UpdateApiClient::validate_update_info(&info);
        assert!(result.is_err());
        if let Err(UpdateError::ParseError(msg)) = result {
            assert!(msg.contains("version"));
        }
    }

    #[test]
    fn test_validate_update_info_missing_sha256() {
        let info = UpdateInfo {
            version: "1.0.0".to_string(),
            published_at: "2024-01-15T10:30:00Z".to_string(),
            download_url: "https://example.com/update.msi".to_string(),
            sha256: "".to_string(),
            signature: None,
            release_notes: "# Release Notes".to_string(),
            changelog_url: None,
            min_app_version: None,
        };
        let result = UpdateApiClient::validate_update_info(&info);
        assert!(result.is_err());
        if let Err(UpdateError::ParseError(msg)) = result {
            assert!(msg.contains("sha256"));
        }
    }
}
