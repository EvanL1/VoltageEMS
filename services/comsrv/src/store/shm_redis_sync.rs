//! ShmRedisSync — background task that scans SHM for dirty slots and batch-flushes to Redis.
//!
//! Replaces the DashMap WriteBuffer for channel data by reading directly from the
//! SHM mmap and writing to Redis via `pipeline_hash_mset` in one shot.
//!
//! # Write path
//!
//! ```text
//! Protocol → comsrv SHM write (UnifiedWriter::set_direct)
//!         → ShmRedisSync (100ms tick) scans dirty slots
//!         → pipeline_hash_mset → Redis comsrv:{channel_id}:{T|S|C|A}
//! ```
//!
//! # Dirty detection
//!
//! Each `PointSlot` carries a `seq` seqlock counter (incremented by 2 per write).
//! The task stores `last_seq[slot]` and skips slots where `seq_raw() == last_seq`.
//! This is O(slot_count) per tick with no Redis round-trips for unchanged slots.
//!
//! # TTL refresh
//!
//! Redis keys are refreshed with a 24h TTL every `TTL_REFRESH_INTERVAL` ticks
//! (~60s at default 100ms flush interval) to match the old WriteBuffer behavior.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use tokio::{sync::watch, task::JoinHandle, time};
use tracing::{debug, info, trace, warn};

use voltage_model::KeySpaceConfig;
use voltage_rtdb::numfmt::{f64_to_bytes, i64_to_bytes};
use voltage_rtdb::Rtdb;
use voltage_rtdb_shm::{ReverseSlotIndex, ShmHandle};
use voltage_routing::RoutingCache;

/// seq == 0 means the slot has never been written. Skip on scan to avoid
/// flooding Redis with zero-value placeholders on startup.
///
/// Note: after ~2^31 writes (~6.8 years at 10Hz), seq wraps back to 0 and
/// this slot would be skipped until the next write (seq=2). Acceptable.
const UNWRITTEN_SEQ: u32 = 0;

/// TTL applied to each Redis key (24 hours), matching the old WriteBuffer config.
const KEY_TTL_SECONDS: i64 = 86_400;

/// Refresh TTL every N ticks. At 100ms flush interval this is ~60 seconds.
const TTL_REFRESH_INTERVAL: u64 = 600;

/// Background task: scan SHM dirty slots and batch-flush values to Redis.
///
/// The `reverse_index` is wrapped in `Arc<ArcSwap<…>>` so that a routing
/// reload can atomically swap in a new index. When a swap is detected,
/// `last_seq` is reset to force a full re-sync.
pub struct ShmRedisSync<R: Rtdb> {
    rtdb: Arc<R>,
    shm_handle: Arc<ShmHandle>,
    reverse_index: Arc<ArcSwap<ReverseSlotIndex>>,
    routing_cache: Arc<RoutingCache>,
    /// Per-slot last-synced seq counter.
    last_seq: Vec<u32>,
    /// Raw pointer of the last-loaded ReverseSlotIndex Arc, used to detect swaps.
    last_rev_ptr: usize,
    /// All Redis keys written since last TTL refresh.
    ttl_keys: HashSet<String>,
    /// Tick counter for TTL refresh cadence.
    tick_count: u64,
    key_space: &'static KeySpaceConfig,
    flush_interval: Duration,
}

impl<R: Rtdb> ShmRedisSync<R> {
    pub fn new(
        rtdb: Arc<R>,
        shm_handle: Arc<ShmHandle>,
        reverse_index: Arc<ArcSwap<ReverseSlotIndex>>,
        routing_cache: Arc<RoutingCache>,
        max_slots: usize,
    ) -> Self {
        Self {
            rtdb,
            shm_handle,
            reverse_index,
            routing_cache,
            last_seq: vec![0u32; max_slots],
            last_rev_ptr: 0,
            ttl_keys: HashSet::new(),
            tick_count: 0,
            key_space: KeySpaceConfig::production_cached(),
            flush_interval: Duration::from_millis(100),
        }
    }

