// Verifier Module
// Handles SHA256 hash verification and optional signature verification
// Requirements: 3.3, 3.4, 3.5, 3.6

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signature, Verifier as Ed25519Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use crate::auto_update::UpdateError;

/// File verifier for hash and signature verification
/// 
/// Requirements:
/// - 3.3: Verify SHA256 hash matches expected value
/// - 3.4: Delete file on hash mismatch
/// - 3.5: Verify package signature before installation
/// - 3.6: Reject update on signature verification failure
pub struct Verifier;

impl Verifier {
    /// Calculate SHA256 hash of a file
    /// 
    /// # Arguments
    /// * `file_path` - Path to the file to hash
    /// 
    /// # Returns
    /// * `Ok(String)` - Lowercase hex-encoded SHA256 hash
    /// * `Err(UpdateError)` - Error if file cannot be read
    pub fn calculate_sha256(file_path: &Path) -> Result<String, UpdateError> {
        let file = File::open(file_path).map_err(|e| {
            UpdateError::FileSystem(format!("Failed to open file for hashing: {}", e))
        })?;

        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192]; // 8KB buffer for efficient reading

        loop {
            let bytes_read = reader.read(&mut buffer).map_err(|e| {
                UpdateError::FileSystem(format!("Failed to read file for hashing: {}", e))
            })?;

            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);
        }

        let hash = hasher.finalize();
        Ok(hex_encode(&hash))
    }

    /// Calculate SHA256 hash of raw bytes
    /// 
    /// # Arguments
    /// * `data` - Byte slice to hash
    /// 
    /// # Returns
    /// * Lowercase hex-encoded SHA256 hash
    pub fn calculate_sha256_bytes(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hasher.finalize();
        hex_encode(&hash)
    }

    /// Verify SHA256 hash of a file matches expected value
    /// 
    /// Requirements: 3.3
    /// - Verify the SHA256 hash matches the expected value
    /// 
    /// # Arguments
    /// * `file_path` - Path to the file to verify
    /// * `expected_hash` - Expected SHA256 hash (hex-encoded, case-insensitive)
    /// 
    /// # Returns
    /// * `Ok(())` - Hash matches
    /// * `Err(UpdateError::HashMismatch)` - Hash does not match
    pub fn verify_sha256(file_path: &Path, expected_hash: &str) -> Result<(), UpdateError> {
        let actual_hash = Self::calculate_sha256(file_path)?;
        let expected_normalized = expected_hash.to_lowercase();

        if actual_hash != expected_normalized {
            return Err(UpdateError::HashMismatch {
                expected: expected_normalized,
                actual: actual_hash,
            });
        }

        Ok(())
    }

    /// Verify SHA256 hash and delete file on mismatch
    /// 
    /// Requirements: 3.3, 3.4
    /// - Verify the SHA256 hash matches the expected value
    /// - Delete the downloaded file if hash verification fails
    /// 
    /// # Arguments
    /// * `file_path` - Path to the file to verify
    /// * `expected_hash` - Expected SHA256 hash (hex-encoded, case-insensitive)
    /// 
    /// # Returns
    /// * `Ok(())` - Hash matches
    /// * `Err(UpdateError::HashMismatch)` - Hash does not match (file deleted)
    pub fn verify_sha256_and_cleanup(
        file_path: &Path,
        expected_hash: &str,
    ) -> Result<(), UpdateError> {
        match Self::verify_sha256(file_path, expected_hash) {
            Ok(()) => Ok(()),
            Err(e) => {
                // Delete the file on hash mismatch
                if let Err(delete_err) = std::fs::remove_file(file_path) {
                    log::warn!(
                        "[Verifier] Failed to delete file after hash mismatch: {}",
                        delete_err
                    );
                } else {
                    log::info!(
                        "[Verifier] Deleted file after hash mismatch: {:?}",
                        file_path
                    );
                }
                Err(e)
            }
        }
    }

    /// Verify Ed25519 signature of a file
    /// 
    /// Requirements: 3.5, 3.6
    /// - Verify the package signature using ed25519
    /// - Reject update if signature verification fails
    /// 
    /// # Arguments
    /// * `file_path` - Path to the file to verify
    /// * `signature_base64` - Base64-encoded Ed25519 signature
    /// * `public_key_base64` - Base64-encoded Ed25519 public key (32 bytes)
    /// 
    /// # Returns
    /// * `Ok(())` - Signature is valid
    /// * `Err(UpdateError::SignatureInvalid)` - Signature is invalid
    pub fn verify_signature(
        file_path: &Path,
        signature_base64: &str,
        public_key_base64: &str,
    ) -> Result<(), UpdateError> {
        // Read the file content
        let file_content = std::fs::read(file_path).map_err(|e| {
            UpdateError::FileSystem(format!("Failed to read file for signature verification: {}", e))
        })?;

        Self::verify_signature_bytes(&file_content, signature_base64, public_key_base64)
    }

    /// Verify Ed25519 signature of raw bytes
    /// 
    /// # Arguments
    /// * `data` - Byte slice to verify
    /// * `signature_base64` - Base64-encoded Ed25519 signature
    /// * `public_key_base64` - Base64-encoded Ed25519 public key (32 bytes)
    /// 
    /// # Returns
    /// * `Ok(())` - Signature is valid
    /// * `Err(UpdateError::SignatureInvalid)` - Signature is invalid
    pub fn verify_signature_bytes(
        data: &[u8],
        signature_base64: &str,
        public_key_base64: &str,
    ) -> Result<(), UpdateError> {
        // Decode public key
        let public_key_bytes = BASE64.decode(public_key_base64).map_err(|e| {
            UpdateError::SignatureInvalid(format!("Invalid public key base64: {}", e))
        })?;

        let public_key_array: [u8; 32] = public_key_bytes.try_into().map_err(|_| {
            UpdateError::SignatureInvalid("Invalid public key length (expected 32 bytes)".to_string())
        })?;

        let verifying_key = VerifyingKey::from_bytes(&public_key_array).map_err(|e| {
            UpdateError::SignatureInvalid(format!("Invalid public key: {}", e))
        })?;

        // Decode signature
        let signature_bytes = BASE64.decode(signature_base64).map_err(|e| {
            UpdateError::SignatureInvalid(format!("Invalid signature base64: {}", e))
        })?;

        let signature_array: [u8; 64] = signature_bytes.try_into().map_err(|_| {
            UpdateError::SignatureInvalid("Invalid signature length (expected 64 bytes)".to_string())
        })?;

        let signature = Signature::from_bytes(&signature_array);

        // Verify signature
        verifying_key.verify(data, &signature).map_err(|_| {
            UpdateError::SignatureInvalid("Signature verification failed".to_string())
        })?;

        Ok(())
    }
}

