//! Batch routing execution module
//!
//! Provides high-performance batch write operations with integrated C2M/C2C routing.
//! This module extracts the core batch processing logic from comsrv/storage.rs
//! for reuse across services.

use anyhow::{Context, Result};
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;
use tracing::{debug, warn};
use voltage_model::PointType;
use voltage_rtdb::numfmt::{f64_to_bytes, i64_to_bytes, precomputed};
use voltage_rtdb::{KeySpaceConfig, Rtdb, WriteBuffer};

use crate::RoutingCache;

/// Type alias for C2C visited tracking (channel_id, point_type, point_id)
type C2CVisited = FxHashSet<(u32, PointType, u32)>;

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
    /// Number of C2C cycles detected and skipped
    pub cycles_detected: usize,
    /// Number of data points dropped because the channel/point had no SHM
    /// slot allocated (e.g. point was created in the DB but the channel
    /// hasn't been reloaded yet). Used as an observability signal — the
    /// previous silent-drop path made it impossible to tell if real device
    /// data was being lost during reload windows.
    pub slot_misses: usize,
}

impl BatchRoutingResult {
    /// Merge another result into this one
    pub fn merge(&mut self, other: Self) {
        self.channel_writes += other.channel_writes;
        self.c2m_writes += other.c2m_writes;
        self.c2c_forwards += other.c2c_forwards;
        self.cycles_detected += other.cycles_detected;
        self.slot_misses += other.slot_misses;
    }
}

/// Write channel batch with C2M/C2C routing (optimized with itoa/ryu)
///
/// Uses precomputed point ID pool and ryu for zero-allocation formatting.
/// Includes C2C cycle detection to prevent A→B→A routing loops.
pub async fn write_channel_batch<R>(
    rtdb: &R,
    routing_cache: &RoutingCache,
    updates: Vec<ChannelPointUpdate>,
) -> Result<BatchRoutingResult>
where
    R: Rtdb,
{
    let mut visited = C2CVisited::default();
    write_channel_batch_impl(rtdb, routing_cache, updates, &mut visited).await
}

/// Internal implementation with cycle tracking
async fn write_channel_batch_impl<R>(
    rtdb: &R,
    routing_cache: &RoutingCache,
    updates: Vec<ChannelPointUpdate>,
    visited: &mut C2CVisited,
) -> Result<BatchRoutingResult>
where
    R: Rtdb,
{
    if updates.is_empty() {
        return Ok(BatchRoutingResult::default());
    }

    // Group updates by (channel_id, point_type) - FxHashMap for faster hashing
    let mut grouped: FxHashMap<(u32, PointType), Vec<ChannelPointUpdate>> = FxHashMap::default();
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
    // Encode once; Bytes clone is O(1) (ref-counted).
    let ts_bytes = i64_to_bytes(timestamp_ms);

    let config = KeySpaceConfig::production_cached();
    let mut result = BatchRoutingResult::default();

    // Cache point_id -> Arc<str> for O(1) clone - FxHashMap for faster hashing
    let mut point_id_str_cache: FxHashMap<u32, Arc<str>> = FxHashMap::default();

    for ((channel_id, point_type), updates) in grouped {
        // Clone visited per-group to prevent cross-group false cycle detection
        // while preserving intra-chain cycle detection (A→B→A)
        let mut group_visited = visited.clone();

        // Prepare 3-layer data
        let mut points_3layer = Vec::with_capacity(updates.len());
        let mut instance_writes: FxHashMap<u32, Vec<(String, bytes::Bytes)>> = FxHashMap::default();
        // Sidecar ts hash writes, parallel to instance_writes. Written to
        // `inst:{id}:M:ts` so the apigateway WebSocket can populate the `ts`
        // field for `source='inst'` subscriptions.
        let mut instance_ts_writes: FxHashMap<u32, Vec<(String, bytes::Bytes)>> =
            FxHashMap::default();
        let mut c2c_forwards: Vec<ChannelPointUpdate> = Vec::new();

        for update in &updates {
            let raw_value = update.raw_value.unwrap_or(update.value);
            points_3layer.push((update.point_id, update.value, raw_value));

            // Track source point for cycle detection
            let source_key = (channel_id, point_type, update.point_id);
            group_visited.insert(source_key);

            // C2M routing lookup - zero-allocation using structured key
            if let Some(target) =
                routing_cache.lookup_c2m_by_parts(channel_id, point_type, update.point_id)
            {
                // Use precomputed pool or itoa, cache Arc<str> for O(1) clone
                let point_id_arc = point_id_str_cache
                    .entry(target.point_id)
                    .or_insert_with(|| precomputed::get_point_id_str_or_alloc(target.point_id));
                let point_id_string = point_id_arc.to_string();

                instance_writes
                    .entry(target.instance_id)
                    .or_default()
                    .push((point_id_string.clone(), f64_to_bytes(update.value)));
                instance_ts_writes
                    .entry(target.instance_id)
                    .or_default()
                    .push((point_id_string, ts_bytes.clone()));
            }

            // C2C routing lookup - zero-allocation using structured key
            if update.cascade_depth < MAX_C2C_CASCADE_DEPTH
                && let Some(target) =
                    routing_cache.lookup_c2c_by_parts(channel_id, point_type, update.point_id)
            {
                let target_key = (target.channel_id, target.point_type, target.point_id);

                // Cycle detection: check if target is in the forwarding chain
                if group_visited.contains(&target_key) {
                    warn!(
                        "C2C cycle detected: {}:{:?}:{} -> {}:{:?}:{} (skipping)",
                        channel_id,
                        point_type,
                        update.point_id,
                        target.channel_id,
                        target.point_type,
                        target.point_id
                    );
                    result.cycles_detected += 1;
                } else {
                    // Apply C2C linear transform to value; raw_value drops to
                    // None so the target's :raw hash reflects only its own
                    // device's pre-scale reading (a "raw" twice-transformed
                    // by both source and C2C scaling would have no physical
                    // meaning at the target channel).
                    let fwd_value = target.transform(update.value);
                    c2c_forwards.push(ChannelPointUpdate {
                        channel_id: target.channel_id,
                        point_type: target.point_type,
                        point_id: target.point_id,
                        value: fwd_value,
                        raw_value: None,
                        cascade_depth: update.cascade_depth + 1,
                    });
                }
            }
        }

        // Write 3-layer channel data
        let channel_key = config.channel_key(channel_id, point_type);
        let written = voltage_rtdb::helpers::write_channel_points(
            rtdb,
            &channel_key,
            points_3layer,
            timestamp_ms,
        )
        .await
        .context("Failed to write channel points")?;
        result.channel_writes += written;

        // Write instance data (C2M results) + sidecar ts hash
        for (instance_id, values) in instance_writes {
            let instance_key = config.instance_measurement_key(instance_id);
            rtdb.hash_mset(&instance_key, values)
                .await
                .context("Failed to write instance measurements")?;
            result.c2m_writes += 1;
        }
        for (instance_id, ts_values) in instance_ts_writes {
            let instance_ts_key = config.instance_measurement_ts_key(instance_id);
            rtdb.hash_mset(&instance_ts_key, ts_values)
                .await
                .context("Failed to write instance measurement timestamps")?;
        }

        // Process C2C forwards recursively
        if !c2c_forwards.is_empty() {
            let forward_count = c2c_forwards.len();
            debug!(
                "Processing {} C2C forwards for channel {}",
                forward_count, channel_id
            );
            let sub_result = Box::pin(write_channel_batch_impl(
                rtdb,
                routing_cache,
                c2c_forwards,
                &mut group_visited,
            ))
            .await?;
            result.c2c_forwards += forward_count;
            result.merge(sub_result);
        }
    }

    Ok(result)
}

