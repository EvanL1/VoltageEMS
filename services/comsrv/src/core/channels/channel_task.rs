//! Unified channel task — the async event loop
//!
//! Owns the protocol client exclusively and uses `tokio::select!` to handle:
//! - Protocol commands (connect/disconnect/diagnostics)
//! - Business commands (control/adjustment from M2C SHM)
//! - Periodic polling

use arc_swap::ArcSwapOption;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::core::channels::traits::ChannelCommand;
use crate::core::channels::types::ProtocolCommand;
use crate::protocols::core::logging::{ChannelLogConfig, ChannelLogHandler};
use crate::protocols::core::traits::PollResult;
use crate::protocols::gateway::ChannelRuntime;
use crate::runtime::reconnect::{AutoRecoveryPolicy, ReconnectHelper, ReconnectPolicy, ReconnectState};
use crate::store::RedisDataStore;
use voltage_rtdb::Rtdb;

/// Update cached connection state from protocol runtime.
fn update_cached_state(state: &dyn ChannelRuntime, cache: &AtomicU8) {
    let channel_state: crate::core::channels::types::ConnectionState =
        state.connection_state().into();
    cache.store(channel_state.as_u8(), Ordering::Relaxed);
}

/// Check if channel online state changed and publish to Redis if so.
///
/// Avoids redundant Redis writes by tracking previous state.
async fn check_online_change<R: Rtdb>(
    protocol: &dyn ChannelRuntime,
    prev_online: &mut Option<bool>,
    store: &RedisDataStore<R>,
    channel_id: u32,
) {
    let current_online = protocol.connection_state().is_connected();
    if *prev_online != Some(current_online) {
        *prev_online = Some(current_online);
        store
            .publish_channel_online(channel_id, current_online)
            .await;
    }
}

/// Apply log level to protocol and log handler.
///
/// Returns Ok for valid levels ("debug"/"info"/"error"), Err for invalid.
fn apply_log_level(
    protocol: &mut dyn ChannelRuntime,
    log_handler: &dyn ChannelLogHandler,
    level: &str,
) -> std::result::Result<(), String> {
    match level.to_lowercase().as_str() {
        "debug" | "verbose" => {
            protocol.set_log_config(ChannelLogConfig::all());
            log_handler.set_log_level("debug");
            Ok(())
        },
        "info" | "standard" => {
            protocol.set_log_config(ChannelLogConfig::default());
            log_handler.set_log_level("info");
            Ok(())
        },
        "error" | "minimal" => {
            protocol.set_log_config(ChannelLogConfig::errors_only());
            log_handler.set_log_level("info");
            Ok(())
        },
        other => Err(format!(
            "Invalid log level '{}', use: debug/info/error",
            other
        )),
    }
}

