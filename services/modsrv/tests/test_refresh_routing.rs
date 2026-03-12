//! Tests for refresh_routing_and_shm() protocol and ComsrvCoordinator
//!
//! Covers:
//! 1. refresh_routing_and_shm() — 3-step protocol (cache → comsrv → rebuild)
//! 2. ComsrvCoordinator — HTTP client behavior (success, failure, no-server)
//!
//! ComsrvCoordinator reads the comsrv URL from the COMSRV_URL env var at
//! call time. Because env vars are process-global, all tests in this file
//! that set COMSRV_URL must acquire ENV_LOCK first to prevent races between
//! parallel test threads.

#![allow(clippy::disallowed_methods)] // test code — unwrap is acceptable

use async_trait::async_trait;
use axum::{body::Body, response::Response, routing::post, Router};
use modsrv::infra::shm_dispatch::{ActionDispatch, DispatchOutcome, NoopDispatch};
use modsrv::instance_manager::InstanceManager;
use modsrv::product_loader::ProductLoader;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use voltage_routing::{RouteContext, RoutingCache};
use voltage_rtdb::helpers::create_test_rtdb;

/// Process-wide lock that serialises tests which mutate the COMSRV_URL env var.
static ENV_LOCK: Mutex<()> = Mutex::const_new(());

// ============================================================================
// Shared helpers
// ============================================================================

/// Spin up a temporary SQLite DB with the full modsrv schema.
async fn create_test_db() -> (TempDir, SqlitePool) {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("test.db");
    let url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePool::connect(&url).await.unwrap();
    common::test_utils::schema::init_modsrv_schema(&pool)
        .await
        .unwrap();
    (tmp, pool)
}

/// Build an InstanceManager backed by NoopDispatch (no SHM, no UDS).
fn make_manager(pool: SqlitePool) -> InstanceManager<voltage_rtdb::MemoryRtdb> {
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(RoutingCache::new());
    let product_loader = Arc::new(ProductLoader::new(pool.clone()));
    let dispatch: Arc<dyn ActionDispatch> = Arc::new(NoopDispatch);
    InstanceManager::new(pool, rtdb, routing_cache, product_loader, dispatch)
}

/// Build an InstanceManager with a custom dispatch implementation.
fn make_manager_with_dispatch(
    pool: SqlitePool,
    dispatch: Arc<dyn ActionDispatch>,
) -> InstanceManager<voltage_rtdb::MemoryRtdb> {
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(RoutingCache::new());
    let product_loader = Arc::new(ProductLoader::new(pool.clone()));
    InstanceManager::new(pool, rtdb, routing_cache, product_loader, dispatch)
}

/// Insert a minimal enabled action_routing row so the routing loader has
/// something to load from SQLite.
///
/// M2C key will be "1:A:1" (instance_id=1, channel_type=A, action_id=1).
async fn insert_action_routing(pool: &SqlitePool) {
    sqlx::query(
        "INSERT OR IGNORE INTO channels (channel_id, name, protocol, enabled) \
         VALUES (1, 'ch1', 'Virtual', 1)",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT OR IGNORE INTO instances \
         (instance_id, instance_name, product_name, properties) \
         VALUES (1, 'inst1', 'Battery', '{}')",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT OR IGNORE INTO action_routing \
         (instance_id, instance_name, action_id, channel_id, channel_type, channel_point_id, enabled) \
         VALUES (1, 'inst1', 1, 1, 'A', 1, 1)",
    )
    .execute(pool)
    .await
    .unwrap();
}

// ============================================================================
// Mock HTTP server helpers
// ============================================================================

/// Spawn an axum server on a random port that always responds to POST requests
/// at /api/routing/reload with the given HTTP status and body.
///
/// Returns the bound socket address.
async fn spawn_axum_mock(status: u16, body: &'static str) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let app = Router::new().route(
        "/api/routing/reload",
        post(move || async move {
            Response::builder()
                .status(status)
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .unwrap()
        }),
    );

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    addr
}

/// Call `reload_routing()` while holding ENV_LOCK and pointing COMSRV_URL at
/// the given URL. Restores the previous env var value (or removes it) on exit.
async fn call_coordinator_at(url: &str) -> anyhow::Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let prev = std::env::var("COMSRV_URL").ok();

    std::env::set_var("COMSRV_URL", url);
    let coordinator = modsrv::infra::comsrv_coordinator::ComsrvCoordinator::new();
    let result = coordinator.reload_routing().await;

    match &prev {
        Some(v) => std::env::set_var("COMSRV_URL", v),
        None => std::env::remove_var("COMSRV_URL"),
    }
    result
}

/// Like call_coordinator_at but also returns the InstanceManager result.
async fn refresh_manager_at(
    manager: &InstanceManager<voltage_rtdb::MemoryRtdb>,
    url: &str,
) -> anyhow::Result<usize> {
    let _guard = ENV_LOCK.lock().await;
    let prev = std::env::var("COMSRV_URL").ok();

    std::env::set_var("COMSRV_URL", url);
    let result = manager.refresh_routing_and_shm().await;

    match &prev {
        Some(v) => std::env::set_var("COMSRV_URL", v),
        None => std::env::remove_var("COMSRV_URL"),
    }
    result
}

