//! Warning Monitor Module
//!
//! Subscribes to Redis Pub/Sub channels for real-time warning notifications
//! from Redis Lua functions (queue overflow, unmapped points, etc.)

use futures::StreamExt;
use redis::{Client, Cmd, RedisResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use voltage_config::modsrv::InstanceRedisKeys;

/// Queue overflow warning data
#[derive(Debug, Serialize, Deserialize)]
pub struct QueueOverflowWarning {
    pub service: String,
    pub channel_id: u16,
    pub point_type: String,
    pub queue_length: usize,
    pub timestamp: i64,
    pub severity: String,
}

/// Unmapped points warning data
#[derive(Debug, Serialize, Deserialize)]
pub struct UnmappedPointsWarning {
    pub service: String,
    pub channel_id: u16,
    pub telemetry_type: String,
    pub unmapped_count: u32,
    pub routed_count: u32,
    pub timestamp: i64,
    pub severity: String,
}

/// Warning statistics for tracking
#[derive(Debug, Default)]
pub struct WarningStats {
    pub queue_overflow_count: u64,
    pub queue_high_count: u64,
    pub unmapped_points_count: u64,
    pub last_queue_overflow: Option<i64>,
    pub last_unmapped_points: Option<i64>,
}

/// Start the warning monitor that subscribes to Redis warning channels
pub async fn start_warning_monitor(redis_url: String, token: CancellationToken) -> RedisResult<()> {
    let client = Client::open(redis_url.as_str())?;
    let mut pubsub = client.get_async_pubsub().await?;

    // Subscribe to warning channels
    pubsub
        .subscribe(&[
            "warnings:queue_overflow",
            "warnings:queue_high",
            "warnings:unmapped_points",
        ])
        .await?;

    info!("WarnMonitor started");

    let stats = Arc::new(RwLock::new(WarningStats::default()));

    let mut pubsub_stream = pubsub.on_message();

    loop {
        tokio::select! {
            Some(msg) = pubsub_stream.next() => {
                let channel = msg.get_channel_name();
                let payload: String = msg.get_payload()?;

                match channel {
                    "warnings:queue_overflow" => {
                        match serde_json::from_str::<QueueOverflowWarning>(&payload) {
                            Ok(data) => {
                                error!(
                                    "QUEUE OVERFLOW: {} Ch{}:{} len={}",
                                    data.service, data.channel_id, data.point_type, data.queue_length
                                );

                                // Update statistics
                                let mut s = stats.write().await;
                                s.queue_overflow_count += 1;
                                s.last_queue_overflow = Some(data.timestamp);

                                // Here you could add more actions:
                                // - Send alerts to monitoring systems
                                // - Trigger auto-recovery procedures
                                // - Log to database for analysis
                            }
                            Err(e) => {
                                error!("Queue overflow parse: {}", e);
                            }
                        }
                    }
                    "warnings:queue_high" => {
                        match serde_json::from_str::<QueueOverflowWarning>(&payload) {
                            Ok(data) => {
                                warn!(
                                    "Queue high: {} Ch{}:{} len={}",
                                    data.service, data.channel_id, data.point_type, data.queue_length
                                );

                                // Update statistics
                                let mut s = stats.write().await;
                                s.queue_high_count += 1;
                            }
                            Err(e) => {
                                error!("Queue high parse: {}", e);
                            }
                        }
                    }
                    "warnings:unmapped_points" => {
                        match serde_json::from_str::<UnmappedPointsWarning>(&payload) {
                            Ok(data) => {
                                if data.unmapped_count > 10 {
                                    warn!(
                                        "Unmapped: {} Ch{}:{} unmapped={} routed={}",
                                        data.service, data.channel_id, data.telemetry_type, data.unmapped_count, data.routed_count
                                    );
                                } else {
                                    debug!(
                                        "Unmapped: {} Ch{}:{} unmapped={} routed={}",
                                        data.service, data.channel_id, data.telemetry_type, data.unmapped_count, data.routed_count
                                    );
                                }

                                // Update statistics
                                let mut s = stats.write().await;
                                s.unmapped_points_count += data.unmapped_count as u64;
                                s.last_unmapped_points = Some(data.timestamp);

                                // Could trigger configuration validation or auto-mapping
                            }
                            Err(e) => {
                                error!("Unmapped parse: {}", e);
                            }
                        }
                    }
                    _ => {
                        debug!("Unknown channel: {}", channel);
                    }
                }
            }
            _ = token.cancelled() => {
                debug!("WarnMonitor stopping");
                break;
            }
        }
    }

    // Print final statistics
    let s = stats.read().await;
    info!(
        "WarnMonitor stats: overflow={} high={} unmapped={}",
        s.queue_overflow_count, s.queue_high_count, s.unmapped_points_count
    );

    Ok(())
}

/// Alternative: Polling-based monitor for warning statistics
pub async fn start_stats_poller(
    redis_url: String,
    interval_ms: u64,
    token: CancellationToken,
) -> RedisResult<()> {
    let client = Client::open(redis_url.as_str())?;
    let mut con = client.get_multiplexed_tokio_connection().await?;
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(interval_ms));

    info!("StatsPoller: {}ms interval", interval_ms);

    let mut last_values = std::collections::HashMap::new();

    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Check modsrv warnings
                let modsrv_warnings: RedisResult<Vec<(String, i64)>> = Cmd::hgetall(InstanceRedisKeys::STATS_WARNINGS)
                    .query_async(&mut con)
                    .await;

                if let Ok(warnings) = modsrv_warnings {
                    for (key, count) in warnings {
                        let last = last_values.get(&key).copied().unwrap_or(0);
                        if count > last {
                            warn!("ModSrv {}: {} -> {}", key, last, count);
                            last_values.insert(key, count);
                        }
                    }
                }

                // Check comsrv unmapped points
                let unmapped: RedisResult<Vec<(String, i64)>> = Cmd::hgetall("comsrv:stats:unmapped_total")
                    .query_async(&mut con)
                    .await;

                if let Ok(unmapped_points) = unmapped {
                    for (key, count) in unmapped_points {
                        if count > 0 {
                            let last_key = format!("unmapped_{}", key);
                            let last = last_values.get(&last_key).copied().unwrap_or(0);

                            if count != last {
                                debug!("Unmapped {}: {}", key, count);
                                last_values.insert(last_key, count);
                            }
                        }
                    }
                }
            }
            _ = token.cancelled() => {
                debug!("StatsPoller stopping");
                break;
            }
        }
    }

    Ok(())
}
