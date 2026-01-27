//! LDAP/Active Directory Authentication Module
//!
//! Provides authentication against LDAP/AD servers for enterprise environments.
//! This is an alternative to Ed25519 key-based authentication.

use ldap3::{LdapConn, Scope, SearchEntry};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// LDAP server configuration
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LdapConfig {
    /// LDAP server URL (e.g., "ldap://192.168.1.10:389" or "ldaps://ad.example.com:636")
    pub server_url: String,

    /// Base DN for user searches (e.g., "DC=example,DC=com")
    pub base_dn: String,

    /// User search filter (e.g., "(&(objectClass=user)(sAMAccountName={username}))")
    /// {username} will be replaced with the actual username
    pub user_filter: String,

    /// Bind DN template for authentication (e.g., "{username}@example.com" or "CN={username},OU=Users,DC=example,DC=com")
    /// {username} will be replaced with the actual username
    pub bind_dn_template: String,

    /// Optional: Group DN that users must be members of (e.g., "CN=Teachers,OU=Groups,DC=example,DC=com")
    pub required_group: Option<String>,

    /// Use TLS/SSL
    pub use_tls: bool,
}

impl Default for LdapConfig {
    fn default() -> Self {
        Self {
            server_url: "ldap://localhost:389".to_string(),
            base_dn: "DC=example,DC=com".to_string(),
            user_filter: "(&(objectClass=user)(sAMAccountName={username}))".to_string(),
            bind_dn_template: "{username}@example.com".to_string(),
            required_group: Some("CN=Teachers,OU=Groups,DC=example,DC=com".to_string()),
            use_tls: false,
        }
    }
}

/// Result of LDAP authentication
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LdapAuthResult {
    pub success: bool,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub groups: Vec<String>,
    pub error: Option<String>,
}

/// Authenticate user against LDAP/AD server
pub async fn authenticate_ldap(
    config: &LdapConfig,
    username: &str,
    password: &str,
) -> LdapAuthResult {
    let result = authenticate_ldap_internal(config, username, password).await;

    match result {
        Ok(auth_result) => auth_result,
        Err(e) => LdapAuthResult {
            success: false,
            username: None,
            display_name: None,
            email: None,
            groups: Vec::new(),
            error: Some(e),
        },
    }
}