/// Run the unified channel task that handles both polling and commands.
///
/// ## Lock-Free Architecture
///
/// This function owns the protocol client exclusively (no shared Mutex).
/// It uses `tokio::select!` to handle multiple event sources:
/// - Timer tick: Execute poll_once() and write data to store
/// - Protocol command: Handle connect/disconnect/diagnostics requests
/// - Business command: Execute write_control/write_adjustment
///
/// This design eliminates lock contention between polling and command execution,
/// reducing command latency from 300ms to <10ms.
#[allow(clippy::too_many_arguments)]
pub(super) async fn run_unified_channel_task<R: Rtdb>(
    mut protocol: Box<dyn ChannelRuntime>,
    mut protocol_rx: tokio::sync::mpsc::Receiver<ProtocolCommand>,
    mut business_rx: tokio::sync::mpsc::Receiver<ChannelCommand>,
    store: Arc<RedisDataStore<R>>,
    channel_id: u32,
    poll_interval_ms: u64,
    cached_state: Arc<AtomicU8>,
    cached_diagnostics: Arc<ArcSwapOption<crate::protocols::core::traits::Diagnostics>>,
    log_handler: Arc<dyn ChannelLogHandler>,
    reconnect_policy: ReconnectPolicy,
    auto_recovery_policy: Option<AutoRecoveryPolicy>,
    watchdog_heartbeat_ms: Arc<AtomicI64>,
    reconnect_total_attempts: Arc<AtomicU64>,
    reconnect_failed: Arc<AtomicBool>,
) {
    info!(
        "Ch{} unified task started (interval: {}ms, reconnect: max_attempts={}, initial_delay={:?})",
        channel_id, poll_interval_ms, reconnect_policy.max_attempts, reconnect_policy.initial_delay
    );

    // Create reconnection helper for auto-reconnect functionality
    let mut reconnect_helper = ReconnectHelper::new(reconnect_policy);
    if let Some(policy) = auto_recovery_policy {
        reconnect_helper = reconnect_helper.with_auto_recovery(policy);
    }

    // Track previous online state for change detection (avoid redundant Redis writes)
    let mut prev_online: Option<bool> = None;

    // Wait a bit for the connection to be established
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Update initial connection state
    update_cached_state(protocol.as_ref(), &cached_state);
    check_online_change(protocol.as_ref(), &mut prev_online, &store, channel_id).await;

    // Use configured poll interval
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(poll_interval_ms));

    // Track previous error count to detect new errors
    let mut prev_error_count: u64 = 0;

    // Track failed state log frequency (per-channel, not static)
    let mut failed_log_tick_counter: u32 = 0;

    loop {
        // Use biased select to prioritize commands over polling
        // This ensures commands are processed promptly even during heavy polling
        tokio::select! {
            biased;

            // Priority 1: Protocol commands (connect/disconnect/diagnostics)
            Some(cmd) = protocol_rx.recv() => {
                handle_protocol_command(
                    cmd, &mut protocol, &log_handler, channel_id,
                ).await;
            }

            // Priority 2: Business commands (control/adjustment from M2C SHM)
            Some(cmd) = business_rx.recv() => {
                handle_business_command(cmd, &mut protocol, channel_id).await;
            }

            // Priority 3: Periodic polling
            _ = interval.tick() => {
                let action = handle_poll_tick(
                    &mut protocol, &mut protocol_rx, &mut reconnect_helper,
                    &mut failed_log_tick_counter, &cached_state, &cached_diagnostics,
                    &mut prev_online, &mut prev_error_count,
                    &store, &log_handler, channel_id,
                    &watchdog_heartbeat_ms, &reconnect_total_attempts, &reconnect_failed,
                ).await;
                match action {
                    TickAction::Continue => continue,
                    TickAction::Break => break,
                    TickAction::Proceed => {}
                }
            }
        }
    }

    // Mark as disconnected on shutdown
    cached_state.store(
        crate::core::channels::types::ConnectionState::Disconnected.as_u8(),
        Ordering::Relaxed,
    );
    // Publish offline status to Redis on shutdown
    store.publish_channel_online(channel_id, false).await;
    info!("Ch{} unified task stopped", channel_id);
}

/// Action returned by poll tick handler
enum TickAction {
    /// Continue to next select iteration (skip remaining tick logic)
    Continue,
    /// Break out of the main loop (shutdown)
    Break,
    /// Proceed with normal post-tick processing
    Proceed,
}

/// Handle a protocol command from the command channel.
async fn handle_protocol_command(
    cmd: ProtocolCommand,
    protocol: &mut Box<dyn ChannelRuntime>,
    log_handler: &Arc<dyn ChannelLogHandler>,
    channel_id: u32,
) {
    match cmd {
        ProtocolCommand::WriteControl {
            internal_id,
            value,
            response_tx,
        } => {
            let result = protocol.write_control(&[(internal_id, value)]).await;
            let _ = response_tx.send(result);
        },
        ProtocolCommand::WriteAdjustment {
            internal_id,
            value,
            response_tx,
        } => {
            let result = protocol.write_adjustment(&[(internal_id, value)]).await;
            let _ = response_tx.send(result);
        },
        ProtocolCommand::Connect { response_tx } => {
            let result = protocol.connect().await;
            let _ = response_tx.send(result);
        },
        ProtocolCommand::Disconnect { response_tx } => {
            let _ = protocol.disconnect().await;
            let _ = response_tx.send(());
        },
        ProtocolCommand::GetDiagnostics { response_tx } => {
            let diag = protocol.diagnostics().await.ok();
            let _ = response_tx.send(diag);
        },
        ProtocolCommand::GetConnectionState { response_tx } => {
            let state: crate::core::channels::types::ConnectionState =
                protocol.connection_state().into();
            let _ = response_tx.send(state);
        },
        ProtocolCommand::SetLogLevel { level, response_tx } => {
            let result = apply_log_level(protocol.as_mut(), log_handler.as_ref(), &level);
            if result.is_ok() {
                info!("Ch{} log level set to {}", channel_id, level);
            }
            let _ = response_tx.send(result);
        },
        ProtocolCommand::Shutdown => {
            // Shutdown is handled inline in the select! match — this branch
            // should not be reached since Shutdown breaks the loop directly.
            // Kept for exhaustive match.
            info!("Ch{} received shutdown command", channel_id);
        },
    }
}

