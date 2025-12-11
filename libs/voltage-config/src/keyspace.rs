use crate::protocols::PointType;
/// 键空间配置（用于 Redis 操作）
/// Keyspace configuration (for Redis operations)
///
/// **设计原则**：
/// **Design Principles:**
/// - 配置即数据（Configuration as Data）
/// - 测试隔离（独立键空间）
/// - Test isolation (dedicated keyspace)
/// - 多环境支持（dev/test/prod）
/// - Multi-environment support (dev/test/prod)
/// - 统一键名管理（Single Source of Truth）
///
/// **使用示例**：
/// **Usage Example:**
/// ```
/// use voltage_config::{KeySpaceConfig, PointType};
///
/// // 生产环境
/// // Production environment
/// let prod_config = KeySpaceConfig::production();
///
/// // 测试环境（完全隔离的键空间）
/// // Test environment (fully isolated keyspace)
/// let test_config = KeySpaceConfig::test();
///
/// // C2M 路由配置
/// // C2M routing configuration
/// let c2m_config = prod_config.for_c2m();
///
/// // M2C 路由配置
/// // M2C routing configuration
/// let m2c_config = prod_config.for_m2c();
///
/// // 键名生成（类型安全）
/// // Key generation (type-safe)
/// let key = prod_config.channel_key(1001, PointType::Telemetry);
/// // => "comsrv:1001:T"
/// ```
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeySpaceConfig {
    /// 数据存储键前缀（如 "comsrv" 或 "test:comsrv"）
    /// Data storage key prefix (e.g., "comsrv" or "test:comsrv")
    pub data_prefix: String,

    /// 实例键前缀（如 "inst" 或 "test:inst"）
    /// Instance key prefix (e.g., "inst" or "test:inst")
    pub inst_prefix: String,

    /// 路由表键名（如 "route:c2m" 或 "test:route:c2m"）
    /// Routing table key (e.g., "route:c2m" or "test:route:c2m")
    pub routing_table: String,

    /// 目标键前缀（仅 M2C 使用，如 "comsrv"）
    /// Target key prefix (M2C only, e.g., "comsrv")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_prefix: Option<String>,

    /// 实例名查询模式（仅 M2C 使用，如 "inst:*:name"）
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
    /// 生产环境配置
    /// Production environment configuration
    ///
    /// 使用标准的键空间命名：
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

    /// 测试环境配置（完全隔离的键空间）
    /// Test environment configuration (fully isolated keyspace)
    ///
    /// 所有键名添加 "test:" 前缀，确保测试数据不污染生产环境。
    /// Adds a "test:" prefix to all keys to prevent test data from polluting production.
    ///
    /// 使用示例：
    /// Example:
    /// ```
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

    /// C2M（Channel to Model）路由配置
    /// C2M (Channel to Model) routing configuration
    ///
    /// 用于 comsrv.batch_update 函数，将通道数据路由到模型实例。
    /// Used by comsrv.batch_update to route channel data to model instances.
    ///
    /// 返回当前配置的克隆（C2M 不需要额外配置）。
    /// Returns a clone of the current configuration (no extra settings needed for C2M).
    pub fn for_c2m(&self) -> Self {
        self.clone()
    }

    /// M2C（Model to Channel）路由配置
    /// M2C (Model to Channel) routing configuration
    ///
    /// 用于 modsrv.set_action_point 函数，将模型动作路由到通道。
    /// Used by modsrv.set_action_point to route model actions to channels.
    ///
    /// 自动设置：
    /// Auto settings:
    /// - target_prefix: 指向 comsrv 数据键
    /// - target_prefix: points to comsrv data keys
    /// - inst_name_pattern: 实例名查询模式
    /// - inst_name_pattern: instance name lookup pattern
    /// - routing_table: 切换到 m2c 路由表
    /// - routing_table: switch to m2c routing table
    ///
    /// 使用示例：
    /// Example:
    /// ```
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
    // Redis 键名生成方法（Single Source of Truth）
    // Redis key generation methods (Single Source of Truth)
    // ============================================================

    /// Build channel data key: comsrv:{channel_id}:{type}
    ///
    /// # Examples
    /// ```
    /// # use voltage_config::{KeySpaceConfig, PointType};
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
    /// # use voltage_config::KeySpaceConfig;
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
    fn test_for_c2m() {
        let config = KeySpaceConfig::production().for_c2m();
        assert_eq!(config.routing_table, "route:c2m");
        assert_eq!(config.data_prefix, "comsrv");
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
    // 键名生成测试
    // Key generation tests
    // ============================================================

    #[test]
    fn test_channel_key_generation() {
        use crate::protocols::PointType;

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
        use crate::protocols::PointType;

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
        use crate::protocols::PointType;

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
    }

    #[test]
    fn test_routing_keys() {
        use crate::protocols::PointType;

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
        use crate::protocols::PointType;

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
        use crate::protocols::PointType;
        use std::borrow::Cow;

        let config = KeySpaceConfig::production();
        let key: Cow<'static, str> = config.channel_key(1001, PointType::Telemetry);

        // Verify it's Owned variant (dynamic allocation)
        assert!(matches!(key, Cow::Owned(_)));
    }
}
