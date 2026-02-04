//! Version Handshake Protocol
//!
//! This module implements the version handshake protocol between Teacher and Student apps.
//! When a Student connects to a Teacher, they exchange version information to determine
//! if the Student needs to update before accessing full functionality.
//!
//! Requirements: 5.1, 5.2, 5.3

use serde::{Deserialize, Serialize};

/// Version handshake request sent from Student to Teacher
/// Requirements: 5.1 - WHEN a Student_App connects to Teacher_App, THE Student_App SHALL send its current_version
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionHandshakeRequest {
    /// Unique identifier for the student (machine ID or generated UUID)
    pub student_id: String,
    /// Current version of the Student app (semver format)
    pub current_version: String,
    /// Human-readable machine name for display
    pub machine_name: String,
}

impl VersionHandshakeRequest {
    /// Create a new handshake request
    pub fn new(student_id: String, current_version: String, machine_name: String) -> Self {
        Self {
            student_id,
            current_version,
            machine_name,
        }
    }
}

/// Version handshake response sent from Teacher to Student
/// Requirements: 5.2, 5.3 - THE Teacher_App SHALL respond with required_version and mandatory_update flag
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionHandshakeResponse {
    /// The Teacher's current version (which Students must match)
    pub required_version: String,
    /// Whether the Student must update before accessing functionality
    pub mandatory_update: bool,
    /// URL to download the update from the Teacher's LAN server (if update required)
    pub update_url: Option<String>,
    /// SHA256 hash of the update package (for verification)
    pub sha256: Option<String>,
}

impl VersionHandshakeResponse {
    /// Create a response indicating versions match (no update required)
    pub fn versions_match(teacher_version: String) -> Self {
        Self {
            required_version: teacher_version,
            mandatory_update: false,
            update_url: None,
            sha256: None,
        }
    }

    /// Create a response indicating update is required
    pub fn update_required(
        teacher_version: String,
        update_url: Option<String>,
        sha256: Option<String>,
    ) -> Self {
        Self {
            required_version: teacher_version,
            mandatory_update: true,
            update_url,
            sha256,
        }
    }
}

/// Check version compatibility between Student and Teacher
///
/// Requirements:
/// - 5.2: THE Teacher_App SHALL respond with required_version (Teacher's current version)
/// - 5.3: WHEN Student version does not match required_version, THE Teacher_App SHALL set mandatory_update to true
///
/// # Arguments
/// * `student_version` - The Student's current version string
/// * `teacher_version` - The Teacher's current version string
///
/// # Returns
/// A `VersionHandshakeResponse` with `mandatory_update` set to true if versions differ
pub fn check_version_compatibility(
    student_version: &str,
    teacher_version: &str,
) -> VersionHandshakeResponse {
    let versions_match = student_version == teacher_version;

    VersionHandshakeResponse {
        required_version: teacher_version.to_string(),
        mandatory_update: !versions_match,
        update_url: None,
        sha256: None,
    }
}

/// Check version compatibility with update URL and hash
///
/// This is the full version that includes LAN distribution information
/// when an update is required.
///
/// # Arguments
/// * `student_version` - The Student's current version string
/// * `teacher_version` - The Teacher's current version string
/// * `update_url` - Optional URL to download update from Teacher's LAN server
/// * `sha256` - Optional SHA256 hash of the update package
///
/// # Returns
/// A `VersionHandshakeResponse` with full update information if versions differ
pub fn check_version_compatibility_with_update(
    student_version: &str,
    teacher_version: &str,
    update_url: Option<String>,
    sha256: Option<String>,
) -> VersionHandshakeResponse {
    let versions_match = student_version == teacher_version;

    if versions_match {
        VersionHandshakeResponse::versions_match(teacher_version.to_string())
    } else {
        VersionHandshakeResponse::update_required(teacher_version.to_string(), update_url, sha256)
    }
}

/// Parse a semver version string into components
/// Returns (major, minor, patch) or None if parsing fails
pub fn parse_semver(version: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    let major = parts[0].parse().ok()?;
    let minor = parts[1].parse().ok()?;
    let patch = parts[2].parse().ok()?;

    Some((major, minor, patch))
}

/// Compare two semver versions
/// Returns:
/// - `Ordering::Less` if v1 < v2
/// - `Ordering::Equal` if v1 == v2
/// - `Ordering::Greater` if v1 > v2
/// - Falls back to string comparison if parsing fails
pub fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    match (parse_semver(v1), parse_semver(v2)) {
        (Some((maj1, min1, pat1)), Some((maj2, min2, pat2))) => {
            (maj1, min1, pat1).cmp(&(maj2, min2, pat2))
        }
        _ => v1.cmp(v2),
    }
}

