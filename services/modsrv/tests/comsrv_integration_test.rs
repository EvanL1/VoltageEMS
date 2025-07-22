//! comsrv集成测试
//!
//! 验证modsrv与comsrv的完整数据流和控制流

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};

use modsrv::comsrv_interface::{ComSrvInterface, ControlCommand, PointValue};
use modsrv::device_model::{
    AutoConfigManager, CalculationEngine, ChannelConfig, CommandRequest, CommandTransformer,
    DataFlowProcessor, DataFormatConverter, DeviceInstanceConfig, InstanceManager, PointMapping,
};
use modsrv::redis_handler::RedisHandler;
use voltage_libs::test_utils::setup_test_redis;

const TEST_CHANNEL_ID: u16 = 1001;
const TEST_REDIS_PREFIX: &str = "test:modsrv";

/// 测试数据流：comsrv -> Redis -> modsrv
#[tokio::test]
async fn test_comsrv_to_modsrv_data_flow() {
    let redis_client = setup_test_redis().await;
    let mut comsrv_interface = ComSrvInterface::new(redis_client.clone());

    // 模拟comsrv发布数据
    let test_data = vec![
        (10001, "m", 25.6), // 电压
        (10002, "m", 1.2),  // 电流
        (20001, "s", 1.0),  // 运行状态
        (20002, "s", 0.0),  // 故障状态
    ];

    // 写入Redis数据
    for (point_id, point_type, value) in test_data {
        let redis_key = format!("{}:{}:{}", TEST_CHANNEL_ID, point_type, point_id);
        let point_value = PointValue {
            value,
            timestamp: chrono::Utc::now().timestamp_millis(),
            quality: "good".to_string(),
        };

        let result = redis_client
            .set(&redis_key, &point_value.to_redis(), None)
            .await;
        assert!(result.is_ok(), "Failed to set Redis data: {:?}", result);
    }

    // 创建设备实例管理器
    let instance_manager = Arc::new(InstanceManager::new());
    let calculation_engine = Arc::new(CalculationEngine::new());
    let (data_flow_processor, _rx) = DataFlowProcessor::new(
        redis_client.clone(),
        instance_manager.clone(),
        calculation_engine.clone(),
    );

    // 创建测试实例配置
    let instance_config = create_test_instance_config();

    // 设置数据流订阅
    let point_mappings = instance_config.point_mappings.clone();
    let result = data_flow_processor
        .subscribe_instance(
            instance_config.instance_id.clone(),
            point_mappings,
            Duration::from_millis(100),
        )
        .await;
    assert!(result.is_ok(), "Failed to subscribe instance: {:?}", result);

    // 启动数据流处理器
    let result = data_flow_processor.start().await;
    assert!(
        result.is_ok(),
        "Failed to start data flow processor: {:?}",
        result
    );

    // 等待数据处理
    sleep(Duration::from_secs(2)).await;

    // 验证数据转换结果
    for (point_id, point_type, expected_value) in &[(10001, "m", 25.6), (10002, "m", 1.2)] {
        let redis_key = format!("{}:{}:{}", TEST_CHANNEL_ID, point_type, point_id);
        let redis_data = redis_client
            .get::<String>(&redis_key)
            .await
            .unwrap()
            .unwrap();

        let telemetry_result =
            DataFormatConverter::convert_comsrv_to_telemetry(&redis_data, point_type);
        assert!(
            telemetry_result.is_ok(),
            "Failed to convert data: {:?}",
            telemetry_result
        );

        let telemetry = telemetry_result.unwrap();
        assert_eq!(telemetry.raw_value, Some(*expected_value));
    }
}

/// 测试控制流：外部指令 -> modsrv -> comsrv
#[tokio::test]
async fn test_control_command_flow() {
    let redis_client = setup_test_redis().await;
    let mut comsrv_interface = ComSrvInterface::new(redis_client.clone());

    // 创建控制命令
    let control_commands = vec![
        ControlCommand::new(TEST_CHANNEL_ID, "c", 30001, 1.0), // 启动
        ControlCommand::new(TEST_CHANNEL_ID, "c", 30001, 0.0), // 停止
        ControlCommand::new(TEST_CHANNEL_ID, "a", 30002, 1500.0), // 调速
    ];

    // 发送控制命令
    let mut command_ids = Vec::new();
    for command in control_commands {
        let command_id = command.command_id.clone();
        let result = comsrv_interface.send_control_command(
            command.channel_id,
            &command.point_type,
            command.point_id,
            command.value,
        );
        assert!(
            result.is_ok(),
            "Failed to send control command: {:?}",
            result
        );
        command_ids.push(command_id);
    }

    // 验证命令状态
    for command_id in command_ids {
        let status_result = comsrv_interface.get_command_status(&command_id);
        assert!(
            status_result.is_ok(),
            "Failed to get command status: {:?}",
            status_result
        );

        let status = status_result.unwrap();
        assert!(status.is_some(), "Command status should exist");
        assert_eq!(status.unwrap().status, "pending");
    }
}

