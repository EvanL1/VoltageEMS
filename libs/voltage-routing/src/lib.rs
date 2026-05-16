//! Voltage Routing Library
//!
//! Provides unified routing functionality for VoltageEMS services:
//! - M2C (Model to Channel) routing for action writes (downlink)
//! - C2M (Channel to Model) routing for measurement reads (uplink)
//! - C2C (Channel to Channel) routing for data forwarding
//! - Batch routing execution with 3-layer data architecture
//! - SQLite routing loader for service initialization
//! - Write-Triggers-Routing pattern implementation
//! - RoutingCache: In-memory routing table cache

#![allow(clippy::disallowed_methods)] // Used in specific contexts

pub mod batch;
pub mod loader;
pub mod routing_cache;

pub use batch::{
    BatchRoutingResult, ChannelPointUpdate, write_channel_batch, write_channel_batch_buffered,
};
pub use loader::{RoutingMaps, load_routing_maps};

// Re-export RoutingCache types (canonical location: voltage-routing)
pub use routing_cache::{C2CTarget, C2MTarget, M2CTarget, RoutingCache, RoutingCacheStats};

use anyhow::{Context, Result};
use voltage_model::{PointType, ValidationConfig, validate_value};
use voltage_rtdb::Rtdb;

/// Status string for successful operations
const STATUS_SUCCESS: &str = "success";

/// Structured representation of an action routing outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRouteOutcome {
    /// Status field (normally `"success"`).
    pub status: String,
    /// Instance ID associated with the action.
    pub instance_id: u32,
    /// Action point ID.
    pub point_id: String,
    /// Value written to the action point.
    pub value: String,
    /// Whether the action was successfully routed to comsrv.
    pub routed: bool,
    /// Additional routing detail (channel id or error code).
    pub route_result: Option<String>,
    /// Optional route context (available when routing succeeded).
    pub route_context: Option<RouteContext>,
}

impl ActionRouteOutcome {
    /// Convenience accessor for success status.
    pub fn is_success(&self) -> bool {
        self.status.eq_ignore_ascii_case("success")
    }
}

/// Additional routing metadata when routing succeeds.
///
/// Contains both string and numeric representations of routing targets.
/// Numeric fields (`target_*`) are provided for efficient SHM writes by callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteContext {
    pub channel_id: String,
    pub point_type: String,
    pub comsrv_point_id: String,
    /// Numeric channel ID for SHM writes
    pub target_channel_id: u32,
    /// Numeric point type (T=0, S=1, C=2, A=3) for SHM writes
    pub target_point_type: u8,
    /// Numeric comsrv point ID for SHM writes
    pub target_point_id: u32,
    /// Timestamp (ms) used for the Redis write
    pub timestamp_ms: i64,
}

/// Execute action routing with application-layer cache
///
/// This function implements the unified M2C routing logic:
/// 1. Looks up M2C routing in cache
/// 2. Writes to instance Action Hash (state storage)
/// 3. Writes to channel Hash (value/ts/raw)
///
/// SHM writes (shared memory) and UDS notifications should be handled by the caller
/// using [`RouteContext`] fields for the target channel/point/timestamp.
///
/// # TOCTOU note
///
/// This function performs its own M2C lookup. Callers that have already
/// consulted the routing cache (e.g. modsrv's channel-offline gate) should
/// prefer [`set_action_point_with_target`] and pass the resolved target as a
/// parameter — otherwise a routing reload between the caller's check and this
/// function's lookup silently degrades a routed action into a local-store-only
/// write, with no error returned to the operator.
///
/// # Arguments
/// * `redis` - RTDB trait object
/// * `routing_cache` - M2C routing cache
/// * `instance_id` - Instance ID (numeric)
/// * `point_id` - Action point ID
/// * `value` - Point value
///
/// # Returns
/// * `Ok(ActionRouteOutcome)` - Routing outcome with metadata
/// * `Err(anyhow::Error)` - Routing error
pub async fn set_action_point<R>(
    redis: &R,
    routing_cache: &RoutingCache,
    instance_id: u32,
    point_id: &str,
    value: f64,
) -> Result<ActionRouteOutcome>
where
    R: Rtdb,
{
    // Lookup M2C routing target (zero-allocation path when point_id is numeric)
    let target_opt = if let Ok(point_id_u32) = point_id.parse::<u32>() {
        // Fast path: use structured key lookup (no string allocation)
        routing_cache.lookup_m2c_by_parts(instance_id, PointType::Adjustment, point_id_u32)
    } else {
        // Fallback: build string key for non-numeric point_id (rare)
        let route_key = format!("{}:A:{}", instance_id, point_id);
        routing_cache.lookup_m2c(&route_key)
    };
    set_action_point_with_target(redis, instance_id, point_id, value, target_opt).await
}

