//! Channel online health cache for the M2C control gate.
//!
//! Mirrors the `comsrv:online` Redis hash (written by comsrv) into a process-local
//! map so `execute_action()` can reject writes targeting offline channels without
//! a Redis round-trip on every command.
//!
//! Semantics:
//! - Channel explicitly marked "0" in `comsrv:online` → offline → action rejected.
//! - Channel absent from the hash (never registered, or comsrv just started) → online (fail-open).
//! - Cache not yet populated (refresh task hasn't run once) → online (fail-open).
//!
//! Fail-open on missing data is intentional: comsrv may not have published the
//! initial snapshot yet at modsrv startup, and we don't want to brick all control
//! during that window. Once comsrv writes a "0", we honor it within one refresh tick.

use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
use voltage_rtdb::Rtdb;

/// Default refresh interval for the cache (comsrv's offline detection window is
/// 90s+, so 100ms is effectively real-time at our scale).
pub const DEFAULT_REFRESH_INTERVAL: Duration = Duration::from_millis(100);

pub struct ChannelHealthCache {
    map: DashMap<u32, bool>,
    initialized: AtomicBool,
}

impl ChannelHealthCache {
    pub fn new() -> Self {
        Self {
            map: DashMap::new(),
            initialized: AtomicBool::new(false),
        }
    }

    /// Returns true when the channel should accept M2C writes.
    /// Fail-open for unknown channels and uninitialized cache (see module docs).
    pub fn is_online(&self, channel_id: u32) -> bool {
        if !self.initialized.load(Ordering::Acquire) {
            return true;
        }
        self.map.get(&channel_id).map(|v| *v).unwrap_or(true)
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire)
    }

    /// Replace the cache contents with a fresh snapshot of `comsrv:online`.
    /// Channels removed from the hash since the last refresh are dropped.
    pub async fn refresh<R: Rtdb>(&self, rtdb: &R, online_key: &str) -> anyhow::Result<usize> {
        let snapshot = rtdb.hash_get_all(online_key).await?;
        self.map.clear();
        for (field, bytes) in snapshot.iter() {
            let Ok(channel_id) = field.parse::<u32>() else {
                continue;
            };
            let online = parse_online_value(bytes);
            self.map.insert(channel_id, online);
        }
        self.initialized.store(true, Ordering::Release);
        Ok(self.map.len())
    }

    #[cfg(test)]
    pub fn set_for_test(&self, channel_id: u32, online: bool) {
        self.map.insert(channel_id, online);
        self.initialized.store(true, Ordering::Release);
    }
}

impl Default for ChannelHealthCache {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_online_value(bytes: &[u8]) -> bool {
    // comsrv writes "1" / "0"; treat anything else (except literal "0" / "false") as online
    // to keep fail-open behavior on schema drift.
    !matches!(bytes, b"0" | b"false")
}

/// Spawn the background refresh task. Returns immediately; cancellation is via the token.
pub fn spawn_refresh_task<R: Rtdb + Send + Sync + 'static>(
    cache: Arc<ChannelHealthCache>,
    rtdb: Arc<R>,
    online_key: String,
    interval: Duration,
    cancel: CancellationToken,
) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = ticker.tick() => {},
            }
            match cache.refresh(rtdb.as_ref(), &online_key).await {
                Ok(n) => debug!("ChannelHealthCache refreshed: {} entries", n),
                Err(e) => warn!("ChannelHealthCache refresh failed: {}", e),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltage_rtdb::MemoryRtdb;

    #[test]
    fn fail_open_when_uninitialized() {
        let cache = ChannelHealthCache::new();
        assert!(cache.is_online(42), "uninitialized cache must fail-open");
    }

    #[test]
    fn unknown_channel_is_online_after_init() {
        let cache = ChannelHealthCache::new();
        cache.set_for_test(1, true);
        assert!(
            cache.is_online(999),
            "channel absent from comsrv:online treated as online (comsrv may not have registered it yet)"
        );
    }

    #[test]
    fn explicit_zero_is_offline() {
        let cache = ChannelHealthCache::new();
        cache.set_for_test(42, false);
        assert!(!cache.is_online(42));
    }

    #[tokio::test]
    async fn refresh_pulls_hash_and_marks_initialized() {
        let rtdb = MemoryRtdb::new();
        rtdb.hash_set("comsrv:online", "100", b"1".to_vec().into())
            .await
            .unwrap();
        rtdb.hash_set("comsrv:online", "200", b"0".to_vec().into())
            .await
            .unwrap();

        let cache = ChannelHealthCache::new();
        let n = cache.refresh(&rtdb, "comsrv:online").await.unwrap();
        assert_eq!(n, 2);
        assert!(cache.is_initialized());
        assert!(cache.is_online(100));
        assert!(!cache.is_online(200));
    }

    #[tokio::test]
    async fn refresh_drops_disappeared_channels() {
        let rtdb = MemoryRtdb::new();
        rtdb.hash_set("comsrv:online", "100", b"0".to_vec().into())
            .await
            .unwrap();

        let cache = ChannelHealthCache::new();
        cache.refresh(&rtdb, "comsrv:online").await.unwrap();
        assert!(!cache.is_online(100));

        rtdb.hash_del("comsrv:online", "100").await.unwrap();
        cache.refresh(&rtdb, "comsrv:online").await.unwrap();
        // After comsrv removes the channel, fail-open kicks in again
        assert!(cache.is_online(100));
    }
}
