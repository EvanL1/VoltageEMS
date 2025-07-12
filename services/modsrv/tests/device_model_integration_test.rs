use modsrv::cache::ModelCacheManager;
use modsrv::device_model::{
    CalculationDefinition, CalculationExpression, CollectionType, Constraints, DataFlowConfig,
    DataType, DeviceModel, DeviceModelSystem, DeviceType, PropertyDefinition, TelemetryDefinition,
    TelemetryMapping,
};
use modsrv::engine::{EngineConfig, OptimizedModelEngine};
use modsrv::redis_handler::RedisHandler;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_device_model_system_integration() {
    // 初始化系统组件
    let redis_handler = Arc::new(RedisHandler::new());
    let cache_manager = Arc::new(ModelCacheManager::new(Duration::from_secs(60)));
    let engine_config = EngineConfig::default();
    let model_engine = Arc::new(OptimizedModelEngine::new(engine_config));
    let dataflow_config = DataFlowConfig::default();

    // 创建设备模型系统
    let device_system = DeviceModelSystem::new(
        redis_handler.clone(),
        cache_manager,
        model_engine,
        dataflow_config,
    )
    .await
    .unwrap();

    // 启动系统
    device_system.start().await.unwrap();

    // 创建一个测试设备实例
    let instance_id = device_system
        .create_instance(
            "power_meter_v1",
            "test_meter_001".to_string(),
            "Test Power Meter".to_string(),
            HashMap::new(),
        )
        .await
        .unwrap();

    println!("Created device instance: {}", instance_id);

    // 获取实例信息
    let instance = device_system.get_instance(&instance_id).await.unwrap();
    assert!(instance.is_some());

    // 模拟数据更新
    // 在实际使用中，这些数据会从comsrv通过Redis发布
    redis_handler
        .set("point:1001", "220.5".to_string())
        .await
        .unwrap();
    redis_handler
        .set("point:1002", "10.2".to_string())
        .await
        .unwrap();
    redis_handler
        .set("point:1003", "2.25".to_string())
        .await
        .unwrap();

    // 等待数据流处理
    sleep(Duration::from_secs(2)).await;

    // 获取遥测数据
    let voltage = device_system
        .get_telemetry(&instance_id, "voltage_a")
        .await
        .unwrap();
    println!("Voltage A: {:?}", voltage);

    // 执行命令
    let mut params = HashMap::new();
    params.insert("value".to_string(), serde_json::json!(1));

    let result = device_system
        .execute_command(&instance_id, "switch_on", params)
        .await;

    match result {
        Ok(res) => println!("Command result: {}", res),
        Err(e) => println!("Command error: {}", e),
    }

    // 停止系统
    device_system.stop().await.unwrap();
}

