//! Redis state management helpers for ModSrv.
//!
//! Lua scripts handle only atomic routing; this module manages
//! control-plane read/write for instances/products, keeping business
//! logic and type safety directly in Rust.

use anyhow::{Result, anyhow};
use bytes::Bytes;
use common::RedisRoutingKeys;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fmt;
use voltage_model::KeySpaceConfig;
use voltage_rtdb::{Rtdb, SystemTimeProvider, TimeProvider};

use crate::product_loader::{ActionPoint, MeasurementPoint};

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

fn value_into_bytes(value: Value) -> Bytes {
    match value {
        Value::String(s) => Bytes::from(s),
        Value::Number(n) => Bytes::from(n.to_string()),
        Value::Bool(b) => Bytes::from(b.to_string()),
        Value::Null => Bytes::new(),
        other => Bytes::from(other.to_string()),
    }
}

/// Convert Redis hash data to a JSON Map, filtering out internal fields (prefixed with '_').
fn hash_to_json_map(data: HashMap<String, bytes::Bytes>) -> Map<String, Value> {
    data.into_iter()
        .filter(|(field, _)| !field.starts_with('_'))
        .map(|(field, value)| {
            (
                field,
                Value::String(String::from_utf8_lossy(&value).into_owned()),
            )
        })
        .collect()
}

/// Scan a routing hash, find entries matching a key prefix, and batch-delete
/// matching entries from both routing tables.
async fn delete_routing_by_prefix<R>(
    redis: &R,
    scan_table: &str,
    other_table: &str,
    prefix: &str,
) -> Result<usize>
where
    R: Rtdb,
{
    let entries = redis.hash_get_all(scan_table).await?;

    let mut keys_to_del: Vec<String> = Vec::new();
    let mut other_keys_to_del: Vec<String> = Vec::new();

    for (key, value_bytes) in entries {
        if key.starts_with(prefix) {
            keys_to_del.push(key);
            let value = String::from_utf8_lossy(&value_bytes).to_string();
            if !value.is_empty() {
                other_keys_to_del.push(value);
            }
        }
    }

    let removed = keys_to_del.len();

    if !keys_to_del.is_empty() {
        redis.hash_del_many(scan_table, &keys_to_del).await?;
    }
    if !other_keys_to_del.is_empty() {
        redis.hash_del_many(other_table, &other_keys_to_del).await?;
    }

    Ok(removed)
}

