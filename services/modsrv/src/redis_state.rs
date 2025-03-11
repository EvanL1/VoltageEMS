//! Redis state management helpers for ModSrv.
//!
//! Lua scripts handle only atomic routing; this module manages
//! control-plane read/write for instances/products, keeping business
//! logic and type safety directly in Rust.

use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fmt;
use voltage_config::common::RedisRoutingKeys;
use voltage_config::modsrv::RedisKeys;
use voltage_rtdb::Rtdb;

use crate::product_loader::{ActionPoint, MeasurementPoint, Product};

/// Routing map entries used to populate Redis hashes.
#[derive(Debug, Clone)]
pub struct RoutingEntry {
    pub comsrv_key: String,
    pub modsrv_key: String,
    pub is_action: bool,
}

/// Routing table selection (forward/backward).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingDirection {
    ChannelToModel,
    ModelToChannel,
}

impl RoutingDirection {
    fn table(self) -> &'static str {
        match self {
            RoutingDirection::ChannelToModel => RedisRoutingKeys::CHANNEL_TO_MODEL,
            RoutingDirection::ModelToChannel => RedisRoutingKeys::MODEL_TO_CHANNEL,
        }
    }
}

impl fmt::Display for RoutingDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RoutingDirection::ChannelToModel => write!(f, "c2m"),
            RoutingDirection::ModelToChannel => write!(f, "m2c"),
        }
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Store routing entries into Redis hashes.
pub async fn store_routing<R>(redis: &R, entries: &[RoutingEntry]) -> Result<usize>
where
    R: Rtdb + ?Sized,
{
    if entries.is_empty() {
        return Ok(0);
    }

    let mut forward_fields = Vec::new();
    let mut reverse_fields = Vec::new();

    for entry in entries {
        forward_fields.push((entry.comsrv_key.clone(), entry.modsrv_key.clone()));
        if entry.is_action {
            reverse_fields.push((entry.modsrv_key.clone(), entry.comsrv_key.clone()));
        }
    }

    if !forward_fields.is_empty() {
        let fields_bytes: Vec<(String, Bytes)> = forward_fields
            .into_iter()
            .map(|(k, v)| (k, Bytes::from(v)))
            .collect();
        redis
            .hash_mset(RedisRoutingKeys::CHANNEL_TO_MODEL, fields_bytes)
            .await?;
    }

    if !reverse_fields.is_empty() {
        let fields_bytes: Vec<(String, Bytes)> = reverse_fields
            .into_iter()
            .map(|(k, v)| (k, Bytes::from(v)))
            .collect();
        redis
            .hash_mset(RedisRoutingKeys::MODEL_TO_CHANNEL, fields_bytes)
            .await?;
    }

    Ok(entries.len())
}

/// Clear all routing tables.
pub async fn clear_routing<R>(redis: &R) -> Result<()>
where
    R: Rtdb + ?Sized,
{
    redis.del(RedisRoutingKeys::CHANNEL_TO_MODEL).await?;
    redis.del(RedisRoutingKeys::MODEL_TO_CHANNEL).await?;
    Ok(())
}

