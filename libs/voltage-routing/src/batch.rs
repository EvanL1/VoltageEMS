//! Batch routing execution module
//!
//! Provides high-performance batch write operations with integrated C2M/C2C routing.
//! This module extracts the core batch processing logic from comsrv/storage.rs
//! for reuse across services.

use anyhow::{Context, Result};
use std::collections::HashMap;
use tracing::debug;
use voltage_model::PointType;
use voltage_rtdb::{KeySpaceConfig, RoutingCache, Rtdb, WriteBuffer};

use crate::MAX_C2C_CASCADE_DEPTH;

/// Channel point update for batch operations
#[derive(Debug, Clone)]
pub struct ChannelPointUpdate {
    /// Channel ID
    pub channel_id: u32,
    /// Point type (T/S/C/A)
    pub point_type: PointType,
    /// Point ID
    pub point_id: u32,
    /// Engineering value (after scale/offset/reverse)
    pub value: f64,
    /// Raw value before transformation (None = same as value)
    pub raw_value: Option<f64>,
    /// Cascade depth for C2C routing (prevents infinite loops)
    pub cascade_depth: u8,
}

impl ChannelPointUpdate {
    /// Create a new update with default cascade depth (0)
    pub fn new(channel_id: u32, point_type: PointType, point_id: u32, value: f64) -> Self {
        Self {
            channel_id,
            point_type,
            point_id,
            value,
            raw_value: None,
            cascade_depth: 0,
        }
    }

    /// Create with explicit raw value
    pub fn with_raw(mut self, raw_value: f64) -> Self {
        self.raw_value = Some(raw_value);
        self
    }
}

/// Result of batch routing execution
#[derive(Debug, Default, Clone)]
pub struct BatchRoutingResult {
    /// Number of channel points written
    pub channel_writes: usize,
    /// Number of instance measurement writes (C2M routing)
    pub c2m_writes: usize,
    /// Number of C2C forwards processed
    pub c2c_forwards: usize,
}

impl BatchRoutingResult {
    /// Merge another result into this one
    pub fn merge(&mut self, other: Self) {
        self.channel_writes += other.channel_writes;
        self.c2m_writes += other.c2m_writes;
        self.c2c_forwards += other.c2c_forwards;
    }
}

/// Batch write channel points with C2M/C2C routing (direct Redis writes)
///
/// This function implements the complete batch write flow:
/// 1. Groups updates by (channel_id, point_type)
/// 2. Writes 3-layer data (value/ts/raw) to channel hashes
/// 3. Executes C2M routing (writes to instance measurement hashes)
/// 4. Executes C2C routing (recursive forwards to other channels)
///
/// # Arguments
/// * `rtdb` - RTDB trait object
/// * `routing_cache` - C2M/C2C routing cache
/// * `updates` - Vector of channel point updates
///
/// # Returns
/// * `Ok(BatchRoutingResult)` - Statistics about writes performed
/// * `Err(anyhow::Error)` - Write error
pub async fn write_channel_batch<R>(
    rtdb: &R,
    routing_cache: &RoutingCache,
    updates: Vec<ChannelPointUpdate>,
) -> Result<BatchRoutingResult>
where
    R: Rtdb + ?Sized,
{
    if updates.is_empty() {
        return Ok(BatchRoutingResult::default());
    }

    // Group updates by (channel_id, point_type)
    let mut grouped: HashMap<(u32, PointType), Vec<ChannelPointUpdate>> = HashMap::new();
    for update in updates {
        grouped
            .entry((update.channel_id, update.point_type))
            .or_default()
            .push(update);
    }

    // Get current timestamp
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("Failed to get timestamp")?
        .as_millis() as i64;

    let config = KeySpaceConfig::production();
    let mut result = BatchRoutingResult::default();

    for ((channel_id, point_type), updates) in grouped {
        let point_type_str = point_type.as_str();

        // Prepare 3-layer data
        let mut points_3layer = Vec::with_capacity(updates.len());
        let mut instance_writes: HashMap<u32, Vec<(String, bytes::Bytes)>> = HashMap::new();
        let mut c2c_forwards: Vec<ChannelPointUpdate> = Vec::new();

        for update in &updates {
            let raw_value = update.raw_value.unwrap_or(update.value);
            points_3layer.push((update.point_id, update.value, raw_value));

            // C2M routing lookup - directly use RoutingCache
            let route_key = format!("{}:{}:{}", channel_id, point_type_str, update.point_id);
            if let Some(target) = routing_cache.lookup_c2m(&route_key) {
                instance_writes
                    .entry(target.instance_id)
                    .or_default()
                    .push((
                        target.point_id.to_string(),
                        bytes::Bytes::from(update.value.to_string()),
                    ));
            }

            // C2C routing lookup - directly use RoutingCache
            if update.cascade_depth < MAX_C2C_CASCADE_DEPTH {
                if let Some(target) = routing_cache.lookup_c2c(&route_key) {
                    c2c_forwards.push(ChannelPointUpdate {
                        channel_id: target.channel_id,
                        point_type: target.point_type,
                        point_id: target.point_id,
                        value: update.value,
                        raw_value: update.raw_value,
                        cascade_depth: update.cascade_depth + 1,
                    });
                }
            }
        }

        // Write 3-layer channel data
        let channel_key = config.channel_key(channel_id, point_type);
        let written = voltage_rtdb::helpers::set_channel_points_3layer(
            rtdb,
            &channel_key,
            points_3layer,
            timestamp_ms,
        )
        .await
        .context("Failed to write channel points")?;
        result.channel_writes += written;

        // Write instance data (C2M results)
        for (instance_id, values) in instance_writes {
            let instance_key = config.instance_measurement_key(instance_id);
            rtdb.hash_mset(&instance_key, values)
                .await
                .context("Failed to write instance measurements")?;
            result.c2m_writes += 1;
        }

        // Process C2C forwards recursively
        if !c2c_forwards.is_empty() {
            let forward_count = c2c_forwards.len();
            debug!(
                "Processing {} C2C forwards for channel {}",
                forward_count, channel_id
            );
            let sub_result =
                Box::pin(write_channel_batch(rtdb, routing_cache, c2c_forwards)).await?;
            result.c2c_forwards += forward_count;
            result.merge(sub_result);
        }
    }

    Ok(result)
}

