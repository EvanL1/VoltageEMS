//! Shared Memory Command Listener (Event-Driven)
//!
//! Listens for M2C commands via Unix Domain Socket notifications.
//! Replaces polling with event-driven architecture for lower latency.
//!
//! ## Architecture
//!
//! ```text
//! modsrv: write SHM → send UDS notification ──►
//!                                               │
//! comsrv: listen UDS ← recv notification → read SHM → dispatch command
//! ```
//!
//! ## vs ShmCommandPoller (polling)
//!
//! | Aspect | Poller | Listener |
//! |--------|--------|----------|
//! | Latency | 10-20ms avg | ~1-2ms |
//! | CPU | Continuous polling | Event-triggered |
//! | Complexity | Simple | Requires connection management |

use dashmap::DashMap;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;
use voltage_model::{validate_value, PointType, ValidationConfig};
use voltage_rtdb_shm::{ShmNotification, UnifiedReader, DEFAULT_UDS_PATH};

use crate::core::channels::types::ChannelCommand;

/// Shared Memory Command Listener (Event-Driven)
pub struct ShmCommandListener {
    reader: Arc<UnifiedReader>,
    command_senders: Arc<DashMap<u32, mpsc::Sender<ChannelCommand>>>,
    uds_path: String,
    shutdown: tokio::sync::watch::Receiver<bool>,
}