/// Clear routing entries associated with an instance.
pub async fn clear_routing_for_instance<R>(redis: &R, instance_name: &str) -> Result<usize>
where
    R: Rtdb + ?Sized,
{
    // 1. Query instance_id by name
    let name_pattern = "inst:*:name";
    let name_keys = redis.scan_match(name_pattern).await?;

    let mut instance_id: Option<u32> = None;
    for key in name_keys {
        let stored_name_bytes = redis.get(&key).await?;
        if let Some(name_bytes) = stored_name_bytes {
            let stored_name = String::from_utf8_lossy(&name_bytes);
            if stored_name == instance_name {
                // Extract ID from "inst:123:name"
                let parts: Vec<&str> = key.split(':').collect();
                if parts.len() == 3 && parts[0] == "inst" && parts[2] == "name" {
                    instance_id = parts[1].parse().ok();
                    break;
                }
            }
        }
    }

    let instance_id =
        instance_id.ok_or_else(|| anyhow::anyhow!("Instance not found: {}", instance_name))?;

    let mut removed = 0usize;

    // 2. Clear M2C routing (using instance_id format)
    let prefix_m2c = format!("{}:A:", instance_id);
    let m2c_mappings_bytes = redis
        .hash_get_all(RedisRoutingKeys::MODEL_TO_CHANNEL)
        .await?;
    let m2c_mappings: HashMap<String, String> = m2c_mappings_bytes
        .into_iter()
        .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
        .collect();

    for (mods_key, com_key) in m2c_mappings {
        if mods_key.starts_with(&prefix_m2c) {
            redis
                .hash_del(RedisRoutingKeys::MODEL_TO_CHANNEL, &mods_key)
                .await?;
            redis
                .hash_del(RedisRoutingKeys::CHANNEL_TO_MODEL, &com_key)
                .await?;
            removed += 1;
        }
    }

    // 3. Clear C2M routing (value contains instance_id)
    let prefix_c2m_value = format!("{}:M:", instance_id);
    let c2m_mappings_bytes = redis
        .hash_get_all(RedisRoutingKeys::CHANNEL_TO_MODEL)
        .await?;
    let c2m_mappings: HashMap<String, String> = c2m_mappings_bytes
        .into_iter()
        .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
        .collect();

    for (com_key, mods_key) in c2m_mappings {
        if mods_key.starts_with(&prefix_c2m_value) {
            redis
                .hash_del(RedisRoutingKeys::CHANNEL_TO_MODEL, &com_key)
                .await?;
            removed += 1;
        }
    }

    Ok(removed)
}

/// Clear routing entries associated with a channel.
pub async fn clear_routing_for_channel<R>(redis: &R, channel_id: &str) -> Result<usize>
where
    R: Rtdb + ?Sized,
{
    // New format: route keys start with channel_id directly (no "comsrv:" prefix)
    let prefix = format!("{}:", channel_id);
    let mut removed = 0usize;

    let mappings_bytes = redis
        .hash_get_all(RedisRoutingKeys::CHANNEL_TO_MODEL)
        .await?;
    let mappings: HashMap<String, String> = mappings_bytes
        .into_iter()
        .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
        .collect();
    for (com_key, mods_key) in mappings {
        if com_key.starts_with(&prefix) {
            let _ = redis
                .hash_del(RedisRoutingKeys::CHANNEL_TO_MODEL, &com_key)
                .await?;
            let _ = redis
                .hash_del(RedisRoutingKeys::MODEL_TO_CHANNEL, &mods_key)
                .await?;
            removed += 1;
        }
    }

    Ok(removed)
}

/// Retrieve routing table entries.
pub async fn get_routing<R>(
    redis: &R,
    direction: RoutingDirection,
    pattern: Option<&str>,
) -> Result<HashMap<String, String>>
where
    R: Rtdb + ?Sized,
{
    let table = direction.table();
    let mappings_bytes = redis.hash_get_all(table).await?;
    let mut mappings: HashMap<String, String> = mappings_bytes
        .into_iter()
        .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
        .collect();

    if let Some(prefix) = pattern {
        mappings.retain(|k, _| k.starts_with(prefix));
    }

    Ok(mappings)
}

/// Write product metadata to Redis, replacing the previous
/// Lua `modsrv_upsert_product` implementation.
pub async fn upsert_product<R>(redis: &R, product: &Product) -> Result<()>
where
    R: Rtdb + ?Sized,
{
    let product_key = RedisKeys::product(&product.product_name);
    let now_ms = redis.time_millis().await?;
    let product_json = serde_json::to_string(product)?;

    let fields: Vec<(String, Bytes)> = vec![
        ("definition".to_string(), Bytes::from(product_json)),
        ("updated_at".to_string(), Bytes::from(now_ms.to_string())),
    ];
    redis.hash_mset(&product_key, fields).await?;

    redis
        .sadd(RedisKeys::PRODUCT_INDEX, &product.product_name)
        .await?;

    if let Some(parent) = &product.parent_name {
        let parent_key = RedisKeys::product_children(parent);
        redis.sadd(&parent_key, &product.product_name).await?;
    }

    write_point_definitions(
        redis,
        &RedisKeys::product_measurements(&product.product_name),
        &product.measurements,
    )
    .await?;

    write_action_definitions(
        redis,
        &RedisKeys::product_actions(&product.product_name),
        &product.actions,
    )
    .await?;

    write_property_definitions(
        redis,
        &RedisKeys::product_properties(&product.product_name),
        &product.properties,
    )
    .await?;

    Ok(())
}