/// Batch write channel points with C2M/C2C routing (buffered writes)
///
/// Similar to `write_channel_batch` but uses WriteBuffer for aggregation
/// instead of direct Redis writes. This reduces network round-trips for
/// high-frequency updates.
///
/// # Arguments
/// * `write_buffer` - WriteBuffer for aggregating writes
/// * `routing_cache` - C2M/C2C routing cache
/// * `updates` - Vector of channel point updates
///
/// # Returns
/// Statistics about writes buffered
pub fn write_channel_batch_buffered(
    write_buffer: &WriteBuffer,
    routing_cache: &RoutingCache,
    updates: Vec<ChannelPointUpdate>,
) -> BatchRoutingResult {
    if updates.is_empty() {
        return BatchRoutingResult::default();
    }

    // Group updates by (channel_id, point_type)
    let mut grouped: HashMap<(u32, PointType), Vec<ChannelPointUpdate>> = HashMap::new();
    for update in updates {
        grouped
            .entry((update.channel_id, update.point_type))
            .or_default()
            .push(update);
    }

    // Get current timestamp
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let config = KeySpaceConfig::production();
    let mut result = BatchRoutingResult::default();

    for ((channel_id, point_type), updates) in grouped {
        let point_type_str = point_type.as_str();

        // Prepare 3-layer data
        let mut points_3layer = Vec::with_capacity(updates.len());
        let mut instance_writes: HashMap<u32, Vec<(String, bytes::Bytes)>> = HashMap::new();
        let mut c2c_forwards: Vec<ChannelPointUpdate> = Vec::new();

        for update in &updates {
            let raw_value = update.raw_value.unwrap_or(update.value);
            points_3layer.push((update.point_id, update.value, raw_value));

            // C2M routing lookup - directly use RoutingCache
            let route_key = format!("{}:{}:{}", channel_id, point_type_str, update.point_id);
            if let Some(target) = routing_cache.lookup_c2m(&route_key) {
                instance_writes
                    .entry(target.instance_id)
                    .or_default()
                    .push((
                        target.point_id.to_string(),
                        bytes::Bytes::from(update.value.to_string()),
                    ));
            }

            // C2C routing lookup - directly use RoutingCache
            if update.cascade_depth < MAX_C2C_CASCADE_DEPTH {
                if let Some(target) = routing_cache.lookup_c2c(&route_key) {
                    c2c_forwards.push(ChannelPointUpdate {
                        channel_id: target.channel_id,
                        point_type: target.point_type,
                        point_id: target.point_id,
                        value: update.value,
                        raw_value: update.raw_value,
                        cascade_depth: update.cascade_depth + 1,
                    });
                }
            }
        }

        // Buffer 3-layer channel data
        let channel_key = config.channel_key(channel_id, point_type);
        let buffered = voltage_rtdb::helpers::buffer_channel_points_3layer(
            write_buffer,
            &channel_key,
            points_3layer,
            timestamp_ms,
        );
        result.channel_writes += buffered;

        // Buffer instance data (C2M results)
        for (instance_id, values) in instance_writes {
            let instance_key = config.instance_measurement_key(instance_id);
            write_buffer.buffer_hash_mset(&instance_key, values);
            result.c2m_writes += 1;
        }

        // Process C2C forwards recursively (also buffered)
        if !c2c_forwards.is_empty() {
            let forward_count = c2c_forwards.len();
            debug!(
                "Processing {} C2C forwards for channel {}",
                forward_count, channel_id
            );
            let sub_result =
                write_channel_batch_buffered(write_buffer, routing_cache, c2c_forwards);
            result.c2c_forwards += forward_count;
            result.merge(sub_result);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_point_update_new() {
        let update = ChannelPointUpdate::new(1001, PointType::Telemetry, 1, 42.5);
        assert_eq!(update.channel_id, 1001);
        assert_eq!(update.point_type, PointType::Telemetry);
        assert_eq!(update.point_id, 1);
        assert_eq!(update.value, 42.5);
        assert!(update.raw_value.is_none());
        assert_eq!(update.cascade_depth, 0);
    }

    #[test]
    fn test_channel_point_update_with_raw() {
        let update = ChannelPointUpdate::new(1001, PointType::Telemetry, 1, 42.5).with_raw(4250.0);
        assert_eq!(update.value, 42.5);
        assert_eq!(update.raw_value, Some(4250.0));
    }

    #[test]
    fn test_batch_routing_result_merge() {
        let mut r1 = BatchRoutingResult {
            channel_writes: 10,
            c2m_writes: 5,
            c2c_forwards: 2,
        };
        let r2 = BatchRoutingResult {
            channel_writes: 3,
            c2m_writes: 1,
            c2c_forwards: 1,
        };
        r1.merge(r2);
        assert_eq!(r1.channel_writes, 13);
        assert_eq!(r1.c2m_writes, 6);
        assert_eq!(r1.c2c_forwards, 3);
    }
}