#[tokio::test]
async fn test_device_model_calculations() {
    // 创建一个包含计算的设备模型
    let mut model = DeviceModel {
        id: "calc_test_model".to_string(),
        name: "Calculation Test Model".to_string(),
        description: "Model for testing calculations".to_string(),
        version: "1.0.0".to_string(),
        device_type: DeviceType::Sensor,
        properties: vec![],
        telemetry: vec![
            TelemetryDefinition {
                identifier: "input1".to_string(),
                name: "Input 1".to_string(),
                data_type: DataType::Float64,
                collection_type: CollectionType::Periodic { interval_ms: 1000 },
                mapping: TelemetryMapping {
                    channel_id: 1,
                    point_type: "YC".to_string(),
                    point_id: 2001,
                    scale: None,
                    offset: None,
                },
                transform: None,
                unit: None,
                description: None,
            },
            TelemetryDefinition {
                identifier: "input2".to_string(),
                name: "Input 2".to_string(),
                data_type: DataType::Float64,
                collection_type: CollectionType::Periodic { interval_ms: 1000 },
                mapping: TelemetryMapping {
                    channel_id: 1,
                    point_type: "YC".to_string(),
                    point_id: 2002,
                    scale: None,
                    offset: None,
                },
                transform: None,
                unit: None,
                description: None,
            },
            TelemetryDefinition {
                identifier: "sum_output".to_string(),
                name: "Sum Output".to_string(),
                data_type: DataType::Float64,
                collection_type: CollectionType::EventDriven,
                mapping: TelemetryMapping {
                    channel_id: 1,
                    point_type: "YC".to_string(),
                    point_id: 2003,
                    scale: None,
                    offset: None,
                },
                transform: None,
                unit: None,
                description: None,
            },
        ],
        commands: vec![],
        events: vec![],
        calculations: vec![CalculationDefinition {
            identifier: "sum_calc".to_string(),
            name: "Sum Calculation".to_string(),
            inputs: vec!["input1".to_string(), "input2".to_string()],
            outputs: vec!["sum_output".to_string()],
            expression: CalculationExpression::BuiltIn {
                function: "sum".to_string(),
                args: vec![],
            },
            condition: None,
            description: Some("Calculate sum of two inputs".to_string()),
        }],
        metadata: HashMap::new(),
    };

    // 验证模型
    model.validate().unwrap();

    // 初始化系统
    let redis_handler = Arc::new(RedisHandler::new());
    let cache_manager = Arc::new(ModelCacheManager::new(Duration::from_secs(60)));
    let engine_config = EngineConfig::default();
    let model_engine = Arc::new(OptimizedModelEngine::new(engine_config));
    let dataflow_config = DataFlowConfig::default();

    let device_system = DeviceModelSystem::new(
        redis_handler.clone(),
        cache_manager,
        model_engine,
        dataflow_config,
    )
    .await
    .unwrap();

    // 注册模型
    device_system.register_model(model).await.unwrap();

    // 创建实例
    let instance_id = device_system
        .create_instance(
            "calc_test_model",
            "calc_instance_001".to_string(),
            "Calculation Test Instance".to_string(),
            HashMap::new(),
        )
        .await
        .unwrap();

    println!("Created calculation test instance: {}", instance_id);
}

#[tokio::test]
async fn test_dataflow_subscriptions() {
    use modsrv::device_model::calculation::CalculationEngine;
    use modsrv::device_model::DataFlowProcessor;
    use modsrv::device_model::{InstanceManager, ModelRegistry};

    // 初始化组件
    let redis_handler = Arc::new(RedisHandler::new());
    let registry = Arc::new(ModelRegistry::new());
    let calculation_engine = Arc::new(CalculationEngine::new());
    let instance_manager = Arc::new(InstanceManager::new(registry.clone()));

    // 创建数据流处理器
    let (dataflow_processor, mut update_receiver) = DataFlowProcessor::new(
        redis_handler.clone(),
        instance_manager.clone(),
        calculation_engine.clone(),
    );

    // 启动处理器
    dataflow_processor.start().await.unwrap();

    // 订阅一个实例
    let mut point_mappings = HashMap::new();
    point_mappings.insert("temperature".to_string(), "point:3001".to_string());
    point_mappings.insert("humidity".to_string(), "point:3002".to_string());

    dataflow_processor
        .subscribe_instance(
            "sensor_001".to_string(),
            point_mappings,
            Duration::from_secs(1),
        )
        .await
        .unwrap();

    // 模拟数据更新
    redis_handler
        .set("point:3001", "25.5".to_string())
        .await
        .unwrap();
    redis_handler
        .set("point:3002", "65.0".to_string())
        .await
        .unwrap();

    // 等待并检查更新
    tokio::select! {
        Some(update) = update_receiver.recv() => {
            println!("Received update: {:?}", update);
            assert_eq!(update.instance_id, "sensor_001");
        }
        _ = sleep(Duration::from_secs(3)) => {
            println!("No update received within timeout");
        }
    }

    // 停止处理器
    dataflow_processor.stop().await.unwrap();
}