async fn write_point_definitions<R>(redis: &R, key: &str, points: &[MeasurementPoint]) -> Result<()>
where
    R: Rtdb + ?Sized,
{
    if points.is_empty() {
        redis.del(key).await?;
        return Ok(());
    }

    let fields: Vec<(String, Bytes)> = points
        .iter()
        .map(|point| {
            let payload = serde_json::to_string(point).with_context(|| {
                format!(
                    "Failed to serialise measurement point {}",
                    point.measurement_id
                )
            })?;
            Ok((point.measurement_id.to_string(), Bytes::from(payload)))
        })
        .collect::<Result<_>>()?;

    redis.hash_mset(key, fields).await
}

async fn write_action_definitions<R>(redis: &R, key: &str, actions: &[ActionPoint]) -> Result<()>
where
    R: Rtdb + ?Sized,
{
    if actions.is_empty() {
        redis.del(key).await?;
        return Ok(());
    }

    let fields: Vec<(String, Bytes)> = actions
        .iter()
        .map(|action| {
            let payload = serde_json::to_string(action).with_context(|| {
                format!("Failed to serialise action point {}", action.action_id)
            })?;
            Ok((action.action_id.to_string(), Bytes::from(payload)))
        })
        .collect::<Result<_>>()?;

    redis.hash_mset(key, fields).await
}

async fn write_property_definitions<R>(
    redis: &R,
    key: &str,
    properties: &[crate::product_loader::PropertyTemplate],
) -> Result<()>
where
    R: Rtdb + ?Sized,
{
    if properties.is_empty() {
        redis.del(key).await?;
        return Ok(());
    }

    let fields: Vec<(String, Bytes)> = properties
        .iter()
        .map(|prop| {
            let payload = serde_json::to_string(prop)
                .with_context(|| format!("Failed to serialise property template {}", prop.name))?;
            Ok((prop.name.clone(), Bytes::from(payload)))
        })
        .collect::<Result<_>>()?;

    redis.hash_mset(key, fields).await
}

/// Register instance metadata.
/// EN: Register instance metadata.
#[allow(clippy::too_many_arguments)]
pub async fn register_instance<R>(
    redis: &R,
    instance_id: u16,
    instance_name: &str,
    product_name: &str,
    properties: &HashMap<String, Value>,
    measurement_mappings: &HashMap<u32, String>,
    action_mappings: &HashMap<u32, String>,
    measurements: &[MeasurementPoint],
    actions: &[ActionPoint],
    parameters: Option<&HashMap<String, Value>>,
) -> Result<()>
where
    R: Rtdb + ?Sized,
{
    let now_ms = redis.time_millis().await?;

    let info_key = format!("instance:{}:info", instance_name);
    let mut info_fields = vec![
        ("id".to_string(), instance_name.to_string()),
        ("product_name".to_string(), product_name.to_string()),
        ("properties".to_string(), serde_json::to_string(properties)?),
        ("updated_at".to_string(), now_ms.to_string()),
        ("created_at".to_string(), now_ms.to_string()),
    ];

    if !measurement_mappings.is_empty() {
        info_fields.push((
            "measurement_mappings".to_string(),
            serde_json::to_string(measurement_mappings)?,
        ));
    }

    if !action_mappings.is_empty() {
        info_fields.push((
            "action_mappings".to_string(),
            serde_json::to_string(action_mappings)?,
        ));
    }

    let info_fields_bytes: Vec<(String, Bytes)> = info_fields
        .into_iter()
        .map(|(k, v)| (k, Bytes::from(v)))
        .collect();
    redis.hash_mset(&info_key, info_fields_bytes).await?;

    let status_key = RedisKeys::status(instance_id);
    let status_fields = vec![
        ("state".to_string(), Bytes::from("offline")),
        ("last_update".to_string(), Bytes::from(now_ms.to_string())),
    ];
    redis.hash_mset(&status_key, status_fields).await?;

    let config_key = RedisKeys::config(instance_id);
    let config_fields: Vec<(String, Bytes)> = properties
        .iter()
        .map(|(k, v)| (k.clone(), Bytes::from(value_to_string(v))))
        .collect();
    if !config_fields.is_empty() {
        redis.hash_mset(&config_key, config_fields).await?;
    } else {
        let _ = redis.del(&config_key).await?;
    }

    if let Some(params) = parameters {
        let param_key = RedisKeys::instance_parameters(instance_id);
        if params.is_empty() {
            let _ = redis.del(&param_key).await?;
        } else {
            let fields: Vec<(String, Bytes)> = params
                .iter()
                .map(|(k, v)| (k.clone(), Bytes::from(value_to_string(v))))
                .collect();
            redis.hash_mset(&param_key, fields).await?;
        }
    }

    write_point_definitions(
        redis,
        &RedisKeys::instance_measurement_points(instance_id),
        measurements,
    )
    .await?;

    write_action_definitions(
        redis,
        &RedisKeys::instance_action_points(instance_id),
        actions,
    )
    .await?;

    redis.sadd(RedisKeys::INSTANCE_INDEX, instance_name).await?;

    // Add inst:{id}:name key for bidirectional lookup and aggregation queries
    redis
        .set(
            &RedisKeys::instance_name(instance_id),
            Bytes::from(instance_name.to_string()),
        )
        .await?;

    // Add reverse index: inst:name:index Hash for O(1) name→ID lookup
    redis
        .hash_set(
            "inst:name:index",
            instance_name,
            Bytes::from(instance_id.to_string()),
        )
        .await?;

    Ok(())
}

