//! Voltage M2C Routing Library
//!
//! Provides unified M2C (Model to Channel) routing functionality for modsrv and rules engine.
//! This library implements the Write-Triggers-Routing pattern, ensuring all action writes
//! automatically trigger downstream channel updates and TODO queues.

#![allow(clippy::disallowed_methods)] // Used in specific contexts

use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use serde::Deserialize;
use voltage_rtdb::Rtdb;

/// Structured representation of an action routing outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRouteOutcome {
    /// Status field (normally `"success"`).
    pub status: String,
    /// Instance name associated with the action.
    pub instance_name: String,
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

#[derive(Debug, Deserialize)]
struct RawActionRouteOutcome {
    status: String,
    instance: String,
    point: String,
    value: serde_json::Value,
    #[serde(default)]
    routed: bool,
    #[serde(default, rename = "route_result")]
    route_result: Option<serde_json::Value>,
    #[serde(default, rename = "route_context")]
    route_context: Option<RawRouteContext>,
}

#[derive(Debug, Deserialize)]
struct RawRouteContext {
    channel_id: serde_json::Value,
    point_type: serde_json::Value,
    #[serde(default)]
    comsrv_point_id: serde_json::Value,
    queue_key: serde_json::Value,
}

impl TryFrom<RawActionRouteOutcome> for ActionRouteOutcome {
    type Error = anyhow::Error;

    fn try_from(raw: RawActionRouteOutcome) -> Result<Self, Self::Error> {
        Ok(Self {
            status: raw.status,
            instance_name: raw.instance,
            point_id: raw.point,
            value: value_to_string(raw.value),
            routed: raw.routed,
            route_result: raw.route_result.map(value_to_string),
            route_context: match raw.route_context {
                Some(ctx) => Some(RouteContext::try_from(ctx)?),
                None => None,
            },
        })
    }
}

impl TryFrom<RawRouteContext> for RouteContext {
    type Error = anyhow::Error;

    fn try_from(raw: RawRouteContext) -> Result<Self, Self::Error> {
        Ok(Self {
            channel_id: value_to_string(raw.channel_id),
            point_type: value_to_string(raw.point_type),
            comsrv_point_id: value_to_string(raw.comsrv_point_id),
            queue_key: value_to_string(raw.queue_key),
        })
    }
}

/// Execute action routing with application-layer cache
///
/// This function implements the unified M2C routing logic:
/// 1. Resolves instance_name to instance_id
/// 2. Looks up M2C routing in cache
/// 3. Writes to instance Action Hash (state storage)
/// 4. Writes to channel Hash + triggers TODO queue (Write-Triggers-Routing pattern)
///
/// # Arguments
/// * `redis` - RTDB trait object
/// * `routing_cache` - M2C routing cache
/// * `instance_name` - Instance name (e.g., "pv_inverter_01")
/// * `point_id` - Action point ID
/// * `value` - Point value
///
/// # Returns
/// * `Ok(ActionRouteOutcome)` - Routing outcome with metadata
/// * `Err(anyhow::Error)` - Routing error
#[allow(deprecated)] // Uses time_millis internally until TimeProvider migration is complete
pub async fn set_action_point<R>(
    redis: &R,
    routing_cache: &voltage_config::RoutingCache,
    instance_name: &str,
    point_id: &str,
    value: f64,
) -> Result<ActionRouteOutcome>
where
    R: Rtdb + ?Sized,
{
    let config = voltage_config::KeySpaceConfig::production();

    // Step 1: Resolve instance name to instance ID using reverse index (O(1) lookup)
    let instance_id_bytes = redis
        .hash_get("inst:name:index", instance_name)
        .await
        .context("Failed to query instance name index")?
        .ok_or_else(|| anyhow!("Instance '{}' not found", instance_name))?;

    let instance_id = String::from_utf8(instance_id_bytes.to_vec())
        .context("Invalid instance ID in index")?
        .parse::<u32>()
        .context("Failed to parse instance ID")?;

    // Step 2: Build M2C routing key and lookup target
    let route_key = format!("{}:A:{}", instance_id, point_id);
    let routed = if let Some(target) = routing_cache.lookup_m2c(&route_key) {
        // Parse target: "2:A:1" -> channel_id=2, point_type=A, point_id=1
        let parts: Vec<&str> = target.split(':').collect();
        if parts.len() != 3 {
            return Ok(ActionRouteOutcome {
                status: "success".to_string(),
                instance_name: instance_name.to_string(),
                point_id: point_id.to_string(),
                value: value.to_string(),
                routed: false,
                route_result: Some(format!("invalid_route_target:{}", target)),
                route_context: None,
            });
        }

        let channel_id = parts[0]
            .parse::<u16>()
            .context("Failed to parse channel_id from route target")?;
        let point_type = parts[1];
        let comsrv_point_id = parts[2];

        // Step 3: Write to instance Action Hash (state storage)
        let instance_action_key = config.instance_action_key(instance_id);
        redis
            .hash_set(
                &instance_action_key,
                point_id,
                Bytes::from(value.to_string()),
            )
            .await
            .context("Failed to write instance action point")?;

        // Step 4: Write to channel Hash + auto-trigger TODO queue (Write-Triggers-Routing pattern)
        use voltage_config::protocols::PointType;
        let point_type_enum = PointType::from_str(point_type)
            .ok_or_else(|| anyhow!("Invalid point type: {}", point_type))?;

        // Get current timestamp (milliseconds)
        let timestamp_ms = redis.time_millis().await.unwrap_or_else(|_| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64
        });

        let comsrv_point_id_u32 = comsrv_point_id
            .parse::<u32>()
            .context("Failed to parse comsrv point_id")?;

        // Use unified helper: writes channel Hash (value/ts/raw) + triggers TODO queue
        voltage_rtdb::helpers::set_channel_point_with_trigger(
            redis,
            &config,
            channel_id,
            point_type_enum,
            comsrv_point_id_u32,
            value,
            timestamp_ms,
        )
        .await
        .context("Failed to set channel point with trigger")?;

        let todo_key = config.todo_queue_key(channel_id, point_type_enum);

        // Build route context
        let route_context = RouteContext {
            channel_id: channel_id.to_string(),
            point_type: point_type.to_string(),
            comsrv_point_id: comsrv_point_id.to_string(),
            queue_key: todo_key.to_string(),
        };

        Ok(ActionRouteOutcome {
            status: "success".to_string(),
            instance_name: instance_name.to_string(),
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
            .hash_set(
                &instance_action_key,
                point_id,
                Bytes::from(value.to_string()),
            )
            .await
            .context("Failed to write instance action point")?;

        Ok(ActionRouteOutcome {
            status: "success".to_string(),
            instance_name: instance_name.to_string(),
            point_id: point_id.to_string(),
            value: value.to_string(),
            routed: false,
            route_result: Some("no_route".to_string()),
            route_context: None,
        })
    };

    routed
}

fn value_to_string(value: serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s,
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    #[test]
    fn test_m2c_config_has_required_fields() {
        // M2C routing requires specific configuration fields
        let config = voltage_config::KeySpaceConfig::production().for_m2c();

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
        let config = voltage_config::KeySpaceConfig::production();

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
        let config = voltage_config::KeySpaceConfig::production().for_m2c();
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
