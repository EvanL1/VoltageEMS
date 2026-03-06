//! SHM/UDS Action Dispatch Infrastructure
//!
//! Provides the low-latency M2C (modsrv-to-comsrv) dispatch path via shared memory
//! and Unix Domain Socket notifications.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tracing::{info, warn};
use voltage_routing::RouteContext;

/// Outcome of an action dispatch operation
#[derive(Debug)]
#[must_use]
pub struct DispatchOutcome {
    /// Whether the value was written to SHM
    pub shm_written: bool,
    /// Whether UDS notification was sent successfully
    pub uds_notified: bool,
    /// Whether UDS failed and comsrv must detect via SHM polling
    pub fallback_used: bool,
}

/// Trait for dispatching action commands to comsrv
///
/// The primary implementation uses SHM + UDS for ~1-2ms latency.
/// Test/fallback implementations can use NoopDispatch.
#[async_trait]
pub trait ActionDispatch: Send + Sync {
    /// Dispatch an action value to the target channel via the fastest available path
    async fn dispatch(&self, ctx: &RouteContext, value: f64) -> DispatchOutcome;

    /// Rebuild internal writer state after routing changes
    ///
    /// Accepts routing_cache so the dispatch layer doesn't need to hold it,
    /// enabling construction before the cache is available.
    fn rebuild_writer(&self, routing_cache: &voltage_routing::RoutingCache);
}

/// SHM + UDS dispatch implementation (production path)
///
/// Writes action values directly to shared memory, then sends a UDS notification
/// to comsrv for immediate processing. Falls back gracefully if either path fails.
pub struct ShmDispatch {
    writer: arc_swap::ArcSwapOption<voltage_rtdb_shm::UnifiedWriter>,
    config: std::sync::OnceLock<voltage_rtdb_shm::SharedConfig>,
    notifier: std::sync::OnceLock<Arc<tokio::sync::Mutex<voltage_rtdb_shm::ShmNotifier>>>,
}

impl Default for ShmDispatch {
    fn default() -> Self {
        Self::new()
    }
}

impl ShmDispatch {
    /// Create a new ShmDispatch (initially unconfigured)
    ///
    /// Call `set_writer()` and `set_notifier()` after construction
    /// to enable SHM and UDS paths respectively.
    pub fn new() -> Self {
        Self {
            writer: arc_swap::ArcSwapOption::empty(),
            config: std::sync::OnceLock::new(),
            notifier: std::sync::OnceLock::new(),
        }
    }

    /// Configure SHM action writer for M2C via shared memory
    ///
    /// Uses ArcSwapOption for runtime-swappable initialization.
    /// Also stores SharedConfig via OnceLock for future rebuilds.
    pub fn set_writer(
        &self,
        writer: Arc<voltage_rtdb_shm::UnifiedWriter>,
        config: voltage_rtdb_shm::SharedConfig,
    ) {
        self.writer.store(Some(writer));
        let _ = self.config.set(config);
    }

    /// Configure UDS notifier for event-driven M2C command dispatch
    ///
    /// Returns true if set successfully, false if already set.
    pub fn set_notifier(
        &self,
        notifier: Arc<tokio::sync::Mutex<voltage_rtdb_shm::ShmNotifier>>,
    ) -> bool {
        self.notifier.set(notifier).is_ok()
    }
}

#[async_trait]
impl ActionDispatch for ShmDispatch {
    async fn dispatch(&self, ctx: &RouteContext, value: f64) -> DispatchOutcome {
        let mut outcome = DispatchOutcome {
            shm_written: false,
            uds_notified: false,
            fallback_used: false,
        };

        // Step 1: Write action value to shared memory (zero-copy IPC)
        if let Some(writer) = self.writer.load().as_ref() {
            let mirrored = writer.set_action(
                ctx.target_channel_id,
                ctx.target_point_type,
                ctx.target_point_id,
                value,
                ctx.timestamp_ms as u64,
            );
            outcome.shm_written = mirrored;
            if !mirrored {
                warn!(
                    "SHM action mirror miss for ch={} pt={} point={}",
                    ctx.target_channel_id, ctx.target_point_type, ctx.target_point_id
                );
            }
        }

        // Step 2: UDS notification for event-driven dispatch (~1-2ms latency)
        if let Some(notifier_lock) = self.notifier.get() {
            match tokio::time::timeout(Duration::from_millis(100), notifier_lock.lock()).await {
                Ok(mut guard) => {
                    if let Some(pt) = voltage_model::PointType::from_u8(ctx.target_point_type) {
                        let result = guard
                            .notify(
                                ctx.target_channel_id,
                                pt,
                                ctx.target_point_id,
                                value,
                                ctx.timestamp_ms as u64,
                            )
                            .await;
                        outcome.uds_notified = !result.fallback_used;
                        outcome.fallback_used = result.fallback_used;
                        if result.fallback_used {
                            warn!(
                                "UDS notify degraded to fallback for ch={} pt={:?} point={}",
                                ctx.target_channel_id, pt, ctx.target_point_id
                            );
                        }
                    }
                },
                Err(_) => {
                    warn!("ShmNotifier lock timeout, comsrv will detect via SHM poll");
                    outcome.fallback_used = true;
                },
            }
        }

        outcome
    }

    fn rebuild_writer(&self, routing_cache: &voltage_routing::RoutingCache) {
        let Some(config) = self.config.get() else {
            return; // SHM not configured
        };
        match voltage_rtdb_shm::UnifiedWriter::open_for_actions(config, routing_cache) {
            Ok(writer) => {
                self.writer.store(Some(Arc::new(writer)));
                info!("SHM action writer rebuilt after routing change");
            },
            Err(e) => {
                warn!("SHM action writer rebuild failed: {}", e);
            },
        }
    }
}

/// No-op dispatch for testing and environments without SHM
pub struct NoopDispatch;

#[async_trait]
impl ActionDispatch for NoopDispatch {
    async fn dispatch(&self, _ctx: &RouteContext, _value: f64) -> DispatchOutcome {
        DispatchOutcome {
            shm_written: false,
            uds_notified: false,
            fallback_used: false,
        }
    }

    fn rebuild_writer(&self, _routing_cache: &voltage_routing::RoutingCache) {
        // No-op
    }
}
