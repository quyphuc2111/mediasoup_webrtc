//! Cryptographic utilities for View Client authentication
//! Supports both Ed25519 PKI and LDAP/AD authentication

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Authentication mode
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum AuthMode {
    /// Ed25519 key-based authentication (simple, no infrastructure)
    Ed25519,
    /// LDAP/Active Directory authentication (enterprise, centralized)
    Ldap,
}

impl Default for AuthMode {
    fn default() -> Self {
        AuthMode::Ed25519
    }
}

/// Key pair information returned to frontend
#[derive(Clone, Serialize, Deserialize)]
pub struct KeyPairInfo {
    pub public_key: String,  // Base64 encoded
    pub private_key: String, // Base64 encoded (only for storage)
    pub fingerprint: String, // Short identifier for display
}

/// Result of signature verification
#[derive(Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    pub valid: bool,
    pub error: Option<String>,
}

/// Generate a new Ed25519 keypair
pub fn generate_keypair() -> KeyPairInfo {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();

    let private_key_bytes = signing_key.to_bytes();
    let public_key_bytes = verifying_key.to_bytes();

    // Create fingerprint from first 8 bytes of public key
    let fingerprint = hex::encode(&public_key_bytes[..4]).to_uppercase();

    KeyPairInfo {
        public_key: BASE64.encode(public_key_bytes),
        private_key: BASE64.encode(private_key_bytes),
        fingerprint: format!("ED25519:{}", fingerprint),
    }
}

/// Export public key as base64 string
pub fn export_public_key(public_key_base64: &str) -> Result<String, String> {
    // Validate it's a valid public key first
    let bytes = BASE64
        .decode(public_key_base64)
        .map_err(|e| format!("Invalid base64: {}", e))?;

    if bytes.len() != 32 {
        return Err("Invalid public key length".to_string());
    }

    // Return formatted for easy sharing
    Ok(format!(
        "-----BEGIN SMARTLAB PUBLIC KEY-----\n{}\n-----END SMARTLAB PUBLIC KEY-----",
        public_key_base64
    ))
}

/// Import public key from formatted string
pub fn import_public_key(key_data: &str) -> Result<String, String> {
    // Strip header/footer if present
    let key_data = key_data
        .replace("-----BEGIN SMARTLAB PUBLIC KEY-----", "")
        .replace("-----END SMARTLAB PUBLIC KEY-----", "")
        .replace("\n", "")
        .replace("\r", "")
        .trim()
        .to_string();

    // Validate
    let bytes = BASE64
        .decode(&key_data)
        .map_err(|e| format!("Invalid base64: {}", e))?;

    if bytes.len() != 32 {
        return Err(format!(
            "Invalid public key length: {} (expected 32)",
            bytes.len()
        ));
    }

    // Verify it can be parsed as a verifying key
    let key_bytes: [u8; 32] = bytes.try_into().map_err(|_| "Failed to convert bytes")?;

    VerifyingKey::from_bytes(&key_bytes).map_err(|e| format!("Invalid public key: {}", e))?;

    Ok(key_data)
}

/// Sign a challenge using private key
pub fn sign_challenge(private_key_base64: &str, challenge: &[u8]) -> Result<String, String> {
    let private_bytes = BASE64
        .decode(private_key_base64)
        .map_err(|e| format!("Invalid private key base64: {}", e))?;

    let key_bytes: [u8; 32] = private_bytes
        .try_into()
        .map_err(|_| "Invalid private key length")?;

    let signing_key = SigningKey::from_bytes(&key_bytes);
    let signature = signing_key.sign(challenge);

    Ok(BASE64.encode(signature.to_bytes()))
}

/// Verify a signature using public key
pub fn verify_signature(
    public_key_base64: &str,
    challenge: &[u8],
    signature_base64: &str,
) -> VerifyResult {
    let result = (|| -> Result<bool, String> {
        let public_bytes = BASE64
            .decode(public_key_base64)
            .map_err(|e| format!("Invalid public key base64: {}", e))?;

        let key_bytes: [u8; 32] = public_bytes
            .try_into()
            .map_err(|_| "Invalid public key length")?;

        let verifying_key = VerifyingKey::from_bytes(&key_bytes)
            .map_err(|e| format!("Invalid public key: {}", e))?;

        let sig_bytes = BASE64
            .decode(signature_base64)
            .map_err(|e| format!("Invalid signature base64: {}", e))?;

        let sig_array: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| "Invalid signature length")?;

        let signature = Signature::from_bytes(&sig_array);

        match verifying_key.verify(challenge, &signature) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    })();

    match result {
        Ok(valid) => VerifyResult { valid, error: None },
        Err(e) => VerifyResult {
            valid: false,
            error: Some(e),
        },
    }
}

/// Generate a random challenge (32 bytes)
pub fn generate_challenge() -> Vec<u8> {
    use rand::RngCore;
    let mut challenge = [0u8; 32];
    OsRng.fill_bytes(&mut challenge);
    challenge.to_vec()
}

