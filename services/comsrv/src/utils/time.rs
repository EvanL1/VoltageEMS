//! Time Utilities
//!
//! This module provides common time-related utilities for consistent time handling
//! across the communication service.
//!
//! # Features
//!
//! - Standardized timestamp formatting
//! - Common time zone handling
//! - Duration utilities
//! - Timeout helpers
//!
//! # Examples
//!
//! ```rust
//! use comsrv::utils::time::{current_timestamp, format_log_timestamp, sleep_ms};
//!
//! // Get current timestamp in RFC3339 format
//! let timestamp = current_timestamp();
//!
//! // Format timestamp for logging
//! let log_time = format_log_timestamp();
//!
//! // Sleep with milliseconds
//! sleep_ms(100).await;
//! ```

use chrono::{DateTime, Utc};
use std::time::Duration;
use tokio::time::sleep;

/// Get current UTC timestamp in RFC3339 format
///
/// Provides a consistent way to get timestamps across the application.
///
/// # Returns
///
/// RFC3339 formatted timestamp string
///
/// # Example
///
/// ```rust
/// use comsrv::utils::time::current_timestamp;
///
/// let timestamp = current_timestamp();
/// println!("Current time: {}", timestamp);
/// ```
pub fn current_timestamp() -> String {
    Utc::now().to_rfc3339()
}

/// Get current UTC timestamp as Unix timestamp
///
/// # Returns
///
/// Unix timestamp in seconds
///
/// # Example
///
/// ```rust
/// use comsrv::utils::time::current_unix_timestamp;
///
/// let timestamp = current_unix_timestamp();
/// println!("Unix timestamp: {}", timestamp);
/// ```
pub fn current_unix_timestamp() -> i64 {
    Utc::now().timestamp()
}

/// Format current time for logging with millisecond precision
///
/// Provides consistent log timestamp format: "HH:MM:SS.mmm"
///
/// # Returns
///
/// Formatted timestamp string for logging
///
/// # Example
///
/// ```rust
/// use comsrv::utils::time::format_log_timestamp;
///
/// let log_time = format_log_timestamp();
/// println!("[{}] Log message", log_time);
/// ```
pub fn format_log_timestamp() -> String {
    Utc::now().format("%H:%M:%S%.3f").to_string()
}

/// Sleep for specified milliseconds
///
/// Convenience wrapper around tokio::time::sleep for millisecond precision.
///
/// # Arguments
///
/// * `ms` - Milliseconds to sleep
///
/// # Example
///
/// ```rust
/// use comsrv::utils::time::sleep_ms;
///
/// // Sleep for 100 milliseconds
/// sleep_ms(100).await;
/// ```
pub async fn sleep_ms(ms: u64) {
    sleep(Duration::from_millis(ms)).await;
}

/// Sleep for specified seconds
///
/// Convenience wrapper around tokio::time::sleep for second precision.
///
/// # Arguments
///
/// * `secs` - Seconds to sleep
///
/// # Example
///
/// ```rust
/// use comsrv::utils::time::sleep_secs;
///
/// // Sleep for 5 seconds
/// sleep_secs(5).await;
/// ```
pub async fn sleep_secs(secs: u64) {
    sleep(Duration::from_secs(secs)).await;
}

/// Create a Duration from milliseconds
///
/// Convenience function for creating Duration objects.
///
/// # Arguments
///
/// * `ms` - Milliseconds
///
/// # Returns
///
/// Duration object
///
/// # Example
///
/// ```rust
/// use comsrv::utils::time::duration_ms;
///
/// let timeout = duration_ms(5000); // 5 seconds
/// ```
pub fn duration_ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

/// Create a Duration from seconds
///
/// Convenience function for creating Duration objects.
///
/// # Arguments
///
/// * `secs` - Seconds
///
/// # Returns
///
/// Duration object
///
/// # Example
///
/// ```rust
/// use comsrv::utils::time::duration_secs;
///
/// let timeout = duration_secs(30); // 30 seconds
/// ```
pub fn duration_secs(secs: u64) -> Duration {
    Duration::from_secs(secs)
}

/// Calculate elapsed time and format for logging
///
/// # Arguments
///
/// * `start` - Start time
///
/// # Returns
///
/// Formatted elapsed time string in milliseconds
///
/// # Example
///
/// ```rust
/// use comsrv::utils::time::elapsed_ms;
/// use std::time::Instant;
///
/// let start = Instant::now();
/// // ... some operation
/// let elapsed = elapsed_ms(start);
/// println!("Operation took: {}", elapsed);
/// ```
pub fn elapsed_ms(start: std::time::Instant) -> String {
    let elapsed = start.elapsed();
    format!("{:.2}ms", elapsed.as_secs_f64() * 1000.0)
}

/// Parse RFC3339 timestamp string to DateTime
///
/// # Arguments
///
/// * `timestamp` - RFC3339 formatted timestamp string
///
/// # Returns
///
/// Result containing parsed DateTime or error
///
/// # Example
///
/// ```rust
/// use comsrv::utils::time::parse_timestamp;
///
/// let timestamp = "2023-12-01T10:30:00Z";
/// match parse_timestamp(timestamp) {
///     Ok(dt) => println!("Parsed: {}", dt),
///     Err(e) => println!("Parse error: {}", e),
/// }
/// ```
pub fn parse_timestamp(timestamp: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
    DateTime::parse_from_rfc3339(timestamp)
        .map(|dt| dt.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Instant;

    #[test]
    fn test_current_timestamp() {
        let timestamp = current_timestamp();
        assert!(!timestamp.is_empty());
        assert!(timestamp.contains('T'));
        assert!(timestamp.ends_with('Z'));
    }

    #[test]
    fn test_current_unix_timestamp() {
        let timestamp = current_unix_timestamp();
        assert!(timestamp > 0);
        // Should be a reasonable timestamp (after 2020)
        assert!(timestamp > 1577836800); // 2020-01-01
    }

    #[test]
    fn test_format_log_timestamp() {
        let log_time = format_log_timestamp();
        assert!(!log_time.is_empty());
        assert!(log_time.contains(':'));
        assert!(log_time.contains('.'));
    }

    #[tokio::test]
    async fn test_sleep_ms() {
        let start = Instant::now();
        sleep_ms(10).await;
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(10));
        assert!(elapsed < Duration::from_millis(100)); // Should not take too long
    }

    #[tokio::test]
    async fn test_sleep_secs() {
        let start = Instant::now();
        sleep_secs(1).await;
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_secs(1));
        assert!(elapsed < Duration::from_secs(2)); // Should not take too long
    }

    #[test]
    fn test_duration_ms() {
        let duration = duration_ms(1500);
        assert_eq!(duration, Duration::from_millis(1500));
    }

    #[test]
    fn test_duration_secs() {
        let duration = duration_secs(30);
        assert_eq!(duration, Duration::from_secs(30));
    }

    #[test]
    fn test_elapsed_ms() {
        let start = std::time::Instant::now();
        std::thread::sleep(Duration::from_millis(10));
        let elapsed_str = elapsed_ms(start);
        assert!(elapsed_str.contains("ms"));
        assert!(!elapsed_str.is_empty());
    }

    #[test]
    fn test_parse_timestamp() {
        let timestamp = "2023-12-01T10:30:00Z";
        let parsed = parse_timestamp(timestamp);
        assert!(parsed.is_ok());

        let invalid_timestamp = "invalid";
        let parsed_invalid = parse_timestamp(invalid_timestamp);
        assert!(parsed_invalid.is_err());
    }
} 