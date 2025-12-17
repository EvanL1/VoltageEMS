//! Redis KeySpace Configuration
//!
//! This module provides the `KeySpaceConfig` struct for generating Redis keys
//! in a consistent and type-safe manner across all VoltageEMS services.

use crate::PointType;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// Keyspace configuration for Redis operations
///
/// **Design Principles:**
/// - Configuration as Data
/// - Test isolation (dedicated keyspace)
/// - Multi-environment support (dev/test/prod)
/// - Single Source of Truth for key naming
///
/// **Usage Example:**
/// ```
/// use voltage_model::{KeySpaceConfig, PointType};
///
/// // Production environment
/// let prod_config = KeySpaceConfig::production();
///
/// // Test environment (fully isolated keyspace)
/// let test_config = KeySpaceConfig::test();
///
/// // M2C routing configuration
/// let m2c_config = prod_config.for_m2c();
///
/// // Key generation (type-safe)
/// let key = prod_config.channel_key(1001, PointType::Telemetry);
/// // => "comsrv:1001:T"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeySpaceConfig {
    /// Data storage key prefix (e.g., "comsrv" or "test:comsrv")
    pub data_prefix: String,

    /// Instance key prefix (e.g., "inst" or "test:inst")
    pub inst_prefix: String,

    /// Routing table key (e.g., "route:c2m" or "test:route:c2m")
    pub routing_table: String,

    /// Target key prefix (M2C only, e.g., "comsrv")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_prefix: Option<String>,

    /// Instance name lookup pattern (M2C only, e.g., "inst:*:name")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inst_name_pattern: Option<String>,
}

impl Default for KeySpaceConfig {
    fn default() -> Self {
        Self::production()
    }
}

impl KeySpaceConfig {
    /// Production environment configuration
    ///
    /// Uses standard keyspace naming:
    /// - data_prefix: "comsrv"
    /// - inst_prefix: "inst"
    /// - routing_table: "route:c2m"
    pub fn production() -> Self {
        Self {
            data_prefix: "comsrv".to_string(),
            inst_prefix: "inst".to_string(),
            routing_table: "route:c2m".to_string(),
            target_prefix: None,
            inst_name_pattern: None,
        }
    }

    /// Test environment configuration (fully isolated keyspace)
    ///
    /// Adds a "test:" prefix to all keys to prevent test data from polluting production.
    ///
    /// Example:
    /// ```
    /// use voltage_model::KeySpaceConfig;
    ///
    /// let test_config = KeySpaceConfig::test();
    /// // data_prefix: "test:comsrv"
    /// // routing_table: "test:route:c2m"
    /// ```
    pub fn test() -> Self {
        Self {
            data_prefix: "test:comsrv".to_string(),
            inst_prefix: "test:inst".to_string(),
            routing_table: "test:route:c2m".to_string(),
            target_prefix: Some("test:comsrv".to_string()),
            inst_name_pattern: Some("test:inst:*:name".to_string()),
        }
    }

    /// M2C (Model to Channel) routing configuration
    ///
    /// Used by modsrv.set_action_point to route model actions to channels.
    ///
    /// Auto settings:
    /// - target_prefix: points to comsrv data keys
    /// - inst_name_pattern: instance name lookup pattern
    /// - routing_table: switch to m2c routing table
    ///
    /// Example:
    /// ```
    /// use voltage_model::KeySpaceConfig;
    ///
    /// let prod_config = KeySpaceConfig::production();
    /// let m2c_config = prod_config.for_m2c();
    /// // routing_table: "route:m2c"
    /// // target_prefix: Some("comsrv")
    /// // inst_name_pattern: Some("inst:*:name")
    /// ```
    pub fn for_m2c(&self) -> Self {
        let target_prefix = self.data_prefix.clone();
        let inst_name_pattern = format!("{}:*:name", self.inst_prefix);
        let routing_table = if self.routing_table.contains("test:") {
            "test:route:m2c".to_string()
        } else {
            "route:m2c".to_string()
        };

        Self {
            data_prefix: self.inst_prefix.clone(), // Not used in M2C
            inst_prefix: self.inst_prefix.clone(),
            routing_table,
            target_prefix: Some(target_prefix),
            inst_name_pattern: Some(inst_name_pattern),
        }
    }