impl ShmCommandListener {
    /// Create a new listener
    pub fn new(
        reader: Arc<UnifiedReader>,
        uds_path: Option<&str>,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Self {
        let path = uds_path.unwrap_or(DEFAULT_UDS_PATH).to_string();
        info!("ShmCommandListener: UDS path = {}", path);

        Self {
            reader,
            command_senders: Arc::new(DashMap::new()),
            uds_path: path,
            shutdown,
        }
    }

    /// Register a channel's command sender
    pub fn register_channel(&self, channel_id: u32, sender: mpsc::Sender<ChannelCommand>) {
        self.command_senders.insert(channel_id, sender);
        debug!("ShmListener: registered channel {}", channel_id);
    }

    /// Unregister a channel
    pub fn unregister_channel(&self, channel_id: u32) {
        self.command_senders.remove(&channel_id);
    }

    /// Start the listener
    pub async fn run(&self) -> std::io::Result<()> {
        // Clean up stale socket file from previous run (e.g., after crash)
        let socket_path = std::path::Path::new(&self.uds_path);
        if socket_path.exists() {
            info!("ShmListener: removing stale socket file {}", self.uds_path);
            if let Err(e) = std::fs::remove_file(socket_path) {
                error!(
                    "ShmListener: failed to remove stale socket {}: {}",
                    self.uds_path, e
                );
                // Continue anyway - bind will fail with a clearer error if socket is still in use
            }
        }

        let listener = match UnixListener::bind(&self.uds_path) {
            Ok(l) => {
                info!("ShmCommandListener started on {}", self.uds_path);
                l
            },
            Err(e) => {
                error!("Failed to bind UDS listener: {}", e);
                return Err(e);
            },
        };

        let mut shutdown = self.shutdown.clone();

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            let reader = Arc::clone(&self.reader);
                            let senders = Arc::clone(&self.command_senders);
                            let shutdown_rx = self.shutdown.clone();

                            tokio::spawn(async move {
                                Self::handle_connection(stream, reader, senders, shutdown_rx).await;
                            });
                        }
                        Err(e) => {
                            warn!("UDS accept error: {}", e);
                        }
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("ShmCommandListener shutdown");
                        break;
                    }
                }
            }
        }

        // Cleanup socket file
        let _ = std::fs::remove_file(&self.uds_path);
        Ok(())
    }

    async fn handle_connection(
        mut stream: tokio::net::UnixStream,
        reader: Arc<UnifiedReader>,
        senders: Arc<DashMap<u32, mpsc::Sender<ChannelCommand>>>,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) {
        debug!("ShmListener: new connection");
        let mut buf = [0u8; ShmNotification::SIZE];

        loop {
            tokio::select! {
                result = stream.read_exact(&mut buf) => {
                    match result {
                        Ok(_) => {
                            let notif = ShmNotification::from_bytes(&buf);
                            Self::handle_notification(&notif, &reader, &senders).await;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                            debug!("ShmListener: connection closed");
                            break;
                        }
                        Err(e) => {
                            warn!("ShmListener read error: {}", e);
                            break;
                        }
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        break;
                    }
                }
            }
        }
    }

    async fn handle_notification(
        notif: &ShmNotification,
        reader: &UnifiedReader,
        senders: &DashMap<u32, mpsc::Sender<ChannelCommand>>,
    ) {
        let channel_id = notif.channel_id;
        let point_type = match notif.get_point_type() {
            Some(pt) => pt,
            None => {
                warn!("Invalid point type in notification: {}", notif.point_type);
                return;
            },
        };
        let point_id = notif.point_id;

        // Read actual value from SHM
        let (value, timestamp) = match reader.get_channel(channel_id, point_type.to_u8(), point_id)
        {
            Some(data) => data,
            None => {
                warn!(
                    "ShmListener: channel {} point {:?}:{} not found in SHM",
                    channel_id, point_type, point_id
                );
                return;
            },
        };

        // Validate value before sending to device (prevents NaN/Infinity from reaching hardware)
        let config = ValidationConfig::default();
        let value = match validate_value(value, &config) {
            Ok(v) => v,
            Err(e) => {
                warn!(
                    "ShmListener: invalid value for ch{}:{:?}:{}: {} - command discarded",
                    channel_id, point_type, point_id, e
                );
                return;
            },
        };

        trace!(
            "ShmListener: ch={} {:?}:{} val={} ts={}",
            channel_id,
            point_type,
            point_id,
            value,
            timestamp
        );

        // Build and send command
        let command = Self::build_command(point_type, point_id, value, timestamp as i64);

        if let Some(sender) = senders.get(&channel_id) {
            // Use send_timeout instead of try_send to handle transient backpressure
            // 50ms timeout allows brief buffer congestion without blocking event loop
            match tokio::time::timeout(std::time::Duration::from_millis(50), sender.send(command))
                .await
            {
                Ok(Ok(())) => {
                    // Successfully sent
                },
                Ok(Err(_)) => {
                    // Channel closed
                    warn!(
                        "ShmListener: channel {} closed, notification discarded",
                        channel_id
                    );
                },
                Err(_) => {
                    // Timeout - sustained backpressure, drop command
                    error!(
                        "ShmListener: channel {} buffer FULL for 50ms, notification DROPPED \
                         (point {:?}:{}, sustained backpressure)",
                        channel_id, point_type, point_id
                    );
                },
            }
        } else {
            debug!(
                "ShmListener: no sender for channel {} (not registered)",
                channel_id
            );
        }
    }

    fn build_command(
        point_type: PointType,
        point_id: u32,
        value: f64,
        timestamp: i64,
    ) -> ChannelCommand {
        let command_id = format!("uds-{}", Uuid::new_v4());
        match point_type {
            PointType::Control => ChannelCommand::Control {
                command_id,
                point_id,
                value,
                timestamp,
            },
            _ => ChannelCommand::Adjustment {
                command_id,
                point_id,
                value,
                timestamp,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_command_control() {
        let cmd = ShmCommandListener::build_command(PointType::Control, 5, 123.45, 1000);
        match cmd {
            ChannelCommand::Control {
                point_id, value, ..
            } => {
                assert_eq!(point_id, 5);
                assert!((value - 123.45).abs() < 0.001);
            },
            _ => panic!("Expected Control command"),
        }
    }

    #[test]
    fn test_build_command_adjustment() {
        let cmd = ShmCommandListener::build_command(PointType::Adjustment, 10, 67.89, 2000);
        match cmd {
            ChannelCommand::Adjustment {
                point_id, value, ..
            } => {
                assert_eq!(point_id, 10);
                assert!((value - 67.89).abs() < 0.001);
            },
            _ => panic!("Expected Adjustment command"),
        }
    }
}
