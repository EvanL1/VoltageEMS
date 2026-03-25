//! Tests for refresh_routing() — local routing cache refresh
//!
//! After the SHM/routing decoupling, refresh_routing() is a pure local operation:
//! load routes from SQLite → update in-memory RoutingCache. No comsrv HTTP call,
//! no SHM rebuild.

#![allow(clippy::disallowed_methods)] // test code — unwrap is acceptable

use async_trait::async_trait;
use modsrv::infra::shm_dispatch::{ActionDispatch, DispatchOutcome, NoopDispatch};
use modsrv::instance_manager::InstanceManager;
use modsrv::product_loader::ProductLoader;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tempfile::TempDir;
use voltage_routing::{RouteContext, RoutingCache};
use voltage_rtdb::helpers::create_test_rtdb;

// ============================================================================
// Shared helpers
// ============================================================================

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

fn make_manager(pool: SqlitePool) -> InstanceManager<voltage_rtdb::MemoryRtdb> {
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(RoutingCache::new());
    let product_loader = Arc::new(ProductLoader::new(pool.clone()));
    let dispatch: Arc<dyn ActionDispatch> = Arc::new(NoopDispatch);
    InstanceManager::new(pool, rtdb, routing_cache, product_loader, dispatch)
}

fn make_manager_with_dispatch(
    pool: SqlitePool,
    dispatch: Arc<dyn ActionDispatch>,
) -> InstanceManager<voltage_rtdb::MemoryRtdb> {
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(RoutingCache::new());
    let product_loader = Arc::new(ProductLoader::new(pool.clone()));
    InstanceManager::new(pool, rtdb, routing_cache, product_loader, dispatch)
}

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
// refresh_routing() tests
// ============================================================================

/// After inserting routing data into SQLite, refresh_routing() must:
/// - Load routes from SQLite into the RoutingCache
/// - Return Ok(count) where count > 0
/// - The RoutingCache must now contain the loaded M2C entry
#[tokio::test]
async fn test_refresh_routing_updates_cache() {
    let (_tmp, pool) = create_test_db().await;
    insert_action_routing(&pool).await;

    let manager = make_manager(pool);

    assert!(
        manager.routing_cache().lookup_m2c("1:A:1").is_none(),
        "Cache should be empty before refresh"
    );

    let result = manager.refresh_routing().await;

    assert!(
        result.is_ok(),
        "refresh_routing should succeed: {:?}",
        result.err()
    );
    assert!(
        result.unwrap() > 0,
        "route count must be > 0 after inserting a row"
    );

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

    fn rebuild_writer(
        &self,
        _channel_points: &voltage_rtdb_shm::ChannelPointCounts,
    ) -> anyhow::Result<()> {
        self.rebuilt.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// refresh_routing() is a local-only operation — it does NOT contact comsrv
/// and does NOT call rebuild_writer.
#[tokio::test]
async fn test_refresh_routing_does_not_call_rebuild_writer() {
    let rebuilt = Arc::new(AtomicBool::new(false));
    let dispatch: Arc<dyn ActionDispatch> = Arc::new(TrackingDispatch {
        rebuilt: Arc::clone(&rebuilt),
    });

    let (_tmp, pool) = create_test_db().await;
    insert_action_routing(&pool).await;
    let manager = make_manager_with_dispatch(pool, dispatch);

    let result = manager.refresh_routing().await;

    assert!(
        result.is_ok(),
        "refresh_routing must succeed: {:?}",
        result.err()
    );

    assert!(
        !rebuilt.load(Ordering::SeqCst),
        "refresh_routing must not call rebuild_writer"
    );
}

/// refresh_routing() succeeds even when no comsrv is reachable.
#[tokio::test]
async fn test_refresh_routing_succeeds_without_comsrv() {
    let (_tmp, pool) = create_test_db().await;
    let manager = make_manager(pool);

    let result = manager.refresh_routing().await;

    assert!(
        result.is_ok(),
        "refresh_routing must succeed without comsrv: {:?}",
        result.err()
    );
}

/// Two concurrent refresh_routing() calls on separate managers must both
/// complete without panic.
#[tokio::test]
async fn test_refresh_routing_concurrent_calls() {
    let (_tmp1, pool1) = create_test_db().await;
    let (_tmp2, pool2) = create_test_db().await;

    let manager1 = Arc::new(make_manager(pool1));
    let manager2 = Arc::new(make_manager(pool2));

    let m1 = Arc::clone(&manager1);
    let h1 = tokio::spawn(async move { m1.refresh_routing().await });

    let m2 = Arc::clone(&manager2);
    let h2 = tokio::spawn(async move { m2.refresh_routing().await });

    let (r1, r2) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        (h1.await, h2.await)
    })
    .await
    .expect("concurrent refresh calls must complete within 10s");

    assert!(r1.is_ok(), "task 1 panicked: {:?}", r1.err());
    assert!(r2.is_ok(), "task 2 panicked: {:?}", r2.err());
}
