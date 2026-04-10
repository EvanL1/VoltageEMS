//! Structural invariants for instance data storage.
//!
//! Every `inst:{id}:M` hash must have a matching `inst:{id}:M:ts` sidecar
//! whose field set is **identical** to the value hash's. Same for `inst:{id}:A`.
//!
//! This invariant exists because the apigateway WebSocket pulls timestamps
//! from the `:ts` sidecar for `source='inst'` subscriptions, and an empty
//! sidecar surfaces to the frontend as `ts: {}`. The bug class "new write
//! path forgets to populate the ts sidecar" is a recurring risk because
//! nothing in the type system enforces the pairing — every author has to
//! remember it by convention.
//!
//! Each test exercises one or more production write paths and then calls
//! `assert_instance_value_ts_invariant` to check the structural relationship.
//! A final `#[should_panic]` test verifies that the invariant helper itself
//! actually catches missing sidecar writes (guards against "the test harness
//! silently no-ops").

#![allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use voltage_model::PointType;
use voltage_routing::{set_action_point, write_channel_batch, ChannelPointUpdate, RoutingCache};
use voltage_rtdb::{KeySpaceConfig, MemoryRtdb, Rtdb};

// ============================================================================
// Helpers
// ============================================================================

fn create_test_rtdb() -> Arc<MemoryRtdb> {
    Arc::new(MemoryRtdb::new())
}

/// Build a RoutingCache with optional C2M and M2C routes.
///
/// Format: `("1001:T:1", "23:M:1")` for C2M; `("42:A:3", "2001:C:10")` for M2C.
fn setup_routing(c2m: Vec<(&str, &str)>, m2c: Vec<(&str, &str)>) -> Arc<RoutingCache> {
    let c2m_map: HashMap<String, String> = c2m
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let m2c_map: HashMap<String, String> = m2c
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Arc::new(RoutingCache::from_maps(c2m_map, m2c_map, HashMap::new()))
}

/// The core invariant.
///
/// For every instance id in `instance_ids`, verify that:
///   field_set(inst:{id}:M)    == field_set(inst:{id}:M:ts)
///   field_set(inst:{id}:A)    == field_set(inst:{id}:A:ts)
///
/// Panics with a descriptive message on the first violation.
async fn assert_instance_value_ts_invariant<R: Rtdb>(rtdb: &R, instance_ids: &[u32]) {
    let config = KeySpaceConfig::production();
    for &id in instance_ids {
        for (val_key, ts_key, label) in [
            (
                config.instance_measurement_key(id),
                config.instance_measurement_ts_key(id),
                "M",
            ),
            (
                config.instance_action_key(id),
                config.instance_action_ts_key(id),
                "A",
            ),
        ] {
            let val_fields: BTreeSet<String> = rtdb
                .hash_get_all(&val_key)
                .await
                .expect("hash_get_all(value) must succeed")
                .into_keys()
                .collect();
            let ts_fields: BTreeSet<String> = rtdb
                .hash_get_all(&ts_key)
                .await
                .expect("hash_get_all(ts) must succeed")
                .into_keys()
                .collect();

            assert_eq!(
                val_fields, ts_fields,
                "invariant broken on inst:{}:{}: \n  \
                 value hash fields = {:?}\n  \
                 ts hash fields    = {:?}\n  \
                 (every value write must have a companion ts write)",
                id, label, val_fields, ts_fields,
            );
        }
    }
}

// ============================================================================
// Positive tests: every production write path must preserve the invariant
// ============================================================================

/// C2M batch path (voltage-routing/batch.rs::write_channel_batch).
///
/// Exercises the same logic used at channel initialization time in
/// comsrv/src/core/channels/channel_creation.rs, as well as structurally
/// equivalent logic in comsrv/src/store/shm_redis_sync.rs C2M hot path.
#[tokio::test]
async fn invariant_holds_after_c2m_batch_write() {
    let rtdb = create_test_rtdb();
    let cache = setup_routing(
        vec![
            ("1001:T:1", "23:M:1"),
            ("1001:T:2", "23:M:2"),
            ("1002:T:5", "42:M:5"),
        ],
        vec![],
    );

    let updates = vec![
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 1,
            value: 230.5,
            raw_value: None,
            cascade_depth: 0,
        },
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 2,
            value: 50.0,
            raw_value: None,
            cascade_depth: 0,
        },
        ChannelPointUpdate {
            channel_id: 1002,
            point_type: PointType::Telemetry,
            point_id: 5,
            value: 99.9,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    write_channel_batch(rtdb.as_ref(), &cache, updates)
        .await
        .expect("write_channel_batch should succeed");

    assert_instance_value_ts_invariant(rtdb.as_ref(), &[23, 42]).await;
}