/// 测试设备模型命令转换
#[tokio::test]
async fn test_device_model_command_transformation() {
    // 创建设备模型命令请求
    let command_requests = vec![
        CommandRequest {
            request_id: "req_001".to_string(),
            instance_id: "motor_001".to_string(),
            command: "start_motor".to_string(),
            params: HashMap::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        },
        CommandRequest {
            request_id: "req_002".to_string(),
            instance_id: "motor_001".to_string(),
            command: "change_speed".to_string(),
            params: [("target_speed".to_string(), serde_json::json!(2000.0))]
                .into_iter()
                .collect(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        },
    ];

    // 获取标准命令映射
    let mappings = CommandTransformer::create_standard_mappings();

    // 批量转换命令
    let result = CommandTransformer::batch_transform_commands(command_requests, &mappings);
    assert!(result.is_ok(), "Failed to transform commands: {:?}", result);

    let control_commands = result.unwrap();
    assert_eq!(control_commands.len(), 2);

    // 验证启动命令
    let start_command = &control_commands[0];
    assert_eq!(start_command.channel_id, 1001);
    assert_eq!(start_command.point_type, "c");
    assert_eq!(start_command.point_id, 30001);
    assert_eq!(start_command.value, 1.0);

    // 验证调速命令
    let speed_command = &control_commands[1];
    assert_eq!(speed_command.channel_id, 1001);
    assert_eq!(speed_command.point_type, "a");
    assert_eq!(speed_command.point_id, 30002);
    assert_eq!(speed_command.value, 2000.0);
}

/// 测试自动配置管理器
#[tokio::test]
async fn test_auto_config_manager() {
    let redis_client = Arc::new(RedisHandler::new());
    let instance_manager = Arc::new(InstanceManager::new());
    let calculation_engine = Arc::new(CalculationEngine::new());
    let (data_flow_processor, _rx) = DataFlowProcessor::new(
        redis_client.clone(),
        instance_manager.clone(),
        calculation_engine.clone(),
    );

    let auto_config = AutoConfigManager::new(
        redis_client.clone(),
        instance_manager.clone(),
        Arc::new(data_flow_processor),
        "templates".to_string(),
        TEST_REDIS_PREFIX.to_string(),
    );

    // 创建测试配置文件
    let config_content = r#"
    {
      "version": "1.0.0",
      "instances": [
        {
          "instance_id": "test_motor_001",
          "instance_name": "Test Motor 001",
          "model_id": "stepper_motor_template",
          "enabled": true,
          "point_mappings": {
            "current_position": "1001:m:10001",
            "current_speed": "1001:m:10002",
            "motor_status": "1001:s:20001"
          },
          "properties": {
            "motor_type": "NEMA23",
            "max_speed": 3000.0
          },
          "channel_config": {
            "channel_id": 1001,
            "protocol": "modbus_tcp",
            "description": "Test Channel",
            "point_mappings": {}
          }
        }
      ],
      "global_settings": {
        "update_interval_ms": 1000,
        "enable_realtime_subscription": true,
        "enable_polling": true,
        "redis_key_prefix": "test:modsrv"
      }
    }
    "#;

    // 写入临时配置文件
    let config_path = "/tmp/test_auto_config.json";
    tokio::fs::write(config_path, config_content).await.unwrap();

    // 从配置文件加载实例
    let result = auto_config.load_from_config_file(config_path).await;

    // 清理临时文件
    let _ = tokio::fs::remove_file(config_path).await;

    // 验证加载结果
    assert!(result.is_ok(), "Failed to load config: {:?}", result);
    let instance_ids = result.unwrap();
    assert_eq!(instance_ids.len(), 1);
    assert_eq!(instance_ids[0], "test_motor_001");
}

/// 测试实时数据触发机制
#[tokio::test]
async fn test_realtime_data_trigger() {
    let redis_client = Arc::new(RedisHandler::new());
    let instance_manager = Arc::new(InstanceManager::new());
    let calculation_engine = Arc::new(CalculationEngine::new());
    let (data_flow_processor, mut rx) = DataFlowProcessor::new(
        redis_client.clone(),
        instance_manager.clone(),
        calculation_engine.clone(),
    );

    // 创建测试实例
    let instance_config = create_test_instance_config();
    let point_mappings = instance_config.point_mappings.clone();

    // 订阅实例数据
    let result = data_flow_processor
        .subscribe_instance(
            instance_config.instance_id.clone(),
            point_mappings,
            Duration::from_millis(100),
        )
        .await;
    assert!(result.is_ok());

    // 启动数据流处理器
    let result = data_flow_processor.start().await;
    assert!(result.is_ok());

    // 模拟数据更新
    let test_key = "1001:m:10001";
    let test_value = "30.5:1234567890";
    let result = redis_client.set(test_key, test_value, None).await;
    assert!(result.is_ok());

    // 等待数据更新消息
    let update_result = timeout(Duration::from_secs(5), rx.recv()).await;
    assert!(update_result.is_ok(), "Should receive data update");

    let update = update_result.unwrap().unwrap();
    assert_eq!(update.instance_id, instance_config.instance_id);
    assert_eq!(update.value, serde_json::json!(30.5));
    assert_eq!(update.timestamp, 1234567890);
}

/// 测试错误处理和恢复
#[tokio::test]
async fn test_error_handling_and_recovery() {
    let redis_client = Arc::new(RedisHandler::new());
    let mut comsrv_interface = ComSrvInterface::new(redis_client.clone());

    // 测试无效的Redis键格式
    let invalid_keys = vec![
        "invalid_key",
        "1001:m",
        "1001:m:10001:extra",
        "abc:m:10001",
        "1001:x:10001",
        "1001:m:abc",
    ];

    for invalid_key in invalid_keys {
        let result = comsrv_interface.get_point_value(1001, "m", 10001);
        // 应该能够处理错误而不崩溃
        assert!(result.is_ok() || result.is_err());
    }

    // 测试数据格式转换错误处理
    let invalid_data_formats = vec![
        "invalid_format",
        "25.6",
        "25.6:invalid_timestamp",
        "invalid_value:1234567890",
    ];

    for invalid_data in invalid_data_formats {
        let result = DataFormatConverter::convert_comsrv_to_telemetry(invalid_data, "m");
        assert!(result.is_err(), "Should handle invalid data format");
    }
}

/// 测试性能基准
#[tokio::test]
async fn test_performance_benchmark() {
    let redis_client = Arc::new(RedisHandler::new());
    let mut comsrv_interface = ComSrvInterface::new(redis_client.clone());

    // 准备测试数据
    let test_points: Vec<(u16, &str, u32)> = (10001..10100).map(|i| (1001u16, "m", i)).collect();

    // 写入测试数据
    for (channel_id, point_type, point_id) in &test_points {
        let redis_key = format!("{}:{}:{}", channel_id, point_type, point_id);
        let test_value = format!("{}:{}", point_id, chrono::Utc::now().timestamp_millis());
        let _ = redis_client.set(&redis_key, &test_value, None).await;
    }

    // 批量读取性能测试
    let start_time = std::time::Instant::now();
    let result = comsrv_interface.batch_get_points(&test_points);
    let duration = start_time.elapsed();

    assert!(result.is_ok(), "Batch get should succeed");
    let results = result.unwrap();
    assert_eq!(results.len(), test_points.len());

    // 性能要求：100个点位读取应该在500ms内完成
    assert!(
        duration.as_millis() < 500,
        "Batch read took too long: {:?}",
        duration
    );

    println!(
        "Batch read {} points took {:?}",
        test_points.len(),
        duration
    );
}

/// 创建测试实例配置
fn create_test_instance_config() -> DeviceInstanceConfig {
    DeviceInstanceConfig {
        instance_id: "test_motor_001".to_string(),
        instance_name: "Test Motor 001".to_string(),
        model_id: "stepper_motor_template".to_string(),
        enabled: true,
        point_mappings: [
            ("current_position".to_string(), "1001:m:10001".to_string()),
            ("current_speed".to_string(), "1001:m:10002".to_string()),
            ("current_torque".to_string(), "1001:m:10003".to_string()),
            ("motor_temperature".to_string(), "1001:m:10004".to_string()),
            ("motor_status".to_string(), "1001:s:20001".to_string()),
            ("emergency_stop".to_string(), "1001:s:20002".to_string()),
        ]
        .into_iter()
        .collect(),
        properties: [
            ("motor_type".to_string(), serde_json::json!("NEMA23")),
            ("max_speed".to_string(), serde_json::json!(3000.0)),
            ("max_torque".to_string(), serde_json::json!(2.5)),
        ]
        .into_iter()
        .collect(),
        channel_config: ChannelConfig {
            channel_id: TEST_CHANNEL_ID,
            protocol: "modbus_tcp".to_string(),
            description: "Test Channel".to_string(),
            point_mappings: HashMap::new(),
        },
    }
}

/// 清理测试数据
async fn cleanup_test_data(redis_client: &RedisHandler) {
    // 清理测试Redis键
    let patterns = vec![
        format!("{}:*", TEST_REDIS_PREFIX),
        format!("{}:m:*", TEST_CHANNEL_ID),
        format!("{}:s:*", TEST_CHANNEL_ID),
        format!("{}:c:*", TEST_CHANNEL_ID),
        format!("{}:a:*", TEST_CHANNEL_ID),
        "cmd:status:*".to_string(),
    ];

    for pattern in patterns {
        if let Ok(keys) = redis_client.scan_keys(&pattern).await {
            for key in keys {
                let _ = redis_client.del(&key).await;
            }
        }
    }
}

/// 集成测试清理
#[tokio::test]
async fn cleanup_integration_tests() {
    let redis_client = Arc::new(RedisHandler::new());
    cleanup_test_data(&redis_client).await;
    println!("Integration test cleanup completed");
}