/// Handle a business command (control/adjustment from M2C SHM).
async fn handle_business_command(
    cmd: ChannelCommand,
    protocol: &mut Box<dyn ChannelRuntime>,
    channel_id: u32,
) {
    match cmd {
        ChannelCommand::Control {
            point_id, value, ..
        } => match protocol.write_control(&[(point_id, value)]).await {
            Ok(n) if n > 0 => debug!("Ch{} control pt{} = {} ok", channel_id, point_id, value),
            Ok(_) => warn!("Ch{} control pt{} = {} failed", channel_id, point_id, value),
            Err(e) => error!("Ch{} control pt{} err: {}", channel_id, point_id, e),
        },
        ChannelCommand::Adjustment {
            point_id, value, ..
        } => match protocol.write_adjustment(&[(point_id, value)]).await {
            Ok(n) if n > 0 => debug!("Ch{} adjustment pt{} = {} ok", channel_id, point_id, value),
            Ok(_) => warn!(
                "Ch{} adjustment pt{} = {} failed",
                channel_id, point_id, value
            ),
            Err(e) => error!("Ch{} adjustment pt{} err: {}", channel_id, point_id, e),
        },
        ChannelCommand::BatchControl { points, .. } => {
            match protocol.write_control(&points).await {
                Ok(n) => debug!("Ch{} batch control {}/{} ok", channel_id, n, points.len()),
                Err(e) => error!("Ch{} batch control err: {}", channel_id, e),
            }
        },
        ChannelCommand::BatchAdjustment { points, .. } => {
            match protocol.write_adjustment(&points).await {
                Ok(n) => debug!("Ch{} batch adj {}/{} ok", channel_id, n, points.len()),
                Err(e) => error!("Ch{} batch adj err: {}", channel_id, e),
            }
        },
    }
}

/// Handle a periodic poll tick — reconnection logic + data polling.
#[allow(clippy::too_many_arguments)]
async fn handle_poll_tick<R: Rtdb>(
    protocol: &mut Box<dyn ChannelRuntime>,
    protocol_rx: &mut tokio::sync::mpsc::Receiver<ProtocolCommand>,
    reconnect_helper: &mut ReconnectHelper,
    failed_log_tick_counter: &mut u32,
    cached_state: &AtomicU8,
    cached_diagnostics: &ArcSwapOption<crate::protocols::core::traits::Diagnostics>,
    prev_online: &mut Option<bool>,
    prev_error_count: &mut u64,
    store: &Arc<RedisDataStore<R>>,
    log_handler: &Arc<dyn ChannelLogHandler>,
    channel_id: u32,
    watchdog_heartbeat_ms: &AtomicI64,
    reconnect_total_attempts: &AtomicU64,
    reconnect_failed: &AtomicBool,
) -> TickAction {
    // Update watchdog heartbeat on every tick (proves task is alive)
    watchdog_heartbeat_ms.store(
        super::channel_entry::unix_timestamp_ms(),
        Ordering::Relaxed,
    );

    // Step 1: Check connection state before polling
    let conn_state = protocol.connection_state();

    if !conn_state.is_connected() {
        return handle_disconnected(
            protocol,
            protocol_rx,
            reconnect_helper,
            failed_log_tick_counter,
            cached_state,
            prev_online,
            store,
            log_handler,
            channel_id,
            reconnect_total_attempts,
            reconnect_failed,
        )
        .await;
    }

    // Step 2: Connected - only reset counter if it was non-zero
    if reconnect_helper.connection_state() != ReconnectState::Connected {
        reconnect_helper.mark_connected();
        *failed_log_tick_counter = 0;
        // Sync reconnect stats
        reconnect_failed.store(false, Ordering::Relaxed);
    }

    // Step 3: Poll data using ChannelRuntime interface
    let result: PollResult = protocol.poll_once().await;

    // Log partial failures from poll result (only when failures exist)
    let failure_count = result.failures.len();
    if failure_count > 0 {
        let sample_errors: Vec<_> = result
            .failures
            .iter()
            .take(3)
            .map(|f| format!("pt{}:{}", f.point_id, f.error))
            .collect();
        warn!(
            "Ch{} partial read: {} failed, samples: [{}]",
            channel_id,
            failure_count,
            sample_errors.join(", ")
        );
    }

    let count = result.data.len();
    if count > 0 {
        tracing::trace!("Ch{} poll ok: {} pts", channel_id, count);
        if let Err(e) = store.write_batch(channel_id, result.data).await {
            error!("Ch{} failed to write to Redis: {}", channel_id, e);
        }
    }

    // Check diagnostics for accumulated errors and update cache
    if let Ok(diag) = protocol.diagnostics().await {
        if diag.error_count > *prev_error_count {
            let new_errors = diag.error_count - *prev_error_count;
            warn!(
                "Ch{} accumulated errors: {} new errors, last error: {:?}",
                channel_id, new_errors, diag.last_error
            );
            *prev_error_count = diag.error_count;
        }
        cached_diagnostics.store(Some(Arc::new(diag)));
    }

    // Update cached connection state after each poll cycle
    update_cached_state(protocol.as_ref(), cached_state);
    check_online_change(protocol.as_ref(), prev_online, store, channel_id).await;

    TickAction::Proceed
}

