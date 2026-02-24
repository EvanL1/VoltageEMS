//! UDS Notification Sender
//!
//! Used by modsrv to send M2C command notifications to comsrv.
//! Supports graceful degradation: does not block on connection failure, only disables notifications.
//!
//! ## Reliability Enhancements
//!
//! `notify()` returns `NotifyResult` instead of a simple `io::Result<()>`,
//! allowing callers to distinguish:
//! - Successfully sent (`uds_sent = true`)
//! - Degraded to polling (`fallback_used = true`)
//! - Completely disabled (`disabled = true`)

use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;
use tracing::{debug, info, warn};
use voltage_model::PointType;

use crate::notification::ShmNotification;

/// Default UDS path
pub const DEFAULT_UDS_PATH: &str = "/tmp/voltage-m2c.sock";

// ============================================================================
// NotifyResult - Notification Result Status
// ============================================================================

/// UDS notification result
///
/// Provides detailed send status, allowing callers to take action based on the result.
#[derive(Debug, Clone, Copy, Default)]
pub struct NotifyResult {
    /// UDS send succeeded
    pub uds_sent: bool,
    /// Degraded to polling fallback (UDS send failed)
    pub fallback_used: bool,
    /// Notifications completely disabled (path is empty or unconfigured)
    pub disabled: bool,
}

impl NotifyResult {
    /// Check if sent successfully (via UDS)
    #[inline]
    pub fn is_success(&self) -> bool {
        self.uds_sent
    }

    /// Check if immediate polling is needed (degraded case)
    #[inline]
    pub fn needs_immediate_poll(&self) -> bool {
        self.fallback_used
    }
}

// ============================================================================
// UdsHealth - Connection Health Status
// ============================================================================

/// UDS connection health status
#[derive(Debug, Clone)]
pub enum UdsHealth {
    /// Connected
    Connected,
    /// Disconnected
    Disconnected {
        /// Time since disconnection
        since: Option<Instant>,
        /// Current backoff duration (milliseconds)
        backoff_ms: u64,
    },
    /// Disabled (path is empty)
    Disabled,
}

/// SHM command notification sender
///
/// Sends M2C command notifications to comsrv via Unix Domain Socket.
/// Supports graceful degradation: if connection fails, notifications are silently ignored.
/// Supports auto-reconnection: uses exponential backoff strategy after disconnection.
pub struct ShmNotifier {
    stream: Option<UnixStream>,
    path: String,
    /// Last connection attempt time
    last_connect_attempt: Option<Instant>,
    /// Current backoff duration (milliseconds)
    backoff_ms: u64,
}

impl ShmNotifier {
    /// Minimum backoff duration (milliseconds)
    const MIN_BACKOFF_MS: u64 = 1000; // 1 second
    /// Maximum backoff duration (milliseconds)
    const MAX_BACKOFF_MS: u64 = 30000; // 30 seconds
    /// Send retry count
    const MAX_RETRIES: u32 = 3;
    /// Retry interval (milliseconds)
    const RETRY_DELAY_MS: u64 = 10;

    /// Connect to UDS listener
    ///
    /// If connection fails, returns a disabled notifier (notifications will be ignored).
    /// Subsequent calls to `notify()` will automatically attempt reconnection.
    pub async fn connect(path: impl AsRef<Path>) -> io::Result<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();