/// Execute action routing with a caller-resolved M2C target.
///
/// Behaves like [`set_action_point`] but skips the internal routing lookup —
/// the target (or its absence) is passed in by the caller. Use this when the
/// caller has its own reason to consult routing first (e.g. an offline-channel
/// gate) and needs the writes to use the same snapshot, immune to concurrent
/// routing reload.
///
/// Semantics of `target`:
/// * `Some(M2CTarget)` — caller decided this action routes to that channel;
///   writes go to both the instance hash and the downstream channel hash.
/// * `None` — caller decided no M2C route exists; only the instance hash is
///   written (local-only "store but don't dispatch"). This is meaningful, so
///   `None` must be a deliberate decision, not a "I don't know" placeholder.
pub async fn set_action_point_with_target<R>(
    redis: &R,
    instance_id: u32,
    point_id: &str,
    value: f64,
    target: Option<M2CTarget>,
) -> Result<ActionRouteOutcome>
where
    R: Rtdb,
{
    // Validate value before M2C routing (prevents NaN/Infinity from reaching devices)
    let validation_config = ValidationConfig::default();
    let value = validate_value(value, &validation_config).map_err(|e| {
        anyhow::anyhow!(
            "M2C data validation failed for inst:{}:A:{}: {}",
            instance_id,
            point_id,
            e
        )
    })?;

    let config = voltage_rtdb::KeySpaceConfig::production_cached();

    // Timestamp (epoch-ms) is computed up-front so the same value is written
    // to both the instance sidecar (inst:{id}:A:ts) and the downstream channel
    // hash (comsrv:{ch}:{A|...}:ts). Needed by the apigateway WebSocket which
    // pulls `ts` from the sidecar for `source='inst'` subscriptions.
    use voltage_rtdb::{SystemTimeProvider, TimeProvider};
    let timestamp_ms = SystemTimeProvider.now_millis();

    if let Some(target) = target {
        // M2CTarget is now a structured type - no parsing needed
        let channel_id = target.channel_id;
        let point_type_enum = target.point_type;
        let comsrv_point_id = target.point_id;

        // Step 1: Write to instance Action Hash (state storage) + sidecar ts
        let instance_action_key = config.instance_action_key(instance_id);
        redis
            .hash_set_f64(&instance_action_key, point_id, value)
            .await
            .context("Failed to write instance action point")?;
        let instance_action_ts_key = config.instance_action_ts_key(instance_id);
        redis
            .hash_set(
                &instance_action_ts_key,
                point_id,
                voltage_rtdb::numfmt::i64_to_bytes(timestamp_ms),
            )
            .await
            .context("Failed to write instance action point timestamp")?;

        // Step 2: Write to channel Hash

        voltage_rtdb::helpers::write_channel_hash_only(
            redis,
            config,
            channel_id,
            point_type_enum,
            comsrv_point_id,
            value,
            timestamp_ms,
        )
        .await
        .context("Failed to write channel hash")?;

        // Build route context with numeric fields for SHM callers
        let route_context = RouteContext {
            channel_id: channel_id.to_string(),
            point_type: point_type_enum.as_str().to_string(),
            comsrv_point_id: comsrv_point_id.to_string(),
            target_channel_id: channel_id,
            target_point_type: point_type_enum.to_u8(),
            target_point_id: comsrv_point_id,
            timestamp_ms,
        };

        Ok(ActionRouteOutcome {
            status: STATUS_SUCCESS.to_string(),
            instance_id,
            point_id: point_id.to_string(),
            value: value.to_string(),
            routed: true,
            route_result: Some(channel_id.to_string()),
            route_context: Some(route_context),
        })
    } else {
        // No routing found - write to instance Hash only (+ sidecar ts)
        let instance_action_key = config.instance_action_key(instance_id);
        redis
            .hash_set_f64(&instance_action_key, point_id, value)
            .await
            .context("Failed to write instance action point")?;
        let instance_action_ts_key = config.instance_action_ts_key(instance_id);
        redis
            .hash_set(
                &instance_action_ts_key,
                point_id,
                voltage_rtdb::numfmt::i64_to_bytes(timestamp_ms),
            )
            .await
            .context("Failed to write instance action point timestamp")?;

        Ok(ActionRouteOutcome {
            status: STATUS_SUCCESS.to_string(),
            instance_id,
            point_id: point_id.to_string(),
            value: value.to_string(),
            routed: false,
            route_result: Some("no_route".to_string()),
            route_context: None,
        })
    }
}