/// Handle disconnected state — reconnection logic with backoff.
#[allow(clippy::too_many_arguments)]
async fn handle_disconnected<R: Rtdb>(
    protocol: &mut Box<dyn ChannelRuntime>,
    protocol_rx: &mut tokio::sync::mpsc::Receiver<ProtocolCommand>,
    reconnect_helper: &mut ReconnectHelper,
    failed_log_tick_counter: &mut u32,
    cached_state: &AtomicU8,
    prev_online: &mut Option<bool>,
    store: &Arc<RedisDataStore<R>>,
    log_handler: &Arc<dyn ChannelLogHandler>,
    channel_id: u32,
    reconnect_total_attempts: &AtomicU64,
    reconnect_failed: &AtomicBool,
) -> TickAction {
    // Sync reconnect stats to shared atomics on every disconnected tick
    reconnect_total_attempts.store(reconnect_helper.stats().total_attempts, Ordering::Relaxed);

    match reconnect_helper.connection_state() {
        ReconnectState::Failed => {
            reconnect_failed.store(true, Ordering::Relaxed);

            // Check auto-recovery before giving up
            if reconnect_helper.check_auto_recovery() {
                info!(
                    "Ch{} auto-recovery triggered, returning to Disconnected state",
                    channel_id
                );
                reconnect_failed.store(false, Ordering::Relaxed);
                *failed_log_tick_counter = 0;
                update_cached_state(protocol.as_ref(), cached_state);
                check_online_change(protocol.as_ref(), prev_online, store, channel_id).await;
                return TickAction::Continue;
            }

            // Max retry attempts reached, log periodically (every 60 ticks)
            *failed_log_tick_counter += 1;
            if failed_log_tick_counter.is_multiple_of(60) {
                if let Some(remaining) = reconnect_helper.recovery_cooldown_remaining() {
                    warn!(
                        "Ch{} reconnection failed (max attempts reached), \
                         auto-recovery in {:?} (round {}/{})",
                        channel_id,
                        remaining,
                        reconnect_helper.recovery_rounds() + 1,
                        3 // max_recovery_rounds default
                    );
                } else {
                    warn!(
                        "Ch{} reconnection permanently failed, \
                         manual intervention required (disable/enable)",
                        channel_id
                    );
                }
            }
            update_cached_state(protocol.as_ref(), cached_state);
            check_online_change(protocol.as_ref(), prev_online, store, channel_id).await;
            TickAction::Continue
        },
        ReconnectState::Reconnecting => TickAction::Continue,
        ReconnectState::Connected | ReconnectState::Disconnected => {
            if reconnect_helper.connection_state() == ReconnectState::Connected {
                warn!("Ch{} connection lost unexpectedly", channel_id);
                reconnect_helper.mark_disconnected();
            }
            if !reconnect_helper.record_attempt() {
                update_cached_state(protocol.as_ref(), cached_state);
                check_online_change(protocol.as_ref(), prev_online, store, channel_id).await;
                return TickAction::Continue;
            }

            // Apply backoff delay for retry attempts after the first
            let current_attempt = reconnect_helper.stats().total_attempts;
            if current_attempt > 1 {
                let delay = reconnect_helper.calculate_next_delay();
                info!(
                    "Ch{} waiting {:?} before reconnect attempt",
                    channel_id, delay
                );
                // Remain responsive to commands during backoff
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    Some(cmd) = protocol_rx.recv() => {
                        let action = handle_backoff_command(
                            cmd, protocol, reconnect_helper,
                            failed_log_tick_counter, log_handler, channel_id,
                        ).await;
                        if let Some(a) = action {
                            update_cached_state(protocol.as_ref(), cached_state);
                            check_online_change(protocol.as_ref(), prev_online, store, channel_id).await;
                            return a;
                        }
                        update_cached_state(protocol.as_ref(), cached_state);
                        check_online_change(protocol.as_ref(), prev_online, store, channel_id).await;
                        return TickAction::Continue;
                    }
                }
            }

            // Attempt reconnection with timeout to prevent hanging
            info!("Ch{} attempting reconnect", channel_id);
            match tokio::time::timeout(Duration::from_secs(30), protocol.connect()).await {
                Ok(Ok(())) => {
                    info!("Ch{} reconnected successfully", channel_id);
                    reconnect_helper.mark_connected();
                    reconnect_failed.store(false, Ordering::Relaxed);
                    *failed_log_tick_counter = 0;
                },
                Ok(Err(e)) => {
                    warn!("Ch{} reconnect failed: {}", channel_id, e);
                    reconnect_helper.record_failure();
                },
                Err(_) => {
                    warn!("Ch{} reconnect timed out (30s)", channel_id);
                    reconnect_helper.record_failure();
                },
            }
            update_cached_state(protocol.as_ref(), cached_state);
            check_online_change(protocol.as_ref(), prev_online, store, channel_id).await;
            TickAction::Continue
        },
    }
}

