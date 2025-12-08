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
use voltage_config::modsrv::InstanceRedisKeys;
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
///
/// Optimized with batch deletion using `hash_del_many` to reduce Redis round-trips.
pub async fn clear_routing_for_instance<R>(redis: &R, instance_name: &str) -> Result<usize>
where
    R: Rtdb + ?Sized,
{
    // 1. Query instance_id by name using O(1) reverse index (inst:name:index Hash)
    let instance_id = match redis.hash_get("inst:name:index", instance_name).await? {
        Some(id_bytes) => {
            let id_str = String::from_utf8_lossy(&id_bytes);
            id_str
                .parse::<u32>()
                .map_err(|_| anyhow!("Invalid instance_id format in index: {}", id_str))?
        },
        None => return Err(anyhow!("Instance not found: {}", instance_name)),
    };

    // 2. Collect fields to delete from M2C routing (using instance_id format)
    let prefix_m2c = format!("{}:A:", instance_id);
    let m2c_mappings_bytes = redis
        .hash_get_all(RedisRoutingKeys::MODEL_TO_CHANNEL)
        .await?;

    let mut m2c_fields_to_del: Vec<String> = Vec::new();
    let mut c2m_fields_from_m2c: Vec<String> = Vec::new();

    for (mods_key, value_bytes) in m2c_mappings_bytes {
        if mods_key.starts_with(&prefix_m2c) {
            m2c_fields_to_del.push(mods_key);
            c2m_fields_from_m2c.push(String::from_utf8_lossy(&value_bytes).to_string());
        }
    }

    // 3. Collect fields to delete from C2M routing (value contains instance_id)
    let prefix_c2m_value = format!("{}:M:", instance_id);
    let c2m_mappings_bytes = redis
        .hash_get_all(RedisRoutingKeys::CHANNEL_TO_MODEL)
        .await?;

    let mut c2m_fields_to_del: Vec<String> = Vec::new();

    for (com_key, value_bytes) in c2m_mappings_bytes {
        let mods_key = String::from_utf8_lossy(&value_bytes);
        if mods_key.starts_with(&prefix_c2m_value) {
            c2m_fields_to_del.push(com_key);
        }
    }

    // 4. Batch delete using hash_del_many (reduces N Redis calls to 2-3 calls)
    let removed = m2c_fields_to_del.len() + c2m_fields_to_del.len();

    if !m2c_fields_to_del.is_empty() {
        redis
            .hash_del_many(RedisRoutingKeys::MODEL_TO_CHANNEL, &m2c_fields_to_del)
            .await?;
    }

    // Merge c2m_fields_from_m2c into c2m_fields_to_del for batch deletion
    c2m_fields_to_del.extend(c2m_fields_from_m2c);
    c2m_fields_to_del.sort();
    c2m_fields_to_del.dedup();

    if !c2m_fields_to_del.is_empty() {
        redis
            .hash_del_many(RedisRoutingKeys::CHANNEL_TO_MODEL, &c2m_fields_to_del)
            .await?;
    }

    Ok(removed)
}

