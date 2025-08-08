//! Reconnection mechanism implementation
//!
//! Provides a generic reconnection helper with exponential backoff and jitter support

use rand::Rng;
use std::future::Future;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Reconnection error types
#[derive(Error, Debug)]
pub enum ReconnectError {
    /// Maximum retry attempts exceeded
    #[error("Maximum reconnection attempts exceeded")]
    MaxAttemptsExceeded,

    /// Connection failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Reconnection was cancelled
    #[error("Reconnection cancelled")]
    Cancelled,
}

/// Connection state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Successfully connected
    Connected,
    /// Disconnected
    Disconnected,
    /// Currently reconnecting
    Reconnecting,
    /// Reconnection failed (max attempts reached)
    Failed,
}

/// Reconnection policy configuration
#[derive(Debug, Clone)]
pub struct ReconnectPolicy {
    /// Maximum retry attempts (0 means unlimited)
    pub max_attempts: u32,
    /// Initial delay between attempts
    pub initial_delay: Duration,
    /// Maximum delay between attempts
    pub max_delay: Duration,
    /// Backoff multiplier for exponential delay
    pub backoff_multiplier: f64,
    /// Whether to add jitter to delays
    pub jitter: bool,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl ReconnectPolicy {
    /// Create from configuration values
    pub fn from_config(
        max_attempts: u32,
        initial_delay_ms: u64,
        max_delay_ms: u64,
        backoff_multiplier: f64,
    ) -> Self {
        Self {
            max_attempts,
            initial_delay: Duration::from_millis(initial_delay_ms),
            max_delay: Duration::from_millis(max_delay_ms),
            backoff_multiplier,
            jitter: true,
        }
    }
}

/// Reconnection context tracking current state and attempts
#[derive(Debug, Clone)]
pub struct ReconnectContext {
    /// Current retry attempt count
    pub current_attempt: u32,
    /// Last retry attempt time
    pub last_attempt: Option<Instant>,
    /// Next scheduled retry time
    pub next_attempt: Option<Instant>,
    /// Connection state enumeration
    pub connection_state: ConnectionState,
}

impl Default for ReconnectContext {
    fn default() -> Self {
        Self {
            current_attempt: 0,
            last_attempt: None,
            next_attempt: None,
            connection_state: ConnectionState::Disconnected,
        }
    }
}

/// Reconnection statistics tracking
#[derive(Debug, Default, Clone)]
pub struct ReconnectStats {
    /// Total reconnection attempts
    pub total_attempts: u64,
    /// Successful reconnection count
    pub successful_reconnects: u64,
    /// Failed reconnection count
    pub failed_reconnects: u64,
    /// Last successful connection time
    pub last_connected: Option<Instant>,
    /// Connection start time
    pub connection_start: Option<Instant>,
}

/// Generic reconnection helper with backoff and statistics
#[derive(Debug)]
pub struct ReconnectHelper {
    /// Reconnection policy configuration
    policy: ReconnectPolicy,
    /// Current reconnection context
    context: ReconnectContext,
    /// Connection statistics
    stats: ReconnectStats,
}

impl ReconnectHelper {
    /// Create a new reconnection helper
    pub fn new(policy: ReconnectPolicy) -> Self {
        Self {
            policy,
            context: ReconnectContext::default(),
            stats: ReconnectStats::default(),
        }
    }

    /// Get the current connection state
    pub fn connection_state(&self) -> ConnectionState {
        self.context.connection_state
    }

    /// Get connection statistics
    pub fn stats(&self) -> &ReconnectStats {
        &self.stats
    }

    /// Reset the reconnection context
    pub fn reset(&mut self) {
        self.context.current_attempt = 0;
        self.context.last_attempt = None;
        self.context.next_attempt = None;
        if self.context.connection_state != ConnectionState::Connected {
            self.context.connection_state = ConnectionState::Disconnected;
        }
    }

    /// Mark the connection as successful
    pub fn mark_connected(&mut self) {
        self.context.connection_state = ConnectionState::Connected;
        self.context.current_attempt = 0;
        self.stats.last_connected = Some(Instant::now());
        self.stats.connection_start = Some(Instant::now());
        debug!("Connection marked as successful");
    }

    /// Mark the connection as disconnected
    pub fn mark_disconnected(&mut self) {
        self.context.connection_state = ConnectionState::Disconnected;
        self.stats.connection_start = None;
        debug!("Connection marked as disconnected");
    }

    /// Calculate the next retry delay with exponential backoff
    pub fn calculate_next_delay(&self) -> Duration {
        let attempt = self.context.current_attempt.saturating_sub(1);
        let base_delay = self.policy.initial_delay;
        let multiplier = self.policy.backoff_multiplier;

        // Exponential backoff: delay = initial_delay * (multiplier ^ attempt)
        let mut delay = base_delay.mul_f64(multiplier.powi(attempt as i32));

        // Cap at maximum delay
        if delay > self.policy.max_delay {
            delay = self.policy.max_delay;
        }

        // Add jitter (±25% of delay)
        if self.policy.jitter {
            let jitter_range = delay.as_millis() as f64 * 0.25;
            let jitter = rand::thread_rng().gen_range(-jitter_range..jitter_range);
            let delay_ms = (delay.as_millis() as f64 + jitter).max(0.0);
            delay = Duration::from_millis(delay_ms as u64);
        }

        delay
    }