/// M2C routed branch (voltage-routing/src/lib.rs::set_action_point with an M2C target).
///
/// Writes to both `inst:{id}:A` and downstream `comsrv:{ch}:{C|...}`. The
/// instance-side ts sidecar must be populated from the same `timestamp_ms`
/// that the channel side uses.
#[tokio::test]
async fn invariant_holds_after_set_action_point_routed() {
    let rtdb = create_test_rtdb();
    let cache = setup_routing(vec![], vec![("42:A:3", "2001:C:10")]);

    set_action_point(rtdb.as_ref(), &cache, 42, "3", 99.0)
        .await
        .expect("set_action_point should succeed");

    assert_instance_value_ts_invariant(rtdb.as_ref(), &[42]).await;
}

/// M2C no-route branch.
///
/// When there is no M2C target for the instance point, the action is still
/// written to `inst:{id}:A` as local state — the sidecar ts must follow.
/// This branch was easy to miss in a naive fix because routed/no-route are
/// two distinct code paths.
#[tokio::test]
async fn invariant_holds_after_set_action_point_no_route() {
    let rtdb = create_test_rtdb();
    let cache = Arc::new(RoutingCache::new()); // empty cache → no-route branch

    set_action_point(rtdb.as_ref(), &cache, 100, "7", 42.0)
        .await
        .expect("set_action_point no-route should succeed");

    assert_instance_value_ts_invariant(rtdb.as_ref(), &[100]).await;
}

/// Mixed-path test: all three write paths interleaved within one rtdb.
///
/// Any regression in any single path would be caught here even if the
/// single-path tests above are removed or refactored. This is the
/// "catch-all" net.
#[tokio::test]
async fn invariant_holds_after_mixed_write_paths() {
    let rtdb = create_test_rtdb();
    let cache = setup_routing(vec![("1001:T:1", "23:M:1")], vec![("42:A:3", "2001:C:10")]);

    // Path 1: C2M batch → inst:23
    write_channel_batch(
        rtdb.as_ref(),
        &cache,
        vec![ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 1,
            value: 230.5,
            raw_value: None,
            cascade_depth: 0,
        }],
    )
    .await
    .expect("write_channel_batch should succeed");

    // Path 2: M2C routed action → inst:42
    set_action_point(rtdb.as_ref(), &cache, 42, "3", 99.0)
        .await
        .expect("routed set_action_point should succeed");

    // Path 3: M2C no-route action → inst:100
    set_action_point(rtdb.as_ref(), &cache, 100, "7", 42.0)
        .await
        .expect("no-route set_action_point should succeed");

    assert_instance_value_ts_invariant(rtdb.as_ref(), &[23, 42, 100]).await;
}

// ============================================================================
// Negative test: the helper must panic when the invariant is violated,
// otherwise it's a silent rubber stamp.
// ============================================================================

/// Verify the invariant helper actually detects the bug class it's meant
/// to catch. Simulates a regression by writing the value hash directly
/// without touching the ts sidecar — `assert_instance_value_ts_invariant`
/// must panic with "invariant broken".
#[tokio::test]
#[should_panic(expected = "invariant broken")]
async fn invariant_helper_detects_missing_ts_sidecar() {
    use bytes::Bytes;

    let rtdb = create_test_rtdb();
    let config = KeySpaceConfig::production();

    // Simulate a future regression: someone adds a new write path that
    // populates the value hash but forgets the ts sidecar.
    rtdb.hash_set(
        &config.instance_measurement_key(999),
        "1",
        Bytes::from("42"),
    )
    .await
    .unwrap();

    // This MUST panic — that's what the test is verifying.
    assert_instance_value_ts_invariant(rtdb.as_ref(), &[999]).await;
}