        match UnixStream::connect(&path_str).await {
            Ok(stream) => {
                debug!("ShmNotifier connected to {}", path_str);
                Ok(Self {
                    stream: Some(stream),
                    path: path_str,
                    last_connect_attempt: None,
                    backoff_ms: Self::MIN_BACKOFF_MS,
                })
            },
            Err(e) => {
                warn!(
                    "ShmNotifier: UDS connect failed ({}), will retry on notify: {}",
                    path_str, e
                );
                Ok(Self {
                    stream: None,
                    path: path_str,
                    last_connect_attempt: Some(Instant::now()),
                    backoff_ms: Self::MIN_BACKOFF_MS,
                })
            },
        }
    }

    /// Connect using the default path
    pub async fn connect_default() -> io::Result<Self> {
        Self::connect(DEFAULT_UDS_PATH).await
    }

    /// Create a disabled notifier (for testing or scenarios that don't need notifications)
    pub fn disabled() -> Self {
        Self {
            stream: None,
            path: String::new(),
            last_connect_attempt: None,
            backoff_ms: Self::MIN_BACKOFF_MS,
        }
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    /// Get connection path
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Send notification
    ///
    /// If not connected, attempts reconnection (using exponential backoff).
    /// Retries up to 3 times on send failure; after all retries fail, marks as disconnected
    /// and triggers reconnection on next call.
    ///
    /// ## Return Value
    ///
    /// Returns `NotifyResult` instead of `io::Result<()>`, allowing callers to distinguish:
    /// - `uds_sent = true`: UDS send succeeded, command will be processed via low-latency path
    /// - `fallback_used = true`: UDS failed, degraded to polling fallback (increased latency)
    /// - `disabled = true`: Notification feature is completely disabled
    ///
    /// ## Usage Example
    ///
    /// ```rust,ignore
    /// let result = notifier.notify(channel_id, point_type, point_id).await;
    /// if result.needs_immediate_poll() {
    ///     // UDS failed, may need to trigger immediate polling
    ///     poller.trigger_immediate_check();
    /// }
    /// ```
    pub async fn notify(
        &mut self,
        channel_id: u32,
        point_type: PointType,
        point_id: u32,
    ) -> NotifyResult {
        // If path is empty, notifications are disabled
        if self.path.is_empty() {
            return NotifyResult {
                disabled: true,
                ..Default::default()
            };
        }

        // If not connected, attempt reconnection
        self.try_reconnect().await;

        if let Some(ref mut stream) = self.stream {
            let notification = ShmNotification::new(channel_id, point_type, point_id);
            let bytes = notification.to_bytes();

            // Retry logic: up to MAX_RETRIES attempts
            for attempt in 0..Self::MAX_RETRIES {
                match stream.write_all(&bytes).await {
                    Ok(_) => {
                        debug!(
                            "ShmNotifier: sent notification channel={} type={:?} point={}",
                            channel_id, point_type, point_id
                        );
                        return NotifyResult {
                            uds_sent: true,
                            ..Default::default()
                        };
                    },
                    Err(e) if attempt < Self::MAX_RETRIES - 1 => {
                        warn!(
                            "ShmNotifier: send attempt {} failed: {}, retrying...",
                            attempt + 1,
                            e
                        );
                        tokio::time::sleep(Duration::from_millis(Self::RETRY_DELAY_MS)).await;
                    },
                    Err(e) => {
                        // All retries failed, mark as disconnected
                        warn!(
                            "ShmNotifier: all {} retries failed for ch{}:{}:{}, \
                             falling back to poller: {}",
                            Self::MAX_RETRIES,
                            channel_id,
                            point_type.as_str(),
                            point_id,
                            e
                        );
                        self.stream = None;
                        self.last_connect_attempt = Some(Instant::now());
                        // Return degraded status so caller can trigger immediate polling
                        return NotifyResult {
                            fallback_used: true,
                            ..Default::default()
                        };
                    },
                }
            }
        }

        // Not connected and reconnection failed, return degraded status
        NotifyResult {
            fallback_used: true,
            ..Default::default()
        }
    }

    /// Attempt reconnection (if disconnected and backoff time has elapsed)
    async fn try_reconnect(&mut self) {
        // Already connected or path is empty, skip
        if self.stream.is_some() || self.path.is_empty() {
            return;
        }

        // Check backoff time
        if let Some(last_attempt) = self.last_connect_attempt {
            if last_attempt.elapsed().as_millis() < self.backoff_ms as u128 {
                return; // Within backoff period, skip
            }
        }

        // Attempt reconnection
        match UnixStream::connect(&self.path).await {
            Ok(stream) => {
                self.stream = Some(stream);
                self.backoff_ms = Self::MIN_BACKOFF_MS;
                self.last_connect_attempt = None;
                info!("ShmNotifier: reconnected to {}", self.path);
            },
            Err(_) => {
                // Increase backoff duration (exponential backoff)
                self.backoff_ms = (self.backoff_ms * 2).min(Self::MAX_BACKOFF_MS);
                self.last_connect_attempt = Some(Instant::now());
            },
        }
    }

    /// Send a pre-built notification (with auto-reconnection)
    ///
    /// Returns `NotifyResult`, consistent with `notify()` behavior.
    pub async fn notify_raw(&mut self, notification: &ShmNotification) -> NotifyResult {
        // If path is empty, notifications are disabled
        if self.path.is_empty() {
            return NotifyResult {
                disabled: true,
                ..Default::default()
            };
        }

        self.try_reconnect().await;

        if let Some(ref mut stream) = self.stream {
            match stream.write_all(&notification.to_bytes()).await {
                Ok(_) => {
                    return NotifyResult {
                        uds_sent: true,
                        ..Default::default()
                    };
                },
                Err(e) => {
                    warn!("ShmNotifier: send_raw failed, marking disconnected: {}", e);
                    self.stream = None;
                    self.last_connect_attempt = Some(Instant::now());
                },
            }
        }

        NotifyResult {
            fallback_used: true,
            ..Default::default()
        }
    }

    /// Check UDS connection health status
    ///
    /// Used for monitoring and diagnostics.
    pub fn health_check(&self) -> UdsHealth {
        if self.path.is_empty() {
            return UdsHealth::Disabled;
        }

        if self.stream.is_some() {
            UdsHealth::Connected
        } else {
            UdsHealth::Disconnected {
                since: self.last_connect_attempt,
                backoff_ms: self.backoff_ms,
            }
        }
    }

    /// Manually force reconnection (bypasses backoff mechanism)
    pub async fn reconnect(&mut self) -> io::Result<bool> {
        if self.path.is_empty() {
            return Ok(false);
        }

        match UnixStream::connect(&self.path).await {
            Ok(stream) => {
                info!("ShmNotifier: force reconnected to {}", self.path);
                self.stream = Some(stream);
                self.backoff_ms = Self::MIN_BACKOFF_MS;
                self.last_connect_attempt = None;
                Ok(true)
            },
            Err(e) => {
                warn!("ShmNotifier: force reconnect failed: {}", e);
                self.last_connect_attempt = Some(Instant::now());
                Ok(false)
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_disabled_notifier() {
        let mut notifier = ShmNotifier::disabled();
        assert!(!notifier.is_connected());

        // Disabled notifier should return disabled = true
        let result = notifier.notify(1001, PointType::Control, 0).await;
        assert!(result.disabled);
        assert!(!result.uds_sent);
        assert!(!result.fallback_used);
    }

    #[tokio::test]
    async fn test_connect_nonexistent_path() {
        let notifier = ShmNotifier::connect("/tmp/nonexistent-test-socket.sock")
            .await
            .unwrap();

        // Connection failed, but returns a disabled notifier instead of an error
        assert!(!notifier.is_connected());
    }

    #[tokio::test]
    async fn test_notify_result_helpers() {
        // Success
        let success = NotifyResult {
            uds_sent: true,
            ..Default::default()
        };
        assert!(success.is_success());
        assert!(!success.needs_immediate_poll());

        // Degraded
        let fallback = NotifyResult {
            fallback_used: true,
            ..Default::default()
        };
        assert!(!fallback.is_success());
        assert!(fallback.needs_immediate_poll());

        // Disabled
        let disabled = NotifyResult {
            disabled: true,
            ..Default::default()
        };
        assert!(!disabled.is_success());
        assert!(!disabled.needs_immediate_poll());
    }

    #[tokio::test]
    async fn test_health_check() {
        // Disabled state
        let notifier = ShmNotifier::disabled();
        assert!(matches!(notifier.health_check(), UdsHealth::Disabled));

        // Disconnected state
        let notifier = ShmNotifier::connect("/tmp/nonexistent-test-socket.sock")
            .await
            .unwrap();
        assert!(matches!(
            notifier.health_check(),
            UdsHealth::Disconnected { .. }
        ));
    }
}
