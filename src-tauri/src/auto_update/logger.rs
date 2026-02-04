// Update Logger - Dedicated logging for Auto-Update System
// Writes update operations to a dedicated log file with rotation support
// Requirements: 13.1, 13.2, 13.3, 13.4, 13.5

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum log file size before rotation (10MB)
const MAX_LOG_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Number of rotated log files to keep
const MAX_ROTATED_FILES: usize = 5;

/// Log level for update operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// A single log entry for update operations
/// Requirements: 13.1, 13.2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateLogEntry {
    /// Timestamp in seconds since UNIX epoch
    pub timestamp: u64,
    /// Log level
    pub level: LogLevel,
    /// Current update state when log was written
    pub state: String,
    /// Log message
    pub message: String,
    /// Optional additional details (JSON)
    pub details: Option<serde_json::Value>,
}

impl UpdateLogEntry {
    /// Create a new log entry with current timestamp
    pub fn new(level: LogLevel, state: &str, message: &str) -> Self {
        Self {
            timestamp: Self::current_timestamp(),
            level,
            state: state.to_string(),
            message: message.to_string(),
            details: None,
        }
    }

    /// Create a new log entry with details
    pub fn with_details(level: LogLevel, state: &str, message: &str, details: serde_json::Value) -> Self {
        Self {
            timestamp: Self::current_timestamp(),
            level,
            state: state.to_string(),
            message: message.to_string(),
            details: Some(details),
        }
    }

    /// Get current timestamp in seconds since UNIX epoch
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Format the log entry as a human-readable string
    pub fn format(&self) -> String {
        let datetime = Self::format_timestamp(self.timestamp);
        let details_str = self.details
            .as_ref()
            .map(|d| format!(" | {}", d))
            .unwrap_or_default();
        
        format!(
            "[{}] [{}] [{}] {}{}",
            datetime,
            self.level,
            self.state,
            self.message,
            details_str
        )
    }

    /// Format timestamp as ISO 8601 string
    fn format_timestamp(timestamp: u64) -> String {
        // Simple ISO 8601 format without external dependencies
        let secs_per_day = 86400u64;
        let secs_per_hour = 3600u64;
        let secs_per_min = 60u64;

        // Days since epoch (1970-01-01)
        let days = timestamp / secs_per_day;
        let remaining = timestamp % secs_per_day;
        
        let hours = remaining / secs_per_hour;
        let remaining = remaining % secs_per_hour;
        
        let minutes = remaining / secs_per_min;
        let seconds = remaining % secs_per_min;

        // Calculate year, month, day from days since epoch
        let (year, month, day) = Self::days_to_ymd(days);

        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            year, month, day, hours, minutes, seconds
        )
    }

    /// Convert days since epoch to year, month, day
    fn days_to_ymd(days: u64) -> (u32, u32, u32) {
        let mut remaining_days = days as i64;
        let mut year = 1970i32;

        // Find the year
        loop {
            let days_in_year = if Self::is_leap_year(year) { 366 } else { 365 };
            if remaining_days < days_in_year {
                break;
            }
            remaining_days -= days_in_year;
            year += 1;
        }

        // Find the month and day
        let days_in_months: [i64; 12] = if Self::is_leap_year(year) {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };

        let mut month = 1u32;
        for days_in_month in days_in_months.iter() {
            if remaining_days < *days_in_month {
                break;
            }
            remaining_days -= days_in_month;
            month += 1;
        }

        let day = remaining_days as u32 + 1;

        (year as u32, month, day)
    }

    /// Check if a year is a leap year
    fn is_leap_year(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }
}


/// Update Logger - writes update operations to a dedicated log file
/// Requirements: 13.1, 13.2, 13.5
pub struct UpdateLogger {
    /// Path to the log file
    log_file_path: PathBuf,
    /// Mutex for thread-safe file access
    file_lock: Mutex<()>,
    /// Last progress percentage logged (for 10% interval tracking)
    last_progress_logged: Mutex<u8>,
    /// Last progress log timestamp (for 30 second interval tracking)
    last_progress_time: Mutex<u64>,
}