    /// Override the flush interval (default: 100 ms).
    pub fn with_flush_interval(mut self, interval: Duration) -> Self {
        self.flush_interval = interval;
        self
    }

    /// Perform one scan-and-flush pass.
    pub async fn flush_once(&mut self) {
        let writer_guard = match self.shm_handle.writer() {
            Some(g) => g,
            None => {
                trace!("ShmRedisSync: SHM writer not available, skipping");
                return;
            }
        };
        let writer = match writer_guard.as_ref() {
            Some(w) => w,
            None => return,
        };

        let slot_count = writer.slot_count();
        if slot_count == 0 {
            return;
        }

        // Grow last_seq if slot_count expanded after routing reload.
        if self.last_seq.len() < slot_count {
            self.last_seq.resize(slot_count, 0);
        }

        // Detect reverse_index swap (routing reload) → reset last_seq for full re-sync.
        let rev_guard = self.reverse_index.load();
        let rev: &ReverseSlotIndex = &rev_guard;
        let rev_ptr = Arc::as_ptr(&rev_guard) as usize;
        if self.last_rev_ptr != 0 && rev_ptr != self.last_rev_ptr {
            info!("ShmRedisSync: reverse index swapped (routing reload), resetting last_seq");
            self.last_seq.fill(0);
        }
        self.last_rev_ptr = rev_ptr;

        // Accumulate redis_key → Vec<(field, bytes)>.
        let mut ops: HashMap<String, Vec<(Arc<str>, bytes::Bytes)>> = HashMap::new();

        for slot_idx in 0..slot_count {
            let slot = writer.slot(slot_idx);
            // Advisory pre-check (Relaxed). Correctness relies on load_consistent below.
            let seq = slot.seq_raw();

            if seq == UNWRITTEN_SEQ || seq == self.last_seq[slot_idx] {
                continue;
            }

            let (value, raw, ts) = match slot.load_consistent() {
                Some(data) => data,
                None => continue, // torn read, retry next tick
            };

            let origin = match rev.get(slot_idx) {
                Some(o) => o,
                None => {
                    self.last_seq[slot_idx] = seq;
                    continue;
                }
            };

            let field: Arc<str> = Arc::from(origin.point_id.to_string().as_str());
            let val_bytes = f64_to_bytes(value);
            let ts_bytes = i64_to_bytes(ts as i64);
            let raw_bytes = f64_to_bytes(raw);

            let val_key = self.key_space.channel_key(origin.channel_id, origin.point_type);
            let ts_key = self.key_space.channel_ts_key(origin.channel_id, origin.point_type);
            let raw_key = self.key_space.channel_raw_key(origin.channel_id, origin.point_type);

            ops.entry(val_key).or_default().push((Arc::clone(&field), val_bytes));
            ops.entry(ts_key).or_default().push((Arc::clone(&field), ts_bytes));
            ops.entry(raw_key).or_default().push((field, raw_bytes));

            // C2M routing → inst:{id}:M
            if let Some(target) = self.routing_cache.lookup_c2m_by_parts(
                origin.channel_id,
                origin.point_type,
                origin.point_id,
            ) {
                let inst_key = self.key_space.instance_measurement_key(target.instance_id);
                let inst_field: Arc<str> = Arc::from(target.point_id.to_string().as_str());
                ops.entry(inst_key).or_default().push((inst_field, f64_to_bytes(value)));
            }

            self.last_seq[slot_idx] = seq;
        }

        if ops.is_empty() {
            self.tick_count += 1;
            return;
        }

        // Track keys for TTL refresh.
        for key in ops.keys() {
            self.ttl_keys.insert(key.clone());
        }

        debug!(keys = ops.len(), "ShmRedisSync: flushing dirty slots to Redis");

        let hash_ops: voltage_rtdb::traits::HashMsetOps = ops.into_iter().collect();
        if let Err(e) = self.rtdb.pipeline_hash_mset(hash_ops).await {
            warn!(error = %e, "ShmRedisSync: pipeline_hash_mset failed");
        }

        // Refresh TTL periodically (~60s at default interval).
        self.tick_count += 1;
        if self.tick_count % TTL_REFRESH_INTERVAL == 0 {
            self.refresh_ttl().await;
        }
    }

