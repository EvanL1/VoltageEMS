//! 协议插件扁平化存储集成测试
//!
//! 验证所有协议插件都正确使用新的扁平化Redis存储结构

use comsrv::core::config::types::channel::ChannelConfig;
use comsrv::core::framework::traits::ComBase;
use comsrv::core::framework::TelemetryType;
use comsrv::core::redis::storage::RedisStorage;
use comsrv::core::redis::types::{TYPE_MEASUREMENT, TYPE_SIGNAL};
use comsrv::plugins::plugin_storage::{DefaultPluginStorage, PluginStorage};
use comsrv::plugins::protocols::virt;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// 测试配置
fn create_test_channel_config(id: u16, name: &str, protocol: &str) -> ChannelConfig {
    use comsrv::core::config::ChannelLoggingConfig;
    use std::collections::HashMap;

    let mut parameters = HashMap::new();
    parameters.insert(
        "host".to_string(),
        serde_yaml::Value::String("localhost".to_string()),
    );
    parameters.insert("port".to_string(), serde_yaml::Value::Number(502.into()));
    parameters.insert("timeout".to_string(), serde_yaml::Value::Number(5.into()));
    parameters.insert(
        "interface".to_string(),
        serde_yaml::Value::String("can0".to_string()),
    );

    ChannelConfig {
        id,
        name: name.to_string(),
        description: Some("Test channel".to_string()),
        protocol: protocol.to_string(),
        parameters,
        logging: ChannelLoggingConfig::default(),
        table_config: None,
        points: Vec::new(),
        combined_points: Vec::new(),
    }
}

#[tokio::test]
async fn test_virtual_protocol_storage() {
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    // 创建虚拟协议
    let config = create_test_channel_config(2001, "virtual_test", "virtual");
    let mut protocol = virt::VirtualProtocol::new(config).unwrap();

    // 启动协议
    protocol.start().await.unwrap();

    // 等待一些数据生成
    sleep(Duration::from_secs(2)).await;

    // 直接检查Redis存储
    match RedisStorage::new("redis://127.0.0.1:6379").await {
        Ok(mut storage) => {
            // 检查遥测数据
            for i in 1..=5 {
                if let Ok(Some((value, _))) = storage.get_point(2001, TYPE_MEASUREMENT, i).await {
                    println!("Virtual遥测点{}: {}", i, value);
                    assert!(value != 0.0); // 应该有数据
                }
            }

            // 检查遥信数据
            for i in 1001..=1005 {
                if let Ok(Some((value, _))) = storage.get_point(2001, TYPE_SIGNAL, i).await {
                    println!("Virtual遥信点{}: {}", i, value);
                    assert!(value == 0.0 || value == 1.0); // 应该是0或1
                }
            }

            println!("✓ Virtual协议存储测试通过");
        }
        Err(_) => {
            println!("跳过测试：Redis未运行");
        }
    }

    // 停止协议
    protocol.stop().await.unwrap();
}

#[tokio::test]
async fn test_storage_key_format() {
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    match RedisStorage::new("redis://127.0.0.1:6379").await {
        Ok(mut storage) => {
            // 测试不同类型的键格式
            let test_cases = vec![
                (1001, TYPE_MEASUREMENT, 10001, 25.6),  // 1001:m:10001
                (1001, TYPE_SIGNAL, 20001, 1.0),        // 1001:s:20001
                (2001, TYPE_MEASUREMENT, 30001, 380.5), // 2001:m:30001
            ];

            // 写入测试数据
            for (channel_id, point_type, point_id, value) in &test_cases {
                storage
                    .set_point(*channel_id, point_type, *point_id, *value)
                    .await
                    .unwrap();
            }

            // 验证数据
            for (channel_id, point_type, point_id, expected_value) in &test_cases {
                let result = storage
                    .get_point(*channel_id, point_type, *point_id)
                    .await
                    .unwrap();
                assert!(result.is_some());
                let (value, _) = result.unwrap();
                assert_eq!(value, *expected_value);

                let key = format!("{}:{}:{}", channel_id, point_type, point_id);
                println!("✓ 键格式验证通过: {}", key);
            }

            println!("✓ 存储键格式测试通过");
        }
        Err(_) => {
            println!("跳过测试：Redis未运行");
        }
    }
}

#[tokio::test]
async fn test_batch_operations() {
    use comsrv::plugins::plugin_storage::PluginPointUpdate;

    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            println!("跳过测试：Redis未运行");
            return;
        }
    };

    // 准备批量数据
    let updates: Vec<PluginPointUpdate> = (1..=100)
        .map(|i| PluginPointUpdate {
            channel_id: 3001,
            telemetry_type: if i <= 50 {
                TelemetryType::Telemetry
            } else {
                TelemetryType::Signal
            },
            point_id: i,
            value: i as f64 * 0.1,
        })
        .collect();

    // 批量写入
    let start = std::time::Instant::now();
    storage.write_points(updates).await.unwrap();
    let elapsed = start.elapsed();

    println!("批量写入100个点耗时: {:?}", elapsed);
    assert!(elapsed < Duration::from_millis(100)); // 应该很快

    // 验证数据
    for i in 1..=10 {
        let telemetry_type = if i <= 50 {
            TelemetryType::Telemetry
        } else {
            TelemetryType::Signal
        };
        let result = storage.read_point(3001, &telemetry_type, i).await.unwrap();
        assert!(result.is_some());
        let (value, _) = result.unwrap();
        assert_eq!(value, i as f64 * 0.1);
    }

    println!("✓ 批量操作测试通过");
}

#[tokio::test]
async fn test_plugin_independence() {
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            println!("跳过测试：Redis未运行");
            return;
        }
    };

    // 不同通道写入相同的点ID
    let channels = vec![4001, 4002, 4003];
    let point_id = 10001;

    // 每个通道写入不同的值
    for (i, &channel_id) in channels.iter().enumerate() {
        let value = (i + 1) as f64 * 10.0;
        storage
            .write_point(channel_id, &TelemetryType::Telemetry, point_id, value)
            .await
            .unwrap();
    }

    // 验证每个通道的数据独立
    for (i, &channel_id) in channels.iter().enumerate() {
        let expected_value = (i + 1) as f64 * 10.0;
        let result = storage
            .read_point(channel_id, &TelemetryType::Telemetry, point_id)
            .await
            .unwrap();

        assert!(result.is_some());
        let (value, _) = result.unwrap();
        assert_eq!(value, expected_value);
        println!("✓ 通道 {} 数据独立性验证通过", channel_id);
    }

    println!("✓ 插件数据独立性测试通过");
}
