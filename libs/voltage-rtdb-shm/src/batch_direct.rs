//! High-performance batch write using direct shared memory writes
//!
//! Provides `write_channel_batch_direct` which combines:
//! - Direct SHM slot writes via ChannelToSlotIndex (~10ns per point)
//! - Redis backup via WriteBuffer
//! - C2C routing with cycle detection

use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;
use tracing::{debug, warn};
use voltage_model::PointType;
use voltage_rtdb::numfmt::{f64_to_bytes, precomputed};
use voltage_rtdb::{KeySpaceConfig, WriteBuffer};

use crate::{ChannelToSlotIndex, UnifiedWriter};
use voltage_routing::batch::{BatchRoutingResult, ChannelPointUpdate};
use voltage_routing::{RoutingCache, MAX_C2C_CASCADE_DEPTH};

/// Type alias for C2C visited tracking (channel_id, point_type, point_id)
type C2CVisited = FxHashSet<(u32, PointType, u32)>;

/// High-performance batch write using direct channel-to-slot mapping
///
/// Uses ChannelToSlotIndex to bypass C2M routing lookup during writes.
/// Uses precomputed point ID pool and ryu for zero-allocation formatting.
/// Includes C2C cycle detection to prevent A→B→A routing loops.
///
/// # Architecture
/// ```text
/// Before (write_channel_batch_buffered):
///   Channel Update → C2M Lookup (~25ns) → Instance Key → WriteBuffer
///
/// After (write_channel_batch_direct):
///   Channel Update → ChannelToSlotIndex (~50ns) → SharedMemory Direct Write (~10ns)
///   Channel Update → WriteBuffer (Redis backup)
/// ```
///
/// # Arguments
/// * `shared_writer` - UnifiedWriter for direct memory writes
/// * `channel_index` - Pre-computed channel-to-slot mapping
/// * `write_buffer` - WriteBuffer for deferred Redis writes (backup path)
/// * `routing_cache` - Routing cache for C2C lookups
/// * `updates` - Point updates to process
///
/// # Returns
/// BatchRoutingResult with write counts.
pub fn write_channel_batch_direct(
    shared_writer: &UnifiedWriter,
    channel_index: &ChannelToSlotIndex,
    write_buffer: &WriteBuffer,
    routing_cache: &RoutingCache,
    updates: Vec<ChannelPointUpdate>,
) -> BatchRoutingResult {
    let mut visited = C2CVisited::default();
    write_channel_batch_direct_impl(
        shared_writer,
        channel_index,
        write_buffer,
        routing_cache,
        updates,
        &mut visited,
    )
}

/// Internal implementation with cycle tracking
fn write_channel_batch_direct_impl(
    shared_writer: &UnifiedWriter,
    channel_index: &ChannelToSlotIndex,
    write_buffer: &WriteBuffer,
    routing_cache: &RoutingCache,
    updates: Vec<ChannelPointUpdate>,
    visited: &mut C2CVisited,
) -> BatchRoutingResult {
    if updates.is_empty() {
        return BatchRoutingResult::default();
    }

    // Get current timestamp
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_millis() as u64;

    let config = KeySpaceConfig::production_cached();
    let mut result = BatchRoutingResult::default();

    // Group updates by (channel_id, point_type) for efficient buffer writes - FxHashMap
    let mut grouped: FxHashMap<(u32, PointType), Vec<ChannelPointUpdate>> = FxHashMap::default();
    for update in updates {
        grouped
            .entry((update.channel_id, update.point_type))
            .or_default()
            .push(update);
    }

    // Cache point_id -> Arc<str> using precomputed pool - FxHashMap
    let mut point_id_str_cache: FxHashMap<u32, Arc<str>> = FxHashMap::default();

    for ((channel_id, point_type), updates) in grouped {
        // Clone visited per-group to prevent cross-group false cycle detection
        // while preserving intra-chain cycle detection (A→B→A)
        let mut group_visited = visited.clone();

        // Prepare 3-layer data for Redis backup
        let mut points_3layer = Vec::with_capacity(updates.len());
        // Instance writes for C2M routing (Redis backup) - FxHashMap
        let mut instance_writes: FxHashMap<u32, Vec<(Arc<str>, bytes::Bytes)>> =
            FxHashMap::default();
        let mut c2c_forwards: Vec<ChannelPointUpdate> = Vec::new();

        for update in &updates {
            let raw_value = update.raw_value.unwrap_or(update.value);
            points_3layer.push((update.point_id, update.value, raw_value));

            // Track source point for cycle detection
            let source_key = (channel_id, point_type, update.point_id);
            group_visited.insert(source_key);

            // Direct shared memory write (fastest path)
            // Single write - UnifiedReader builds both channel and instance indexes from SlotMeta
            if let Some(slot) = channel_index.lookup(channel_id, point_type, update.point_id) {
                shared_writer.set_direct(slot, update.value, raw_value, timestamp_ms);
                result.channel_writes += 1;
            }

            // C2M routing for Redis backup
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
                    .push((point_id_str, f64_to_bytes(update.value)));
            }

            // C2C routing lookup
            if update.cascade_depth < MAX_C2C_CASCADE_DEPTH {
                if let Some(target) =
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
        }

        // Buffer 3-layer channel data to WriteBuffer (Redis backup)
        let channel_key = config.channel_key(channel_id, point_type);
        voltage_rtdb::helpers::buffer_channel_points(
            write_buffer,
            &channel_key,
            points_3layer,
            timestamp_ms as i64,
        );

        // Buffer instance data (C2M results for Redis)
        for (instance_id, values) in instance_writes {
            let instance_key = config.instance_measurement_key(instance_id);
            write_buffer.buffer_hash_mset(&instance_key, values);
            result.c2m_writes += 1;
        }

        // Process C2C forwards recursively (per-group)
        if !c2c_forwards.is_empty() {
            let forward_count = c2c_forwards.len();
            debug!(
                "Processing {} C2C forwards with direct write",
                forward_count
            );
            let sub_result = write_channel_batch_direct_impl(
                shared_writer,
                channel_index,
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
