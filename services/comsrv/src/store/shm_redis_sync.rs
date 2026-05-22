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
//! # Authority
//!
//! SHM is the source of truth for current values. Redis is a derived cache
//! consumed by other services (modsrv, apigateway, …). On comsrv startup the
//! snapshot restore repopulates SHM and the first `flush_once()` does a
//! `full_scan` (`tick_count == 0`), pushing every non-zero seq slot back into
//! Redis — so Redis converges to SHM without any TTL bookkeeping.
//!
//! # Dirty detection
//!
//! `UnifiedWriter` keeps a process-local dirty bitmap for slots written by
//! comsrv. The task drains that bitmap on normal ticks and periodically falls
//! back to a full seq scan so cross-process or missed writes are still caught.
//! `last_seq[slot]` is advanced only after Redis accepts the pipeline.
//!
//! # No TTL
//!
//! Redis keys/fields are not given a TTL. Stale entries (deleted channels or
//! removed point IDs) are reaped at startup by `voltage_rtdb::cleanup`, which
//! is routing-driven and explicit. TTL would re-introduce the failure mode
//! where a >24h device outage silently expires real values from Redis.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::{sync::watch, task::JoinHandle, time};
use tracing::{debug, info, trace, warn};

use voltage_model::KeySpaceConfig;
use voltage_routing::RoutingCache;
use voltage_rtdb::Rtdb;
use voltage_rtdb::numfmt::{f64_to_bytes, i64_to_bytes, precomputed};
use voltage_rtdb_shm::ShmHandle;

/// seq == 0 means the slot has never been written. Used as a fast-path
/// pre-check; the canonical "unwritten" marker since SHM v3 is the NaN
/// sentinel in `value_bits` (see `is_finite(value)` filter below).
///
/// Note: after ~2^31 writes (~6.8 years at 10Hz), seq wraps back to 0 and
/// this slot would be skipped until the next write (seq=2). Acceptable.
/// Sentinel for "never written" seq. Used in tests only — production code
/// relies on the NaN value sentinel to filter unwritten slots, so seq wrap
/// to 0 is not misclassified as unwritten.
#[allow(dead_code)]
const UNWRITTEN_SEQ: u32 = 0;

/// Maximum slots flushed in a single Redis pipeline.
/// Bounds memory and Redis-side latency on full-scan ticks with high slot
/// counts. Excess slots are deferred to retry_slots for the next tick.
const MAX_PIPELINE_SLOTS: usize = 1024;

/// Upper bound on retry_slots so sustained overload does not grow it
/// without limit. retry_slots is sorted ascending before this cap is
/// applied (see flush_once), so eviction by `drain(..n)` removes the
/// LOWEST-INDEXED slots — not the insertion-order oldest. Mid-range
/// slot indices are the ones that wait longest under sustained
/// overload; they are re-discovered by the periodic full-scan
/// (FULL_SCAN_INTERVAL ≈ 60s), so no data is lost permanently.
const MAX_RETRY_SLOTS: usize = MAX_PIPELINE_SLOTS * 4;

/// Force a full seq scan periodically as a safety net for external writers.
const FULL_SCAN_INTERVAL: u64 = 600;

/// Background task: scan SHM dirty slots and batch-flush values to Redis.
///
/// `ShmHandle` exposes a coherent layout snapshot. When the layout Arc changes
/// after routing reload, `last_seq` is reset to force a full re-sync.
pub struct ShmRedisSync<R: Rtdb> {
    rtdb: Arc<R>,
    shm_handle: Arc<ShmHandle>,
    routing_cache: Arc<RoutingCache>,
    /// Per-slot last-synced seq counter.
    last_seq: Vec<u32>,
    /// Raw pointer of the last-loaded ShmLayout Arc, used to detect swaps.
    last_layout_ptr: usize,
    /// Content hash of the routing cache at last scan. If the routing content
    /// changes without a SHM layout swap (pure RoutingCache reload), C2M
    /// targets may have shifted and unchanged-seq slots must republish.
    last_routing_hash: u64,
    /// Slots that failed to flush to Redis and must be retried.
    retry_slots: Vec<usize>,
    /// Tick counter for periodic full-scan cadence.
    tick_count: u64,
    key_space: &'static KeySpaceConfig,
    flush_interval: Duration,
}