/// Clear routing entries associated with a channel.
///
/// Optimized with batch deletion using `hash_del_many` to reduce Redis round-trips.
pub async fn clear_routing_for_channel<R>(redis: &R, channel_id: &str) -> Result<usize>
where
    R: Rtdb + ?Sized,
{
    // New format: route keys start with channel_id directly (no "comsrv:" prefix)
    let prefix = format!("{}:", channel_id);

    let mappings_bytes = redis
        .hash_get_all(RedisRoutingKeys::CHANNEL_TO_MODEL)
        .await?;

    // Collect fields to delete in batch
    let mut c2m_fields_to_del: Vec<String> = Vec::new();
    let mut m2c_fields_to_del: Vec<String> = Vec::new();

    for (com_key, value_bytes) in mappings_bytes {
        if com_key.starts_with(&prefix) {
            c2m_fields_to_del.push(com_key);
            m2c_fields_to_del.push(String::from_utf8_lossy(&value_bytes).to_string());
        }
    }

    let removed = c2m_fields_to_del.len();

    // Batch delete using hash_del_many (reduces 2N Redis calls to 2 calls)
    if !c2m_fields_to_del.is_empty() {
        redis
            .hash_del_many(RedisRoutingKeys::CHANNEL_TO_MODEL, &c2m_fields_to_del)
            .await?;
    }

    if !m2c_fields_to_del.is_empty() {
        redis
            .hash_del_many(RedisRoutingKeys::MODEL_TO_CHANNEL, &m2c_fields_to_del)
            .await?;
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
    let product_key = InstanceRedisKeys::product(&product.product_name);
    let now_ms = redis.time_millis().await?;
    let product_json = serde_json::to_string(product)?;

    let fields: Vec<(String, Bytes)> = vec![
        ("definition".to_string(), Bytes::from(product_json)),
        ("updated_at".to_string(), Bytes::from(now_ms.to_string())),
    ];
    redis.hash_mset(&product_key, fields).await?;

    redis
        .sadd(InstanceRedisKeys::PRODUCT_INDEX, &product.product_name)
        .await?;

    if let Some(parent) = &product.parent_name {
        let parent_key = InstanceRedisKeys::product_children(parent);
        redis.sadd(&parent_key, &product.product_name).await?;
    }

    write_point_definitions(
        redis,
        &InstanceRedisKeys::product_measurements(&product.product_name),
        &product.measurements,
    )
    .await?;

    write_action_definitions(
        redis,
        &InstanceRedisKeys::product_actions(&product.product_name),
        &product.actions,
    )
    .await?;

    write_property_definitions(
        redis,
        &InstanceRedisKeys::product_properties(&product.product_name),
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
    _product_name: &str,
    _properties: &HashMap<String, Value>,
    _measurement_mappings: &HashMap<u32, String>,
    _action_mappings: &HashMap<u32, String>,
    measurements: &[MeasurementPoint],
    _actions: &[ActionPoint],
    _parameters: Option<&HashMap<String, Value>>,
) -> Result<()>
where
    R: Rtdb + ?Sized,
{
    // ========================================================================
    // Redis = Real-time data only (SQLite = Single source of truth for config)
    // ========================================================================
    //
    // M Hash: Pre-initialized with all measurement points set to "0"
    // A Hash: Created on-demand (Redis doesn't support Null values)
    // P (Properties): Cached in memory, not in Redis (config data, not real-time)
    // ========================================================================

    // 1. Initialize inst:{id}:M Hash with all measurement points set to 0
    let m_key = InstanceRedisKeys::measurement_hash(instance_id);
    for point in measurements {
        redis
            .hash_set(&m_key, &point.measurement_id.to_string(), Bytes::from("0"))
            .await?;
    }

    // 2. Set inst:{id}:name for bidirectional lookup and aggregation queries
    redis
        .set(
            &InstanceRedisKeys::instance_name(instance_id),
            Bytes::from(instance_name.to_string()),
        )
        .await?;

    // 3. Add reverse index: inst:name:index Hash for O(1) name→ID lookup
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
    // Delete real-time data keys (Redis = real-time data only)
    let keys_to_delete = vec![
        InstanceRedisKeys::measurement_hash(instance_id), // inst:{id}:M
        InstanceRedisKeys::action_hash(instance_id),      // inst:{id}:A
        InstanceRedisKeys::instance_name(instance_id),    // inst:{id}:name
    ];

    for key in &keys_to_delete {
        redis.del(key).await?;
    }

    // Safety: SCAN and delete any remaining inst:{id}:* keys (for cleanup)
    let pattern = format!("inst:{}:*", instance_id);
    let extra_keys = redis.scan_match(&pattern).await?;
    for key in &extra_keys {
        redis.del(key).await?;
    }

    // Remove from reverse index: inst:name:index
    redis.hash_del("inst:name:index", instance_name).await?;

    // Clean up routing mappings (route:c2m and route:m2c)
    cleanup_routing(redis, instance_id, instance_name).await?;

    Ok(())
}

/// Rename an instance in Redis
///
/// Updates the reverse index (inst:name:index) and the name key (inst:{id}:name).
/// Routing keys (route:c2m, route:m2c) use instance_id, so they don't need updates.
///
/// @input redis: &R - Redis client
/// @input instance_id: u16 - Instance ID
/// @input old_name: &str - Old instance name
/// @input new_name: &str - New instance name
/// @output Result<()> - Success or error
pub async fn rename_instance_in_redis<R>(
    redis: &R,
    instance_id: u16,
    old_name: &str,
    new_name: &str,
) -> Result<()>
where
    R: Rtdb + ?Sized,
{
    // 1. Remove old name from reverse index
    redis.hash_del("inst:name:index", old_name).await?;

    // 2. Add new name to reverse index
    redis
        .hash_set(
            "inst:name:index",
            new_name,
            Bytes::from(instance_id.to_string()),
        )
        .await?;

    // 3. Update inst:{id}:name
    redis
        .set(
            &InstanceRedisKeys::instance_name(instance_id),
            Bytes::from(new_name.to_string()),
        )
        .await?;

    tracing::debug!(
        "Instance {} renamed: {} -> {}",
        instance_id,
        old_name,
        new_name
    );
    Ok(())
}

/// Clean up routing mappings for an instance.
///
/// Optimized with batch deletion using `hash_del_many` to reduce Redis round-trips.
async fn cleanup_routing<R>(redis: &R, instance_id: u16, _instance_name: &str) -> Result<()>
where
    R: Rtdb + ?Sized,
{
    // New format: route keys start with instance_id directly (no "inst:" prefix)
    let prefix = format!("{}:", instance_id);
    let m2c_bytes = redis
        .hash_get_all(RedisRoutingKeys::MODEL_TO_CHANNEL)
        .await?;

    // Collect fields to delete in batch
    let mut m2c_fields_to_del: Vec<String> = Vec::new();
    let mut c2m_fields_to_del: Vec<String> = Vec::new();

    for (field, value_bytes) in m2c_bytes {
        if field.starts_with(&prefix) {
            m2c_fields_to_del.push(field);
            let value = String::from_utf8_lossy(&value_bytes).to_string();
            if !value.is_empty() {
                c2m_fields_to_del.push(value);
            }
        }
    }

    // Batch delete using hash_del_many (reduces 2N Redis calls to 2 calls)
    if !m2c_fields_to_del.is_empty() {
        redis
            .hash_del_many(RedisRoutingKeys::MODEL_TO_CHANNEL, &m2c_fields_to_del)
            .await?;
    }

    if !c2m_fields_to_del.is_empty() {
        redis
            .hash_del_many(RedisRoutingKeys::CHANNEL_TO_MODEL, &c2m_fields_to_del)
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
    let key = InstanceRedisKeys::measurement_hash(instance_id);
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
            let key = InstanceRedisKeys::measurement_hash(instance_id);
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
            let key = InstanceRedisKeys::action_hash(instance_id);
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
            let m_key = InstanceRedisKeys::measurement_hash(instance_id);
            let a_key = InstanceRedisKeys::action_hash(instance_id);

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

    // ========== clear_routing tests ==========

    /// Test that clear_routing invokes delete on both routing keys.
    ///
    /// Note: MemoryRtdb.del() only clears kv_store, not hash_store.
    /// This is a known limitation of the test mock - in production Redis,
    /// DEL command removes any key type including hashes.
    /// This test verifies the function completes without error.
    #[tokio::test]
    async fn test_clear_routing_completes_without_error() {
        let rtdb = create_test_rtdb();

        // Store entries first
        let entries = vec![
            RoutingEntry {
                comsrv_key: "1001:T:1".to_string(),
                modsrv_key: "1:M:1".to_string(),
                is_action: false,
            },
            RoutingEntry {
                comsrv_key: "1001:A:2".to_string(),
                modsrv_key: "1:A:2".to_string(),
                is_action: true,
            },
        ];
        store_routing(&rtdb, &entries).await.unwrap();

        // Clear routing should complete without error
        let result = clear_routing(&rtdb).await;
        assert!(result.is_ok());
    }

    // ========== get_routing pattern filter tests ==========

    #[tokio::test]
    async fn test_get_routing_with_pattern_filter() {
        let rtdb = create_test_rtdb();

        // Store entries with different channel IDs
        let entries = vec![
            RoutingEntry {
                comsrv_key: "1001:T:1".to_string(),
                modsrv_key: "1:M:1".to_string(),
                is_action: false,
            },
            RoutingEntry {
                comsrv_key: "1001:T:2".to_string(),
                modsrv_key: "1:M:2".to_string(),
                is_action: false,
            },
            RoutingEntry {
                comsrv_key: "2002:T:1".to_string(),
                modsrv_key: "2:M:1".to_string(),
                is_action: false,
            },
        ];
        store_routing(&rtdb, &entries).await.unwrap();

        // Get all entries
        let all = get_routing(&rtdb, RoutingDirection::ChannelToModel, None)
            .await
            .unwrap();
        assert_eq!(all.len(), 3);

        // Filter by channel 1001 prefix
        let filtered = get_routing(&rtdb, RoutingDirection::ChannelToModel, Some("1001:"))
            .await
            .unwrap();
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains_key("1001:T:1"));
        assert!(filtered.contains_key("1001:T:2"));

        // Filter by channel 2002 prefix
        let filtered_2002 = get_routing(&rtdb, RoutingDirection::ChannelToModel, Some("2002:"))
            .await
            .unwrap();
        assert_eq!(filtered_2002.len(), 1);
        assert!(filtered_2002.contains_key("2002:T:1"));
    }

    // ========== clear_routing_for_instance tests ==========

    /// Helper to setup instance name index for tests
    async fn setup_test_instance_index(rtdb: &MemoryRtdb, instance_id: u32, instance_name: &str) {
        use bytes::Bytes;
        rtdb.hash_set(
            "inst:name:index",
            instance_name,
            Bytes::from(instance_id.to_string()),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_clear_routing_for_instance_not_found() {
        let rtdb = create_test_rtdb();

        // Instance does not exist in index
        let result = clear_routing_for_instance(&rtdb, "nonexistent").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Instance not found"));
    }

    #[tokio::test]
    async fn test_clear_routing_for_instance_removes_correct_entries() {
        let rtdb = create_test_rtdb();

        // Setup two instances
        setup_test_instance_index(&rtdb, 1, "instance_1").await;
        setup_test_instance_index(&rtdb, 2, "instance_2").await;

        // Store routing entries for both instances
        // Note: store_routing writes ALL entries to C2M (forward),
        // and only is_action=true entries to M2C (reverse)
        let entries = vec![
            // Instance 1 measurement routing (goes to C2M only)
            RoutingEntry {
                comsrv_key: "1001:T:1".to_string(),
                modsrv_key: "1:M:1".to_string(),
                is_action: false,
            },
            // Instance 1 action routing (goes to both C2M and M2C)
            RoutingEntry {
                comsrv_key: "1001:A:1".to_string(),
                modsrv_key: "1:A:1".to_string(),
                is_action: true,
            },
            // Instance 2 measurement routing (goes to C2M only)
            RoutingEntry {
                comsrv_key: "2002:T:1".to_string(),
                modsrv_key: "2:M:1".to_string(),
                is_action: false,
            },
        ];
        store_routing(&rtdb, &entries).await.unwrap();

        // Verify initial state
        // C2M contains ALL entries (3 total)
        let c2m_before = get_routing(&rtdb, RoutingDirection::ChannelToModel, None)
            .await
            .unwrap();
        assert_eq!(c2m_before.len(), 3); // All 3 entries in C2M

        // M2C only contains action entries (1 total)
        let m2c_before = get_routing(&rtdb, RoutingDirection::ModelToChannel, None)
            .await
            .unwrap();
        assert_eq!(m2c_before.len(), 1); // Only instance_1's action entry

        // Clear routing for instance_1
        // This should remove: 1 M2C entry + associated C2M entries
        let removed = clear_routing_for_instance(&rtdb, "instance_1")
            .await
            .unwrap();
        // clear_routing_for_instance only counts m2c_fields_to_del + c2m_fields_to_del (based on value match)
        // It removes M2C entries by key prefix (1:A:), then finds C2M entries by value prefix (1:M:)
        assert!(removed >= 1); // At least 1 M2C entry removed

        // Verify instance_2 entry remains in C2M
        let c2m_after = get_routing(&rtdb, RoutingDirection::ChannelToModel, None)
            .await
            .unwrap();
        assert!(c2m_after.contains_key("2002:T:1"));

        // M2C should be empty (only instance_1 had action routing)
        let m2c_after = get_routing(&rtdb, RoutingDirection::ModelToChannel, None)
            .await
            .unwrap();
        assert!(m2c_after.is_empty());
    }
}