    /// EXPIRE all tracked keys with 24h TTL, then clear the set.
    async fn refresh_ttl(&mut self) {
        if self.ttl_keys.is_empty() {
            return;
        }
        let count = self.ttl_keys.len();
        for key in self.ttl_keys.drain() {
            if let Err(e) = self.rtdb.expire(&key, KEY_TTL_SECONDS).await {
                warn!(key = %key, error = %e, "ShmRedisSync: EXPIRE failed");
            }
        }
        debug!(keys = count, "ShmRedisSync: refreshed TTL on {} keys", count);
    }

    /// Background loop — runs until `shutdown` fires or sender is dropped.
    pub async fn run(mut self, mut shutdown: watch::Receiver<bool>) {
        let mut interval = time::interval(self.flush_interval);
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                biased;
                result = shutdown.changed() => {
                    let _ = result;
                    debug!("ShmRedisSync: shutdown, performing final flush + TTL refresh");
                    self.flush_once().await;
                    self.refresh_ttl().await;
                    break;
                }
                _ = interval.tick() => {
                    self.flush_once().await;
                }
            }
        }

        debug!("ShmRedisSync: stopped");
    }

    /// Spawn the background loop as a tokio task.
    pub fn start(self, shutdown: watch::Receiver<bool>) -> JoinHandle<()>
    where
        R: 'static,
    {
        tokio::spawn(self.run(shutdown))
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use voltage_rtdb::helpers::create_test_rtdb;

    fn make_reverse_index_empty() -> Arc<ArcSwap<ReverseSlotIndex>> {
        let dir = tempfile::tempdir().unwrap();
        let config = voltage_rtdb_shm::SharedConfig::default()
            .with_path(dir.path().join("test.shm"))
            .with_max_slots(100);
        let points =
            voltage_rtdb_shm::ChannelPointCounts::from_map(std::collections::BTreeMap::new());
        let writer = voltage_rtdb_shm::UnifiedWriter::create(&config, &points).unwrap();
        let forward = voltage_rtdb_shm::ChannelToSlotIndex::from_unified_writer(&writer);
        let rev = ReverseSlotIndex::from_forward(&forward, 0);
        Arc::new(ArcSwap::new(Arc::new(rev)))
    }

    fn make_routing_cache() -> Arc<voltage_routing::RoutingCache> {
        Arc::new(voltage_routing::RoutingCache::new())
    }

    #[test]
    fn constructs_without_panic() {
        let rtdb = create_test_rtdb();
        let config = voltage_rtdb_shm::SharedConfig::default();
        let handle = Arc::new(voltage_rtdb_shm::ShmHandle::empty(config));
        let rev = make_reverse_index_empty();
        let rc = make_routing_cache();
        let sync = ShmRedisSync::new(rtdb, handle, rev, rc, 1024);
        assert_eq!(sync.flush_interval, Duration::from_millis(100));
        assert_eq!(sync.last_seq.len(), 1024);
    }

    #[tokio::test]
    async fn flush_once_no_op_when_shm_unavailable() {
        let rtdb = create_test_rtdb();
        let config = voltage_rtdb_shm::SharedConfig::default();
        let handle = Arc::new(voltage_rtdb_shm::ShmHandle::empty(config));
        let rev = make_reverse_index_empty();
        let rc = make_routing_cache();
        let mut sync = ShmRedisSync::new(rtdb, handle, rev, rc, 1024);
        sync.flush_once().await;
        assert_eq!(sync.tick_count, 0); // no-op, tick not incremented
    }
}