impl<R: Rtdb> ShmRedisSync<R> {
    pub fn new(
        rtdb: Arc<R>,
        shm_handle: Arc<ShmHandle>,
        routing_cache: Arc<RoutingCache>,
        max_slots: usize,
    ) -> Self {
        Self {
            rtdb,
            shm_handle,
            routing_cache,
            last_seq: vec![0u32; max_slots],
            last_layout_ptr: 0,
            last_routing_hash: 0,
            retry_slots: Vec::new(),
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
        let layout_guard = match self.shm_handle.layout() {
            Some(g) => g,
            None => {
                trace!("ShmRedisSync: SHM layout not available, skipping");
                return;
            },
        };
        let layout = match layout_guard.as_ref() {
            Some(layout) => layout,
            None => return,
        };
        let writer = layout.writer.as_ref();
        let reverse_index = layout.reverse_index.as_ref();

        let slot_count = writer.slot_count();
        if slot_count == 0 {
            return;
        }

        // Reconfigure-in-progress sentinel: comsrv flips writer_generation
        // to an odd value while it is clearing slots in reconfigure_existing.
        // Reading slots during this window would yield all-zero garbage
        // (the byte-fill in progress). Skip this tick and retry next
        // iteration — the even parity returns once reconfigure completes,
        // and the subsequent layout/content-hash change will already force
        // a full_seq reset for the new layout.
        if writer.generation() & 1 == 1 {
            trace!("ShmRedisSync: reconfigure in progress (odd generation), skipping tick");
            self.tick_count += 1;
            return;
        }

        // Grow last_seq if slot_count expanded after routing reload.
        if self.last_seq.len() < slot_count {
            self.last_seq.resize(slot_count, 0);
        }

        // Detect layout swap (routing reload) → reset last_seq for full re-sync.
        let layout_ptr = Arc::as_ptr(layout) as usize;
        let layout_changed = self.last_layout_ptr != 0 && layout_ptr != self.last_layout_ptr;

        // Detect routing content change even when SHM layout pointer is stable:
        // a pure RoutingCache reload (no SHM rebuild) still changes C2M targets,
        // so unchanged-seq slots must republish under the new mapping or stay
        // permanently pointed at the old instance.
        let routing_hash = self.routing_cache.content_hash();
        let routing_changed = self.last_routing_hash != 0 && routing_hash != self.last_routing_hash;

        if layout_changed || routing_changed {
            info!(
                "ShmRedisSync: reset last_seq (layout_changed={}, routing_changed={})",
                layout_changed, routing_changed
            );
            self.last_seq.fill(0);
            self.retry_slots.clear();
        }
        self.last_layout_ptr = layout_ptr;
        self.last_routing_hash = routing_hash;

        let full_scan = layout_changed
            || self.tick_count == 0
            || self.tick_count.is_multiple_of(FULL_SCAN_INTERVAL);
        let mut candidate_slots: Vec<usize> = if full_scan {
            (0..slot_count).collect()
        } else {
            writer.take_dirty_slots()
        };

        if !self.retry_slots.is_empty() {
            candidate_slots.append(&mut self.retry_slots);
        }

        if candidate_slots.is_empty() {
            self.tick_count += 1;
            return;
        }
        candidate_slots.sort_unstable();
        candidate_slots.dedup();

        // Bound pipeline size — large full-scans with thousands of dirty
        // slots could stall Redis on a single round-trip. Excess slots go
        // back into retry_slots for the next tick (we already deduped).
        if candidate_slots.len() > MAX_PIPELINE_SLOTS {
            let overflow = candidate_slots.split_off(MAX_PIPELINE_SLOTS);
            self.retry_slots.extend(overflow);
        }

        // Cap retry_slots so sustained overload (consistent >1024 dirty
        // slots per tick) does not accumulate unbounded memory. Dropped
        // entries are re-discovered by the periodic full-scan.
        if self.retry_slots.len() > MAX_RETRY_SLOTS {
            let drop_count = self.retry_slots.len() - MAX_RETRY_SLOTS;
            // retry_slots is sorted ascending at this point, so drain(..n)
            // removes the lowest-indexed slots in the overflow set. Those
            // slots are re-discovered by the periodic FULL_SCAN.
            tracing::warn!(
                "ShmRedisSync: dropping {} lowest-indexed retry slots (cap {})",
                drop_count,
                MAX_RETRY_SLOTS
            );
            self.retry_slots.drain(..drop_count);
        }

        // Accumulate redis_key → Vec<(field, bytes)>.
        let mut ops: HashMap<String, Vec<(Arc<str>, bytes::Bytes)>> = HashMap::new();
        let mut synced_slots = Vec::new();

        for slot_idx in candidate_slots {
            if slot_idx >= slot_count {
                continue;
            }
            let slot = writer.slot(slot_idx);
            // Advisory pre-check (Relaxed). Correctness relies on load_consistent below.
            let seq = slot.seq_raw();

            // Only skip when seq matches last_seq exactly. Do not treat
            // seq == 0 as "unwritten" — after 2^32 writes the seq wraps to
            // 0, which is a legitimate update. The NaN-sentinel filter
            // below correctly catches truly-unwritten slots regardless of
            // seq value.
            if seq == self.last_seq[slot_idx] {
                continue;
            }

            let (value, raw, ts) = match slot.load_consistent() {
                Some(data) => data,
                None => {
                    // Torn read: writer was mid-update through this slot's
                    // seqlock. take_dirty_slots() already cleared the dirty
                    // bit, so without explicit retry the slot would not be
                    // visited again until the next writer update (which may
                    // not happen for a long time) or the periodic
                    // FULL_SCAN_INTERVAL. Push it back to retry_slots so
                    // the next tick re-reads it.
                    self.retry_slots.push(slot_idx);
                    continue;
                },
            };

            // Skip slots whose value is the unwritten NaN sentinel (SHM v3).
            //
            // Defence-in-depth on top of the seq check. In production, NaN
            // cannot reach this point: batch_to_updates filters non-finite
            // from the protocol side (redis_store.rs), set_action rejects
            // via validate_action_value, and SHM seeds slots with NaN only
            // before the first write. If a NaN somehow leaks through (writer
            // bug, v2 snapshot, partial reconfigure), we drop it here rather
            // than persist a meaningless value to Redis. "Unavailable" is
            // represented downstream by an ABSENT hash field — not a NaN
            // string — which keeps the JSON protocol simple and matches the
            // CLAUDE.md "NaN means never-written" sentinel semantics for SHM
            // internals only.
            if !value.is_finite() || !raw.is_finite() {
                self.last_seq[slot_idx] = seq;
                continue;
            }

            let origin = match reverse_index.get(slot_idx) {
                Some(o) => o,
                None => {
                    self.last_seq[slot_idx] = seq;
                    continue;
                },
            };

            let field: Arc<str> = precomputed::get_point_id_str_or_alloc(origin.point_id);
            let val_bytes = f64_to_bytes(value);
            let ts_bytes = i64_to_bytes(ts as i64);
            let raw_bytes = f64_to_bytes(raw);

            let val_key = self
                .key_space
                .channel_key(origin.channel_id, origin.point_type);
            let ts_key = self
                .key_space
                .channel_ts_key(origin.channel_id, origin.point_type);
            let raw_key = self
                .key_space
                .channel_raw_key(origin.channel_id, origin.point_type);

            ops.entry(val_key)
                .or_default()
                .push((Arc::clone(&field), val_bytes));
            ops.entry(ts_key)
                .or_default()
                .push((Arc::clone(&field), ts_bytes));
            ops.entry(raw_key).or_default().push((field, raw_bytes));

            // C2M routing → inst:{id}:M (+ sidecar inst:{id}:M:ts)
            if let Some(target) = self.routing_cache.lookup_c2m_by_parts(
                origin.channel_id,
                origin.point_type,
                origin.point_id,
            ) {
                let inst_key = self.key_space.instance_measurement_key(target.instance_id);
                let inst_ts_key = self
                    .key_space
                    .instance_measurement_ts_key(target.instance_id);
                let inst_field: Arc<str> = precomputed::get_point_id_str_or_alloc(target.point_id);
                ops.entry(inst_key)
                    .or_default()
                    .push((Arc::clone(&inst_field), f64_to_bytes(value)));
                ops.entry(inst_ts_key)
                    .or_default()
                    .push((inst_field, i64_to_bytes(ts as i64)));
            }

            synced_slots.push((slot_idx, seq));
        }

        if ops.is_empty() {
            self.tick_count += 1;
            return;
        }

        debug!(
            keys = ops.len(),
            slots = synced_slots.len(),
            "ShmRedisSync: flushing dirty slots to Redis"
        );

        let hash_ops: voltage_rtdb::traits::HashMsetOps = ops.into_iter().collect();
        if let Err(e) = self.rtdb.pipeline_hash_mset(hash_ops).await {
            warn!(error = %e, "ShmRedisSync: pipeline_hash_mset failed");
            self.retry_slots
                .extend(synced_slots.into_iter().map(|(slot, _)| slot));
            self.retry_slots.sort_unstable();
            self.retry_slots.dedup();
            self.tick_count += 1;
            return;
        }

        for (slot_idx, seq) in synced_slots {
            self.last_seq[slot_idx] = seq;
        }

        self.tick_count += 1;
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
                    debug!("ShmRedisSync: shutdown, performing final flush");
                    self.flush_once().await;
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
#[allow(clippy::disallowed_methods, clippy::manual_async_fn)]
mod tests {
    use super::*;
    use anyhow::Result;
    use bytes::Bytes;
    use std::collections::{BTreeMap, HashMap};
    use std::future::Future;
    use std::sync::atomic::{AtomicBool, Ordering};
    use voltage_model::PointType;
    use voltage_rtdb::helpers::create_test_rtdb;
    use voltage_rtdb::traits::HashMsetOps;
    use voltage_rtdb::{MemoryRtdb, Rtdb};

    struct FailOnceRtdb {
        inner: MemoryRtdb,
        fail_next_pipeline: AtomicBool,
    }

    impl FailOnceRtdb {
        fn new() -> Self {
            Self {
                inner: MemoryRtdb::new(),
                fail_next_pipeline: AtomicBool::new(true),
            }
        }
    }

    impl Rtdb for FailOnceRtdb {
        fn get<'a>(
            &'a self,
            key: &'a str,
        ) -> impl Future<Output = Result<Option<Bytes>>> + Send + 'a {
            async move { self.inner.get(key).await }
        }

        fn set<'a>(
            &'a self,
            key: &'a str,
            value: Bytes,
        ) -> impl Future<Output = Result<()>> + Send + 'a {
            async move { self.inner.set(key, value).await }
        }