/// Delete instance-related Redis data and clean up routing mappings.
/// EN: Remove Redis data related to an instance and clean up routing mappings.
pub async fn unregister_instance<R>(redis: &R, instance_id: u16, instance_name: &str) -> Result<()>
where
    R: Rtdb + ?Sized,
{
    let mut keys_to_delete = vec![
        RedisKeys::instance_info(instance_id),
        RedisKeys::instance_attributes(instance_id),
        RedisKeys::instance_parameters(instance_id),
        RedisKeys::status(instance_id),
        RedisKeys::config(instance_id),
        RedisKeys::measurement_hash(instance_id),
        RedisKeys::action_hash(instance_id),
        RedisKeys::instance_measurement_points(instance_id),
        RedisKeys::instance_action_points(instance_id),
        RedisKeys::instance_name(instance_id),
    ];

    let pattern = format!("inst:{}:*", instance_id);
    let extra = redis.scan_match(&pattern).await?;
    keys_to_delete.extend(extra);

    if !keys_to_delete.is_empty() {
        for key in &keys_to_delete {
            redis.del(key).await?;
        }
    }

    redis.srem(RedisKeys::INSTANCE_INDEX, instance_name).await?;

    // Remove from reverse index
    redis.hash_del("inst:name:index", instance_name).await?;

    cleanup_routing(redis, instance_id, instance_name).await?;

    Ok(())
}

async fn cleanup_routing<R>(redis: &R, instance_id: u16, _instance_name: &str) -> Result<()>
where
    R: Rtdb + ?Sized,
{
    // New format: route keys start with instance_id directly (no "inst:" prefix)
    let prefix = format!("{}:", instance_id);
    let m2c_bytes = redis
        .hash_get_all(RedisRoutingKeys::MODEL_TO_CHANNEL)
        .await?;
    let mut m2c: HashMap<String, String> = m2c_bytes
        .into_iter()
        .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
        .collect();
    let mut remove_c2m: Vec<String> = Vec::new();

    for (field, value) in m2c.clone() {
        if field.starts_with(&prefix) {
            redis
                .hash_del(RedisRoutingKeys::MODEL_TO_CHANNEL, &field)
                .await?;
            if !value.is_empty() {
                remove_c2m.push(value);
            }
            m2c.remove(&field);
        }
    }

    for comsrv_key in remove_c2m {
        redis
            .hash_del(RedisRoutingKeys::CHANNEL_TO_MODEL, &comsrv_key)
            .await?;
    }

    Ok(())
}