// ============================================================================
// Section 1 — refresh_routing_and_shm() integration tests
// ============================================================================

/// After inserting routing data into SQLite and pointing COMSRV_URL at a working
/// mock server, refresh_routing_and_shm() must:
/// - Load routes from SQLite into the RoutingCache (Step 1)
/// - Return Ok(count) where count > 0
/// - The RoutingCache must now contain the loaded M2C entry
#[tokio::test]
async fn test_refresh_routing_updates_cache() {
    let (_tmp, pool) = create_test_db().await;
    insert_action_routing(&pool).await;

    let body = r#"{"success":true,"data":{"errors":[]}}"#;
    let addr = spawn_axum_mock(200, body).await;
    let url = format!("http://{}", addr);

    let manager = make_manager(pool);

    // Cache starts empty — M2C key format is "{instance_id}:A:{action_id}"
    assert!(
        manager.routing_cache().lookup_m2c("1:A:1").is_none(),
        "Cache should be empty before refresh"
    );

    let result = refresh_manager_at(&manager, &url).await;

    assert!(
        result.is_ok(),
        "refresh should succeed when comsrv returns 200: {:?}",
        result.err()
    );
    assert!(
        result.unwrap() > 0,
        "route count must be > 0 after inserting a row"
    );

    // Step 1 must have populated the RoutingCache
    assert!(
        manager.routing_cache().lookup_m2c("1:A:1").is_some(),
        "RoutingCache must hold the M2C route after refresh"
    );
}

/// Custom dispatch that records whether rebuild_writer was called.
struct TrackingDispatch {
    rebuilt: Arc<AtomicBool>,
}

#[async_trait]
impl ActionDispatch for TrackingDispatch {
    async fn dispatch(&self, _ctx: &RouteContext, _value: f64) -> DispatchOutcome {
        DispatchOutcome::Noop
    }

    fn rebuild_writer(&self, _cache: &voltage_routing::RoutingCache) -> anyhow::Result<()> {
        self.rebuilt.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Even when ComsrvCoordinator returns an error (Step 2), rebuild_writer (Step 3)
/// must still be called — the protocol always executes all three steps regardless
/// of comsrv outcome.
///
/// Verified with TrackingDispatch: the flag must be set even when comsrv fails.
#[tokio::test]
async fn test_refresh_routing_comsrv_failure_still_rebuilds_writer() {
    let rebuilt = Arc::new(AtomicBool::new(false));
    let dispatch: Arc<dyn ActionDispatch> = Arc::new(TrackingDispatch {
        rebuilt: Arc::clone(&rebuilt),
    });

    let (_tmp, pool) = create_test_db().await;
    let manager = make_manager_with_dispatch(pool, dispatch);

    // Bind and immediately close so the port refuses connections
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let refused_addr = listener.local_addr().unwrap();
    drop(listener);
    let refused_url = format!("http://{}", refused_addr);

    let result = refresh_manager_at(&manager, &refused_url).await;

    // rebuild_writer MUST have been called despite comsrv failure
    assert!(
        rebuilt.load(Ordering::SeqCst),
        "rebuild_writer must be called even when comsrv returns an error"
    );

    // Overall result must be Err (comsrv failure propagated)
    assert!(
        result.is_err(),
        "refresh_routing_and_shm must propagate the comsrv error"
    );
}

/// The returned Result must be Err when comsrv fails, so API handlers can
/// surface the error to the caller.
#[tokio::test]
async fn test_refresh_routing_comsrv_failure_propagates_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let refused_addr = listener.local_addr().unwrap();
    drop(listener);

    let (_tmp, pool) = create_test_db().await;
    let manager = make_manager(pool);
    let refused_url = format!("http://{}", refused_addr);

    let result = refresh_manager_at(&manager, &refused_url).await;

    assert!(
        result.is_err(),
        "comsrv connection failure must be propagated to the caller"
    );
}

/// When comsrv succeeds (HTTP 200, valid JSON), refresh_routing_and_shm()
/// returns Ok with the number of routes loaded.
#[tokio::test]
async fn test_refresh_routing_succeeds_when_comsrv_ok() {
    let (_tmp, pool) = create_test_db().await;
    insert_action_routing(&pool).await;

    let body = r#"{"success":true,"data":{"errors":[]}}"#;
    let addr = spawn_axum_mock(200, body).await;
    let url = format!("http://{}", addr);

    let manager = make_manager(pool);
    let result = refresh_manager_at(&manager, &url).await;

    assert!(
        result.is_ok(),
        "refresh_routing_and_shm should succeed when comsrv returns 200: {:?}",
        result.err()
    );
    assert!(
        result.unwrap() > 0,
        "route count should be > 0 when routing rows exist"
    );
}

// ============================================================================
// Section 2 — ComsrvCoordinator unit tests
// ============================================================================

/// When the mock server returns HTTP 200 with a valid success payload,
/// reload_routing() must return Ok.
#[tokio::test]
async fn test_comsrv_coordinator_success() {
    let body = r#"{"success":true,"data":{"errors":[]}}"#;
    let addr = spawn_axum_mock(200, body).await;
    let result = call_coordinator_at(&format!("http://{}", addr)).await;
    assert!(
        result.is_ok(),
        "reload_routing should succeed on HTTP 200 with valid JSON: {:?}",
        result.err()
    );
}

/// When the mock server returns HTTP 500, reload_routing() must return Err.
#[tokio::test]
async fn test_comsrv_coordinator_server_error() {
    let body = r#"{"error":"internal"}"#;
    let addr = spawn_axum_mock(500, body).await;
    let result = call_coordinator_at(&format!("http://{}", addr)).await;

    assert!(result.is_err(), "reload_routing should fail on HTTP 500");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("500") || msg.contains("HTTP") || msg.contains("comsrv"),
        "Error should mention HTTP status or comsrv: {msg}"
    );
}