/// Check if a version is older than another
pub fn is_version_older(current: &str, required: &str) -> bool {
    compare_versions(current, required) == std::cmp::Ordering::Less
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_request_creation() {
        let request = VersionHandshakeRequest::new(
            "student-123".to_string(),
            "1.0.0".to_string(),
            "Lab-PC-01".to_string(),
        );

        assert_eq!(request.student_id, "student-123");
        assert_eq!(request.current_version, "1.0.0");
        assert_eq!(request.machine_name, "Lab-PC-01");
    }

    #[test]
    fn test_handshake_response_versions_match() {
        let response = VersionHandshakeResponse::versions_match("1.2.0".to_string());

        assert_eq!(response.required_version, "1.2.0");
        assert!(!response.mandatory_update);
        assert!(response.update_url.is_none());
        assert!(response.sha256.is_none());
    }

    #[test]
    fn test_handshake_response_update_required() {
        let response = VersionHandshakeResponse::update_required(
            "1.2.0".to_string(),
            Some("http://192.168.1.1:9280/update".to_string()),
            Some("abc123".to_string()),
        );

        assert_eq!(response.required_version, "1.2.0");
        assert!(response.mandatory_update);
        assert_eq!(
            response.update_url,
            Some("http://192.168.1.1:9280/update".to_string())
        );
        assert_eq!(response.sha256, Some("abc123".to_string()));
    }

    #[test]
    fn test_check_version_compatibility_matching() {
        let response = check_version_compatibility("1.0.0", "1.0.0");

        assert_eq!(response.required_version, "1.0.0");
        assert!(!response.mandatory_update);
    }

    #[test]
    fn test_check_version_compatibility_mismatch() {
        let response = check_version_compatibility("1.0.0", "1.1.0");

        assert_eq!(response.required_version, "1.1.0");
        assert!(response.mandatory_update);
    }

    #[test]
    fn test_check_version_compatibility_student_newer() {
        // Even if student is newer, they must match exactly
        let response = check_version_compatibility("1.2.0", "1.1.0");

        assert_eq!(response.required_version, "1.1.0");
        assert!(response.mandatory_update);
    }

    #[test]
    fn test_check_version_compatibility_with_update() {
        let response = check_version_compatibility_with_update(
            "1.0.0",
            "1.1.0",
            Some("http://192.168.1.1:9280/update".to_string()),
            Some("sha256hash".to_string()),
        );

        assert_eq!(response.required_version, "1.1.0");
        assert!(response.mandatory_update);
        assert!(response.update_url.is_some());
        assert!(response.sha256.is_some());
    }

    #[test]
    fn test_parse_semver_valid() {
        assert_eq!(parse_semver("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver("0.0.0"), Some((0, 0, 0)));
        assert_eq!(parse_semver("10.20.30"), Some((10, 20, 30)));
    }

    #[test]
    fn test_parse_semver_invalid() {
        assert_eq!(parse_semver("1.2"), None);
        assert_eq!(parse_semver("1.2.3.4"), None);
        assert_eq!(parse_semver("abc"), None);
        assert_eq!(parse_semver("1.a.3"), None);
    }

    #[test]
    fn test_compare_versions() {
        use std::cmp::Ordering;

        assert_eq!(compare_versions("1.0.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0", "1.0.1"), Ordering::Less);
        assert_eq!(compare_versions("1.0.1", "1.0.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0", "2.0.0"), Ordering::Less);
        assert_eq!(compare_versions("1.1.0", "1.0.0"), Ordering::Greater);
    }

    #[test]
    fn test_is_version_older() {
        assert!(is_version_older("1.0.0", "1.0.1"));
        assert!(is_version_older("1.0.0", "2.0.0"));
        assert!(!is_version_older("1.0.0", "1.0.0"));
        assert!(!is_version_older("1.0.1", "1.0.0"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let request = VersionHandshakeRequest::new(
            "student-123".to_string(),
            "1.0.0".to_string(),
            "Lab-PC-01".to_string(),
        );

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: VersionHandshakeRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request, deserialized);
    }

    #[test]
    fn test_response_serialization_roundtrip() {
        let response = VersionHandshakeResponse::update_required(
            "1.2.0".to_string(),
            Some("http://localhost:9280/update".to_string()),
            Some("abc123def456".to_string()),
        );

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: VersionHandshakeResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(response, deserialized);
    }
}
