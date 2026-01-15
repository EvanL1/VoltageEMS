//! Voltage Routing Library
//!
//! Provides unified routing functionality for VoltageEMS services:
//! - M2C (Model to Channel) routing for action writes (downlink)
//! - C2M (Channel to Model) routing for measurement reads (uplink)
//! - C2C (Channel to Channel) routing for data forwarding
//! - Batch routing execution with 3-layer data architecture
//! - SQLite routing loader for service initialization
//! - Write-Triggers-Routing pattern implementation

#![allow(clippy::disallowed_methods)] // Used in specific contexts

pub mod batch;
pub mod loader;

pub use batch::{
    write_channel_batch, write_channel_batch_buffered, write_channel_batch_direct,
    BatchRoutingResult, ChannelPointUpdate,
};
pub use loader::{load_routing_maps, RoutingMaps};

// Re-export RoutingCache for convenience
pub use voltage_rtdb::RoutingCache;

use anyhow::{Context, Result};
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteContext {
    pub channel_id: String,
    pub point_type: String,
    pub comsrv_point_id: String,
    pub queue_key: String,
}

/// Execute action routing with application-layer cache
///
/// This function implements the unified M2C routing logic:
/// 1. Looks up M2C routing in cache
/// 2. Writes to instance Action Hash (state storage)
/// 3. Writes to channel Hash + triggers TODO queue (Write-Triggers-Routing pattern)
/// 4. Optionally writes to SHM (shared memory) for zero-copy M2C notification
/// 5. Optionally sends UDS notification for event-driven M2C command dispatch
///
/// # Arguments
/// * `redis` - RTDB trait object
/// * `routing_cache` - M2C routing cache
/// * `instance_id` - Instance ID (numeric)
/// * `point_id` - Action point ID
/// * `value` - Point value
/// * `shm_writer` - Optional SHM writer for M2C via shared memory
/// * `notifier` - Optional UDS notifier for event-driven command notification
///
/// # Returns
/// * `Ok(ActionRouteOutcome)` - Routing outcome with metadata
/// * `Err(anyhow::Error)` - Routing error
#[allow(deprecated)] // Uses time_millis internally until TimeProvider migration is complete
pub async fn set_action_point<R>(
    redis: &R,
    routing_cache: &voltage_rtdb::RoutingCache,
    instance_id: u32,
    point_id: &str,
    value: f64,
    shm_writer: Option<&voltage_rtdb::UnifiedWriter>,
    notifier: Option<&mut voltage_rtdb::ShmNotifier>,
) -> Result<ActionRouteOutcome>
where
    R: Rtdb,
{
    let config = voltage_rtdb::KeySpaceConfig::production_cached();

    // Lookup M2C routing target (zero-allocation path when point_id is numeric)
    let target_opt = if let Ok(point_id_u32) = point_id.parse::<u32>() {
        // Fast path: use structured key lookup (no string allocation)
        routing_cache.lookup_m2c_by_parts(
            instance_id,
            voltage_model::PointType::Adjustment,
            point_id_u32,
        )
    } else {
        // Fallback: build string key for non-numeric point_id (rare)
        let route_key = format!("{}:A:{}", instance_id, point_id);
        routing_cache.lookup_m2c(&route_key)
    };

    let routed = if let Some(target) = target_opt {
        // M2CTarget is now a structured type - no parsing needed
        let channel_id = target.channel_id;
        let point_type_enum = target.point_type;
        let comsrv_point_id = target.point_id;

        // Step 3: Write to instance Action Hash (state storage)
        let instance_action_key = config.instance_action_key(instance_id);
        redis
            .hash_set_f64(&instance_action_key, point_id, value)
            .await
            .context("Failed to write instance action point")?;

        // Step 4: Write to channel Hash + auto-trigger TODO queue (Write-Triggers-Routing pattern)
        // Get current timestamp (milliseconds) - use local system time for efficiency
        use voltage_rtdb::{SystemTimeProvider, TimeProvider};
        let timestamp_ms = SystemTimeProvider.now_millis();

        // Use unified helper: writes channel Hash (value/ts/raw) + triggers TODO queue
        voltage_rtdb::helpers::set_channel_point_with_trigger(
            redis,
            config,
            channel_id,
            point_type_enum,
            comsrv_point_id,
            value,
            timestamp_ms,
        )
        .await
        .context("Failed to set channel point with trigger")?;

        // Step 5: Write to SHM if available (primary path for M2C notification)
        // This enables comsrv's ShmCommandPoller to detect the command via timestamp change
        let shm_written = if let Some(writer) = shm_writer {
            let written = writer.set_action(
                channel_id,
                point_type_enum.to_u8(),
                comsrv_point_id,
                value,
                timestamp_ms as u64,
            );
            if !written {
                tracing::warn!(
                    "SHM write failed for ch{}:{:?}:{}",
                    channel_id,
                    point_type_enum,
                    comsrv_point_id
                );
            }
            written
        } else {
            false
        };

        // Step 6: Send UDS notification (event-driven M2C dispatch)
        // Only notify if SHM write succeeded (avoid duplicate notifications via TODO queue)
        if shm_written {
            if let Some(notifier) = notifier {
                if let Err(e) = notifier
                    .notify(channel_id, point_type_enum, comsrv_point_id)
                    .await
                {
                    tracing::warn!(
                        "UDS notify failed for ch{}:{:?}:{}: {}",
                        channel_id,
                        point_type_enum,
                        comsrv_point_id,
                        e
                    );
                }
            }
        }

        let todo_key = config.todo_queue_key(channel_id, point_type_enum);

        // Build route context
        let route_context = RouteContext {
            channel_id: channel_id.to_string(),
            point_type: point_type_enum.as_str().to_string(),
            comsrv_point_id: comsrv_point_id.to_string(),
            queue_key: todo_key.to_string(),
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
        // No routing found - write to instance Hash only (no TODO queue)
        let instance_action_key = config.instance_action_key(instance_id);
        redis
            .hash_set_f64(&instance_action_key, point_id, value)
            .await
            .context("Failed to write instance action point")?;

        Ok(ActionRouteOutcome {
            status: STATUS_SUCCESS.to_string(),
            instance_id,
            point_id: point_id.to_string(),
            value: value.to_string(),
            routed: false,
            route_result: Some("no_route".to_string()),
            route_context: None,
        })
    };

    routed
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
    use voltage_model::PointType;
    use voltage_rtdb::{helpers::create_test_rtdb, RoutingCache};

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
        // Without M2C routing configured, should write to instance hash only
        let rtdb = create_test_rtdb();
        let routing_cache = create_test_routing_cache();

        let result = set_action_point(&*rtdb, &routing_cache, 1001, "1", 42.5, None, None).await;

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
        // Configure M2C routing and verify full routing path
        let rtdb = create_test_rtdb();
        let routing_cache = create_routing_cache_with_m2c();

        // M2C route: instance 1001:A:1 -> channel 2001:C:10
        let result = set_action_point(&*rtdb, &routing_cache, 1001, "1", 100.0, None, None).await;

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
    }

    #[tokio::test]
    async fn test_set_action_point_writes_to_instance_hash() {
        let rtdb = create_test_rtdb();
        let routing_cache = create_test_routing_cache();

        // Write action point
        set_action_point(&*rtdb, &routing_cache, 5001, "42", 123.456, None, None)
            .await
            .unwrap();

        // Verify instance action hash was written
        let config = voltage_rtdb::KeySpaceConfig::production_cached();
        let instance_key = config.instance_action_key(5001);
        let stored_value = rtdb.hash_get(&instance_key, "42").await.unwrap();

        assert!(stored_value.is_some(), "Instance hash should have value");
        let value_str = String::from_utf8(stored_value.unwrap().to_vec()).unwrap();
        assert_eq!(value_str, "123.456");
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
        // C2M route: channel 1001:T:1 -> instance 5001:M:100
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
        // C2C route: channel 1001:T:1 -> channel 2001:T:1
        let routing_cache = create_routing_cache_with_c2c();

        // Create update at max cascade depth - should NOT forward
        let updates = vec![ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 1,
            value: 100.0,
            raw_value: None,
            cascade_depth: MAX_C2C_CASCADE_DEPTH, // At limit
        }];

        let result = write_channel_batch(&*rtdb, &routing_cache, updates).await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.c2c_forwards, 0, "Should NOT forward at max depth");
    }

    #[tokio::test]
    async fn test_c2c_cascade_depth_allows_forward() {
        let rtdb = create_test_rtdb();
        // C2C route: channel 1001:T:1 -> channel 2001:T:1
        let routing_cache = create_routing_cache_with_c2c();

        // Create update below max cascade depth - should forward
        let updates = vec![ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 1,
            value: 100.0,
            raw_value: None,
            cascade_depth: 0, // Below limit
        }];

        let result = write_channel_batch(&*rtdb, &routing_cache, updates).await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert!(stats.c2c_forwards > 0, "Should forward below max depth");
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
        };

        let result2 = BatchRoutingResult {
            channel_writes: 3,
            c2m_writes: 1,
            c2c_forwards: 1,
        };

        result1.merge(result2);

        assert_eq!(result1.channel_writes, 13);
        assert_eq!(result1.c2m_writes, 6);
        assert_eq!(result1.c2c_forwards, 3);
    }

    // ========================================================================
    // Configuration Tests (existing)
    // ========================================================================

    #[test]
    fn test_m2c_config_has_required_fields() {
        // M2C routing requires specific configuration fields
        let config = voltage_rtdb::KeySpaceConfig::production().for_m2c();

        // inst_name_pattern is REQUIRED for resolving instance names to IDs
        assert!(
            config.inst_name_pattern.is_some(),
            "M2C config must have inst_name_pattern for name resolution"
        );
        assert_eq!(
            config.inst_name_pattern.as_ref().unwrap(),
            "inst:*:name",
            "inst_name_pattern should match 'inst:*:name' pattern"
        );

        // target_prefix is REQUIRED for routing to channels
        assert!(
            config.target_prefix.is_some(),
            "M2C config must have target_prefix for channel routing"
        );
        assert_eq!(
            config.target_prefix.as_ref().unwrap(),
            "comsrv",
            "target_prefix should point to comsrv keys"
        );

        // routing_table should be m2c (not c2m)
        assert_eq!(
            config.routing_table, "route:m2c",
            "M2C config should use route:m2c table"
        );
    }

    #[test]
    fn test_production_config_is_incomplete_for_m2c() {
        // Verify that raw production() config is NOT suitable for M2C
        let config = voltage_rtdb::KeySpaceConfig::production_cached();

        // Without for_m2c(), these fields are None
        assert!(
            config.inst_name_pattern.is_none(),
            "production() config lacks inst_name_pattern - must use .for_m2c()"
        );
        assert!(
            config.target_prefix.is_none(),
            "production() config lacks target_prefix - must use .for_m2c()"
        );

        // production() uses C2M routing table by default
        assert_eq!(
            config.routing_table, "route:c2m",
            "production() uses C2M routing table, not M2C"
        );
    }

    #[test]
    fn test_config_serialization() {
        // Verify that config can be serialized for Module calls
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
