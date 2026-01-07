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
#[allow(deprecated)] // Uses time_millis internally until TimeProvider migration is complete
pub async fn set_action_point<R>(
    redis: &R,
    routing_cache: &voltage_rtdb::RoutingCache,
    instance_id: u32,
    point_id: &str,
    value: f64,
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