    // ============================================================
    // Redis key generation methods (Single Source of Truth)
    // ============================================================

    /// Build channel data key: comsrv:{channel_id}:{type}
    ///
    /// # Examples
    /// ```
    /// use voltage_model::{KeySpaceConfig, PointType};
    ///
    /// let config = KeySpaceConfig::production();
    /// assert_eq!(config.channel_key(1001, PointType::Telemetry).as_ref(), "comsrv:1001:T");
    /// ```
    pub fn channel_key(&self, channel_id: u32, point_type: PointType) -> Cow<'static, str> {
        Cow::Owned(format!(
            "{}:{}:{}",
            self.data_prefix,
            channel_id,
            point_type.as_str()
        ))
    }

    /// Build channel timestamp key: comsrv:{channel_id}:{type}:ts
    pub fn channel_ts_key(&self, channel_id: u32, point_type: PointType) -> Cow<'static, str> {
        Cow::Owned(format!(
            "{}:{}:{}:ts",
            self.data_prefix,
            channel_id,
            point_type.as_str()
        ))
    }

    /// Build channel raw value key: comsrv:{channel_id}:{type}:raw
    pub fn channel_raw_key(&self, channel_id: u32, point_type: PointType) -> Cow<'static, str> {
        Cow::Owned(format!(
            "{}:{}:{}:raw",
            self.data_prefix,
            channel_id,
            point_type.as_str()
        ))
    }

    /// Build TODO queue key: comsrv:{channel_id}:{type}:TODO
    pub fn todo_queue_key(&self, channel_id: u32, point_type: PointType) -> Cow<'static, str> {
        let target = self.target_prefix.as_ref().unwrap_or(&self.data_prefix);
        Cow::Owned(format!(
            "{}:{}:{}:TODO",
            target,
            channel_id,
            point_type.as_str()
        ))
    }

    /// Build instance measurement key: inst:{instance_id}:M
    ///
    /// # Examples
    /// ```
    /// use voltage_model::KeySpaceConfig;
    ///
    /// let config = KeySpaceConfig::production();
    /// assert_eq!(config.instance_measurement_key(1).as_ref(), "inst:1:M");
    /// ```
    pub fn instance_measurement_key(&self, instance_id: u32) -> Cow<'static, str> {
        Cow::Owned(format!("{}:{}:M", self.inst_prefix, instance_id))
    }

    /// Build instance action key: inst:{instance_id}:A
    pub fn instance_action_key(&self, instance_id: u32) -> Cow<'static, str> {
        Cow::Owned(format!("{}:{}:A", self.inst_prefix, instance_id))
    }

    /// Build instance name key: inst:{instance_id}:name
    pub fn instance_name_key(&self, instance_id: u32) -> Cow<'static, str> {
        Cow::Owned(format!("{}:{}:name", self.inst_prefix, instance_id))
    }

    /// Build instance status key: inst:{instance_id}:status
    pub fn instance_status_key(&self, instance_id: u32) -> Cow<'static, str> {
        Cow::Owned(format!("{}:{}:status", self.inst_prefix, instance_id))
    }

    /// Build instance config key: inst:{instance_id}:config
    pub fn instance_config_key(&self, instance_id: u32) -> Cow<'static, str> {
        Cow::Owned(format!("{}:{}:config", self.inst_prefix, instance_id))
    }

    /// Build instance measurement points key: inst:{instance_id}:measurement_points
    pub fn instance_measurement_points_key(&self, instance_id: u32) -> Cow<'static, str> {
        Cow::Owned(format!(
            "{}:{}:measurement_points",
            self.inst_prefix, instance_id
        ))
    }

    /// Build instance action points key: inst:{instance_id}:action_points
    pub fn instance_action_points_key(&self, instance_id: u32) -> Cow<'static, str> {
        Cow::Owned(format!(
            "{}:{}:action_points",
            self.inst_prefix, instance_id
        ))
    }

    /// Build instance measurement point key: inst:{instance_id}:M:{point_id}
    ///
    /// # Examples
    /// ```
    /// use voltage_model::KeySpaceConfig;
    /// let config = KeySpaceConfig::production();
    /// assert_eq!(config.instance_measurement_point_key(1, "101").as_ref(), "inst:1:M:101");
    /// ```
    pub fn instance_measurement_point_key(
        &self,
        instance_id: u32,
        point_id: &str,
    ) -> Cow<'static, str> {
        Cow::Owned(format!(
            "{}:{}:M:{}",
            self.inst_prefix, instance_id, point_id
        ))
    }

    /// Build instance action point key: inst:{instance_id}:A:{point_id}
    ///
    /// # Examples
    /// ```
    /// use voltage_model::KeySpaceConfig;
    /// let config = KeySpaceConfig::production();
    /// assert_eq!(config.instance_action_point_key(1, "1").as_ref(), "inst:1:A:1");
    /// ```
    pub fn instance_action_point_key(&self, instance_id: u32, point_id: &str) -> Cow<'static, str> {
        Cow::Owned(format!(
            "{}:{}:A:{}",
            self.inst_prefix, instance_id, point_id
        ))
    }

    /// Build instance pattern for SCAN/KEYS: inst:{instance_id}:*
    pub fn instance_pattern(&self, instance_id: u32) -> Cow<'static, str> {
        Cow::Owned(format!("{}:{}:*", self.inst_prefix, instance_id))
    }

    /// Build C2M route key: {channel_id}:{type}:{point_id}
    ///
    /// Used as hash field in route:c2m routing table
    pub fn c2m_route_key(
        &self,
        channel_id: u32,
        point_type: PointType,
        point_id: &str,
    ) -> Cow<'static, str> {
        Cow::Owned(format!(
            "{}:{}:{}",
            channel_id,
            point_type.as_str(),
            point_id
        ))
    }

    /// Build M2C route key: {instance_id}:{type}:{point_id}
    ///
    /// Used as hash field in route:m2c routing table
    pub fn m2c_route_key(
        &self,
        instance_id: u32,
        point_type: PointType,
        point_id: &str,
    ) -> Cow<'static, str> {
        Cow::Owned(format!(
            "{}:{}:{}",
            instance_id,
            point_type.as_str(),
            point_id
        ))
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_production_config() {
        let config = KeySpaceConfig::production();
        assert_eq!(config.data_prefix, "comsrv");
        assert_eq!(config.inst_prefix, "inst");
        assert_eq!(config.routing_table, "route:c2m");
        assert_eq!(config.target_prefix, None);
        assert_eq!(config.inst_name_pattern, None);
    }

    #[test]
    fn test_test_config() {
        let config = KeySpaceConfig::test();
        assert_eq!(config.data_prefix, "test:comsrv");
        assert_eq!(config.inst_prefix, "test:inst");
        assert_eq!(config.routing_table, "test:route:c2m");
        assert_eq!(config.target_prefix, Some("test:comsrv".to_string()));
        assert_eq!(
            config.inst_name_pattern,
            Some("test:inst:*:name".to_string())
        );
    }

    #[test]
    fn test_for_m2c() {
        let config = KeySpaceConfig::production().for_m2c();
        assert_eq!(config.routing_table, "route:m2c");
        assert_eq!(config.target_prefix, Some("comsrv".to_string()));
        assert_eq!(config.inst_name_pattern, Some("inst:*:name".to_string()));
    }

    #[test]
    fn test_for_m2c_test_env() {
        let config = KeySpaceConfig::test().for_m2c();
        assert_eq!(config.routing_table, "test:route:m2c");
        assert_eq!(config.target_prefix, Some("test:comsrv".to_string()));
        assert_eq!(
            config.inst_name_pattern,
            Some("test:inst:*:name".to_string())
        );
    }

    #[test]
    fn test_serialization() {
        let config = KeySpaceConfig::test();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: KeySpaceConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_default() {
        let config = KeySpaceConfig::default();
        assert_eq!(config, KeySpaceConfig::production());
    }

    // ============================================================
    // Key generation tests
    // ============================================================

    #[test]
    fn test_channel_key_generation() {
        let config = KeySpaceConfig::production();

        assert_eq!(
            config.channel_key(1001, PointType::Telemetry).as_ref(),
            "comsrv:1001:T"
        );
        assert_eq!(
            config.channel_key(1001, PointType::Signal).as_ref(),
            "comsrv:1001:S"
        );
        assert_eq!(
            config.channel_key(1001, PointType::Control).as_ref(),
            "comsrv:1001:C"
        );
        assert_eq!(
            config.channel_key(1001, PointType::Adjustment).as_ref(),
            "comsrv:1001:A"
        );
    }

    #[test]
    fn test_channel_ts_and_raw_keys() {
        let config = KeySpaceConfig::production();

        assert_eq!(
            config.channel_ts_key(1001, PointType::Telemetry).as_ref(),
            "comsrv:1001:T:ts"
        );
        assert_eq!(
            config.channel_raw_key(1001, PointType::Telemetry).as_ref(),
            "comsrv:1001:T:raw"
        );
    }

    #[test]
    fn test_todo_queue_key() {
        let config = KeySpaceConfig::production();
        assert_eq!(
            config.todo_queue_key(1001, PointType::Control).as_ref(),
            "comsrv:1001:C:TODO"
        );

        // M2C mode should use target_prefix
        let m2c_config = config.for_m2c();
        assert_eq!(
            m2c_config.todo_queue_key(1001, PointType::Control).as_ref(),
            "comsrv:1001:C:TODO"
        );
    }

    #[test]
    fn test_instance_keys() {
        let config = KeySpaceConfig::production();

        assert_eq!(config.instance_measurement_key(1).as_ref(), "inst:1:M");
        assert_eq!(config.instance_action_key(1).as_ref(), "inst:1:A");
        assert_eq!(config.instance_name_key(1).as_ref(), "inst:1:name");
        assert_eq!(config.instance_status_key(1).as_ref(), "inst:1:status");
        assert_eq!(config.instance_config_key(1).as_ref(), "inst:1:config");
        assert_eq!(
            config.instance_measurement_points_key(1).as_ref(),
            "inst:1:measurement_points"
        );
        assert_eq!(
            config.instance_action_points_key(1).as_ref(),
            "inst:1:action_points"
        );
        assert_eq!(config.instance_pattern(1).as_ref(), "inst:1:*");
    }

    #[test]
    fn test_instance_point_keys() {
        let config = KeySpaceConfig::production();

        assert_eq!(
            config.instance_measurement_point_key(1, "101").as_ref(),
            "inst:1:M:101"
        );
        assert_eq!(
            config.instance_action_point_key(1, "1").as_ref(),
            "inst:1:A:1"
        );

        // Test environment
        let test_config = KeySpaceConfig::test();
        assert_eq!(
            test_config
                .instance_measurement_point_key(1, "101")
                .as_ref(),
            "test:inst:1:M:101"
        );
        assert_eq!(
            test_config.instance_action_point_key(1, "1").as_ref(),
            "test:inst:1:A:1"
        );
    }

    #[test]
    fn test_routing_keys() {
        let config = KeySpaceConfig::production();

        // C2M route key
        assert_eq!(
            config
                .c2m_route_key(1001, PointType::Telemetry, "T1")
                .as_ref(),
            "1001:T:T1"
        );

        // M2C route key
        assert_eq!(
            config
                .m2c_route_key(1, PointType::Adjustment, "A1")
                .as_ref(),
            "1:A:A1"
        );
    }

    #[test]
    fn test_key_generation_with_test_environment() {
        let config = KeySpaceConfig::test();

        // All keys should have test: prefix
        assert_eq!(
            config.channel_key(1001, PointType::Telemetry).as_ref(),
            "test:comsrv:1001:T"
        );
        assert_eq!(config.instance_measurement_key(1).as_ref(), "test:inst:1:M");
        assert_eq!(
            config.todo_queue_key(1001, PointType::Control).as_ref(),
            "test:comsrv:1001:C:TODO"
        );
    }

    #[test]
    fn test_key_generation_cow_type() {
        use std::borrow::Cow;

        let config = KeySpaceConfig::production();
        let key: Cow<'static, str> = config.channel_key(1001, PointType::Telemetry);

        // Verify it's Owned variant (dynamic allocation)
        assert!(matches!(key, Cow::Owned(_)));
    }
}