/// Encode bytes as lowercase hex string
fn hex_encode(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        result.push(HEX_CHARS[(byte >> 4) as usize] as char);
        result.push(HEX_CHARS[(byte & 0x0f) as usize] as char);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_calculate_sha256_bytes() {
        // Known SHA256 hash for "hello world"
        let data = b"hello world";
        let hash = Verifier::calculate_sha256_bytes(data);
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_calculate_sha256_empty() {
        // Known SHA256 hash for empty input
        let data = b"";
        let hash = Verifier::calculate_sha256_bytes(data);
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_calculate_sha256_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"hello world").unwrap();
        temp_file.flush().unwrap();

        let hash = Verifier::calculate_sha256(temp_file.path()).unwrap();
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_verify_sha256_success() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"hello world").unwrap();
        temp_file.flush().unwrap();

        let expected_hash = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        let result = Verifier::verify_sha256(temp_file.path(), expected_hash);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_sha256_case_insensitive() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"hello world").unwrap();
        temp_file.flush().unwrap();

        // Uppercase hash should also work
        let expected_hash = "B94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9";
        let result = Verifier::verify_sha256(temp_file.path(), expected_hash);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_sha256_mismatch() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"hello world").unwrap();
        temp_file.flush().unwrap();

        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let result = Verifier::verify_sha256(temp_file.path(), wrong_hash);
        
        assert!(result.is_err());
        match result {
            Err(UpdateError::HashMismatch { expected, actual }) => {
                assert_eq!(expected, wrong_hash);
                assert_eq!(
                    actual,
                    "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
                );
            }
            _ => panic!("Expected HashMismatch error"),
        }
    }

    #[test]
    fn test_verify_sha256_and_cleanup_success() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"hello world").unwrap();
        temp_file.flush().unwrap();

        let expected_hash = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        let path = temp_file.path().to_path_buf();
        
        let result = Verifier::verify_sha256_and_cleanup(&path, expected_hash);
        assert!(result.is_ok());
        
        // File should still exist after successful verification
        assert!(path.exists());
    }

    #[test]
    fn test_verify_sha256_and_cleanup_deletes_on_mismatch() {
        // Create a temp file that won't be auto-deleted
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("verifier_test_{}.tmp", std::process::id()));
        
        std::fs::write(&file_path, b"hello world").unwrap();
        assert!(file_path.exists());

        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let result = Verifier::verify_sha256_and_cleanup(&file_path, wrong_hash);
        
        assert!(result.is_err());
        // File should be deleted after hash mismatch
        assert!(!file_path.exists());
    }

    #[test]
    fn test_verify_signature_success() {
        // Generate a keypair
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        // Sign some data
        let data = b"test data for signing";
        let signature = signing_key.sign(data);

        // Encode keys and signature
        let public_key_base64 = BASE64.encode(verifying_key.to_bytes());
        let signature_base64 = BASE64.encode(signature.to_bytes());

        // Verify
        let result = Verifier::verify_signature_bytes(data, &signature_base64, &public_key_base64);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_signature_wrong_key() {
        // Generate two keypairs
        let mut csprng = OsRng;
        let signing_key1 = SigningKey::generate(&mut csprng);
        let signing_key2 = SigningKey::generate(&mut csprng);
        let verifying_key2 = signing_key2.verifying_key();

        // Sign with key1
        let data = b"test data for signing";
        let signature = signing_key1.sign(data);

        // Try to verify with key2
        let public_key_base64 = BASE64.encode(verifying_key2.to_bytes());
        let signature_base64 = BASE64.encode(signature.to_bytes());

        let result = Verifier::verify_signature_bytes(data, &signature_base64, &public_key_base64);
        assert!(result.is_err());
        match result {
            Err(UpdateError::SignatureInvalid(_)) => {}
            _ => panic!("Expected SignatureInvalid error"),
        }
    }

    #[test]
    fn test_verify_signature_tampered_data() {
        // Generate a keypair
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        // Sign original data
        let original_data = b"original data";
        let signature = signing_key.sign(original_data);

        // Try to verify with tampered data
        let tampered_data = b"tampered data";
        let public_key_base64 = BASE64.encode(verifying_key.to_bytes());
        let signature_base64 = BASE64.encode(signature.to_bytes());

        let result = Verifier::verify_signature_bytes(tampered_data, &signature_base64, &public_key_base64);
        assert!(result.is_err());
        match result {
            Err(UpdateError::SignatureInvalid(_)) => {}
            _ => panic!("Expected SignatureInvalid error"),
        }
    }

    #[test]
    fn test_verify_signature_file() {
        // Generate a keypair
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        // Create a temp file
        let mut temp_file = NamedTempFile::new().unwrap();
        let data = b"file content for signing";
        temp_file.write_all(data).unwrap();
        temp_file.flush().unwrap();

        // Sign the file content
        let signature = signing_key.sign(data);

        // Encode keys and signature
        let public_key_base64 = BASE64.encode(verifying_key.to_bytes());
        let signature_base64 = BASE64.encode(signature.to_bytes());

        // Verify
        let result = Verifier::verify_signature(temp_file.path(), &signature_base64, &public_key_base64);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_signature_invalid_base64() {
        let result = Verifier::verify_signature_bytes(
            b"data",
            "not-valid-base64!!!",
            "also-not-valid!!!",
        );
        assert!(result.is_err());
        match result {
            Err(UpdateError::SignatureInvalid(msg)) => {
                assert!(msg.contains("Invalid public key base64"));
            }
            _ => panic!("Expected SignatureInvalid error"),
        }
    }

    #[test]
    fn test_verify_signature_invalid_key_length() {
        let result = Verifier::verify_signature_bytes(
            b"data",
            &BASE64.encode([0u8; 64]), // Valid signature length
            &BASE64.encode([0u8; 16]), // Invalid key length (should be 32)
        );
        assert!(result.is_err());
        match result {
            Err(UpdateError::SignatureInvalid(msg)) => {
                assert!(msg.contains("Invalid public key length"));
            }
            _ => panic!("Expected SignatureInvalid error"),
        }
    }

    #[test]
    fn test_verify_signature_invalid_signature_length() {
        let result = Verifier::verify_signature_bytes(
            b"data",
            &BASE64.encode([0u8; 32]), // Invalid signature length (should be 64)
            &BASE64.encode([0u8; 32]), // Valid key length
        );
        assert!(result.is_err());
        match result {
            Err(UpdateError::SignatureInvalid(msg)) => {
                // Could be either invalid key or invalid signature length
                assert!(msg.contains("Invalid"));
            }
            _ => panic!("Expected SignatureInvalid error"),
        }
    }

    #[test]
    fn test_hex_encode() {
        assert_eq!(hex_encode(&[0x00]), "00");
        assert_eq!(hex_encode(&[0xff]), "ff");
        assert_eq!(hex_encode(&[0xab, 0xcd]), "abcd");
        assert_eq!(hex_encode(&[0x12, 0x34, 0x56, 0x78]), "12345678");
        assert_eq!(hex_encode(&[]), "");
    }

    #[test]
    fn test_calculate_sha256_nonexistent_file() {
        let result = Verifier::calculate_sha256(Path::new("/nonexistent/file.txt"));
        assert!(result.is_err());
        match result {
            Err(UpdateError::FileSystem(msg)) => {
                assert!(msg.contains("Failed to open file"));
            }
            _ => panic!("Expected FileSystem error"),
        }
    }

    #[test]
    fn test_sha256_hash_roundtrip() {
        // Property: For any data, calculating hash and verifying against it should succeed
        let test_cases = vec![
            b"".to_vec(),
            b"a".to_vec(),
            b"hello world".to_vec(),
            vec![0u8; 1000],
            (0..256).map(|i| i as u8).collect::<Vec<_>>(),
        ];

        for data in test_cases {
            let mut temp_file = NamedTempFile::new().unwrap();
            temp_file.write_all(&data).unwrap();
            temp_file.flush().unwrap();

            let hash = Verifier::calculate_sha256(temp_file.path()).unwrap();
            let result = Verifier::verify_sha256(temp_file.path(), &hash);
            assert!(result.is_ok(), "Hash roundtrip failed for data of length {}", data.len());
        }
    }
}
