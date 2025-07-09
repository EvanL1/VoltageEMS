use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Base communication statistics that all protocols can extend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseCommStats {
    /// Total number of requests/messages sent
    pub total_requests: u64,
    /// Number of successful requests/messages
    pub successful_requests: u64,
    /// Number of failed requests/messages
    pub failed_requests: u64,
    /// Number of timeout errors
    pub timeout_errors: u64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// Last successful communication timestamp
    pub last_successful_communication: Option<SystemTime>,

    /// Start time for calculating uptime
    pub start_time: SystemTime,
    /// Protocol-specific error counters
    pub error_counters: HashMap<String, u64>,
}

impl BaseCommStats {
    /// Create new statistics instance
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            timeout_errors: 0,
            avg_response_time_ms: 0.0,
            last_successful_communication: None,
            start_time: SystemTime::now(),
            error_counters: HashMap::new(),
        }
    }

    /// Update statistics after a request/operation
    pub fn update_request_stats(
        &mut self,
        success: bool,
        response_time: Duration,
        error_type: Option<&str>,
    ) {
        self.total_requests += 1;

        if success {
            self.successful_requests += 1;
            self.last_successful_communication = Some(SystemTime::now());
        } else {
            self.failed_requests += 1;

            // Handle specific error types
            if let Some(error) = error_type {
                if error == "timeout" {
                    self.timeout_errors += 1;
                }
                *self.error_counters.entry(error.to_string()).or_insert(0) += 1;
            }
        }

        // Update average response time
        let current_avg = self.avg_response_time_ms;
        let new_time = response_time.as_millis() as f64;
        self.avg_response_time_ms = if self.total_requests == 1 {
            new_time
        } else {
            (current_avg * (self.total_requests - 1) as f64 + new_time) / self.total_requests as f64
        };
    }

    /// Get uptime since statistics started
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed().unwrap_or_default()
    }

    /// Reset all statistics
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Get error count for specific error type
    pub fn get_error_count(&self, error_type: &str) -> u64 {
        self.error_counters.get(error_type).copied().unwrap_or(0)
    }

    /// Increment specific error counter
    pub fn increment_error_counter(&mut self, error_type: &str) {
        *self
            .error_counters
            .entry(error_type.to_string())
            .or_insert(0) += 1;
    }
}

impl Default for BaseCommStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Base connection statistics for connection-oriented protocols
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseConnectionStats {
    /// Number of reconnection attempts
    pub reconnect_attempts: u64,
    /// Number of currently connected clients (for servers)
    pub connected_clients: u32,
    /// Total connections made
    pub total_connections: u64,
    /// Connection drops/disconnections
    pub connection_drops: u64,
    /// Last connection time
    pub last_connection_time: Option<SystemTime>,
    /// Last disconnection time
    pub last_disconnection_time: Option<SystemTime>,
}

impl BaseConnectionStats {
    /// Create new connection statistics instance
    pub fn new() -> Self {
        Self {
            reconnect_attempts: 0,
            connected_clients: 0,
            total_connections: 0,
            connection_drops: 0,
            last_connection_time: None,
            last_disconnection_time: None,
        }
    }

    /// Record a successful connection
    pub fn record_connection(&mut self) {
        self.total_connections += 1;
        self.last_connection_time = Some(SystemTime::now());
        self.connected_clients += 1;
    }

    /// Record a disconnection
    pub fn record_disconnection(&mut self) {
        self.connection_drops += 1;
        self.last_disconnection_time = Some(SystemTime::now());
        if self.connected_clients > 0 {
            self.connected_clients -= 1;
        }
    }

    /// Record a reconnection attempt
    pub fn record_reconnection_attempt(&mut self) {
        self.reconnect_attempts += 1;
    }

    /// Reset connection statistics
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for BaseConnectionStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_comm_stats_creation() {
        let stats = BaseCommStats::new();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.successful_requests, 0);
        assert_eq!(stats.failed_requests, 0);
        assert_eq!(stats.timeout_errors, 0);
    }

    #[test]
    fn test_base_comm_stats_update_success() {
        let mut stats = BaseCommStats::new();
        stats.update_request_stats(true, Duration::from_millis(100), None);

        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.failed_requests, 0);
        assert_eq!(stats.avg_response_time_ms, 100.0);
        assert!(stats.last_successful_communication.is_some());
    }

    #[test]
    fn test_base_comm_stats_update_failure() {
        let mut stats = BaseCommStats::new();
        stats.update_request_stats(false, Duration::from_millis(50), Some("timeout"));

        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 0);
        assert_eq!(stats.failed_requests, 1);
        assert_eq!(stats.timeout_errors, 1);
        assert_eq!(stats.get_error_count("timeout"), 1);
    }

    #[test]
    fn test_base_connection_stats_creation() {
        let stats = BaseConnectionStats::new();
        assert_eq!(stats.reconnect_attempts, 0);
        assert_eq!(stats.connected_clients, 0);
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.connection_drops, 0);
    }

    #[test]
    fn test_base_connection_stats_operations() {
        let mut stats = BaseConnectionStats::new();

        // Test connection
        stats.record_connection();
        assert_eq!(stats.total_connections, 1);
        assert_eq!(stats.connected_clients, 1);
        assert!(stats.last_connection_time.is_some());

        // Test disconnection
        stats.record_disconnection();
        assert_eq!(stats.connection_drops, 1);
        assert_eq!(stats.connected_clients, 0);
        assert!(stats.last_disconnection_time.is_some());

        // Test reconnection attempt
        stats.record_reconnection_attempt();
        assert_eq!(stats.reconnect_attempts, 1);
    }

    #[test]
    fn test_error_counters() {
        let mut stats = BaseCommStats::new();

        stats.increment_error_counter("crc_error");
        stats.increment_error_counter("crc_error");
        stats.increment_error_counter("protocol_error");

        assert_eq!(stats.get_error_count("crc_error"), 2);
        assert_eq!(stats.get_error_count("protocol_error"), 1);
        assert_eq!(stats.get_error_count("unknown_error"), 0);
    }
}