impl UpdateLogger {
    /// Create a new UpdateLogger
    ///
    /// # Arguments
    /// * `log_dir` - Directory where log files will be stored
    pub fn new(log_dir: &Path) -> std::io::Result<Self> {
        // Ensure log directory exists
        fs::create_dir_all(log_dir)?;

        let log_file_path = log_dir.join("update.log");

        Ok(Self {
            log_file_path,
            file_lock: Mutex::new(()),
            last_progress_logged: Mutex::new(0),
            last_progress_time: Mutex::new(0),
        })
    }

    /// Get the log file path
    pub fn log_file_path(&self) -> &Path {
        &self.log_file_path
    }

    /// Log an entry to the file
    /// Requirements: 13.1
    pub fn log(&self, entry: &UpdateLogEntry) -> std::io::Result<()> {
        let _lock = self.file_lock.lock().unwrap();

        // Check if rotation is needed before writing
        self.rotate_if_needed()?;

        // Open file in append mode
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path)?;

        let mut writer = BufWriter::new(file);
        writeln!(writer, "{}", entry.format())?;
        writer.flush()?;

        Ok(())
    }

    /// Log an info message
    pub fn info(&self, state: &str, message: &str) -> std::io::Result<()> {
        let entry = UpdateLogEntry::new(LogLevel::Info, state, message);
        self.log(&entry)
    }

    /// Log a warning message
    pub fn warn(&self, state: &str, message: &str) -> std::io::Result<()> {
        let entry = UpdateLogEntry::new(LogLevel::Warn, state, message);
        self.log(&entry)
    }

    /// Log an error message
    /// Requirements: 13.2
    pub fn error(&self, state: &str, message: &str) -> std::io::Result<()> {
        let entry = UpdateLogEntry::new(LogLevel::Error, state, message);
        self.log(&entry)
    }

    /// Log an error with details
    /// Requirements: 13.2
    pub fn error_with_details(&self, state: &str, message: &str, details: serde_json::Value) -> std::io::Result<()> {
        let entry = UpdateLogEntry::with_details(LogLevel::Error, state, message, details);
        self.log(&entry)
    }

    /// Log a debug message
    pub fn debug(&self, state: &str, message: &str) -> std::io::Result<()> {
        let entry = UpdateLogEntry::new(LogLevel::Debug, state, message);
        self.log(&entry)
    }

    /// Log download progress at regular intervals (every 10% or 30 seconds)
    /// Requirements: 13.3
    pub fn log_progress(&self, state: &str, progress_percent: f32, bytes_downloaded: u64, total_bytes: u64) -> std::io::Result<()> {
        let current_percent = progress_percent as u8;
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut last_percent = self.last_progress_logged.lock().unwrap();
        let mut last_time = self.last_progress_time.lock().unwrap();

        // Check if we should log (10% interval or 30 seconds elapsed)
        let percent_threshold = current_percent >= *last_percent + 10 || current_percent < *last_percent;
        let time_threshold = current_time >= *last_time + 30;

        if percent_threshold || time_threshold {
            *last_percent = current_percent;
            *last_time = current_time;

            let message = format!(
                "Download progress: {}% ({} / {} bytes)",
                current_percent, bytes_downloaded, total_bytes
            );
            
            drop(last_percent);
            drop(last_time);
            
            self.info(state, &message)?;
        }

        Ok(())
    }

    /// Log a successful update completion
    /// Requirements: 13.4
    pub fn log_update_complete(&self, old_version: &str, new_version: &str, duration_secs: u64) -> std::io::Result<()> {
        let details = serde_json::json!({
            "old_version": old_version,
            "new_version": new_version,
            "duration_seconds": duration_secs
        });

        let message = format!(
            "Update completed successfully: {} -> {} (took {} seconds)",
            old_version, new_version, duration_secs
        );

        let entry = UpdateLogEntry::with_details(LogLevel::Info, "Done", &message, details);
        self.log(&entry)
    }

    /// Log a state transition
    pub fn log_state_transition(&self, from_state: &str, to_state: &str) -> std::io::Result<()> {
        let message = format!("State transition: {} -> {}", from_state, to_state);
        self.info(to_state, &message)
    }

    /// Reset progress tracking (call when starting a new download)
    pub fn reset_progress_tracking(&self) {
        *self.last_progress_logged.lock().unwrap() = 0;
        *self.last_progress_time.lock().unwrap() = 0;
    }

    /// Check file size and rotate if needed
    /// Requirements: 13.5
    fn rotate_if_needed(&self) -> std::io::Result<()> {
        if !self.log_file_path.exists() {
            return Ok(());
        }

        let metadata = fs::metadata(&self.log_file_path)?;
        if metadata.len() >= MAX_LOG_FILE_SIZE {
            self.rotate_logs()?;
        }

        Ok(())
    }

    /// Rotate log files
    /// Requirements: 13.5
    fn rotate_logs(&self) -> std::io::Result<()> {
        let log_dir = self.log_file_path.parent().unwrap_or(Path::new("."));
        let base_name = self.log_file_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("update");
        let extension = self.log_file_path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("log");

        // Delete the oldest file if we're at the limit
        let oldest_path = log_dir.join(format!("{}.{}.{}", base_name, MAX_ROTATED_FILES, extension));
        if oldest_path.exists() {
            fs::remove_file(&oldest_path)?;
        }

        // Shift existing rotated files
        for i in (1..MAX_ROTATED_FILES).rev() {
            let old_path = log_dir.join(format!("{}.{}.{}", base_name, i, extension));
            let new_path = log_dir.join(format!("{}.{}.{}", base_name, i + 1, extension));
            if old_path.exists() {
                fs::rename(&old_path, &new_path)?;
            }
        }

        // Rename current log file to .1
        let rotated_path = log_dir.join(format!("{}.1.{}", base_name, extension));
        fs::rename(&self.log_file_path, &rotated_path)?;

        Ok(())
    }

    /// Get the current log file size in bytes
    pub fn current_file_size(&self) -> std::io::Result<u64> {
        if self.log_file_path.exists() {
            let metadata = fs::metadata(&self.log_file_path)?;
            Ok(metadata.len())
        } else {
            Ok(0)
        }
    }

    /// List all log files (current + rotated)
    pub fn list_log_files(&self) -> std::io::Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        
        if self.log_file_path.exists() {
            files.push(self.log_file_path.clone());
        }

        let log_dir = self.log_file_path.parent().unwrap_or(Path::new("."));
        let base_name = self.log_file_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("update");
        let extension = self.log_file_path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("log");

        for i in 1..=MAX_ROTATED_FILES {
            let rotated_path = log_dir.join(format!("{}.{}.{}", base_name, i, extension));
            if rotated_path.exists() {
                files.push(rotated_path);
            }
        }

        Ok(files)
    }

    /// Clean up all log files
    pub fn cleanup_all(&self) -> std::io::Result<()> {
        let files = self.list_log_files()?;
        for file in files {
            fs::remove_file(file)?;
        }
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    // Atomic counter for unique test directories
    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn create_test_logger() -> (UpdateLogger, PathBuf) {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "update_logger_test_{}_{}", 
            std::process::id(),
            counter
        ));
        let _ = fs::remove_dir_all(&temp_dir); // Clean up any existing
        let _ = fs::create_dir_all(&temp_dir);
        let logger = UpdateLogger::new(&temp_dir).unwrap();
        (logger, temp_dir)
    }

    fn cleanup_test_dir(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_log_entry_creation() {
        let entry = UpdateLogEntry::new(LogLevel::Info, "Checking", "Checking for updates");
        
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.state, "Checking");
        assert_eq!(entry.message, "Checking for updates");
        assert!(entry.details.is_none());
        assert!(entry.timestamp > 0);
    }

    #[test]
    fn test_log_entry_with_details() {
        let details = serde_json::json!({"version": "1.2.0"});
        let entry = UpdateLogEntry::with_details(
            LogLevel::Error,
            "Failed",
            "Download failed",
            details.clone()
        );
        
        assert_eq!(entry.level, LogLevel::Error);
        assert!(entry.details.is_some());
        assert_eq!(entry.details.unwrap(), details);
    }

    #[test]
    fn test_log_entry_format() {
        let entry = UpdateLogEntry {
            timestamp: 1704067200, // 2024-01-01 00:00:00 UTC
            level: LogLevel::Info,
            state: "Idle".to_string(),
            message: "Test message".to_string(),
            details: None,
        };
        
        let formatted = entry.format();
        assert!(formatted.contains("[INFO]"));
        assert!(formatted.contains("[Idle]"));
        assert!(formatted.contains("Test message"));
        assert!(formatted.contains("2024-01-01"));
    }

    #[test]
    fn test_log_entry_format_with_details() {
        let entry = UpdateLogEntry {
            timestamp: 1704067200,
            level: LogLevel::Error,
            state: "Failed".to_string(),
            message: "Error occurred".to_string(),
            details: Some(serde_json::json!({"code": 500})),
        };
        
        let formatted = entry.format();
        assert!(formatted.contains("[ERROR]"));
        assert!(formatted.contains("500"));
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(format!("{}", LogLevel::Debug), "DEBUG");
        assert_eq!(format!("{}", LogLevel::Info), "INFO");
        assert_eq!(format!("{}", LogLevel::Warn), "WARN");
        assert_eq!(format!("{}", LogLevel::Error), "ERROR");
    }

    #[test]
    fn test_logger_creation() {
        let (logger, temp_dir) = create_test_logger();
        
        assert!(logger.log_file_path().parent().unwrap().exists());
        assert!(logger.log_file_path().ends_with("update.log"));
        
        cleanup_test_dir(&temp_dir);
    }

    #[test]
    fn test_logger_write_entry() {
        let (logger, temp_dir) = create_test_logger();
        
        let entry = UpdateLogEntry::new(LogLevel::Info, "Idle", "Test log entry");
        logger.log(&entry).unwrap();
        
        assert!(logger.log_file_path().exists());
        let content = fs::read_to_string(logger.log_file_path()).unwrap();
        assert!(content.contains("Test log entry"));
        assert!(content.contains("[INFO]"));
        
        cleanup_test_dir(&temp_dir);
    }

    #[test]
    fn test_logger_convenience_methods() {
        let (logger, temp_dir) = create_test_logger();
        
        logger.info("Checking", "Info message").unwrap();
        logger.warn("Downloading", "Warning message").unwrap();
        logger.error("Failed", "Error message").unwrap();
        logger.debug("Idle", "Debug message").unwrap();
        
        let content = fs::read_to_string(logger.log_file_path()).unwrap();
        assert!(content.contains("[INFO]"));
        assert!(content.contains("[WARN]"));
        assert!(content.contains("[ERROR]"));
        assert!(content.contains("[DEBUG]"));
        
        cleanup_test_dir(&temp_dir);
    }

    #[test]
    fn test_logger_error_with_details() {
        let (logger, temp_dir) = create_test_logger();
        
        let details = serde_json::json!({
            "error_code": 404,
            "url": "https://example.com/update"
        });
        logger.error_with_details("Failed", "Download failed", details).unwrap();
        
        let content = fs::read_to_string(logger.log_file_path()).unwrap();
        assert!(content.contains("404"));
        assert!(content.contains("example.com"));
        
        cleanup_test_dir(&temp_dir);
    }

    #[test]
    fn test_logger_progress_10_percent_interval() {
        let (logger, temp_dir) = create_test_logger();
        
        // Reset tracking
        logger.reset_progress_tracking();
        
        // Log at 0%
        logger.log_progress("Downloading", 0.0, 0, 1000).unwrap();
        
        // Log at 5% - should NOT log (less than 10% change)
        logger.log_progress("Downloading", 5.0, 50, 1000).unwrap();
        
        // Log at 10% - should log
        logger.log_progress("Downloading", 10.0, 100, 1000).unwrap();
        
        // Log at 15% - should NOT log
        logger.log_progress("Downloading", 15.0, 150, 1000).unwrap();
        
        // Log at 20% - should log
        logger.log_progress("Downloading", 20.0, 200, 1000).unwrap();
        
        let content = fs::read_to_string(logger.log_file_path()).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        
        // Should have 3 entries: 0%, 10%, 20%
        assert_eq!(lines.len(), 3);
        assert!(content.contains("0%"));
        assert!(content.contains("10%"));
        assert!(content.contains("20%"));
        
        cleanup_test_dir(&temp_dir);
    }

    #[test]
    fn test_logger_update_complete() {
        let (logger, temp_dir) = create_test_logger();
        
        logger.log_update_complete("1.0.0", "1.1.0", 120).unwrap();
        
        let content = fs::read_to_string(logger.log_file_path()).unwrap();
        assert!(content.contains("1.0.0"));
        assert!(content.contains("1.1.0"));
        assert!(content.contains("120"));
        assert!(content.contains("completed successfully"));
        
        cleanup_test_dir(&temp_dir);
    }

    #[test]
    fn test_logger_state_transition() {
        let (logger, temp_dir) = create_test_logger();
        
        logger.log_state_transition("Idle", "Checking").unwrap();
        
        let content = fs::read_to_string(logger.log_file_path()).unwrap();
        assert!(content.contains("Idle"));
        assert!(content.contains("Checking"));
        assert!(content.contains("State transition"));
        
        cleanup_test_dir(&temp_dir);
    }

    #[test]
    fn test_logger_file_size() {
        let (logger, temp_dir) = create_test_logger();
        
        // Initially 0
        assert_eq!(logger.current_file_size().unwrap(), 0);
        
        // Write some entries
        logger.info("Idle", "Test message 1").unwrap();
        logger.info("Idle", "Test message 2").unwrap();
        
        // Should be > 0 now
        assert!(logger.current_file_size().unwrap() > 0);
        
        cleanup_test_dir(&temp_dir);
    }

    #[test]
    fn test_logger_list_files() {
        let (logger, temp_dir) = create_test_logger();
        
        // Write to create the file
        logger.info("Idle", "Test").unwrap();
        
        let files = logger.list_log_files().unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("update.log"));
        
        cleanup_test_dir(&temp_dir);
    }

    #[test]
    fn test_logger_cleanup() {
        let (logger, temp_dir) = create_test_logger();
        
        // Write to create the file
        logger.info("Idle", "Test").unwrap();
        assert!(logger.log_file_path().exists());
        
        // Cleanup
        logger.cleanup_all().unwrap();
        assert!(!logger.log_file_path().exists());
        
        cleanup_test_dir(&temp_dir);
    }

    #[test]
    fn test_log_rotation() {
        let (logger, temp_dir) = create_test_logger();
        
        // Write enough data to trigger rotation
        // We'll manually trigger rotation for testing
        logger.info("Idle", "Initial entry").unwrap();
        
        // Manually call rotate
        {
            let _lock = logger.file_lock.lock().unwrap();
            logger.rotate_logs().unwrap();
        }
        
        // Write new entry to new log file
        logger.info("Idle", "After rotation").unwrap();
        
        let files = logger.list_log_files().unwrap();
        assert_eq!(files.len(), 2);
        
        // Check that rotated file exists
        let rotated_path = temp_dir.join("update.1.log");
        assert!(rotated_path.exists());
        
        // Check content of rotated file
        let rotated_content = fs::read_to_string(&rotated_path).unwrap();
        assert!(rotated_content.contains("Initial entry"));
        
        // Check content of current file
        let current_content = fs::read_to_string(logger.log_file_path()).unwrap();
        assert!(current_content.contains("After rotation"));
        
        cleanup_test_dir(&temp_dir);
    }

    #[test]
    fn test_log_rotation_keeps_max_files() {
        let (logger, temp_dir) = create_test_logger();
        
        // Create more than MAX_ROTATED_FILES rotations
        for i in 0..=MAX_ROTATED_FILES + 2 {
            logger.info("Idle", &format!("Entry {}", i)).unwrap();
            {
                let _lock = logger.file_lock.lock().unwrap();
                let _ = logger.rotate_logs();
            }
        }
        
        // Write final entry
        logger.info("Idle", "Final entry").unwrap();
        
        let files = logger.list_log_files().unwrap();
        
        // Should have at most MAX_ROTATED_FILES + 1 (current + rotated)
        assert!(files.len() <= MAX_ROTATED_FILES + 1);
        
        cleanup_test_dir(&temp_dir);
    }

    #[test]
    fn test_timestamp_formatting() {
        // Test known timestamp: 2024-01-01 00:00:00 UTC
        let formatted = UpdateLogEntry::format_timestamp(1704067200);
        assert_eq!(formatted, "2024-01-01T00:00:00Z");
        
        // Test another known timestamp: 2024-06-15 11:30:45 UTC
        // 1718451045 = 2024-06-15T11:30:45Z
        let formatted2 = UpdateLogEntry::format_timestamp(1718451045);
        assert_eq!(formatted2, "2024-06-15T11:30:45Z");
    }

    #[test]
    fn test_leap_year_detection() {
        assert!(UpdateLogEntry::is_leap_year(2000)); // Divisible by 400
        assert!(UpdateLogEntry::is_leap_year(2024)); // Divisible by 4, not by 100
        assert!(!UpdateLogEntry::is_leap_year(1900)); // Divisible by 100, not by 400
        assert!(!UpdateLogEntry::is_leap_year(2023)); // Not divisible by 4
    }

    #[test]
    fn test_days_to_ymd() {
        // 1970-01-01 is day 0
        assert_eq!(UpdateLogEntry::days_to_ymd(0), (1970, 1, 1));
        
        // 1970-01-02 is day 1
        assert_eq!(UpdateLogEntry::days_to_ymd(1), (1970, 1, 2));
        
        // 1970-02-01 is day 31
        assert_eq!(UpdateLogEntry::days_to_ymd(31), (1970, 2, 1));
        
        // 1971-01-01 is day 365
        assert_eq!(UpdateLogEntry::days_to_ymd(365), (1971, 1, 1));
        
        // 2024-01-01 (leap year)
        // Days from 1970 to 2024: 54 years
        // Leap years: 1972, 1976, 1980, 1984, 1988, 1992, 1996, 2000, 2004, 2008, 2012, 2016, 2020 = 13
        // Regular years: 54 - 13 = 41
        // Total days: 13 * 366 + 41 * 365 = 4758 + 14965 = 19723
        assert_eq!(UpdateLogEntry::days_to_ymd(19723), (2024, 1, 1));
    }

    #[test]
    fn test_log_entry_serialization() {
        let entry = UpdateLogEntry::new(LogLevel::Info, "Checking", "Test message");
        
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: UpdateLogEntry = serde_json::from_str(&json).unwrap();
        
        assert_eq!(entry.level, deserialized.level);
        assert_eq!(entry.state, deserialized.state);
        assert_eq!(entry.message, deserialized.message);
    }

    #[test]
    fn test_log_level_serialization() {
        let levels = vec![LogLevel::Debug, LogLevel::Info, LogLevel::Warn, LogLevel::Error];
        
        for level in levels {
            let json = serde_json::to_string(&level).unwrap();
            let deserialized: LogLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(level, deserialized);
        }
    }
}