// ============================================================================
// C2C Routing Constants
// ============================================================================

/// Maximum cascade depth for C2C routing to prevent infinite loops
pub const MAX_C2C_CASCADE_DEPTH: u8 = 2;

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use voltage_rtdb::helpers::create_test_rtdb;

    // ========================================================================
    // Helper functions
    // ========================================================================

    fn create_test_routing_cache() -> Arc<RoutingCache> {
        Arc::new(RoutingCache::new())
    }

    /// Create a routing cache with M2C routing pre-configured
    fn create_routing_cache_with_m2c() -> Arc<RoutingCache> {
        let mut m2c_data = HashMap::new();
        // M2C route: instance 1001:A:1 -> channel 2001:C:10
        m2c_data.insert("1001:A:1".to_string(), "2001:C:10".to_string());
        Arc::new(RoutingCache::from_maps(
            HashMap::new(),
            m2c_data,
            HashMap::new(),
        ))
    }

    /// Create a routing cache with C2M routing pre-configured
    fn create_routing_cache_with_c2m() -> Arc<RoutingCache> {
        let mut c2m_data = HashMap::new();
        // C2M route: channel 1001:T:1 -> instance 5001:M:100
        c2m_data.insert("1001:T:1".to_string(), "5001:M:100".to_string());
        Arc::new(RoutingCache::from_maps(
            c2m_data,
            HashMap::new(),
            HashMap::new(),
        ))
    }

    /// Create a routing cache with C2C routing pre-configured
    fn create_routing_cache_with_c2c() -> Arc<RoutingCache> {
        let mut c2c_data = HashMap::new();
        // C2C route: channel 1001:T:1 -> channel 2001:T:1
        c2c_data.insert("1001:T:1".to_string(), "2001:T:1".to_string());
        Arc::new(RoutingCache::from_maps(
            HashMap::new(),
            HashMap::new(),
            c2c_data,
        ))
    }

    // ========================================================================
    // set_action_point() Tests
    // ========================================================================

    #[tokio::test]
    async fn test_set_action_point_no_route() {
        let rtdb = create_test_rtdb();
        let routing_cache = create_test_routing_cache();

        let result = set_action_point(&*rtdb, &routing_cache, 1001, "1", 42.5).await;

        assert!(result.is_ok(), "set_action_point should succeed");
        let outcome = result.unwrap();
        assert!(outcome.is_success(), "Status should be success");
        assert_eq!(outcome.instance_id, 1001);
        assert_eq!(outcome.point_id, "1");
        assert_eq!(outcome.value, "42.5");
        assert!(!outcome.routed, "Should not be routed without M2C config");
        assert_eq!(outcome.route_result, Some("no_route".to_string()));
        assert!(outcome.route_context.is_none());
    }

    #[tokio::test]
    async fn test_set_action_point_with_route() {
        let rtdb = create_test_rtdb();
        let routing_cache = create_routing_cache_with_m2c();

        let result = set_action_point(&*rtdb, &routing_cache, 1001, "1", 100.0).await;

        assert!(result.is_ok(), "set_action_point should succeed");
        let outcome = result.unwrap();
        assert!(outcome.is_success());
        assert!(outcome.routed, "Should be routed with M2C config");
        assert_eq!(outcome.route_result, Some("2001".to_string()));

        // Verify route context
        let ctx = outcome.route_context.expect("Should have route context");
        assert_eq!(ctx.channel_id, "2001");
        assert_eq!(ctx.point_type, "C");
        assert_eq!(ctx.comsrv_point_id, "10");
        // Verify numeric fields
        assert_eq!(ctx.target_channel_id, 2001);
        assert_eq!(ctx.target_point_id, 10);
    }

    #[tokio::test]
    async fn test_set_action_point_writes_to_instance_hash() {
        let rtdb = create_test_rtdb();
        let routing_cache = create_test_routing_cache();

        set_action_point(&*rtdb, &routing_cache, 5001, "42", 123.456)
            .await
            .unwrap();

        let config = voltage_rtdb::KeySpaceConfig::production_cached();
        let instance_key = config.instance_action_key(5001);
        let stored_value = rtdb.hash_get(&instance_key, "42").await.unwrap();

        assert!(stored_value.is_some(), "Instance hash should have value");
        let value_str = String::from_utf8(stored_value.unwrap().to_vec()).unwrap();
        assert_eq!(value_str, "123.456");

        // Sidecar inst:{id}:A:ts must be populated (needed by apigateway WebSocket)
        let instance_ts_key = config.instance_action_ts_key(5001);
        let stored_ts = rtdb
            .hash_get(&instance_ts_key, "42")
            .await
            .unwrap()
            .expect("instance_action_ts_key should have ts for point 42");
        let ts_str = String::from_utf8(stored_ts.to_vec()).unwrap();
        let ts_i64: i64 = ts_str.parse().expect("ts should parse as i64");
        assert!(ts_i64 > 0, "timestamp should be positive, got {}", ts_i64);
    }

    #[tokio::test]
    async fn test_set_action_point_no_route_writes_ts() {
        // No-route branch must also populate the sidecar ts hash.
        let rtdb = create_test_rtdb();
        let routing_cache = RoutingCache::new(); // empty: no M2C route

        set_action_point(&*rtdb, &routing_cache, 9999, "7", 42.0)
            .await
            .unwrap();

        let config = voltage_rtdb::KeySpaceConfig::production_cached();
        let instance_ts_key = config.instance_action_ts_key(9999);
        let stored_ts = rtdb
            .hash_get(&instance_ts_key, "7")
            .await
            .unwrap()
            .expect("no-route branch should still write instance action ts");
        let ts_str = String::from_utf8(stored_ts.to_vec()).unwrap();
        let ts_i64: i64 = ts_str.parse().expect("ts should parse as i64");
        assert!(ts_i64 > 0, "timestamp should be positive, got {}", ts_i64);
    }

    // ========================================================================
    // write_channel_batch() Tests
    // ========================================================================

    #[tokio::test]
    async fn test_write_channel_batch_empty() {
        let rtdb = create_test_rtdb();
        let routing_cache = create_test_routing_cache();

        let result = write_channel_batch(&*rtdb, &routing_cache, vec![]).await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.channel_writes, 0);
        assert_eq!(stats.c2m_writes, 0);
        assert_eq!(stats.c2c_forwards, 0);
    }

    #[tokio::test]
    async fn test_write_channel_batch_single_point() {
        let rtdb = create_test_rtdb();
        let routing_cache = create_test_routing_cache();

        let updates = vec![ChannelPointUpdate::new(
            1001,
            PointType::Telemetry,
            1,
            220.5,
        )];

        let result = write_channel_batch(&*rtdb, &routing_cache, updates).await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.channel_writes, 1, "Should write 1 channel point");
    }

    #[tokio::test]
    async fn test_write_channel_batch_multiple_points() {
        let rtdb = create_test_rtdb();
        let routing_cache = create_test_routing_cache();

        let updates = vec![
            ChannelPointUpdate::new(1001, PointType::Telemetry, 1, 220.0),
            ChannelPointUpdate::new(1001, PointType::Telemetry, 2, 380.0),
            ChannelPointUpdate::new(1001, PointType::Signal, 1, 1.0),
        ];

        let result = write_channel_batch(&*rtdb, &routing_cache, updates).await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.channel_writes, 3, "Should write 3 channel points");
    }

    #[tokio::test]
    async fn test_write_channel_batch_with_c2m_routing() {
        let rtdb = create_test_rtdb();
        let routing_cache = create_routing_cache_with_c2m();

        let updates = vec![ChannelPointUpdate::new(1001, PointType::Telemetry, 1, 42.0)];

        let result = write_channel_batch(&*rtdb, &routing_cache, updates).await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.channel_writes, 1);
        assert_eq!(stats.c2m_writes, 1, "Should have 1 C2M write");
    }

    // ========================================================================
    // C2C Cascade Depth Tests
    // ========================================================================

    #[tokio::test]
    async fn test_c2c_cascade_depth_limit() {
        let rtdb = create_test_rtdb();
        let routing_cache = create_routing_cache_with_c2c();

        let updates = vec![ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 1,
            value: 100.0,
            raw_value: None,
            cascade_depth: MAX_C2C_CASCADE_DEPTH,
        }];

        let result = write_channel_batch(&*rtdb, &routing_cache, updates).await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.c2c_forwards, 0, "Should NOT forward at max depth");
    }

    #[tokio::test]
    async fn test_c2c_cascade_depth_allows_forward() {
        let rtdb = create_test_rtdb();
        let routing_cache = create_routing_cache_with_c2c();

        let updates = vec![ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 1,
            value: 100.0,
            raw_value: None,
            cascade_depth: 0,
        }];

        let result = write_channel_batch(&*rtdb, &routing_cache, updates).await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert!(stats.c2c_forwards > 0, "Should forward below max depth");
    }

    // ========================================================================
    // C2C Cycle Detection Tests
    // ========================================================================

    fn create_routing_cache_with_c2c_cycle() -> Arc<RoutingCache> {
        let mut c2c_data = HashMap::new();
        c2c_data.insert("1001:T:1".to_string(), "2001:T:1".to_string());
        c2c_data.insert("2001:T:1".to_string(), "1001:T:1".to_string());
        Arc::new(RoutingCache::from_maps(
            HashMap::new(),
            HashMap::new(),
            c2c_data,
        ))
    }

    #[tokio::test]
    async fn test_c2c_cycle_detection() {
        let rtdb = create_test_rtdb();
        let routing_cache = create_routing_cache_with_c2c_cycle();

        let updates = vec![ChannelPointUpdate::new(
            1001,
            PointType::Telemetry,
            1,
            100.0,
        )];

        let result = write_channel_batch(&*rtdb, &routing_cache, updates).await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert!(stats.c2c_forwards > 0, "Should have some forwards");
        assert!(
            stats.cycles_detected > 0,
            "Should detect A→B→A cycle and skip"
        );
    }

    #[tokio::test]
    async fn test_c2c_no_cycle_without_loop() {
        let rtdb = create_test_rtdb();
        let routing_cache = create_routing_cache_with_c2c();

        let updates = vec![ChannelPointUpdate::new(
            1001,
            PointType::Telemetry,
            1,
            100.0,
        )];

        let result = write_channel_batch(&*rtdb, &routing_cache, updates).await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.cycles_detected, 0, "No cycle should be detected");
    }

    // ========================================================================
    // ChannelPointUpdate Tests
    // ========================================================================

    #[test]
    fn test_channel_point_update_new() {
        let update = ChannelPointUpdate::new(1001, PointType::Telemetry, 42, 123.456);

        assert_eq!(update.channel_id, 1001);
        assert_eq!(update.point_type, PointType::Telemetry);
        assert_eq!(update.point_id, 42);
        assert_eq!(update.value, 123.456);
        assert!(update.raw_value.is_none());
        assert_eq!(update.cascade_depth, 0);
    }

    #[test]
    fn test_channel_point_update_with_raw() {
        let update = ChannelPointUpdate::new(1001, PointType::Telemetry, 1, 220.0).with_raw(2200.0);

        assert_eq!(update.value, 220.0);
        assert_eq!(update.raw_value, Some(2200.0));
    }

    // ========================================================================
    // BatchRoutingResult Tests
    // ========================================================================

    #[test]
    fn test_batch_routing_result_merge() {
        let mut result1 = BatchRoutingResult {
            channel_writes: 10,
            c2m_writes: 5,
            c2c_forwards: 2,
            cycles_detected: 0,
        };

        let result2 = BatchRoutingResult {
            channel_writes: 3,
            c2m_writes: 1,
            c2c_forwards: 1,
            cycles_detected: 1,
        };

        result1.merge(result2);

        assert_eq!(result1.channel_writes, 13);
        assert_eq!(result1.c2m_writes, 6);
        assert_eq!(result1.c2c_forwards, 3);
        assert_eq!(result1.cycles_detected, 1);
    }

    // ========================================================================
    // Configuration Tests
    // ========================================================================

    #[test]
    fn test_m2c_config_has_required_fields() {
        let config = voltage_rtdb::KeySpaceConfig::production().for_m2c();

        assert!(
            config.inst_name_pattern.is_some(),
            "M2C config must have inst_name_pattern for name resolution"
        );
        assert_eq!(
            config.inst_name_pattern.as_ref().unwrap(),
            "inst:*:name",
            "inst_name_pattern should match 'inst:*:name' pattern"
        );

        assert!(
            config.target_prefix.is_some(),
            "M2C config must have target_prefix for channel routing"
        );
        assert_eq!(
            config.target_prefix.as_ref().unwrap(),
            "comsrv",
            "target_prefix should point to comsrv keys"
        );

        assert_eq!(
            config.routing_table, "route:m2c",
            "M2C config should use route:m2c table"
        );
    }

    #[test]
    fn test_production_config_is_incomplete_for_m2c() {
        let config = voltage_rtdb::KeySpaceConfig::production_cached();

        assert!(
            config.inst_name_pattern.is_none(),
            "production() config lacks inst_name_pattern - must use .for_m2c()"
        );
        assert!(
            config.target_prefix.is_none(),
            "production() config lacks target_prefix - must use .for_m2c()"
        );

        assert_eq!(
            config.routing_table, "route:c2m",
            "production() uses C2M routing table, not M2C"
        );
    }

    #[test]
    fn test_config_serialization() {
        let config = voltage_rtdb::KeySpaceConfig::production().for_m2c();
        let config_json = serde_json::to_string(&config);

        assert!(
            config_json.is_ok(),
            "M2C config should be serializable to JSON"
        );

        let json_str = config_json.unwrap();
        assert!(
            json_str.contains("inst_name_pattern"),
            "Serialized JSON should contain inst_name_pattern"
        );
        assert!(
            json_str.contains("target_prefix"),
            "Serialized JSON should contain target_prefix"
        );
    }
}
