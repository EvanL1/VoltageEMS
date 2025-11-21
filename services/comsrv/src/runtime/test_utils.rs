//! Test utilities for comsrv
//!
//! Provides shared helper functions for unit and integration tests

use std::collections::HashMap;
use std::sync::Arc;
use voltage_config::{KeySpaceConfig, RoutingCache};
use voltage_rtdb::Rtdb;

// ==================== Basic Test Infrastructure ====================

/// Create an in-memory RTDB for unit testing
///
/// This creates a MemoryRtdb that doesn't require any external services.
/// Suitable for unit tests that should not depend on Redis.
///
/// Returns Arc<dyn Rtdb> which can be used anywhere Redis would be used.
pub fn create_test_rtdb() -> Arc<dyn Rtdb> {
    Arc::new(voltage_rtdb::MemoryRtdb::new())
}

/// Create a test routing cache for unit testing
///
/// This creates an empty RoutingCache for unit tests.
/// Suitable for tests that don't need specific routing configuration.
///
/// Returns Arc<RoutingCache> which can be used for ChannelManager and routing tests.
pub fn create_test_routing_cache() -> Arc<RoutingCache> {
    Arc::new(RoutingCache::new())
}

/// Create a mock Redis client for testing (DEPRECATED - use create_test_rtdb instead)
///
/// This creates a real Redis client connection for testing purposes.
/// Tests using this function require a Redis server running at localhost:6379.
///
/// **DEPRECATED**: For unit tests, use `create_test_rtdb()` instead, which doesn't
/// require external services. Only use this for integration tests.
#[deprecated(note = "Use create_test_rtdb() for unit tests instead")]
pub async fn create_test_redis_client() -> Arc<common::redis::RedisClient> {
    Arc::new(
        common::redis::RedisClient::new("redis://localhost:6379")
            .await
            .expect("Failed to create test Redis client - ensure Redis is running"),
    )
}

/// Create a mock Redis client synchronously (for non-async test setup)
///
/// Note: This is a workaround for tests that need Arc<RedisClient> in non-async context.
/// The returned client is not actually connected and should not be used for real operations.
/// For actual Redis operations in tests, use `create_test_redis_client()` in async tests.
#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
pub fn create_mock_redis_client_sync() -> Arc<common::redis::RedisClient> {
    // This is a placeholder that will panic if actually used
    // It's only for satisfying type requirements in test setup
    panic!("Mock Redis client should not be used for actual operations. Use create_test_redis_client() in async tests instead.")
}

// ==================== Routing Test Setup Functions ====================

/// Create test environment with C2M routing configuration
///
/// # Arguments
/// * `c2m_routes` - C2M routing mappings: [("1001:T:1", "23:M:1"), ...]
///
/// # Returns
/// * `(Arc<dyn Rtdb>, Arc<RoutingCache>)` - RTDB and routing cache instances
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
) -> (Arc<dyn Rtdb>, Arc<RoutingCache>) {
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

/// Create test environment with C2C routing configuration
///
/// # Arguments
/// * `c2c_routes` - C2C routing mappings: [("1001:T:1", "1002:T:2"), ...]
///
/// # Returns
/// * `(Arc<dyn Rtdb>, Arc<RoutingCache>)` - RTDB and routing cache instances
///
/// # Example
/// ```no_run
/// use comsrv::test_utils::*;
///
/// #[tokio::test]
/// async fn test_c2c() {
///     let (rtdb, routing_cache) = setup_c2c_routing(vec![
///         ("1001:T:1", "1002:T:2"),
///         ("1002:T:2", "1003:T:3"),
///     ]).await;
///     // Use rtdb and routing_cache in tests
/// }
/// ```
pub async fn setup_c2c_routing(
    c2c_routes: Vec<(&str, &str)>,
) -> (Arc<dyn Rtdb>, Arc<RoutingCache>) {
    let rtdb = create_test_rtdb();
    let mut c2c_map = HashMap::new();
    for (source, target) in c2c_routes {
        c2c_map.insert(source.to_string(), target.to_string());
    }
    let routing_cache = Arc::new(RoutingCache::from_maps(
        HashMap::new(),
        HashMap::new(),
        c2c_map,
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
pub async fn assert_channel_value(
    rtdb: &dyn Rtdb,
    channel_id: u16,
    point_type: &str,
    point_id: u32,
    expected_value: f64,
) {
    use voltage_config::protocols::PointType;

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

/// Verify channel point timestamp exists
///
/// # Arguments
/// * `rtdb` - RTDB instance
/// * `channel_id` - Channel ID
/// * `point_type` - Point type (T/S/C/A)
/// * `point_id` - Point ID
///
/// # Example
/// ```no_run
/// use comsrv::test_utils::*;
///
/// #[tokio::test]
/// async fn test_timestamp() {
///     let rtdb = create_test_rtdb();
///     // ... write data ...
///     assert_channel_timestamp_exists(&rtdb, 1001, "T", 1).await;
/// }
/// ```
#[allow(clippy::disallowed_methods)] // Test utility - unwrap is acceptable for test data conversion
pub async fn assert_channel_timestamp_exists(
    rtdb: &dyn Rtdb,
    channel_id: u16,
    point_type: &str,
    point_id: u32,
) {
    use voltage_config::protocols::PointType;

    let config = KeySpaceConfig::production();
    let point_type_enum = PointType::from_str(point_type).unwrap();
    let ts_key = config.channel_ts_key(channel_id, point_type_enum);

    let ts_value = rtdb
        .hash_get(&ts_key, &point_id.to_string())
        .await
        .expect("Failed to read timestamp")
        .expect("Timestamp should exist");

    let ts_str = String::from_utf8(ts_value.to_vec()).unwrap();
    let timestamp: u64 = ts_str.parse().expect("Timestamp should be valid u64");

    assert!(
        timestamp > 0,
        "Timestamp for channel {}:{}:{} should be > 0",
        channel_id,
        point_type,
        point_id
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
pub async fn assert_instance_measurement(
    rtdb: &dyn Rtdb,
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
