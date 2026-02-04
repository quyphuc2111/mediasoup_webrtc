// Update Configuration File Handler
// Loads and saves update configuration from/to a JSON file

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

use crate::auto_update::UpdateConfig;

/// Configuration file name
const CONFIG_FILE_NAME: &str = "update_config.json";

/// Get the configuration file path
/// Stores in app data directory: ~/.smartlab/update_config.json
pub fn get_config_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".smartlab").join(CONFIG_FILE_NAME)
}

/// Load update configuration from file
/// Returns default config if file doesn't exist
pub fn load_config() -> UpdateConfig {
    let config_path = get_config_path();
    
    if !config_path.exists() {
        log::info!("[UpdateConfig] Config file not found, using defaults");
        return UpdateConfig::default();
    }
    
    match fs::read_to_string(&config_path) {
        Ok(content) => {
            match serde_json::from_str::<UpdateConfig>(&content) {
                Ok(config) => {
                    log::info!("[UpdateConfig] Loaded config from {:?}", config_path);
                    log::info!("[UpdateConfig] API URL: {}", config.api_base_url);
                    config
                }
                Err(e) => {
                    log::warn!("[UpdateConfig] Failed to parse config: {}, using defaults", e);
                    UpdateConfig::default()
                }
            }
        }
        Err(e) => {
            log::warn!("[UpdateConfig] Failed to read config: {}, using defaults", e);
            UpdateConfig::default()
        }
    }
}

/// Save update configuration to file
pub fn save_config(config: &UpdateConfig) -> Result<(), String> {
    let config_path = get_config_path();
    
    // Create parent directory if needed
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    
    fs::write(&config_path, json)
        .map_err(|e| format!("Failed to write config: {}", e))?;
    
    log::info!("[UpdateConfig] Saved config to {:?}", config_path);
    Ok(())
}

/// Create default config file if it doesn't exist
pub fn ensure_config_exists() -> Result<PathBuf, String> {
    let config_path = get_config_path();
    
    if !config_path.exists() {
        let default_config = UpdateConfig::default();
        save_config(&default_config)?;
        log::info!("[UpdateConfig] Created default config at {:?}", config_path);
    }
    
    Ok(config_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_get_config_path() {
        let path = get_config_path();
        assert!(path.to_string_lossy().contains(".smartlab"));
        assert!(path.to_string_lossy().contains("update_config.json"));
    }
    
    #[test]
    fn test_load_config_default() {
        // Should return default when file doesn't exist
        let config = load_config();
        assert!(!config.api_base_url.is_empty());
    }
}