/// Write measurement data (replaces `modsrv_sync_measurement`).
/// EN: Write measurement data (replaces `modsrv_sync_measurement`).
pub async fn sync_measurement<R>(
    redis: &R,
    instance_id: u16,
    measurement: &HashMap<String, Value>,
) -> Result<()>
where
    R: Rtdb + ?Sized,
{
    let key = RedisKeys::measurement_hash(instance_id);
    let now_ms = redis.time_millis().await?;
    let mut fields: Vec<(String, Bytes)> = measurement
        .iter()
        .map(|(k, v)| (k.clone(), Bytes::from(value_to_string(v))))
        .collect();
    fields.push(("_updated_at".to_string(), Bytes::from(now_ms.to_string())));

    redis.hash_mset(&key, fields).await
}

/// Read instance real-time data (replaces `modsrv_get_instance_data`).
/// EN: Read real-time instance data (replaces `modsrv_get_instance_data`).
pub async fn get_instance_data<R>(
    redis: &R,
    instance_id: u16,
    data_type: Option<&str>,
) -> Result<Value>
where
    R: Rtdb + ?Sized,
{
    match data_type {
        Some("measurement") => {
            // Return measurement data only
            let key = RedisKeys::measurement_hash(instance_id);
            let data_bytes = redis.hash_get_all(&key).await?;
            let data: HashMap<String, String> = data_bytes
                .into_iter()
                .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
                .collect();
            let mut map = Map::new();
            for (field, value) in data {
                if !field.starts_with('_') {
                    map.insert(field, Value::String(value));
                }
            }
            Ok(Value::Object(map))
        },
        Some("action") => {
            // Return control data only
            let key = RedisKeys::action_hash(instance_id);
            let data_bytes = redis.hash_get_all(&key).await?;
            let data: HashMap<String, String> = data_bytes
                .into_iter()
                .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
                .collect();
            let mut map = Map::new();
            for (field, value) in data {
                if !field.starts_with('_') {
                    map.insert(field, Value::String(value));
                }
            }
            Ok(Value::Object(map))
        },
        None => {
            // Return both as structured data
            let m_key = RedisKeys::measurement_hash(instance_id);
            let a_key = RedisKeys::action_hash(instance_id);

            let m_data_bytes = redis.hash_get_all(&m_key).await?;
            let m_data: HashMap<String, String> = m_data_bytes
                .into_iter()
                .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
                .collect();

            let a_data_bytes = redis.hash_get_all(&a_key).await?;
            let a_data: HashMap<String, String> = a_data_bytes
                .into_iter()
                .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
                .collect();

            let mut measurements = Map::new();
            for (field, value) in m_data {
                if !field.starts_with('_') {
                    measurements.insert(field, Value::String(value));
                }
            }

            let mut actions = Map::new();
            for (field, value) in a_data {
                if !field.starts_with('_') {
                    actions.insert(field, Value::String(value));
                }
            }

            let mut result = Map::new();
            result.insert("measurements".to_string(), Value::Object(measurements));
            result.insert("actions".to_string(), Value::Object(actions));

            Ok(Value::Object(result))
        },
        Some(other) => Err(anyhow!(
            "Unknown data type '{}'; use 'measurement', 'action', or omit for both",
            other
        )),
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use voltage_rtdb::MemoryRtdb;

    /// Helper function to create MemoryRtdb for tests
    fn create_test_rtdb() -> MemoryRtdb {
        MemoryRtdb::new()
    }

    #[tokio::test]
    async fn test_store_and_get_routing() {
        let rtdb = create_test_rtdb();

        let entries = vec![RoutingEntry {
            comsrv_key: "1001:T:1".to_string(),
            modsrv_key: "modsrv:inv_01:M:1".to_string(),
            is_action: false,
        }];

        store_routing(&rtdb, &entries).await.expect("store routing");

        let all = get_routing(&rtdb, RoutingDirection::ChannelToModel, None)
            .await
            .expect("get routing");
        assert_eq!(all.len(), 1);
    }
}
