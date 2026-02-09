//! Shared Memory Command Poller
//!
//! Polls shared memory for Control/Adjustment point changes and dispatches commands.
//! This replaces the Redis TODO queue mechanism with direct SHM monitoring.

use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, info, trace, warn};
use uuid::Uuid;
use voltage_model::PointType;
use voltage_rtdb::{RoutingCache, UnifiedReader};

use crate::core::channels::types::ChannelCommand;

/// Configuration for a channel's action points to poll
#[derive(Debug, Clone)]
pub struct ChannelPollerConfig {
    pub channel_id: u32,
    pub control_point_count: u32,
    pub adjustment_point_count: u32,
}

/// Shared Memory Command Poller
pub struct ShmCommandPoller {
    reader: Arc<UnifiedReader>,
    channel_configs: Vec<ChannelPollerConfig>,
    last_timestamps: DashMap<(u32, u8, u32), u64>,
    poll_interval: Duration,
    command_senders: DashMap<u32, mpsc::Sender<ChannelCommand>>,
    shutdown: tokio::sync::watch::Receiver<bool>,
}

impl ShmCommandPoller {
    pub fn new(
        reader: Arc<UnifiedReader>,
        routing_cache: &RoutingCache,
        poll_interval: Duration,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Self {
        let channel_configs = Self::build_channel_configs(routing_cache);
        info!(
            "ShmCommandPoller: {} channels, poll={}ms",
            channel_configs.len(),
            poll_interval.as_millis()
        );

        Self {
            reader,
            channel_configs,
            last_timestamps: DashMap::new(),
            poll_interval,
            command_senders: DashMap::new(),
            shutdown,
        }
    }

    fn build_channel_configs(routing_cache: &RoutingCache) -> Vec<ChannelPollerConfig> {
        use std::collections::HashMap;
        let mut channel_points: HashMap<u32, [u32; 2]> = HashMap::new();

        for (_key, target) in routing_cache.m2c_iter() {
            let counts = channel_points.entry(target.channel_id).or_insert([0, 0]);
            let type_idx = match target.point_type {
                PointType::Control => 0,
                PointType::Adjustment => 1,
                _ => continue,
            };
            counts[type_idx] = counts[type_idx].max(target.point_id + 1);
        }

        channel_points
            .into_iter()
            .filter(|(_, counts)| counts[0] > 0 || counts[1] > 0)
            .map(|(channel_id, counts)| ChannelPollerConfig {
                channel_id,
                control_point_count: counts[0],
                adjustment_point_count: counts[1],
            })
            .collect()
    }

    pub fn register_channel(&self, channel_id: u32, sender: mpsc::Sender<ChannelCommand>) {
        self.command_senders.insert(channel_id, sender);
        debug!("ShmPoller: registered channel {}", channel_id);
    }

    pub fn unregister_channel(&self, channel_id: u32) {
        self.command_senders.remove(&channel_id);
    }

    pub async fn run(&self) {
        info!("ShmCommandPoller starting");
        let mut shutdown = self.shutdown.clone();
        loop {
            tokio::select! {
                _ = tokio::time::sleep(self.poll_interval) => {
                    self.poll_all_channels().await;
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("ShmCommandPoller shutdown");
                        break;
                    }
                }
            }
        }
    }

    async fn poll_all_channels(&self) {
        for config in &self.channel_configs {
            if !self.command_senders.contains_key(&config.channel_id) {
                continue;
            }
            for point_id in 0..config.control_point_count {
                self.check_point(config.channel_id, PointType::Control, point_id)
                    .await;
            }
            for point_id in 0..config.adjustment_point_count {
                self.check_point(config.channel_id, PointType::Adjustment, point_id)
                    .await;
            }
        }
    }

    async fn check_point(&self, channel_id: u32, point_type: PointType, point_id: u32) {
        let type_u8 = point_type.to_u8();
        let key = (channel_id, type_u8, point_id);

        let (value, current_ts) = match self.reader.get_channel(channel_id, type_u8, point_id) {
            Some(data) => data,
            None => return,
        };

        let should_trigger = match self.last_timestamps.get(&key) {
            Some(last_ts) if current_ts > *last_ts => true,
            None if current_ts > 0 => true,
            _ => false,
        };

        if !should_trigger {
            return;
        }

        self.last_timestamps.insert(key, current_ts);
        trace!(
            "SHM change: ch={} {:?}:{} val={}",
            channel_id,
            point_type,
            point_id,
            value
        );

        let command = Self::build_command(point_type, point_id, value, current_ts as i64);
        if let Some(sender) = self.command_senders.get(&channel_id) {
            // Use send_timeout instead of try_send to handle transient backpressure
            match tokio::time::timeout(std::time::Duration::from_millis(50), sender.send(command))
                .await
            {
                Ok(Ok(())) => {},
                Ok(Err(_)) => {
                    warn!("ShmPoller: channel {} closed", channel_id);
                },
                Err(_) => {
                    warn!("ShmPoller: channel {} buffer full for 50ms", channel_id);
                },
            }
        }
    }

    fn build_command(
        point_type: PointType,
        point_id: u32,
        value: f64,
        timestamp: i64,
    ) -> ChannelCommand {
        let command_id = format!("shm-{}", Uuid::new_v4());
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
