//! Model Service (ModSrv)
//!
//! Model management service supporting measurement/action separation architecture.
//! Rule Engine API is integrated on the same port (6002).

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
#[cfg(feature = "swagger-ui")]
use utoipa::OpenApi;
#[cfg(feature = "swagger-ui")]
use utoipa_swagger_ui::SwaggerUi;

// modsrv imports
#[cfg(feature = "swagger-ui")]
use modsrv::rule_routes::RuleApiDoc;
use modsrv::{
    DEFAULT_TICK_MS, Result, RuleScheduler, bootstrap, routes,
    rule_routes::{RuleEngineState, create_rule_routes},
};
use voltage_calc::RtdbStateStore;
use voltage_rtdb_shm::{PointWatchListener, SubscriptionBitmap, bitmap_path_from_shm};
use voltage_rtdb_shm::{SharedConfig, UnifiedReader, is_shm_available};
use voltage_rules::{PointWatchDispatcher, WatchEvent};

#[tokio::main]
async fn main() -> Result<()> {
    // Create service info
    let service_info = bootstrap::create_service_info();

    // Initialize cancellation token for graceful shutdown
    let shutdown_token = CancellationToken::new();
    debug!("Shutdown token initialized");

    // Create application state with all initialized components
    let state = bootstrap::create_app_state(&service_info).await?;

    // Create API routes using the routes module
    let app = routes::create_routes(Arc::clone(&state));

    #[cfg(feature = "swagger-ui")]
    let app = {
        info!("Swagger UI feature ENABLED - initializing at /docs");
        // Merge ModsrvApiDoc with RuleApiDoc for complete OpenAPI documentation
        let openapi = routes::ModsrvApiDoc::openapi().nest("", RuleApiDoc::openapi());
        let merged = app.merge(SwaggerUi::new("/docs").url("/openapi.json", openapi));
        info!("Swagger UI configured successfully (including Rule Engine API)");
        merged
    };

    #[cfg(not(feature = "swagger-ui"))]
    info!("Swagger UI feature DISABLED");

    // ============================================================================
    // Initialize Rule Engine (integrated on port 6002)
    // ============================================================================
    let sqlite_pool = state.instance_manager.pool().clone();
    let rtdb = state.instance_manager.rtdb.clone();
    let routing_cache = state.instance_manager.routing_cache().clone();

    // Load tick_ms from global config (SQLite key-value table)
    let tick_ms: u64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM service_config WHERE service_name = 'global' AND key = 'rules.tick_ms'",
    )
    .fetch_optional(&sqlite_pool)
    .await
    .ok()
    .flatten()
    .and_then(|s| s.parse().ok())
    .unwrap_or(DEFAULT_TICK_MS);

    debug!("Rule scheduler tick_ms: {}", tick_ms);

    // Initialize SharedConfig for shared memory access
    // Load SharedConfig parameters from database
    let shm_config = {
        let mut cfg = SharedConfig::default();

        // Helper to load usize value from service_config
        async fn load_usize(pool: &sqlx::SqlitePool, key: &str) -> Option<usize> {
            sqlx::query_scalar::<_, String>(
                "SELECT value FROM service_config WHERE service_name = 'global' AND key = ?",
            )
            .bind(key)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
        }

        if let Some(v) = load_usize(&sqlite_pool, "shared_memory.max_slots").await {
            cfg = cfg.with_max_slots(v);
        }

        debug!("SharedConfig: max_slots={:?}", cfg.max_slots());
        cfg
    };

    // Load channel point counts for SHM layout (routing-independent)
    let channel_points = voltage_rtdb_shm::ChannelPointCounts::load_from_db(&sqlite_pool)
        .await
        .unwrap_or_else(|e| {
            warn!("Failed to load channel points: {}, using empty layout", e);
            voltage_rtdb_shm::ChannelPointCounts::new()
        });

    // Initialize UnifiedReader for cross-process zero-copy reads
    // Simplified: Header + PointSlots only, indexes built from channel points
    // Added retry mechanism for cold start race condition
    let shared_reader = {
        const MAX_RETRIES: u32 = 10;
        const BASE_DELAY_MS: u64 = 1000;
        const MAX_DELAY_MS: u64 = 15000;
        let mut retry_count = 0;

        loop {
            if is_shm_available(&shm_config) {
                // Open reader with RoutingCache (builds indexes from routing)
                match UnifiedReader::open(&shm_config, &channel_points) {
                    Ok(reader) => {
                        info!(
                            "UnifiedReader opened: {} slots, {} instances, {} channels",
                            reader.slot_count(),
                            reader.instance_ids(&routing_cache).len(),
                            reader.channel_ids().len()
                        );
                        break Some(Arc::new(reader));
                    },
                    Err(e) if retry_count < MAX_RETRIES => {
                        let delay_ms = (BASE_DELAY_MS * 2u64.pow(retry_count)).min(MAX_DELAY_MS);
                        info!(
                            "SharedMemory not ready (retry {}/{}, next in {}ms): {}",
                            retry_count + 1,
                            MAX_RETRIES,
                            delay_ms,
                            e
                        );
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        retry_count += 1;
                    },
                    Err(e) => {
                        warn!(
                            "UnifiedReader unavailable after {} retries: {}",
                            MAX_RETRIES, e
                        );
                        break None;
                    },
                }
            } else if retry_count < MAX_RETRIES {
                let delay_ms = (BASE_DELAY_MS * 2u64.pow(retry_count)).min(MAX_DELAY_MS);
                info!(
                    "SharedMemory path not found (retry {}/{}, next in {}ms), waiting for comsrv...",
                    retry_count + 1,
                    MAX_RETRIES,
                    delay_ms
                );
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                retry_count += 1;
            } else {
                warn!(
                    "SharedMemory unavailable after {} retries; M2C dispatch will be unavailable (SHM writer not configured)",
                    MAX_RETRIES
                );
                break None;
            }
        }
    };

    // Initialize UnifiedWriter for M2C actions (Control/Adjustment via SHM)
    // Only open if reader succeeded (SHM file exists)
    let shm_action_writer = if shared_reader.is_some() {
        match voltage_rtdb_shm::UnifiedWriter::open_for_actions(&shm_config, &channel_points) {
            Ok(writer) => {
                info!("UnifiedWriter (actions) opened for M2C via SHM");
                Some(Arc::new(writer))
            },
            Err(e) => {
                warn!("UnifiedWriter (actions) unavailable: {}", e);
                None
            },
        }
    } else {
        None
    };

    // Configure ShmDispatch with SHM components for M2C via shared memory
    // ShmDispatch uses ArcSwapOption/OnceLock for delayed initialization
    // ShmNotifier is shared between ShmDispatch and RuleScheduler for unified M2C dispatch
    // Held only to keep the Arc alive while ShmDispatch borrows it; rule
    // engine now consumes ShmDispatch directly, not the raw notifier.
    let _shm_notifier: Option<Arc<tokio::sync::Mutex<voltage_rtdb_shm::ShmNotifier>>> =
        if let Some(ref writer) = shm_action_writer {
            // Set SHM action writer for direct M2C writes (+ store config for rebuild)
            state
                .shm_dispatch
                .set_writer(Arc::clone(writer), shm_config.clone());
            info!("ShmDispatch: SHM action writer configured");

            // Connect ShmNotifier for event-driven M2C dispatch (~1-2ms latency).
            // connect_default() never returns Err (connect failure yields a degraded Ok),
            // but handle the Err arm defensively: always call set_notifier so that the
            // auto-reconnect logic inside ShmNotifier can activate on future notify() calls.
            match voltage_rtdb_shm::ShmNotifier::connect_default().await {
                Ok(notifier) => {
                    let notifier = Arc::new(tokio::sync::Mutex::new(notifier));
                    if state.shm_dispatch.set_notifier(Arc::clone(&notifier)) {
                        info!("ShmDispatch: ShmNotifier configured for event-driven dispatch");
                    }
                    Some(notifier)
                },
                Err(e) => {
                    // Defensive path: create a degraded notifier so set_notifier is always
                    // called and auto-reconnect can activate when comsrv UDS becomes ready.
                    info!(
                        "ShmNotifier unavailable (UDS listener not ready), will auto-reconnect: {}",
                        e
                    );
                    let notifier = Arc::new(tokio::sync::Mutex::new(
                        voltage_rtdb_shm::ShmNotifier::disabled(),
                    ));
                    state.shm_dispatch.set_notifier(Arc::clone(&notifier));
                    Some(notifier)
                },
            }
        } else {
            None
        };

    // Spawn SHM writer auto-rebuild task.
    // When dispatch() detects a generation mismatch (comsrv restarted), it fires
    // rebuild_trigger. This task re-opens the writer with exponential backoff,
    // restoring M2C dispatch without a modsrv restart.
    {
        let rebuild_notify = state.shm_dispatch.rebuild_trigger();
        let rebuild_dispatch = Arc::clone(&state.shm_dispatch);
        let rebuild_pool = sqlite_pool.clone();
        let rebuild_shm_config = shm_config.clone();
        let rebuild_token = shutdown_token.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = rebuild_notify.notified() => {},
                    _ = rebuild_token.cancelled() => break,
                }
                info!("SHM rebuild triggered — attempting to reopen writer...");
                const MAX_RETRIES: u32 = 10;
                const BASE_DELAY_MS: u64 = 1000;
                const MAX_DELAY_MS: u64 = 15000;
                let mut retry_count = 0u32;
                let ok = loop {
                    // Reload channel points (layout may have changed)
                    let cp = voltage_rtdb_shm::ChannelPointCounts::load_from_db(&rebuild_pool)
                        .await
                        .unwrap_or_else(|e| {
                            warn!("SHM rebuild: failed to load channel points: {}", e);
                            voltage_rtdb_shm::ChannelPointCounts::new()
                        });
                    match voltage_rtdb_shm::UnifiedWriter::open_for_actions(
                        &rebuild_shm_config,
                        &cp,
                    ) {
                        Ok(writer) => {
                            let writer = Arc::new(writer);
                            rebuild_dispatch
                                .set_writer(Arc::clone(&writer), rebuild_shm_config.clone());
                            info!("SHM rebuild: writer restored successfully");
                            break true;
                        },
                        Err(e) if retry_count < MAX_RETRIES => {
                            let delay = (BASE_DELAY_MS * 2u64.pow(retry_count)).min(MAX_DELAY_MS);
                            info!(
                                "SHM rebuild retry {}/{} in {}ms: {}",
                                retry_count + 1,
                                MAX_RETRIES,
                                delay,
                                e
                            );
                            tokio::time::sleep(Duration::from_millis(delay)).await;
                            retry_count += 1;
                        },
                        Err(e) => {
                            warn!(
                                "SHM rebuild failed after {} retries: {}. \
                                 Will retry on next generation mismatch.",
                                MAX_RETRIES, e
                            );
                            break false;
                        },
                    }
                };
                if ok {
                    info!("SHM auto-rebuild complete — M2C dispatch restored");
                }
            }
        });
    }

    // Spawn SHM canonical-path inode watcher.
    //
    // Step 3 of the SHM decoupling roadmap replaces in-place
    // reconfigure_existing with `ShmHandle::rebuild_via_swap`: comsrv
    // creates a new SHM file at a staging path, then POSIX-renames it
    // over the canonical path. modsrv's existing UnifiedWriter is still
    // mmap'd to the *previous* inode (now unlinked but live in memory),
    // so its `writer.generation()` reads stay constant — the existing
    // dispatch-time generation-mismatch check never fires for swap-based
    // reloads.
    //
    // To learn about the swap, periodically `stat(canonical_path)` and
    // compare the inode against a cached baseline. On change, fire the
    // existing `rebuild_trigger` Notify, which the auto-rebuild task
    // above already handles end-to-end (open_for_actions on the new
    // inode, swap into ShmDispatch via set_writer).
    {
        use std::os::unix::fs::MetadataExt;
        const WATCH_INTERVAL: Duration = Duration::from_secs(5);

        let watch_notify = state.shm_dispatch.rebuild_trigger();
        let watch_path = shm_config.path().to_path_buf();
        let watch_token = shutdown_token.clone();

        tokio::spawn(async move {
            // Baseline: capture initial inode (None if canonical does not
            // yet exist — comsrv may not have created it yet). We only
            // fire on a *change* from a known-good value to avoid a
            // spurious rebuild during cold boot.
            let mut last_inode = std::fs::metadata(&watch_path).ok().map(|m| m.ino());
            if let Some(ino) = last_inode {
                info!(
                    "SHM inode watcher: baseline inode={} for {:?}",
                    ino, watch_path
                );
            } else {
                info!(
                    "SHM inode watcher: canonical path {:?} not yet present; will start tracking once it appears",
                    watch_path
                );
            }

            loop {
                tokio::select! {
                    _ = tokio::time::sleep(WATCH_INTERVAL) => {},
                    _ = watch_token.cancelled() => break,
                }

                let current_inode = match std::fs::metadata(&watch_path) {
                    Ok(m) => Some(m.ino()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                    Err(e) => {
                        warn!(
                            "SHM inode watcher: stat {:?} failed (non-NotFound): {}",
                            watch_path, e
                        );
                        continue;
                    },
                };

                match (last_inode, current_inode) {
                    (Some(old), Some(new)) if old != new => {
                        info!(
                            "SHM inode watcher: canonical path inode changed {} → {} \
                             (comsrv likely performed an atomic-swap reload); \
                             triggering writer rebuild",
                            old, new
                        );
                        last_inode = Some(new);
                        watch_notify.notify_one();
                    },
                    (None, Some(new)) => {
                        info!(
                            "SHM inode watcher: canonical path now exists (inode={}); \
                             tracking baseline",
                            new
                        );
                        last_inode = Some(new);
                    },
                    (Some(_), None) => {
                        warn!(
                            "SHM inode watcher: canonical path {:?} disappeared — \
                             comsrv may be mid-restart; keeping prior baseline",
                            watch_path
                        );
                    },
                    _ => {}, // no change
                }
            }
        });
    }

    // Spawn channel health refresh task: mirrors `comsrv:online` into a
    // process-local cache so the M2C control gate in execute_action() can reject
    // writes targeting offline channels without per-call Redis round-trips.
    {
        let cache = Arc::clone(state.instance_manager.channel_health());
        let rtdb_for_health = Arc::clone(&rtdb);
        let online_key = voltage_model::KeySpaceConfig::production_cached().channel_online_key();
        modsrv::infra::channel_health::spawn_refresh_task(
            cache,
            rtdb_for_health,
            online_key,
            modsrv::infra::channel_health::DEFAULT_REFRESH_INTERVAL,
            shutdown_token.clone(),
        );
    }

    // Load max_concurrency from global config (SQLite key-value table)
    let max_concurrency: usize = sqlx::query_scalar::<_, String>(
        "SELECT value FROM service_config WHERE service_name = 'global' AND key = 'rules.max_concurrency'",
    )
    .fetch_optional(&sqlite_pool)
    .await
    .ok()
    .flatten()
    .and_then(|s| s.parse().ok())
    .unwrap_or(4);

    // ── PointWatch bootstrap (modsrv side) ──────────────────────────────────────
    // Only active when SHM is available (shared_reader.is_some()).
    //
    // 1. Open the SubscriptionBitmap created by comsrv (modsrv writes bits,
    //    comsrv reads them in the hot path).
    // 2. Create a PointWatchListener UDS server that receives PointWatchEvents
    //    from comsrv's drain task.
    // 3. Create a PointWatchDispatcher (subscription index + WatchEvent forwarder).
    // 4. Wire the WatchEvent receiver into RuleScheduler via set_watch_receiver.
    // 5. After rules load, call rebuild_point_watch to populate the subscription index.
    //
    // Graceful degradation: any failure disables the event-driven path; modsrv
    // still works via the 100 ms tick fallback.
    // PointWatch bootstrap result: all four values are None if SHM is unavailable
    // or bitmap open fails (graceful degradation).
    //
    // Returned values:
    //   pw_bitmap    — mmap'd subscription bitmap (modsrv sets bits, comsrv reads)
    //   pw_dispatcher — subscription index; call rebuild_point_watch after load_rules
    //   pw_event_rx  — raw PointWatchEvent channel from PointWatchListener
    //   pw_watch_rx  — WatchEvent channel wired into RuleScheduler
    //
    // After load_rules: call rebuild_point_watch on pw_dispatcher, then spawn the
    // bridge task that reads pw_event_rx and calls dispatcher.dispatch() → pw_watch_rx.
    type PwInitResult = (
        Option<Arc<SubscriptionBitmap>>,
        Option<PointWatchDispatcher>,
        Option<tokio::sync::mpsc::Receiver<voltage_rtdb_shm::PointWatchEvent>>,
        Option<tokio::sync::mpsc::Receiver<WatchEvent>>,
    );
    let (pw_bitmap, pw_dispatcher, pw_event_rx, pw_watch_rx): PwInitResult =
        if shared_reader.is_some() {
            let bitmap_path = bitmap_path_from_shm(shm_config.path());
            match SubscriptionBitmap::open(&bitmap_path) {
                Ok(bitmap) => {
                    let bitmap = Arc::new(bitmap);

                    // Listener shutdown via watch channel (matches ShmCommandListener pattern).
                    let (pw_shutdown_tx, pw_shutdown_rx) = tokio::sync::watch::channel(false);

                    // event_rx: raw PointWatchEvents forwarded from the UDS socket.
                    let (listener, event_rx) = PointWatchListener::new(None, pw_shutdown_rx);

                    // Spawn the UDS listener run loop (accepts comsrv connection).
                    let shutdown_token_for_pw = shutdown_token.clone();
                    let pw_shutdown_tx = Arc::new(pw_shutdown_tx);
                    let pw_sd = Arc::clone(&pw_shutdown_tx);
                    tokio::spawn(async move {
                        tokio::select! {
                            result = listener.run() => {
                                if let Err(e) = result {
                                    warn!("PointWatchListener exited with error: {}", e);
                                }
                            }
                            _ = shutdown_token_for_pw.cancelled() => {
                                let _ = pw_sd.send(true);
                            }
                        }
                    });
                    info!(
                        "PointWatchListener bound ({})",
                        voltage_rtdb_shm::POINT_WATCH_UDS_PATH
                    );

                    // Create dispatcher (empty sub index until rebuild_point_watch).
                    // watch_rx flows to RuleScheduler; dispatcher.dispatch() sends onto it.
                    let (dispatcher, watch_rx) = PointWatchDispatcher::new();

                    (
                        Some(bitmap),
                        Some(dispatcher),
                        Some(event_rx),
                        Some(watch_rx),
                    )
                },
                Err(e) => {
                    warn!(
                        "PointWatch disabled (bitmap open failed — is comsrv running?): {}",
                        e
                    );
                    (None, None, None, None)
                },
            }
        } else {
            debug!("PointWatch skipped (SHM not available)");
            (None, None, None, None)
        };

    // Create rule scheduler with two-tier priority (SharedMemory > Redis)
    // SHM writer enables M2C actions via shared memory (primary path)
    // ShmNotifier enables UDS event notification for immediate dispatch
    // RtdbStateStore ensures stateful functions (period_delta, integrate, etc.) persist across restarts
    let rule_log_root = PathBuf::from("logs/modsrv");
    let state_store = Arc::new(RtdbStateStore::new(Arc::clone(&rtdb)));
    let mut scheduler = RuleScheduler::with_state_store(
        rtdb,
        routing_cache,
        sqlite_pool.clone(),
        tick_ms,
        rule_log_root,
        state_store,
        shared_reader,
        // Share the SAME ShmDispatch instance as modsrv's HTTP control path.
        // generation checks + rebuild signals now apply uniformly to both
        // rule-engine actions and HTTP control writes.
        Some(Arc::clone(&state.shm_dispatch) as Arc<dyn voltage_rtdb_shm::ActionDispatch>),
    );
    scheduler.set_max_concurrency(max_concurrency);

    // Wire PointWatch event receiver into the scheduler (before Arc::new).
    // When present, RuleScheduler::start() selects on this channel alongside
    // the 100 ms tick for sub-millisecond OnChange rule dispatch.
    if let Some(watch_rx) = pw_watch_rx {
        scheduler.set_watch_receiver(watch_rx);
        info!("PointWatch watch_rx wired into RuleScheduler");
    }

    // Wrap dispatcher in Arc<Mutex<>> so the bridge task and the scheduler's
    // reload_rules path can share it. std::sync::Mutex (not tokio::sync) since
    // dispatch() and rebuild_from_rules() never .await inside the critical
    // section — async overhead would only add cost on the hot path.
    let pw_dispatcher_arc = pw_dispatcher.map(|d| Arc::new(std::sync::Mutex::new(d)));

    // Build ChannelToSlotIndex once and Arc-share with both the initial rebuild
    // (below) and the scheduler's reload path (rebuilds on POST /api/scheduler/reload).
    let pw_channel_slot_index_arc = shm_action_writer
        .as_ref()
        .map(|w| Arc::new(voltage_rtdb_shm::ChannelToSlotIndex::from_unified_writer(w)));

    // Wire rebuild handles into scheduler so reload_rules can refresh the
    // SubscriptionBitmap + dispatcher index without a service restart.
    if let (Some(disp_arc), Some(slot_idx_arc), Some(bitmap)) = (
        pw_dispatcher_arc.as_ref(),
        pw_channel_slot_index_arc.as_ref(),
        pw_bitmap.as_ref(),
    ) {
        scheduler.set_point_watch_rebuild_handles(
            Arc::clone(disp_arc),
            Arc::clone(state.instance_manager.routing_cache()),
            Arc::clone(slot_idx_arc),
            Arc::clone(bitmap),
        );
        info!("PointWatch rebuild handles wired into RuleScheduler");
    }

    let scheduler = Arc::new(scheduler);

    info!(
        "Rule scheduler: tick_ms={}, max_concurrency={}",
        tick_ms, max_concurrency
    );

    // Load rules + initial PointWatch subscription rebuild (if handles wired
    // above). reload_rules calls load_rules internally and then rebuilds the
    // SubscriptionBitmap + dispatcher index in a single lock-correct path,
    // so this also doubles as the initial PointWatch bootstrap.
    match scheduler.reload_rules().await {
        Ok(count) => info!("Rule Engine: loaded {} rules", count),
        Err(e) => warn!("Rule Engine: failed to load rules: {}", e),
    }

    // Spawn the PointWatch bridge task if PointWatch is enabled. The
    // subscription index has already been built by reload_rules above; this
    // task just routes raw UDS events → dispatcher.dispatch() → watch_rx.
    if let (Some(dispatcher_arc), Some(mut event_rx)) = (pw_dispatcher_arc, pw_event_rx) {
        // Spawn the bridge task: drains raw PointWatchEvents from the listener
        // and calls dispatcher.dispatch() which sends WatchEvents onto the
        // channel that RuleScheduler reads via set_watch_receiver.
        //
        // dispatcher_arc is shared with scheduler.reload_rules — the lock is
        // held briefly (just for the try_send hash lookup, no .await inside).
        let dispatcher_for_bridge = Arc::clone(&dispatcher_arc);
        let shutdown_token_bridge = shutdown_token.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    ev = event_rx.recv() => {
                        match ev {
                            Some(e) => {
                                // Recover from mutex poison: prior panic in another
                                // thread doesn't invalidate the dispatcher state.
                                let d = dispatcher_for_bridge
                                    .lock()
                                    .unwrap_or_else(|p| p.into_inner());
                                d.dispatch(&e);
                            }
                            None => break, // listener channel closed
                        }
                    }
                    _ = shutdown_token_bridge.cancelled() => break,
                }
            }
            debug!("PointWatch bridge task stopped");
        });
        info!("PointWatch bridge task spawned");
    }

    // Create rule engine state and routes
    let rule_state = Arc::new(RuleEngineState::new(sqlite_pool, Arc::clone(&scheduler)));
    let rule_routes = create_rule_routes(rule_state);

    // Merge rule routes into the main app (both on port 6002)
    let app = app.merge(rule_routes);

    // Start HTTP service (model API + rule engine - port 6002)
    let addr = SocketAddr::from(([0, 0, 0, 0], state.config.api.port));

    // Create socket for unified API (port 6002)
    let socket = tokio::net::TcpSocket::new_v4()?;
    socket.set_reuseaddr(true)?;
    socket.bind(addr)?;
    let listener = socket.listen(1024)?;

    info!("Model Service (with Rule Engine) started on {}", addr);
    info!("");
    info!("Model API endpoints (port {}):", state.config.api.port);
    info!("  GET /health - Health check");
    info!("  GET/POST /api/instances - Instance management");
    info!("  GET /api/products - Product management");
    info!("  GET /api/instances/:id/data - Get instance data");
    info!("  POST /api/instances/:id/sync - Sync measurement");
    info!("  POST /api/instances/:id/action - Execute action");
    info!("  POST /api/instances/sync/all - Sync all instances");
    info!("");
    info!(
        "Rule Engine API endpoints (port {}):",
        state.config.api.port
    );
    info!("  GET/POST /api/rules - Rule management");
    info!("  GET/PUT/DELETE /api/rules/:id - Single rule operations");
    info!("  POST /api/rules/:id/execute - Execute rule manually");
    info!("  GET /api/scheduler/status - Scheduler status");
    info!("  POST /api/scheduler/reload - Reload rules");

    // Prepare graceful shutdown
    let cancel_token = shutdown_token.clone();
    let shutdown_signal = async move {
        cancel_token.cancelled().await;
        info!("Shutdown signal received, stopping service...");
    };

    // Spawn server task
    let server_task = async move {
        if let Err(e) = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal)
            .await
        {
            error!("Server error: {}", e);
        }
    };

    // Spawn server task
    let server_handle = tokio::spawn(server_task);
    info!("Server started (port {})", state.config.api.port);

    // Start rule scheduler in background
    let scheduler_handle = {
        let scheduler = Arc::clone(&scheduler);
        tokio::spawn(async move {
            scheduler.start().await;
        })
    };
    info!("Rule scheduler started");

    // Wait for shutdown signal (Ctrl+C or SIGTERM)
    common::shutdown::wait_for_shutdown().await;
    info!("Initiating graceful shutdown...");

    // Signal all tasks to shutdown
    shutdown_token.cancel();

    // Stop scheduler
    scheduler.stop();

    // Wait for tasks to complete with timeout
    let shutdown_timeout = tokio::time::Duration::from_secs(30);

    // Wait for server task
    match tokio::time::timeout(shutdown_timeout, server_handle).await {
        Ok(Ok(())) => info!("Server shut down gracefully"),
        Ok(Err(e)) => error!("Server task failed: {}", e),
        Err(_) => {
            error!("Server shutdown timed out");
        },
    }

    // Wait for scheduler to stop
    match tokio::time::timeout(shutdown_timeout, scheduler_handle).await {
        Ok(Ok(())) => info!("Scheduler shut down gracefully"),
        Ok(Err(e)) => error!("Scheduler task failed: {}", e),
        Err(_) => {
            error!("Scheduler shutdown timed out");
        },
    }

    info!("Model Service (with Rule Engine) shutdown complete");
    Ok(())
}