    /// Execute a reconnection attempt
    pub async fn execute_reconnect<F, Fut, E>(
        &mut self,
        mut connect_fn: F,
    ) -> Result<(), ReconnectError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<(), E>>,
        E: std::fmt::Display,
    {
        // Check if maximum retry attempts reached
        if self.policy.max_attempts > 0 && self.context.current_attempt >= self.policy.max_attempts
        {
            self.context.connection_state = ConnectionState::Failed;
            warn!(
                "Maximum reconnection attempts ({}) exceeded",
                self.policy.max_attempts
            );
            return Err(ReconnectError::MaxAttemptsExceeded);
        }

        // Update connection state
        self.context.connection_state = ConnectionState::Reconnecting;
        self.context.current_attempt += 1;
        self.stats.total_attempts += 1;

        info!(
            "Starting reconnection attempt {}/{}",
            self.context.current_attempt,
            if self.policy.max_attempts == 0 {
                "∞".to_string()
            } else {
                self.policy.max_attempts.to_string()
            }
        );

        // If not the first attempt, calculate and wait for delay
        if self.context.current_attempt > 1 {
            let delay = self.calculate_next_delay();
            info!("Waiting {:?} before reconnection attempt", delay);
            tokio::time::sleep(delay).await;
        }

        // Record attempt time
        let start_time = Instant::now();
        self.context.last_attempt = Some(start_time);

        // Attempt connection
        match connect_fn().await {
            Ok(()) => {
                // Connection successful
                let reconnect_time = start_time.elapsed();
                info!(
                    "Reconnection successful after {:?} (attempt {})",
                    reconnect_time, self.context.current_attempt
                );

                self.mark_connected();
                self.stats.successful_reconnects += 1;

                Ok(())
            },
            Err(e) => {
                // Connection failed
                warn!(
                    "Reconnection attempt {} failed: {}",
                    self.context.current_attempt, e
                );

                self.stats.failed_reconnects += 1;

                // If more retry attempts available, maintain reconnecting state
                if self.policy.max_attempts == 0
                    || self.context.current_attempt < self.policy.max_attempts
                {
                    self.context.connection_state = ConnectionState::Disconnected;
                } else {
                    self.context.connection_state = ConnectionState::Failed;
                }

                Err(ReconnectError::ConnectionFailed(e.to_string()))
            },
        }
    }

    /// Get the next retry delay (for display purposes)
    pub fn next_delay(&self) -> Option<Duration> {
        if self.context.connection_state == ConnectionState::Failed {
            return None;
        }

        if self.policy.max_attempts > 0 && self.context.current_attempt >= self.policy.max_attempts
        {
            return None;
        }

        Some(self.calculate_next_delay())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_exponential_backoff() {
        let policy = ReconnectPolicy {
            max_attempts: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let mut helper = ReconnectHelper::new(policy);

        // First attempt has no delay
        assert_eq!(helper.context.current_attempt, 0);

        // Set current attempt count and validate delay
        helper.context.current_attempt = 1;
        assert_eq!(helper.calculate_next_delay(), Duration::from_millis(100));

        helper.context.current_attempt = 2;
        assert_eq!(helper.calculate_next_delay(), Duration::from_millis(200));

        helper.context.current_attempt = 3;
        assert_eq!(helper.calculate_next_delay(), Duration::from_millis(400));

        helper.context.current_attempt = 4;
        assert_eq!(helper.calculate_next_delay(), Duration::from_millis(800));
    }

    #[tokio::test]
    async fn test_max_delay_limit() {
        let policy = ReconnectPolicy {
            max_attempts: 10,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let mut helper = ReconnectHelper::new(policy);

        // Test that delay doesn't exceed maximum
        helper.context.current_attempt = 10;
        let delay = helper.calculate_next_delay();
        assert!(delay <= Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_max_attempts() {
        let policy = ReconnectPolicy {
            max_attempts: 2,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let mut helper = ReconnectHelper::new(policy);

        // Simulate a failing connection function
        let connect_fn = || async { Err::<(), _>("Connection failed") };

        // First attempt
        let result = helper.execute_reconnect(connect_fn).await;
        assert!(result.is_err());
        assert_eq!(helper.context.current_attempt, 1);

        // Second attempt
        let result = helper.execute_reconnect(connect_fn).await;
        assert!(result.is_err());
        assert_eq!(helper.context.current_attempt, 2);

        // Third attempt should fail immediately
        let result = helper.execute_reconnect(connect_fn).await;
        assert!(matches!(result, Err(ReconnectError::MaxAttemptsExceeded)));
        assert_eq!(helper.context.connection_state, ConnectionState::Failed);
    }

    #[tokio::test]
    async fn test_successful_reconnect() {
        let policy = ReconnectPolicy::default();
        let mut helper = ReconnectHelper::new(policy);

        // Simulate a successful connection function
        let connect_fn = || async { Ok::<(), &str>(()) };

        let result = helper.execute_reconnect(connect_fn).await;
        assert!(result.is_ok());
        assert_eq!(helper.context.connection_state, ConnectionState::Connected);
        assert_eq!(helper.context.current_attempt, 0);
        assert_eq!(helper.stats.successful_reconnects, 1);
    }
}
