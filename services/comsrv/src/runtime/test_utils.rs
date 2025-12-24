//! Test utilities for comsrv
//!
//! Provides shared helper functions for unit and integration tests

use std::collections::HashMap;
use std::sync::Arc;
use voltage_rtdb::MemoryRtdb;
use voltage_rtdb::Rtdb; // Trait must be in scope for generic bounds
use voltage_rtdb::{KeySpaceConfig, RoutingCache};

// ==================== Basic Test Infrastructure ====================

// Re-exported from voltage_rtdb::helpers for backward compatibility
pub use voltage_rtdb::helpers::create_test_rtdb;

/// Create a test routing cache for unit testing
///
/// This creates an empty RoutingCache for unit tests.
/// Suitable for tests that don't need specific routing configuration.
///
/// Returns Arc<RoutingCache> which can be used for ChannelManager and routing tests.
pub fn create_test_routing_cache() -> Arc<RoutingCache> {
    Arc::new(RoutingCache::new())
}

// create_test_redis_client() removed - use create_test_rtdb() instead

// ==================== Routing Test Setup Functions ====================

/// Create test environment with C2M routing configuration
///
/// # Arguments
/// * `c2m_routes` - C2M routing mappings: [("1001:T:1", "23:M:1"), ...]
///
/// # Returns
/// * `(Arc<MemoryRtdb>, Arc<RoutingCache>)` - RTDB and routing cache instances
///
/// # Example
/// ```no_run
/// use comsrv::test_utils::*;
///
/// #[tokio::test]
/// async fn test_c2m() {
///     let (rtdb, routing_cache) = setup_c2m_routing(vec![
///         ("1001:T:1", "23:M:1"),
///         ("1001:T:2", "23:M:2"),
///     ]).await;
///     // Use rtdb and routing_cache in tests
/// }
/// ```
pub async fn setup_c2m_routing(
    c2m_routes: Vec<(&str, &str)>,
) -> (Arc<MemoryRtdb>, Arc<RoutingCache>) {
    let rtdb = create_test_rtdb();
    let mut c2m_map = HashMap::new();
    for (source, target) in c2m_routes {
        c2m_map.insert(source.to_string(), target.to_string());
    }
    let routing_cache = Arc::new(RoutingCache::from_maps(
        c2m_map,
        HashMap::new(),
        HashMap::new(),
    ));
    (rtdb, routing_cache)
}

// ==================== Routing Test Assertion Functions ====================

/// Verify channel point value (engineering value layer)
///
/// # Arguments
/// * `rtdb` - RTDB instance
/// * `channel_id` - Channel ID
/// * `point_type` - Point type (T/S/C/A)
/// * `point_id` - Point ID
/// * `expected_value` - Expected value
///
/// # Example
/// ```no_run
/// use comsrv::test_utils::*;
///
/// #[tokio::test]
/// async fn test_channel_value() {
///     let rtdb = create_test_rtdb();
///     // ... write data ...
///     assert_channel_value(&rtdb, 1001, "T", 1, 100.0).await;
/// }
/// ```
#[allow(clippy::disallowed_methods)] // Test utility - unwrap is acceptable for test data conversion
pub async fn assert_channel_value<R: Rtdb>(
    rtdb: &R,
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    expected_value: f64,
) {
    use voltage_model::PointType;

    let config = KeySpaceConfig::production();
    let point_type_enum = PointType::from_str(point_type).unwrap();
    let channel_key = config.channel_key(channel_id, point_type_enum);

    let value = rtdb
        .hash_get(&channel_key, &point_id.to_string())
        .await
        .expect("Failed to read channel value")
        .expect("Channel value should exist");

    let actual_value: f64 = String::from_utf8(value.to_vec()).unwrap().parse().unwrap();

    assert_eq!(
        actual_value, expected_value,
        "Channel {}:{}:{} value mismatch",
        channel_id, point_type, point_id
    );
}

/// Verify instance measurement value
///
/// # Arguments
/// * `rtdb` - RTDB instance
/// * `instance_id` - Instance ID
/// * `point_id` - Point ID
/// * `expected_value` - Expected value
///
/// # Example
/// ```no_run
/// use comsrv::test_utils::*;
///
/// #[tokio::test]
/// async fn test_instance_measurement() {
///     let rtdb = create_test_rtdb();
///     // ... write data ...
///     assert_instance_measurement(&rtdb, 23, 1, 100.0).await;
/// }
/// ```
#[allow(clippy::disallowed_methods)] // Test utility - unwrap is acceptable for test data conversion
pub async fn assert_instance_measurement<R: Rtdb>(
    rtdb: &R,
    instance_id: u32,
    point_id: u32,
    expected_value: f64,
) {
    let config = KeySpaceConfig::production();
    let inst_key = config.instance_measurement_key(instance_id);

    let value = rtdb
        .hash_get(&inst_key, &point_id.to_string())
        .await
        .expect("Failed to read instance measurement")
        .expect("Instance measurement should exist");

    let actual_value: f64 = String::from_utf8(value.to_vec()).unwrap().parse().unwrap();

    assert_eq!(
        actual_value, expected_value,
        "Instance {} measurement point {} value mismatch",
        instance_id, point_id
    );
}