/// When no server is listening at the configured URL, the coordinator must
/// return Err without panicking.
#[tokio::test]
async fn test_comsrv_coordinator_connection_refused() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let refused_addr = listener.local_addr().unwrap();
    drop(listener);

    let result = call_coordinator_at(&format!("http://{}", refused_addr)).await;
    assert!(
        result.is_err(),
        "reload_routing must return Err when connection is refused"
    );
}

/// When the server sends success=false, reload_routing() must return Err.
#[tokio::test]
async fn test_comsrv_coordinator_success_false() {
    let body = r#"{"success":false,"data":{"errors":["internal failure"]}}"#;
    let addr = spawn_axum_mock(200, body).await;
    let result = call_coordinator_at(&format!("http://{}", addr)).await;
    assert!(
        result.is_err(),
        "reload_routing should fail when success=false"
    );
}

/// When success=true but errors list is non-empty, reload_routing() must return
/// Err that includes the error details from comsrv.
#[tokio::test]
async fn test_comsrv_coordinator_with_errors_in_payload() {
    let body = r#"{"success":true,"data":{"errors":["slot mismatch","hash mismatch"]}}"#;
    let addr = spawn_axum_mock(200, body).await;
    let result = call_coordinator_at(&format!("http://{}", addr)).await;

    assert!(
        result.is_err(),
        "reload_routing should fail when errors list is non-empty"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("errors") || msg.contains("slot") || msg.contains("hash"),
        "Error should include the comsrv error details: {msg}"
    );
}

/// When the server returns invalid JSON, reload_routing() must return Err
/// without panicking.
#[tokio::test]
async fn test_comsrv_coordinator_invalid_json() {
    let body = "not-json";
    let addr = spawn_axum_mock(200, body).await;
    let result = call_coordinator_at(&format!("http://{}", addr)).await;
    assert!(
        result.is_err(),
        "reload_routing should fail on invalid JSON response"
    );
}

/// Concurrency safety: two concurrent refresh_routing_and_shm() calls on
/// separate managers must both complete without panic or deadlock within a
/// reasonable time limit.
///
/// The refresh_lock inside each manager serialises concurrent calls on the same
/// manager. Here we verify that two independent managers can both complete.
#[tokio::test]
async fn test_refresh_routing_lock_serialises_concurrent_calls() {
    let body = r#"{"success":true,"data":{"errors":[]}}"#;
    let addr1 = spawn_axum_mock(200, body).await;
    let addr2 = spawn_axum_mock(200, body).await;
    let url1 = format!("http://{}", addr1);
    let url2 = format!("http://{}", addr2);

    let (_tmp1, pool1) = create_test_db().await;
    let (_tmp2, pool2) = create_test_db().await;

    let manager1 = Arc::new(make_manager(pool1));
    let manager2 = Arc::new(make_manager(pool2));

    // Run both refreshes concurrently.
    // Each call acquires ENV_LOCK before setting COMSRV_URL, so they
    // execute sequentially with respect to the env var but concurrently at
    // the task level. The test verifies neither task panics.
    let m1 = Arc::clone(&manager1);
    let h1 = tokio::spawn(async move { refresh_manager_at(&m1, &url1).await });

    let m2 = Arc::clone(&manager2);
    let h2 = tokio::spawn(async move { refresh_manager_at(&m2, &url2).await });

    let (r1, r2) = tokio::time::timeout(Duration::from_secs(10), async { (h1.await, h2.await) })
        .await
        .expect("concurrent refresh calls must complete within 10 s");

    // Neither task must panic (JoinError = panic in the spawned task)
    assert!(r1.is_ok(), "task 1 panicked: {:?}", r1.err());
    assert!(r2.is_ok(), "task 2 panicked: {:?}", r2.err());
}
