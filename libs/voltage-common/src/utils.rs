//! Common utility functions for VoltageEMS services

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Get current timestamp in milliseconds since Unix epoch
pub fn current_timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as u64
}

/// Get current timestamp in seconds since Unix epoch
pub fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

/// Parse duration from string (e.g., "10s", "5m", "1h")
pub fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: u64 = num_str.parse().ok()?;

    match unit {
        "s" => Some(Duration::from_secs(num)),
        "m" => Some(Duration::from_secs(num * 60)),
        "h" => Some(Duration::from_secs(num * 3600)),
        "d" => Some(Duration::from_secs(num * 86400)),
        _ => None,
    }
}

/// Format duration to human-readable string
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();

    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
    }
}

/// Hex encoding utilities (re-export from hex crate)
pub mod hex {
    pub use hex::{decode, encode, encode_upper};

    /// Format bytes as hex string with spaces
    pub fn format_hex_pretty(data: &[u8]) -> String {
        data.iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Retry a fallible operation with exponential backoff
pub async fn retry_with_backoff<F, Fut, T, E>(
    mut f: F,
    max_retries: u32,
    initial_delay: Duration,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut delay = initial_delay;
    let mut retries = 0;

    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                retries += 1;
                if retries >= max_retries {
                    return Err(e);
                }
                tokio::time::sleep(delay).await;
                delay = delay.saturating_mul(2);
            }
        }
    }
}

/// Round a float to specified decimal places
pub fn round_to_decimals(value: f64, decimals: u32) -> f64 {
    let multiplier = 10f64.powi(decimals as i32);
    (value * multiplier).round() / multiplier
}

/// Clamp a value between min and max
pub fn clamp<T: PartialOrd>(value: T, min: T, max: T) -> T {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Calculate simple moving average
pub struct MovingAverage {
    window_size: usize,
    values: Vec<f64>,
    sum: f64,
    index: usize,
    filled: bool,
}

impl MovingAverage {
    pub fn new(window_size: usize) -> Self {
        assert!(window_size > 0, "Window size must be greater than 0");
        Self {
            window_size,
            values: vec![0.0; window_size],
            sum: 0.0,
            index: 0,
            filled: false,
        }
    }

    pub fn add(&mut self, value: f64) {
        let old_value = self.values[self.index];
        self.values[self.index] = value;
        self.sum = self.sum - old_value + value;

        self.index = (self.index + 1) % self.window_size;
        if self.index == 0 {
            self.filled = true;
        }
    }

    pub fn average(&self) -> Option<f64> {
        if !self.filled && self.index == 0 {
            None
        } else {
            let count = if self.filled {
                self.window_size
            } else {
                self.index
            };
            Some(self.sum / count as f64)
        }
    }

    pub fn reset(&mut self) {
        self.values.fill(0.0);
        self.sum = 0.0;
        self.index = 0;
        self.filled = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("10s"), Some(Duration::from_secs(10)));
        assert_eq!(parse_duration("5m"), Some(Duration::from_secs(300)));
        assert_eq!(parse_duration("2h"), Some(Duration::from_secs(7200)));
        assert_eq!(parse_duration("1d"), Some(Duration::from_secs(86400)));
        assert_eq!(parse_duration("invalid"), None);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
        assert_eq!(format_duration(Duration::from_secs(125)), "2m 5s");
        assert_eq!(format_duration(Duration::from_secs(3665)), "1h 1m");
        assert_eq!(format_duration(Duration::from_secs(90000)), "1d 1h");
    }

    #[test]
    fn test_hex_formatting() {
        let data = vec![0x01, 0x23, 0xAB, 0xCD];
        assert_eq!(hex::format_hex_pretty(&data), "01 23 AB CD");
    }

    #[test]
    fn test_round_to_decimals() {
        assert_eq!(round_to_decimals(std::f64::consts::PI, 2), 3.14);
        assert_eq!(round_to_decimals(std::f64::consts::PI, 3), 3.142);
        assert_eq!(round_to_decimals(std::f64::consts::PI, 0), 3.0);
    }

    #[test]
    fn test_clamp() {
        assert_eq!(clamp(5, 0, 10), 5);
        assert_eq!(clamp(-5, 0, 10), 0);
        assert_eq!(clamp(15, 0, 10), 10);
    }

    #[test]
    fn test_moving_average() {
        let mut ma = MovingAverage::new(3);

        assert_eq!(ma.average(), None);

        ma.add(1.0);
        assert_eq!(ma.average(), Some(1.0));

        ma.add(2.0);
        assert_eq!(ma.average(), Some(1.5));

        ma.add(3.0);
        assert_eq!(ma.average(), Some(2.0));

        ma.add(4.0); // Window is now full, oldest value (1.0) is replaced
        assert_eq!(ma.average(), Some(3.0));
    }

    #[tokio::test]
    async fn test_retry_with_backoff() {
        let mut attempts = 0;
        let result = retry_with_backoff(
            || {
                attempts += 1;
                async move {
                    if attempts < 3 {
                        Err("error")
                    } else {
                        Ok("success")
                    }
                }
            },
            5,
            Duration::from_millis(10),
        )
        .await;

        assert_eq!(result, Ok("success"));
        assert_eq!(attempts, 3);
    }
}