/// Write channel points to buffer (Redis only)
///
/// Uses precomputed point ID pool and ryu for zero-allocation formatting.
/// Includes C2C cycle detection to prevent A→B→A routing loops.
///
/// # Arguments
/// * `write_buffer` - WriteBuffer for deferred Redis writes
/// * `routing_cache` - Routing cache for C2M/C2C lookups
/// * `updates` - Point updates to process
pub fn write_channel_batch_buffered(
    write_buffer: &WriteBuffer,
    routing_cache: &RoutingCache,
    updates: Vec<ChannelPointUpdate>,
) -> BatchRoutingResult {
    let mut visited = C2CVisited::default();
    write_channel_batch_buffered_impl(write_buffer, routing_cache, updates, &mut visited)
}

/// Internal implementation with cycle tracking
fn write_channel_batch_buffered_impl(
    write_buffer: &WriteBuffer,
    routing_cache: &RoutingCache,
    updates: Vec<ChannelPointUpdate>,
    visited: &mut C2CVisited,
) -> BatchRoutingResult {
    if updates.is_empty() {
        return BatchRoutingResult::default();
    }

    // Group updates by (channel_id, point_type) - FxHashMap for faster hashing
    let mut grouped: FxHashMap<(u32, PointType), Vec<ChannelPointUpdate>> = FxHashMap::default();
    for update in updates {
        grouped
            .entry((update.channel_id, update.point_type))
            .or_default()
            .push(update);
    }

    // Get current timestamp
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_millis() as i64;
    // Encode once; Bytes clone is O(1) (ref-counted).
    let ts_bytes = i64_to_bytes(timestamp_ms);

    let config = KeySpaceConfig::production_cached();
    let mut result = BatchRoutingResult::default();

    // Cache point_id -> Arc<str> using precomputed pool - FxHashMap
    let mut point_id_str_cache: FxHashMap<u32, Arc<str>> = FxHashMap::default();

    for ((channel_id, point_type), updates) in grouped {
        // Clone visited per-group to prevent cross-group false cycle detection
        // while preserving intra-chain cycle detection (A→B→A)
        let mut group_visited = visited.clone();

        // Prepare 3-layer data
        let mut points_3layer = Vec::with_capacity(updates.len());
        // Use Arc<str> for field names to match WriteBuffer signature - FxHashMap
        let mut instance_writes: FxHashMap<u32, Vec<(Arc<str>, bytes::Bytes)>> =
            FxHashMap::default();
        // Sidecar ts hash writes, parallel to instance_writes. See the async
        // variant for the rationale (inst:{id}:M:ts for WebSocket `ts` field).
        let mut instance_ts_writes: FxHashMap<u32, Vec<(Arc<str>, bytes::Bytes)>> =
            FxHashMap::default();
        let mut c2c_forwards: Vec<ChannelPointUpdate> = Vec::new();

        for update in &updates {
            let raw_value = update.raw_value.unwrap_or(update.value);
            points_3layer.push((update.point_id, update.value, raw_value));

            // Track source point for cycle detection
            let source_key = (channel_id, point_type, update.point_id);
            group_visited.insert(source_key);

            // C2M routing lookup - zero-allocation using structured key
            if let Some(target) =
                routing_cache.lookup_c2m_by_parts(channel_id, point_type, update.point_id)
            {
                // Use precomputed pool (0-255) or itoa, O(1) Arc clone
                let point_id_str = point_id_str_cache
                    .entry(target.point_id)
                    .or_insert_with(|| precomputed::get_point_id_str_or_alloc(target.point_id))
                    .clone();

                instance_writes
                    .entry(target.instance_id)
                    .or_default()
                    .push((Arc::clone(&point_id_str), f64_to_bytes(update.value)));
                instance_ts_writes
                    .entry(target.instance_id)
                    .or_default()
                    .push((point_id_str, ts_bytes.clone()));
            }

            // C2C routing lookup - zero-allocation using structured key
            if update.cascade_depth < MAX_C2C_CASCADE_DEPTH
                && let Some(target) =
                    routing_cache.lookup_c2c_by_parts(channel_id, point_type, update.point_id)
            {
                let target_key = (target.channel_id, target.point_type, target.point_id);

                // Cycle detection: check if target is in the forwarding chain
                if group_visited.contains(&target_key) {
                    warn!(
                        "C2C cycle detected: {}:{:?}:{} -> {}:{:?}:{} (skipping)",
                        channel_id,
                        point_type,
                        update.point_id,
                        target.channel_id,
                        target.point_type,
                        target.point_id
                    );
                    result.cycles_detected += 1;
                } else {
                    // Same semantics as the buffered path: transform value,
                    // drop raw_value so :raw is not polluted with a
                    // twice-transformed pseudo-raw.
                    let fwd_value = target.transform(update.value);
                    c2c_forwards.push(ChannelPointUpdate {
                        channel_id: target.channel_id,
                        point_type: target.point_type,
                        point_id: target.point_id,
                        value: fwd_value,
                        raw_value: None,
                        cascade_depth: update.cascade_depth + 1,
                    });
                }
            }
        }

        let channel_key = config.channel_key(channel_id, point_type);

        // Buffer 3-layer channel data to WriteBuffer (for Redis)
        let buffered = voltage_rtdb::helpers::buffer_channel_points(
            write_buffer,
            &channel_key,
            points_3layer,
            timestamp_ms,
        );
        result.channel_writes += buffered;

        // Buffer instance data (C2M results) + sidecar ts hash.
        // Surface BufferOverflow via warn! and slot_misses so dropped
        // fields are observable rather than silently lost; the c2m_writes
        // counter still increments on attempt so call sites that don't
        // distinguish "tried" from "succeeded" stay backward-compatible.
        for (instance_id, values) in instance_writes {
            let instance_key = config.instance_measurement_key(instance_id);
            if let Err(e) = write_buffer.buffer_hash_mset(&instance_key, values) {
                warn!("C2M instance buffer dropped: {}", e);
                result.slot_misses += e.dropped_fields;
            }
            result.c2m_writes += 1;
        }
        for (instance_id, ts_values) in instance_ts_writes {
            let instance_ts_key = config.instance_measurement_ts_key(instance_id);
            if let Err(e) = write_buffer.buffer_hash_mset(&instance_ts_key, ts_values) {
                warn!("C2M ts buffer dropped: {}", e);
                result.slot_misses += e.dropped_fields;
            }
        }

        // Process C2C forwards recursively (also buffered)
        if !c2c_forwards.is_empty() {
            let forward_count = c2c_forwards.len();
            debug!(
                "Processing {} C2C forwards for channel {}",
                forward_count, channel_id
            );
            let sub_result = write_channel_batch_buffered_impl(
                write_buffer,
                routing_cache,
                c2c_forwards,
                &mut group_visited,
            );
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
            cycles_detected: 1,
            slot_misses: 4,
        };
        let r2 = BatchRoutingResult {
            channel_writes: 3,
            c2m_writes: 1,
            c2c_forwards: 1,
            cycles_detected: 2,
            slot_misses: 1,
        };
        r1.merge(r2);
        assert_eq!(r1.channel_writes, 13);
        assert_eq!(r1.c2m_writes, 6);
        assert_eq!(r1.c2c_forwards, 3);
        assert_eq!(r1.cycles_detected, 3);
        assert_eq!(r1.slot_misses, 5);
    }
}
