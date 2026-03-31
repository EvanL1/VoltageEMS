//! Shared Memory Command Listener (Event-Driven)
//!
//! Listens for M2C commands via Unix Domain Socket notifications.
//! Replaces polling with event-driven architecture for lower latency.
//!
//! ## Architecture
//!
//! ```text
//! modsrv: write Redis/SHM mirror → send UDS command event ──►
//!                                                       │
//! comsrv: listen UDS ← recv full command event → dispatch command
//! ```
//!
//! Replaced the former ShmCommandPoller (polling-based) with lower latency (~1-2ms vs 10-20ms avg)
//! and event-triggered CPU usage instead of continuous polling.

use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace, warn};
use voltage_model::{validate_value, PointType, ValidationConfig};
use voltage_rtdb_shm::{ShmNotification, DEFAULT_UDS_PATH};

use crate::core::channels::types::ChannelCommand;

/// (channel_id, point_type, address) → (sequence, timestamp)
type SequenceMap = DashMap<(u32, u8, u32), (u64, u64)>;

/// Shared Memory Command Listener (Event-Driven)
pub struct ShmCommandListener {
    command_senders: Arc<DashMap<u32, mpsc::Sender<ChannelCommand>>>,
    last_sequences: Arc<SequenceMap>,
    uds_path: String,
    shutdown: tokio::sync::watch::Receiver<bool>,
    dropped_count: Arc<AtomicU64>,
}

impl ShmCommandListener {
    /// Create a new listener
    pub fn new(uds_path: Option<&str>, shutdown: tokio::sync::watch::Receiver<bool>) -> Self {
        let path = uds_path.unwrap_or(DEFAULT_UDS_PATH).to_string();
        info!("ShmCommandListener: UDS path = {}", path);

        Self {
            command_senders: Arc::new(DashMap::new()),
            last_sequences: Arc::new(DashMap::new()),
            uds_path: path,
            shutdown,
            dropped_count: Arc::new(AtomicU64::new(0)),
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
        // Probe first: if another listener is alive, refuse to start
        let socket_path = std::path::Path::new(&self.uds_path);
        if socket_path.exists() {
            if std::os::unix::net::UnixStream::connect(socket_path).is_ok() {
                error!(
                    "ShmListener: another listener is active on {}",
                    self.uds_path
                );
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    format!("another listener is active on {}", self.uds_path),
                ));
            }
            // Connection failed → stale socket, safe to remove
            info!("ShmListener: removing stale socket file {}", self.uds_path);
            if let Err(e) = tokio::fs::remove_file(socket_path).await {
                error!(
                    "ShmListener: failed to remove stale socket {}: {}",
                    self.uds_path, e
                );
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
                            let senders = Arc::clone(&self.command_senders);
                            let last_sequences = Arc::clone(&self.last_sequences);
                            let shutdown_rx = self.shutdown.clone();
                            let dropped_count = Arc::clone(&self.dropped_count);

                            tokio::spawn(async move {
                                Self::handle_connection(
                                    stream,
                                    senders,
                                    last_sequences,
                                    shutdown_rx,
                                    dropped_count,
                                )
                                .await;
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
        let _ = tokio::fs::remove_file(&self.uds_path).await;
        Ok(())
    }

    async fn handle_connection(
        mut stream: tokio::net::UnixStream,
        senders: Arc<DashMap<u32, mpsc::Sender<ChannelCommand>>>,
        last_sequences: Arc<SequenceMap>,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
        dropped_count: Arc<AtomicU64>,
    ) {
        debug!("ShmListener: new connection");
        let mut buf = [0u8; ShmNotification::SIZE];

        loop {
            tokio::select! {
                result = stream.read_exact(&mut buf) => {
                    match result {
                        Ok(_) => {
                            let notif = ShmNotification::from_bytes(&buf);
                            Self::handle_notification(&notif, &senders, &last_sequences, &dropped_count).await;
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
        senders: &DashMap<u32, mpsc::Sender<ChannelCommand>>,
        last_sequences: &DashMap<(u32, u8, u32), (u64, u64)>,
        dropped_count: &AtomicU64,
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
        let seq_key = (channel_id, notif.point_type, point_id);

        match last_sequences.entry(seq_key) {
            Entry::Occupied(mut entry) => {
                let (last_producer, last_seq) = *entry.get();
                if last_producer == notif.producer_id && notif.seq <= last_seq {
                    trace!(
                        "ShmListener: stale event dropped ch={} {:?}:{} producer={} seq={}<= {}",
                        channel_id,
                        point_type,
                        point_id,
                        notif.producer_id,
                        notif.seq,
                        last_seq
                    );
                    return;
                }
                entry.insert((notif.producer_id, notif.seq));
            },
            Entry::Vacant(entry) => {
                entry.insert((notif.producer_id, notif.seq));
            },
        }

        let value = notif.value();
        let timestamp = notif.timestamp_ms;

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
            "ShmListener: ch={} {:?}:{} val={} ts={} producer={} seq={}",
            channel_id,
            point_type,
            point_id,
            value,
            timestamp,
            notif.producer_id,
            notif.seq
        );

        // Build and send command
        let command = Self::build_command(
            point_type,
            point_id,
            value,
            timestamp as i64,
            notif.producer_id,
            notif.seq,
        );

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
                    dropped_count.fetch_add(1, Ordering::Relaxed);
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

    /// Returns the number of commands dropped due to channel backpressure.
    pub fn dropped_count(&self) -> u64 {
        self.dropped_count.load(Ordering::Relaxed)
    }

    fn build_command(
        point_type: PointType,
        point_id: u32,
        value: f64,
        timestamp: i64,
        producer_id: u64,
        seq: u64,
    ) -> ChannelCommand {
        let command_id = format!("uds-{producer_id:016x}-{seq}");
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
        let cmd = ShmCommandListener::build_command(PointType::Control, 5, 123.45, 1000, 7, 9);
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
        let cmd = ShmCommandListener::build_command(PointType::Adjustment, 10, 67.89, 2000, 7, 10);
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