/// Get the default key storage directory
pub fn get_key_storage_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;

    let smartlab_dir = home.join(".smartlab");

    if !smartlab_dir.exists() {
        fs::create_dir_all(&smartlab_dir)
            .map_err(|e| format!("Failed to create .smartlab directory: {}", e))?;
    }

    Ok(smartlab_dir)
}

/// Save keypair to disk
pub fn save_keypair(keypair: &KeyPairInfo) -> Result<PathBuf, String> {
    let dir = get_key_storage_dir()?;
    let key_file = dir.join("teacher_keypair.json");

    let json = serde_json::to_string_pretty(keypair)
        .map_err(|e| format!("Failed to serialize keypair: {}", e))?;

    fs::write(&key_file, json).map_err(|e| format!("Failed to write keypair: {}", e))?;

    Ok(key_file)
}

/// Load keypair from disk
pub fn load_keypair() -> Result<KeyPairInfo, String> {
    let dir = get_key_storage_dir()?;
    let key_file = dir.join("teacher_keypair.json");

    if !key_file.exists() {
        return Err("No keypair found. Please generate one first.".to_string());
    }

    let json =
        fs::read_to_string(&key_file).map_err(|e| format!("Failed to read keypair: {}", e))?;

    serde_json::from_str(&json).map_err(|e| format!("Failed to parse keypair: {}", e))
}

/// Save teacher's public key on student machine
pub fn save_teacher_public_key(public_key: &str) -> Result<PathBuf, String> {
    let dir = get_key_storage_dir()?;
    let key_file = dir.join("teacher_public_key.txt");

    // Validate the key first
    import_public_key(public_key)?;

    fs::write(&key_file, public_key)
        .map_err(|e| format!("Failed to write teacher public key: {}", e))?;

    Ok(key_file)
}

/// Load teacher's public key on student machine
pub fn load_teacher_public_key() -> Result<String, String> {
    let dir = get_key_storage_dir()?;
    let key_file = dir.join("teacher_public_key.txt");

    if !key_file.exists() {
        return Err("No teacher public key found. Please import one first.".to_string());
    }

    let key = fs::read_to_string(&key_file)
        .map_err(|e| format!("Failed to read teacher public key: {}", e))?;

    // Validate and normalize
    import_public_key(&key)
}

/// Check if keypair exists
pub fn has_keypair() -> bool {
    if let Ok(dir) = get_key_storage_dir() {
        dir.join("teacher_keypair.json").exists()
    } else {
        false
    }
}

/// Check if teacher public key exists (for students)
pub fn has_teacher_public_key() -> bool {
    if let Ok(dir) = get_key_storage_dir() {
        dir.join("teacher_public_key.txt").exists()
    } else {
        false
    }
}

/// Save authentication mode preference
pub fn save_auth_mode(mode: AuthMode) -> Result<(), String> {
    let dir = get_key_storage_dir()?;
    let mode_file = dir.join("auth_mode.json");

    let json = serde_json::to_string(&mode)
        .map_err(|e| format!("Failed to serialize auth mode: {}", e))?;

    fs::write(&mode_file, json).map_err(|e| format!("Failed to write auth mode: {}", e))?;

    Ok(())
}

/// Load authentication mode preference
pub fn load_auth_mode() -> AuthMode {
    let dir = match get_key_storage_dir() {
        Ok(d) => d,
        Err(_) => return AuthMode::default(),
    };

    let mode_file = dir.join("auth_mode.json");

    if !mode_file.exists() {
        return AuthMode::default();
    }

    let json = match fs::read_to_string(&mode_file) {
        Ok(j) => j,
        Err(_) => return AuthMode::default(),
    };

    serde_json::from_str(&json).unwrap_or_default()
}

// Add hex encoding for fingerprint
mod hex {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    pub fn encode(bytes: &[u8]) -> String {
        let mut result = String::with_capacity(bytes.len() * 2);
        for &byte in bytes {
            result.push(HEX_CHARS[(byte >> 4) as usize] as char);
            result.push(HEX_CHARS[(byte & 0x0f) as usize] as char);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let keypair = generate_keypair();
        assert!(!keypair.public_key.is_empty());
        assert!(!keypair.private_key.is_empty());
        assert!(keypair.fingerprint.starts_with("ED25519:"));
    }

    #[test]
    fn test_sign_and_verify() {
        let keypair = generate_keypair();
        let challenge = generate_challenge();

        let signature = sign_challenge(&keypair.private_key, &challenge).unwrap();
        let result = verify_signature(&keypair.public_key, &challenge, &signature);

        assert!(result.valid);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_wrong_signature() {
        let keypair1 = generate_keypair();
        let keypair2 = generate_keypair();
        let challenge = generate_challenge();

        let signature = sign_challenge(&keypair1.private_key, &challenge).unwrap();
        let result = verify_signature(&keypair2.public_key, &challenge, &signature);

        assert!(!result.valid);
    }

    #[test]
    fn test_import_export_public_key() {
        let keypair = generate_keypair();
        let exported = export_public_key(&keypair.public_key).unwrap();
        let imported = import_public_key(&exported).unwrap();

        assert_eq!(keypair.public_key, imported);
    }
}