/// Store routing entries into Redis hashes.
pub async fn store_routing<R>(redis: &R, entries: &[RoutingEntry]) -> Result<usize>
where
    R: Rtdb,
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
    R: Rtdb,
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
    R: Rtdb,
{
    // 1. Query instance_id by name using O(1) reverse index (inst:name:index Hash)
    let keyspace = KeySpaceConfig::production_cached();
    let instance_id = match redis
        .hash_get(&keyspace.instance_name_index_key(), instance_name)
        .await?
    {
        Some(id_bytes) => {
            let id_str = String::from_utf8_lossy(&id_bytes);
            id_str
                .parse::<u32>()
                .map_err(|_| anyhow!("Invalid instance_id format in index: {}", id_str))?
        },
        None => return Err(anyhow!("Instance not found: {}", instance_name)),
    };

    // 2. Collect fields to delete from M2C routing (using instance_id format)
    let prefix_m2c = KeySpaceConfig::route_action_prefix(instance_id);
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
    let prefix_c2m_value = KeySpaceConfig::route_measurement_prefix(instance_id);
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

/// Retrieve routing table entries.
pub async fn get_routing<R>(
    redis: &R,
    direction: RoutingDirection,
    pattern: Option<&str>,
) -> Result<HashMap<String, String>>
where
    R: Rtdb,
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

/// Register instance metadata.
/// EN: Register instance metadata.
#[allow(clippy::too_many_arguments)]
pub async fn register_instance<R>(
    redis: &R,
    instance_id: u32,
    instance_name: &str,
    _product_name: &str,
    _properties: &HashMap<String, Value>,
    _measurement_mappings: &HashMap<u32, String>,
    _action_mappings: &HashMap<u32, String>,
    measurements: &[MeasurementPoint],
    actions: &[ActionPoint],
    _parameters: Option<&HashMap<String, Value>>,
) -> Result<()>
where
    R: Rtdb,
{
    let keyspace = KeySpaceConfig::production_cached();

    // ========================================================================
    // Redis = Real-time data only (SQLite = Single source of truth for config)
    // ========================================================================
    //
    // M Hash: Pre-initialized with all measurement points set to "0"
    // A Hash: Pre-initialized with all action points set to "0" (consistent with M)
    // P (Properties): Cached in memory, not in Redis (config data, not real-time)
    // ========================================================================

    // 1. Initialize inst:{id}:M Hash with all measurement points set to 0
    //    Sidecar inst:{id}:M:ts is pre-seeded with 0 so apigateway WebSocket
    //    never returns an empty `ts` map for an active instance (parity with
    //    the comsrv:{ch}:{T|S}:ts convention).
    let m_key = keyspace.instance_measurement_key(instance_id);
    let m_ts_key = keyspace.instance_measurement_ts_key(instance_id);
    for point in measurements {
        let field = point.measurement_id.to_string();
        redis.hash_set(&m_key, &field, Bytes::from("0")).await?;
        redis.hash_set(&m_ts_key, &field, Bytes::from("0")).await?;
    }

    // 2. Initialize inst:{id}:A Hash with all action points set to 0
    //    Consistent with M points: pre-initialize at startup for queries and
    //    M2C routing validation. Sidecar :A:ts follows the same convention.
    let a_key = keyspace.instance_action_key(instance_id);
    let a_ts_key = keyspace.instance_action_ts_key(instance_id);
    for action in actions {
        let field = action.action_id.to_string();
        redis.hash_set(&a_key, &field, Bytes::from("0")).await?;
        redis.hash_set(&a_ts_key, &field, Bytes::from("0")).await?;
    }

    // 3. Set inst:{id}:name for bidirectional lookup and aggregation queries
    redis
        .set(
            &keyspace.instance_name_key(instance_id),
            Bytes::from(instance_name.to_string()),
        )
        .await?;

    // 6. Add reverse index: inst:name:index Hash for O(1) name→ID lookup
    redis
        .hash_set(
            &keyspace.instance_name_index_key(),
            instance_name,
            Bytes::from(instance_id.to_string()),
        )
        .await?;

    Ok(())
}

/// Delete instance-related Redis data and clean up routing mappings.
/// EN: Remove Redis data related to an instance and clean up routing mappings.
pub async fn unregister_instance<R>(redis: &R, instance_id: u32, instance_name: &str) -> Result<()>
where
    R: Rtdb,
{
    let keyspace = KeySpaceConfig::production_cached();

    // Delete real-time data keys (Redis = real-time data only)
    let keys_to_delete = vec![
        keyspace.instance_measurement_key(instance_id), // inst:{id}:M
        keyspace.instance_measurement_ts_key(instance_id), // inst:{id}:M:ts
        keyspace.instance_action_key(instance_id),      // inst:{id}:A
        keyspace.instance_action_ts_key(instance_id),   // inst:{id}:A:ts
        keyspace.instance_name_key(instance_id),        // inst:{id}:name
    ];

    for key in &keys_to_delete {
        redis.del(key).await?;
    }

    // Safety: SCAN and delete any remaining inst:{id}:* keys (for cleanup)
    let pattern = keyspace.instance_pattern(instance_id);
    let extra_keys = redis.scan_match(&pattern).await?;
    for key in &extra_keys {
        redis.del(key).await?;
    }

    // Remove from reverse index: inst:name:index
    redis
        .hash_del(&keyspace.instance_name_index_key(), instance_name)
        .await?;

    // Clean up routing mappings (route:c2m and route:m2c)
    cleanup_routing(redis, instance_id, instance_name).await?;

    Ok(())
}

/// Rename an instance in Redis
///
/// Updates the reverse index (inst:name:index) and the name key (inst:{id}:name).
/// Routing keys (route:c2m, route:m2c) use instance_id, so they don't need updates.
pub async fn rename_instance_in_redis<R>(
    redis: &R,
    instance_id: u32,
    old_name: &str,
    new_name: &str,
) -> Result<()>
where
    R: Rtdb,
{
    let keyspace = KeySpaceConfig::production_cached();

    // 1. Remove old name from reverse index
    redis
        .hash_del(&keyspace.instance_name_index_key(), old_name)
        .await?;

    // 2. Add new name to reverse index
    redis
        .hash_set(
            &keyspace.instance_name_index_key(),
            new_name,
            Bytes::from(instance_id.to_string()),
        )
        .await?;

    // 3. Update inst:{id}:name
    redis
        .set(
            &keyspace.instance_name_key(instance_id),
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
async fn cleanup_routing<R>(redis: &R, instance_id: u32, _instance_name: &str) -> Result<()>
where
    R: Rtdb,
{
    let prefix = format!("{}:", instance_id);
    delete_routing_by_prefix(
        redis,
        RedisRoutingKeys::MODEL_TO_CHANNEL,
        RedisRoutingKeys::CHANNEL_TO_MODEL,
        &prefix,
    )
    .await?;
    Ok(())
}

/// Write measurement data (replaces `modsrv_sync_measurement`).
/// EN: Write measurement data (replaces `modsrv_sync_measurement`).
pub async fn sync_measurement<R>(
    redis: &R,
    instance_id: u32,
    measurement: HashMap<String, Value>,
) -> Result<()>
where
    R: Rtdb,
{
    let keyspace = KeySpaceConfig::production_cached();
    let key = keyspace.instance_measurement_key(instance_id);
    let now_ms = SystemTimeProvider.now_millis();
    // Use into_iter() to consume ownership and avoid cloning keys
    let mut fields: Vec<(String, Bytes)> = measurement
        .into_iter()
        .map(|(k, v)| (k, value_into_bytes(v)))
        .collect();
    fields.push(("_updated_at".to_string(), Bytes::from(now_ms.to_string())));

    redis.hash_mset(&key, fields).await
}

/// Read instance real-time data (replaces `modsrv_get_instance_data`).
/// EN: Read real-time instance data (replaces `modsrv_get_instance_data`).
pub async fn get_instance_data<R>(
    redis: &R,
    instance_id: u32,
    data_type: Option<&str>,
) -> Result<Value>
where
    R: Rtdb,
{
    let keyspace = KeySpaceConfig::production_cached();

    match data_type {
        Some("measurement") => {
            let data = redis
                .hash_get_all(&keyspace.instance_measurement_key(instance_id))
                .await?;
            Ok(Value::Object(hash_to_json_map(data)))
        },
        Some("action") => {
            let data = redis
                .hash_get_all(&keyspace.instance_action_key(instance_id))
                .await?;
            Ok(Value::Object(hash_to_json_map(data)))
        },
        None => {
            let m_data = redis
                .hash_get_all(&keyspace.instance_measurement_key(instance_id))
                .await?;
            let a_data = redis
                .hash_get_all(&keyspace.instance_action_key(instance_id))
                .await?;
            let mut result = Map::new();
            result.insert(
                "measurements".to_string(),
                Value::Object(hash_to_json_map(m_data)),
            );
            result.insert(
                "actions".to_string(),
                Value::Object(hash_to_json_map(a_data)),
            );
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
        let keyspace = KeySpaceConfig::production();
        rtdb.hash_set(
            &keyspace.instance_name_index_key(),
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Instance not found")
        );
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

    // ========== register_instance tests ==========

    #[tokio::test]
    async fn test_register_instance_initializes_m_and_a_hashes() {
        use crate::config::{ActionPoint, MeasurementPoint};
        use std::collections::HashMap;

        let rtdb = create_test_rtdb();

        // Create test measurement points
        let measurements = vec![
            MeasurementPoint {
                measurement_id: 1,
                name: "voltage".to_string(),
                unit: Some("V".to_string()),
                description: None,
            },
            MeasurementPoint {
                measurement_id: 2,
                name: "current".to_string(),
                unit: Some("A".to_string()),
                description: None,
            },
        ];

        // Create test action points
        let actions = vec![
            ActionPoint {
                action_id: 1,
                name: "set_power".to_string(),
                unit: Some("kW".to_string()),
                description: None,
            },
            ActionPoint {
                action_id: 2,
                name: "set_voltage".to_string(),
                unit: Some("V".to_string()),
                description: None,
            },
            ActionPoint {
                action_id: 3,
                name: "enable".to_string(),
                unit: None,
                description: None,
            },
        ];

        // Register instance
        let instance_id = 42;
        let instance_name = "test_instance";
        register_instance(
            &rtdb,
            instance_id,
            instance_name,
            "TestProduct",
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &measurements,
            &actions,
            None,
        )
        .await
        .expect("register_instance should succeed");

        // Verify inst:{id}:M Hash was initialized
        let keyspace = KeySpaceConfig::production_cached();
        let m_key = keyspace.instance_measurement_key(instance_id);
        let m_data = rtdb.hash_get_all(&m_key).await.unwrap();
        assert_eq!(m_data.len(), 2, "M Hash should have 2 measurement points");
        assert!(m_data.contains_key("1"), "M Hash should contain point 1");
        assert!(m_data.contains_key("2"), "M Hash should contain point 2");

        // Verify inst:{id}:A Hash was initialized (THIS IS THE NEW BEHAVIOR)
        let a_key = keyspace.instance_action_key(instance_id);
        let a_data = rtdb.hash_get_all(&a_key).await.unwrap();
        assert_eq!(a_data.len(), 3, "A Hash should have 3 action points");
        assert!(a_data.contains_key("1"), "A Hash should contain point 1");
        assert!(a_data.contains_key("2"), "A Hash should contain point 2");
        assert!(a_data.contains_key("3"), "A Hash should contain point 3");

        // Verify all values are initialized to "0"
        for (key, value) in &a_data {
            assert_eq!(
                value.as_ref(),
                b"0",
                "A Hash point {} should be initialized to '0'",
                key
            );
        }

        // Verify sidecar :ts hashes are pre-seeded so the apigateway WebSocket
        // never sees an empty `ts` map on first subscribe.
        let m_ts_key = keyspace.instance_measurement_ts_key(instance_id);
        let m_ts_data = rtdb.hash_get_all(&m_ts_key).await.unwrap();
        assert_eq!(m_ts_data.len(), 2, "M:ts sidecar should mirror M fields");
        assert!(m_ts_data.contains_key("1"));
        assert!(m_ts_data.contains_key("2"));

        let a_ts_key = keyspace.instance_action_ts_key(instance_id);
        let a_ts_data = rtdb.hash_get_all(&a_ts_key).await.unwrap();
        assert_eq!(a_ts_data.len(), 3, "A:ts sidecar should mirror A fields");
        assert!(a_ts_data.contains_key("1"));
        assert!(a_ts_data.contains_key("2"));
        assert!(a_ts_data.contains_key("3"));

        // Verify inst:{id}:name was set
        let name_key = keyspace.instance_name_key(instance_id);
        let name_value = rtdb.get(&name_key).await.unwrap();
        assert!(name_value.is_some(), "inst:{{id}}:name should be set");
        assert_eq!(name_value.unwrap().as_ref(), instance_name.as_bytes());

        // Verify inst:name:index was set
        let index_key = keyspace.instance_name_index_key();
        let index_value = rtdb.hash_get(&index_key, instance_name).await.unwrap();
        assert!(
            index_value.is_some(),
            "inst:name:index should contain entry"
        );
        assert_eq!(
            index_value.unwrap().as_ref(),
            instance_id.to_string().as_bytes()
        );
    }

    #[tokio::test]
    async fn test_register_instance_with_empty_actions() {
        use crate::config::{ActionPoint, MeasurementPoint};
        use std::collections::HashMap;

        let rtdb = create_test_rtdb();

        let measurements = vec![MeasurementPoint {
            measurement_id: 1,
            name: "power".to_string(),
            unit: Some("kW".to_string()),
            description: None,
        }];

        // Empty actions (like Load device)
        let actions: Vec<ActionPoint> = vec![];

        let instance_id = 99;
        register_instance(
            &rtdb,
            instance_id,
            "load_device",
            "Load",
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &measurements,
            &actions,
            None,
        )
        .await
        .expect("register_instance should succeed");

        // M Hash should have 1 point
        let keyspace = KeySpaceConfig::production_cached();
        let m_key = keyspace.instance_measurement_key(instance_id);
        let m_data = rtdb.hash_get_all(&m_key).await.unwrap();
        assert_eq!(m_data.len(), 1);

        // A Hash should be empty (but the loop still runs, just 0 iterations)
        let a_key = keyspace.instance_action_key(instance_id);
        let a_data = rtdb.hash_get_all(&a_key).await.unwrap();
        assert_eq!(
            a_data.len(),
            0,
            "A Hash should be empty for devices with no actions"
        );
    }
}
