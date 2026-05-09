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

/// seq == 0 means the slot has never been written. Skip on scan to avoid
/// flooding Redis with zero-value placeholders on startup.
///
/// Note: after ~2^31 writes (~6.8 years at 10Hz), seq wraps back to 0 and
/// this slot would be skipped until the next write (seq=2). Acceptable.
const UNWRITTEN_SEQ: u32 = 0;

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

        // Grow last_seq if slot_count expanded after routing reload.
        if self.last_seq.len() < slot_count {
            self.last_seq.resize(slot_count, 0);
        }

        // Detect layout swap (routing reload) → reset last_seq for full re-sync.
        let layout_ptr = Arc::as_ptr(layout) as usize;
        let layout_changed = self.last_layout_ptr != 0 && layout_ptr != self.last_layout_ptr;
        if layout_changed {
            info!("ShmRedisSync: SHM layout swapped (routing reload), resetting last_seq");
            self.last_seq.fill(0);
            self.retry_slots.clear();
        }
        self.last_layout_ptr = layout_ptr;

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

            if seq == UNWRITTEN_SEQ || seq == self.last_seq[slot_idx] {
                continue;
            }

            let (value, raw, ts) = match slot.load_consistent() {
                Some(data) => data,
                None => continue, // torn read, retry next tick
            };

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