async fn authenticate_ldap_internal(
    config: &LdapConfig,
    username: &str,
    password: &str,
) -> Result<LdapAuthResult, String> {
    // Sanitize username to prevent LDAP injection
    let sanitized_username = sanitize_ldap_input(username);

    // Build bind DN from template
    let bind_dn = config
        .bind_dn_template
        .replace("{username}", &sanitized_username);

    // Build search filter
    let search_filter = config
        .user_filter
        .replace("{username}", &sanitized_username);

    let base_dn = config.base_dn.clone();
    let required_group = config.required_group.clone();
    let password_clone = password.to_string();
    let server_url = config.server_url.clone();

    // Perform LDAP operations in blocking task
    let result: Result<LdapAuthResult, String> = tokio::task::spawn_blocking(move || {
        // Connect to LDAP server
        let mut ldap =
            LdapConn::new(&server_url).map_err(|e| format!("LDAP connection failed: {:?}", e))?;

        // Attempt to bind (authenticate)
        let bind_result = ldap
            .simple_bind(&bind_dn, &password_clone)
            .map_err(|e| format!("LDAP bind failed: {:?}", e))?;

        if !bind_result.success().is_ok() {
            return Err("Authentication failed: Invalid username or password".to_string());
        }

        // Search for user details
        let search_result = ldap
            .search(
                &base_dn,
                Scope::Subtree,
                &search_filter,
                vec!["cn", "displayName", "mail", "memberOf"],
            )
            .map_err(|e| format!("LDAP search failed: {:?}", e))?;

        let (entries, _res) = search_result
            .success()
            .map_err(|e| format!("Search failed: {:?}", e))?;

        if entries.is_empty() {
            ldap.unbind().ok();
            return Err("User not found in directory".to_string());
        }

        let entry = SearchEntry::construct(entries[0].clone());

        // Extract user information
        let display_name = entry
            .attrs
            .get("displayName")
            .and_then(|v| v.first())
            .or_else(|| entry.attrs.get("cn").and_then(|v| v.first()))
            .cloned();

        let email = entry.attrs.get("mail").and_then(|v| v.first()).cloned();

        let groups = entry
            .attrs
            .get("memberOf")
            .map(|v| v.clone())
            .unwrap_or_default();

        // Check required group membership if configured
        if let Some(required) = required_group {
            if !groups.iter().any(|g| g.contains(&required)) {
                ldap.unbind().ok();
                return Err(format!(
                    "User is not a member of required group: {}",
                    required
                ));
            }
        }

        // Unbind
        ldap.unbind().ok();

        Ok(LdapAuthResult {
            success: true,
            username: Some(sanitized_username.clone()),
            display_name,
            email,
            groups,
            error: None,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?;

    result
}

/// Sanitize LDAP input to prevent injection attacks
fn sanitize_ldap_input(input: &str) -> String {
    input
        .replace('\\', "\\5c")
        .replace('*', "\\2a")
        .replace('(', "\\28")
        .replace(')', "\\29")
        .replace('\0', "\\00")
}

/// Get the LDAP config storage path
pub fn get_ldap_config_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let smartlab_dir = home.join(".smartlab");

    if !smartlab_dir.exists() {
        fs::create_dir_all(&smartlab_dir)
            .map_err(|e| format!("Failed to create .smartlab directory: {}", e))?;
    }

    Ok(smartlab_dir.join("ldap_config.json"))
}

/// Save LDAP configuration
pub fn save_ldap_config(config: &LdapConfig) -> Result<(), String> {
    let path = get_ldap_config_path()?;
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(&path, json).map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(())
}

/// Load LDAP configuration
pub fn load_ldap_config() -> Result<LdapConfig, String> {
    let path = get_ldap_config_path()?;

    if !path.exists() {
        return Ok(LdapConfig::default());
    }

    let json = fs::read_to_string(&path).map_err(|e| format!("Failed to read config: {}", e))?;

    serde_json::from_str(&json).map_err(|e| format!("Failed to parse config: {}", e))
}

/// Test LDAP connection and configuration
pub async fn test_ldap_connection(config: &LdapConfig) -> Result<String, String> {
    let server_url = config.server_url.clone();

    // Validate URL format first
    if !server_url.starts_with("ldap://") && !server_url.starts_with("ldaps://") {
        return Err("Invalid URL format. Must start with ldap:// or ldaps://".to_string());
    }

    // Extract hostname for better error messages
    let hostname = server_url
        .trim_start_matches("ldap://")
        .trim_start_matches("ldaps://")
        .split(':')
        .next()
        .unwrap_or("unknown")
        .to_string();

    tokio::task::spawn_blocking(move || {
        match LdapConn::new(&server_url) {
            Ok(mut ldap) => {
                ldap.unbind().ok();
                Ok(format!(
                    "✅ Successfully connected to LDAP server at {}",
                    hostname
                ))
            }
            Err(e) => {
                let error_str = format!("{:?}", e);

                // Provide helpful error messages
                if error_str.contains("lookup address") || error_str.contains("nodename") {
                    Err(format!(
                        "DNS lookup failed for '{}'. Make sure:\n\
                         • The hostname is correct and resolvable\n\
                         • Use IP address (e.g., ldap://192.168.1.10:389) for testing\n\
                         • The LDAP server is accessible from this machine",
                        hostname
                    ))
                } else if error_str.contains("Connection refused") {
                    Err(format!(
                        "Connection refused to '{}'. Make sure:\n\
                         • The LDAP server is running\n\
                         • Port is correct (389 for LDAP, 636 for LDAPS)\n\
                         • Firewall allows the connection",
                        hostname
                    ))
                } else if error_str.contains("timed out") {
                    Err(format!(
                        "Connection timed out to '{}'. Make sure:\n\
                         • The server is reachable\n\
                         • Network is not blocking the connection",
                        hostname
                    ))
                } else {
                    Err(format!("Connection failed: {}", error_str))
                }
            }
        }
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_ldap_input() {
        assert_eq!(sanitize_ldap_input("user*"), "user\\2a");
        assert_eq!(sanitize_ldap_input("user(test)"), "user\\28test\\29");
        assert_eq!(sanitize_ldap_input("user\\name"), "user\\5cname");
    }

    #[test]
    fn test_ldap_config_default() {
        let config = LdapConfig::default();
        assert!(!config.server_url.is_empty());
        assert!(!config.base_dn.is_empty());
    }
}
