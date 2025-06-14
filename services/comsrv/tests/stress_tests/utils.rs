use std::collections::HashMap;
use std::net::TcpListener;
use std::time::{Duration, Instant};
use serde_json::json;
use redis::Commands;

/// Test configuration
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub channels: usize,
    pub points_per_channel: usize,
    pub duration_secs: u64,
    pub base_port: u16,
    pub redis_batch_size: usize,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            channels: 5,
            points_per_channel: 100,
            duration_secs: 60,
            base_port: 5020,
            redis_batch_size: 100,
        }
    }
}

/// Performance statistics
#[derive(Debug, Clone)]
pub struct PerformanceStats {
    pub total_reads: u64,
    pub successful_reads: u64,
    pub failed_reads: u64,
    pub total_points: u64,
    pub redis_writes: u64,
    pub redis_errors: u64,
    pub start_time: Option<Instant>,
    pub last_update: Option<Instant>,
}

impl PerformanceStats {
    pub fn new() -> Self {
        Self {
            total_reads: 0,
            successful_reads: 0,
            failed_reads: 0,
            total_points: 0,
            redis_writes: 0,
            redis_errors: 0,
            start_time: None,
            last_update: None,
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_reads == 0 {
            0.0
        } else {
            (self.successful_reads as f64 / self.total_reads as f64) * 100.0
        }
    }

    pub fn operations_per_second(&self) -> f64 {
        if let (Some(start), Some(last)) = (self.start_time, self.last_update) {
            let duration = last.duration_since(start).as_secs_f64();
            if duration > 0.0 {
                self.total_reads as f64 / duration
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
}

/// Check whether a Redis instance is reachable
pub fn check_redis_connection() -> Result<redis::Client, Box<dyn std::error::Error>> {
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let client = redis::Client::open(redis_url)?;
    let mut conn = client.get_connection()?;
    let _: String = redis::cmd("PING").query(&mut conn)?;
    Ok(client)
}

/// Test if the specified TCP port is available
pub fn check_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}