        fn del<'a>(&'a self, key: &'a str) -> impl Future<Output = Result<bool>> + Send + 'a {
            async move { self.inner.del(key).await }
        }

        fn exists<'a>(&'a self, key: &'a str) -> impl Future<Output = Result<bool>> + Send + 'a {
            async move { self.inner.exists(key).await }
        }

        fn incrbyfloat<'a>(
            &'a self,
            key: &'a str,
            increment: f64,
        ) -> impl Future<Output = Result<f64>> + Send + 'a {
            async move { self.inner.incrbyfloat(key, increment).await }
        }

        fn hash_set<'a>(
            &'a self,
            key: &'a str,
            field: &'a str,
            value: Bytes,
        ) -> impl Future<Output = Result<()>> + Send + 'a {
            async move { self.inner.hash_set(key, field, value).await }
        }

        fn hash_setnx<'a>(
            &'a self,
            key: &'a str,
            field: &'a str,
            value: Bytes,
        ) -> impl Future<Output = Result<bool>> + Send + 'a {
            async move { self.inner.hash_setnx(key, field, value).await }
        }

        fn hash_get<'a>(
            &'a self,
            key: &'a str,
            field: &'a str,
        ) -> impl Future<Output = Result<Option<Bytes>>> + Send + 'a {
            async move { self.inner.hash_get(key, field).await }
        }

        fn hash_mget<'a>(
            &'a self,
            key: &'a str,
            fields: &'a [&'a str],
        ) -> impl Future<Output = Result<Vec<Option<Bytes>>>> + Send + 'a {
            async move { self.inner.hash_mget(key, fields).await }
        }

        fn hash_mset<'a>(
            &'a self,
            key: &'a str,
            fields: Vec<(String, Bytes)>,
        ) -> impl Future<Output = Result<()>> + Send + 'a {
            async move { self.inner.hash_mset(key, fields).await }
        }

        fn hash_get_all<'a>(
            &'a self,
            key: &'a str,
        ) -> impl Future<Output = Result<HashMap<String, Bytes>>> + Send + 'a {
            async move { self.inner.hash_get_all(key).await }
        }

        fn hash_del<'a>(
            &'a self,
            key: &'a str,
            field: &'a str,
        ) -> impl Future<Output = Result<bool>> + Send + 'a {
            async move { self.inner.hash_del(key, field).await }
        }

        fn hash_del_many<'a>(
            &'a self,
            key: &'a str,
            fields: &'a [String],
        ) -> impl Future<Output = Result<usize>> + Send + 'a {
            async move { self.inner.hash_del_many(key, fields).await }
        }

        fn hincrby<'a>(
            &'a self,
            key: &'a str,
            field: &'a str,
            increment: i64,
        ) -> impl Future<Output = Result<i64>> + Send + 'a {
            async move { self.inner.hincrby(key, field, increment).await }
        }

        fn list_lpush<'a>(
            &'a self,
            key: &'a str,
            value: Bytes,
        ) -> impl Future<Output = Result<()>> + Send + 'a {
            async move { self.inner.list_lpush(key, value).await }
        }

        fn list_rpush<'a>(
            &'a self,
            key: &'a str,
            value: Bytes,
        ) -> impl Future<Output = Result<()>> + Send + 'a {
            async move { self.inner.list_rpush(key, value).await }
        }

        fn list_lpop<'a>(
            &'a self,
            key: &'a str,
        ) -> impl Future<Output = Result<Option<Bytes>>> + Send + 'a {
            async move { self.inner.list_lpop(key).await }
        }

        fn list_rpop<'a>(
            &'a self,
            key: &'a str,
        ) -> impl Future<Output = Result<Option<Bytes>>> + Send + 'a {
            async move { self.inner.list_rpop(key).await }
        }

        fn list_blpop<'a>(
            &'a self,
            keys: &'a [&'a str],
            timeout_seconds: u64,
        ) -> impl Future<Output = Result<Option<(String, Bytes)>>> + Send + 'a {
            async move { self.inner.list_blpop(keys, timeout_seconds).await }
        }

        fn list_range<'a>(
            &'a self,
            key: &'a str,
            start: isize,
            stop: isize,
        ) -> impl Future<Output = Result<Vec<Bytes>>> + Send + 'a {
            async move { self.inner.list_range(key, start, stop).await }
        }

        fn list_trim<'a>(
            &'a self,
            key: &'a str,
            start: isize,
            stop: isize,
        ) -> impl Future<Output = Result<()>> + Send + 'a {
            async move { self.inner.list_trim(key, start, stop).await }
        }

        fn sadd<'a>(
            &'a self,
            key: &'a str,
            member: &'a str,
        ) -> impl Future<Output = Result<bool>> + Send + 'a {
            async move { self.inner.sadd(key, member).await }
        }

        fn srem<'a>(
            &'a self,
            key: &'a str,
            member: &'a str,
        ) -> impl Future<Output = Result<bool>> + Send + 'a {
            async move { self.inner.srem(key, member).await }
        }

        fn smembers<'a>(
            &'a self,
            key: &'a str,
        ) -> impl Future<Output = Result<Vec<String>>> + Send + 'a {
            async move { self.inner.smembers(key).await }
        }

        fn scan_match<'a>(
            &'a self,
            pattern: &'a str,
        ) -> impl Future<Output = Result<Vec<String>>> + Send + 'a {
            async move { self.inner.scan_match(pattern).await }
        }

        fn expire<'a>(
            &'a self,
            key: &'a str,
            seconds: i64,
        ) -> impl Future<Output = Result<bool>> + Send + 'a {
            async move { self.inner.expire(key, seconds).await }
        }

        fn pipeline_hash_mset(
            &self,
            operations: HashMsetOps,
        ) -> impl Future<Output = Result<()>> + Send + '_ {
            async move {
                if self.fail_next_pipeline.swap(false, Ordering::AcqRel) {
                    anyhow::bail!("injected pipeline failure");
                }
                self.inner.pipeline_hash_mset(operations).await
            }
        }
    }

    fn make_routing_cache() -> Arc<voltage_routing::RoutingCache> {
        Arc::new(voltage_routing::RoutingCache::new())
    }

    fn make_single_slot_handle() -> (tempfile::TempDir, Arc<voltage_rtdb_shm::ShmHandle>) {
        let dir = tempfile::tempdir().unwrap();
        let config = voltage_rtdb_shm::SharedConfig::default()
            .with_path(dir.path().join("test.shm"))
            .with_max_slots(8);
        let points =
            voltage_rtdb_shm::ChannelPointCounts::from_map(BTreeMap::from([(1001, [1, 0, 0, 0])]));
        let writer = voltage_rtdb_shm::UnifiedWriter::create(&config, &points).unwrap();
        let index = voltage_rtdb_shm::ChannelToSlotIndex::from_unified_writer(&writer);
        let handle = Arc::new(voltage_rtdb_shm::ShmHandle::new(config, writer, index));
        (dir, handle)
    }

    #[test]
    fn constructs_without_panic() {
        let rtdb = create_test_rtdb();
        let config = voltage_rtdb_shm::SharedConfig::default();
        let handle = Arc::new(voltage_rtdb_shm::ShmHandle::empty(config));
        let rc = make_routing_cache();
        let sync = ShmRedisSync::new(rtdb, handle, rc, 1024);
        assert_eq!(sync.flush_interval, Duration::from_millis(100));
        assert_eq!(sync.last_seq.len(), 1024);
    }

    #[tokio::test]
    async fn flush_once_no_op_when_shm_unavailable() {
        let rtdb = create_test_rtdb();
        let config = voltage_rtdb_shm::SharedConfig::default();
        let handle = Arc::new(voltage_rtdb_shm::ShmHandle::empty(config));
        let rc = make_routing_cache();
        let mut sync = ShmRedisSync::new(rtdb, handle, rc, 1024);
        sync.flush_once().await;
        assert_eq!(sync.tick_count, 0); // no-op, tick not incremented
    }

    #[tokio::test]
    async fn pipeline_failure_retries_without_advancing_seq() {
        let (_dir, handle) = make_single_slot_handle();
        let rtdb = Arc::new(FailOnceRtdb::new());
        let rc = make_routing_cache();
        let mut sync = ShmRedisSync::new(Arc::clone(&rtdb), Arc::clone(&handle), rc, 1);

        let layout = handle.layout_arc().unwrap();
        layout.writer.set_direct(0, 12.5, 12.5, 1000);
        let seq = layout.writer.slot(0).seq_raw();
        assert_ne!(seq, UNWRITTEN_SEQ);

        sync.flush_once().await;
        assert_eq!(sync.last_seq[0], 0);
        assert_eq!(sync.retry_slots, vec![0]);

        let key = sync.key_space.channel_key(1001, PointType::Telemetry);
        assert!(rtdb.hash_get(&key, "0").await.unwrap().is_none());

        sync.flush_once().await;
        assert_eq!(sync.last_seq[0], seq);
        assert!(sync.retry_slots.is_empty());

        let value = rtdb.hash_get(&key, "0").await.unwrap().unwrap();
        assert_eq!(&value[..], b"12.5");
    }
}
