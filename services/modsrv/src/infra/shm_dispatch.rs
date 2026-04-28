//! SHM/UDS Action Dispatch Infrastructure
//!
//! Provides the low-latency M2C (modsrv-to-comsrv) dispatch path via shared memory
//! and Unix Domain Socket notifications.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use tracing::warn;
use voltage_routing::RouteContext;

/// Outcome of an action dispatch operation
///
/// Each variant represents a mutually exclusive dispatch result.
/// Using an enum instead of multiple bools prevents invalid state combinations.
#[derive(Debug)]
#[must_use]
pub enum DispatchOutcome {
    /// SHM written + UDS notification sent (happy path, ~1-2ms)
    Delivered,
    /// SHM written but UDS notification failed or skipped.
    /// comsrv delivery is not guaranteed without UDS.
    ShmOnly { reason: &'static str },
    /// Dispatch failed — no SHM writer configured
    NoWriter,
    /// No-op dispatch (test environment, intentionally skips all transport)
    Noop,
}

/// Trait for dispatching action commands to comsrv
///
/// The primary implementation uses SHM + UDS for ~1-2ms latency.
/// Test implementations use NoopDispatch (no-op, skips all transport).
#[async_trait]
pub trait ActionDispatch: Send + Sync {
    /// Dispatch an action value to the target channel via the fastest available path
    async fn dispatch(&self, ctx: &RouteContext, value: f64) -> DispatchOutcome;
}

/// SHM + UDS dispatch implementation (production path)
///
/// Writes action values directly to shared memory, then sends a UDS notification
/// to comsrv for immediate processing. Degrades gracefully if UDS notification fails
/// (SHM value is written but comsrv delivery is not guaranteed without UDS).
pub struct ShmDispatch {
    writer: arc_swap::ArcSwapOption<voltage_rtdb_shm::UnifiedWriter>,
    config: std::sync::OnceLock<voltage_rtdb_shm::SharedConfig>,
    notifier: std::sync::OnceLock<Arc<tokio::sync::Mutex<voltage_rtdb_shm::ShmNotifier>>>,
    expected_generation: AtomicU64,
    /// Signal to trigger background SHM writer rebuild after generation mismatch
    rebuild_trigger: Arc<tokio::sync::Notify>,
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
            expected_generation: AtomicU64::new(0),
            rebuild_trigger: Arc::new(tokio::sync::Notify::new()),
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
        // Capture generation so dispatch() can detect comsrv restarts or SHM rebuilds.
        self.expected_generation
            .store(writer.generation(), Ordering::Release);
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

    /// Returns true if the SHM writer is currently configured and available.
    ///
    /// A `false` result means comsrv may have restarted and the writer was
    /// cleared (generation mismatch). Actions will return `DispatchDegraded`
    /// until routing is refreshed and the writer is rebuilt.
    pub fn is_writer_available(&self) -> bool {
        self.writer.load().is_some()
    }

    /// Returns true if the UDS notifier has been configured.
    ///
    /// Note: this checks only whether `set_notifier()` was called, not whether
    /// the UDS socket is currently connected. Use `is_writer_available()` as
    /// the primary liveness signal; UDS reconnects automatically on failure.
    pub fn is_notifier_configured(&self) -> bool {
        self.notifier.get().is_some()
    }

    /// Get the rebuild trigger for spawning a background rebuild task.
    ///
    /// The returned `Notify` fires when `dispatch()` detects a generation
    /// mismatch (comsrv restarted). The background task should reopen
    /// the SHM writer and call `set_writer()` to restore dispatch.
    pub fn rebuild_trigger(&self) -> Arc<tokio::sync::Notify> {
        Arc::clone(&self.rebuild_trigger)
    }
}

#[async_trait]
impl ActionDispatch for ShmDispatch {
    async fn dispatch(&self, ctx: &RouteContext, value: f64) -> DispatchOutcome {
        // Step 1: Write action value to shared memory (zero-copy IPC)
        let writer_guard = self.writer.load();
        let Some(writer) = writer_guard.as_ref() else {
            return DispatchOutcome::NoWriter;
        };

        // Generation check: detect comsrv restarts that changed the SHM layout.
        // expected == 0 should not occur in practice (set_writer always writes the generation),
        // but guard it anyway to avoid false positives during initialization.
        let current_gen = writer.generation();
        let expected = self.expected_generation.load(Ordering::Acquire);
        if expected != 0 && current_gen != expected {
            warn!(
                "SHM writer generation mismatch (expected={}, actual={}). \
                 comsrv may have restarted. Clearing writer and triggering rebuild.",
                expected, current_gen
            );
            self.writer.store(None);
            self.rebuild_trigger.notify_one();
            return DispatchOutcome::NoWriter;
        }

        let mirrored = writer.set_action(
            ctx.target_channel_id,
            ctx.target_point_type,
            ctx.target_point_id,
            value,
            ctx.timestamp_ms as u64,
        );
        if !mirrored {
            warn!(
                "SHM action mirror miss for ch={} pt={} point={}",
                ctx.target_channel_id, ctx.target_point_type, ctx.target_point_id
            );
        }

        // Step 2: UDS notification for event-driven dispatch (~1-2ms latency)
        let Some(notifier_lock) = self.notifier.get() else {
            return DispatchOutcome::ShmOnly {
                reason: "notifier not configured",
            };
        };

        let mut guard = match tokio::time::timeout(Duration::from_millis(100), notifier_lock.lock())
            .await
        {
            Ok(guard) => guard,
            Err(_) => {
                warn!(
                    "ShmNotifier lock timeout; SHM value written but UDS notification skipped — action may not reach comsrv"
                );
                return DispatchOutcome::ShmOnly {
                    reason: "notifier lock timeout",
                };
            },
        };

        let pt = match voltage_model::PointType::from_u8(ctx.target_point_type) {
            Some(pt) => pt,
            None => {
                warn!(
                    "Ch{} point_type {} invalid, UDS notification skipped",
                    ctx.target_channel_id, ctx.target_point_type
                );
                return DispatchOutcome::ShmOnly {
                    reason: "invalid point_type",
                };
            },
        };

        let result = guard
            .notify(
                ctx.target_channel_id,
                pt,
                ctx.target_point_id,
                value,
                ctx.timestamp_ms as u64,
            )
            .await;

        if result.uds_sent {
            DispatchOutcome::Delivered
        } else {
            if result.fallback_used {
                warn!(
                    "UDS notify degraded for ch={} pt={:?} point={}",
                    ctx.target_channel_id, pt, ctx.target_point_id
                );
            }
            DispatchOutcome::ShmOnly {
                reason: if result.fallback_used {
                    "UDS degraded"
                } else {
                    "UDS disabled"
                },
            }
        }
    }
}

/// No-op dispatch for testing and environments without SHM
pub struct NoopDispatch;

#[async_trait]
impl ActionDispatch for NoopDispatch {
    async fn dispatch(&self, _ctx: &RouteContext, _value: f64) -> DispatchOutcome {
        DispatchOutcome::Noop
    }
}
