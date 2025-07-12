//! 额外的功能集成测试
//!
//! 不需要外部依赖的测试用例

use comsrv::core::config::ChannelConfig;
use comsrv::core::framework::factory::ProtocolFactory;
use std::sync::Arc;
use std::sync::Once;
use tokio::sync::RwLock;
use tracing::info;

static INIT: Once = Once::new();

/// 设置测试环境
fn setup_test_env() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    // 确保插件只加载一次
    INIT.call_once(|| {
        let _ = comsrv::plugins::plugin_registry::discovery::load_all_plugins();
    });
}

#[tokio::test]
async fn test_channel_creation_without_start() {
    setup_test_env();
    info!("测试通道创建（不启动）");

    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // 创建通道配置
    let config = create_test_channel_config(401, "creation_test", "virtual");

    // 创建通道但不启动
    {
        let factory_guard = factory.write().await;
        factory_guard
            .create_channel(config)
            .await
            .expect("Failed to create channel");
    }

    // 验证通道已创建
    {
        let factory_guard = factory.read().await;
        assert_eq!(factory_guard.channel_count(), 1);
        let channel_exists = factory_guard.get_channel(401).await.is_some();
        assert!(channel_exists);
    }

    info!("✓ 通道创建测试通过");
}

#[tokio::test]
async fn test_config_type_mapping() {
    setup_test_env();
    info!("测试配置类型映射");

    use comsrv::core::config::types::{CombinedPoint, ScalingInfo};
    use std::collections::HashMap;

    // 创建测试点
    let point = CombinedPoint {
        point_id: 1001,
        signal_name: "test_point".to_string(),
        chinese_name: "测试点".to_string(),
        telemetry_type: "Measurement".to_string(),
        data_type: "FLOAT".to_string(),
        protocol_params: HashMap::new(),
        scaling: Some(ScalingInfo {
            scale: 0.1,
            offset: 0.0,
            unit: Some("V".to_string()),
        }),
    };

    // 验证点的属性
    assert_eq!(point.point_id, 1001);
    assert_eq!(point.telemetry_type, "Measurement");
    assert!(point.scaling.is_some());
    assert_eq!(point.scaling.as_ref().unwrap().scale, 0.1);

    info!("✓ 配置类型映射测试通过");
}

#[tokio::test]
async fn test_channel_config_validation() {
    setup_test_env();
    info!("测试通道配置验证");

    use comsrv::core::config::ChannelLoggingConfig;

    // 创建有效的通道配置
    let config = create_test_channel_config(501, "validation_test", "virtual");

    // 验证必需字段
    assert!(config.id > 0);
    assert!(!config.name.is_empty());
    assert!(!config.protocol.is_empty());

    // 验证参数存在
    assert!(config.parameters.contains_key("host"));
    assert!(config.parameters.contains_key("port"));

    // 验证日志配置
    let default_logging = ChannelLoggingConfig::default();
    assert_eq!(config.logging.level, default_logging.level);

    info!("✓ 通道配置验证测试通过");
}

// 辅助函数：创建测试通道配置
fn create_test_channel_config(id: u16, name: &str, protocol: &str) -> ChannelConfig {
    use comsrv::core::config::ChannelLoggingConfig;
    use std::collections::HashMap;

    let mut parameters = HashMap::new();
    parameters.insert(
        "host".to_string(),
        serde_yaml::Value::String("127.0.0.1".to_string()),
    );
    parameters.insert("port".to_string(), serde_yaml::Value::Number(502.into()));
    parameters.insert(
        "timeout".to_string(),
        serde_yaml::Value::Number(5000.into()),
    );
    parameters.insert(
        "update_interval".to_string(),
        serde_yaml::Value::Number(1000.into()),
    );

    ChannelConfig {
        id,
        name: name.to_string(),
        description: Some("Functional test channel".to_string()),
        protocol: protocol.to_string(),
        parameters,
        logging: ChannelLoggingConfig::default(),
        table_config: None,
        points: Vec::new(),
        combined_points: Vec::new(),
    }
}