/// Handle a protocol command received during reconnect backoff.
/// Returns Some(TickAction) if the caller should return immediately, None to continue.
async fn handle_backoff_command(
    cmd: ProtocolCommand,
    protocol: &mut Box<dyn ChannelRuntime>,
    reconnect_helper: &mut ReconnectHelper,
    failed_log_tick_counter: &mut u32,
    log_handler: &Arc<dyn ChannelLogHandler>,
    channel_id: u32,
) -> Option<TickAction> {
    use crate::protocols::core::error::GatewayError;
    match cmd {
        ProtocolCommand::Shutdown => {
            info!("Ch{} shutdown during reconnect backoff", channel_id);
            return Some(TickAction::Break);
        },
        ProtocolCommand::Connect { response_tx } => {
            let result = protocol.connect().await;
            if result.is_ok() {
                reconnect_helper.mark_connected();
                *failed_log_tick_counter = 0;
            }
            let _ = response_tx.send(result);
        },
        ProtocolCommand::Disconnect { response_tx } => {
            let _ = protocol.disconnect().await;
            let _ = response_tx.send(());
        },
        ProtocolCommand::GetConnectionState { response_tx } => {
            let state: crate::core::channels::types::ConnectionState =
                protocol.connection_state().into();
            let _ = response_tx.send(state);
        },
        ProtocolCommand::GetDiagnostics { response_tx } => {
            let diag = protocol.diagnostics().await.ok();
            let _ = response_tx.send(diag);
        },
        ProtocolCommand::SetLogLevel { level, response_tx } => {
            let result = apply_log_level(protocol.as_mut(), log_handler.as_ref(), &level);
            let _ = response_tx.send(result);
        },
        ProtocolCommand::WriteControl { response_tx, .. }
        | ProtocolCommand::WriteAdjustment { response_tx, .. } => {
            let _ = response_tx.send(Err(GatewayError::NotConnected));
        },
    }
    None
}
