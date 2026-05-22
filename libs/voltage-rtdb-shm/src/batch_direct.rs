//! High-performance batch write using direct shared memory writes
//!
//! Provides `write_channel_batch_direct` which combines:
//! - Direct SHM slot writes via ChannelToSlotIndex (~10ns per point)
//! - C2C routing with cycle detection
//!
//! Redis sync is handled asynchronously by `ShmRedisSync` (comsrv background task).

use rustc_hash::{FxHashMap, FxHashSet};
use tracing::{debug, warn};
use voltage_model::PointType;

use crate::{ChannelToSlotIndex, UnifiedWriter};
use voltage_routing::batch::{BatchRoutingResult, ChannelPointUpdate};
use voltage_routing::{MAX_C2C_CASCADE_DEPTH, RoutingCache};

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
/// * `routing_cache` - Routing cache for C2C lookups
/// * `updates` - Point updates to process
///
/// # Returns
/// BatchRoutingResult with write counts.
pub fn write_channel_batch_direct(
    shared_writer: &UnifiedWriter,
    channel_index: &ChannelToSlotIndex,
    routing_cache: &RoutingCache,
    updates: Vec<ChannelPointUpdate>,
) -> BatchRoutingResult {
    let mut visited = C2CVisited::default();
    write_channel_batch_direct_impl(
        shared_writer,
        channel_index,
        routing_cache,
        updates,
        &mut visited,
    )
}

/// Internal implementation with cycle tracking
fn write_channel_batch_direct_impl(
    shared_writer: &UnifiedWriter,
    channel_index: &ChannelToSlotIndex,
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

    let mut result = BatchRoutingResult::default();

    // Group updates by (channel_id, point_type) for C2C tracking.
    // Single-writer invariant: comsrv owns Telemetry/Status slots only;
    // C/A slots belong to modsrv. Silently dropping cross-writer points
    // would mask protocol-adapter bugs, so we warn and skip.
    let mut grouped: FxHashMap<(u32, PointType), Vec<ChannelPointUpdate>> = FxHashMap::default();
    for update in updates {
        if !update.point_type.is_measurement() {
            warn!(
                "batch_direct refused cross-writer slot ch={} pt={:?} point={} (only T/S allowed)",
                update.channel_id, update.point_type, update.point_id
            );
            continue;
        }
        grouped
            .entry((update.channel_id, update.point_type))
            .or_default()
            .push(update);
    }

    for ((channel_id, point_type), updates) in grouped {
        let mut group_visited = visited.clone();
        let mut c2c_forwards: Vec<ChannelPointUpdate> = Vec::new();

        for update in &updates {
            let raw_value = update.raw_value.unwrap_or(update.value);

            // Track source point for cycle detection
            group_visited.insert((channel_id, point_type, update.point_id));

            // Direct shared memory write — the only hot-path write.
            // Redis sync is handled by ShmRedisSync background task.
            if let Some(slot) = channel_index.lookup(channel_id, point_type, update.point_id) {
                shared_writer.set_direct(slot, update.value, raw_value, timestamp_ms);
                result.channel_writes += 1;
            }

            // C2C routing lookup
            if update.cascade_depth < MAX_C2C_CASCADE_DEPTH
                && let Some(target) =
                    routing_cache.lookup_c2c_by_parts(channel_id, point_type, update.point_id)
            {
                let target_key = (target.channel_id, target.point_type, target.point_id);

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
                    // Apply optional linear transform configured on the C2C route.
                    let fwd_value = target.transform(update.value);
                    let fwd_raw = update.raw_value.map(|r| target.transform(r));
                    c2c_forwards.push(ChannelPointUpdate {
                        channel_id: target.channel_id,
                        point_type: target.point_type,
                        point_id: target.point_id,
                        value: fwd_value,
                        raw_value: fwd_raw,
                        cascade_depth: update.cascade_depth + 1,
                    });
                }
            }
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
