# VoltageEMS Integration Testing Strategy

## Overview

This comprehensive integration testing strategy covers the hybrid Rust microservices + Redis Lua Functions architecture of VoltageEMS. The strategy ensures reliability, performance, and fault tolerance across all system components.

## Architecture Under Test

```
Client → Nginx(:80) → API Gateway(:6005) → Microservices → Redis(:6379) + InfluxDB
                                        ↓
Services: comsrv(:6000), modsrv(:6001), alarmsrv(:6002), 
         rulesrv(:6003), hissrv(:6004), netsrv(:6006)
```

## 1. Service Integration Tests

### 1.1 comsrv → Redis Data Persistence Tests

**Test Category**: Protocol-to-Storage Integration
**Purpose**: Verify data flows from industrial protocols to Redis storage

#### Test Cases:

**TC-COMSRV-001**: Modbus TCP Data Collection
```rust
#[tokio::test]
async fn test_modbus_tcp_to_redis_flow() {
    // Setup
    let test_env = TestEnvironment::new().await;
    let modbus_server = MockModbusServer::start("127.0.0.1:5020").await;
    
    // Configure test data
    modbus_server.set_holding_register(1, 1234);
    modbus_server.set_coil(2, true);
    
    // Start comsrv with test config
    let comsrv_config = ComsrvConfig {
        channels: vec![Channel {
            id: 1001,
            protocol: "modbus",
            transport_config: ModbusTcpConfig {
                address: "127.0.0.1:5020",
            },
            polling_config: PollingConfig {
                interval_ms: 100,
                batch_size: 50,
            },
            table_config: load_test_csv_config(),
        }],
    };
    
    let comsrv = start_comsrv_service(comsrv_config).await;
    
    // Wait for data collection
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Verify Redis storage
    let redis_client = test_env.redis_client();
    let telemetry_value: f64 = redis_client
        .hget("comsrv:1001:T", "1")
        .await
        .expect("Telemetry point should exist");
    
    assert_eq!(telemetry_value, 1234.000000); // 6 decimal precision
    
    let signal_value: i32 = redis_client
        .hget("comsrv:1001:S", "2")
        .await
        .expect("Signal point should exist");
    
    assert_eq!(signal_value, 1);
}
```

**TC-COMSRV-002**: Virtual Protocol Simulation
```rust
#[tokio::test]
async fn test_virtual_protocol_simulation() {
    let test_env = TestEnvironment::new().await;
    
    let virt_config = VirtualChannelConfig {
        id: 1002,
        protocol: "virtual",
        simulation_config: SimulationConfig {
            sine_wave: SineWaveConfig {
                amplitude: 100.0,
                frequency: 1.0,
                phase: 0.0,
            },
            random_walk: RandomWalkConfig {
                initial_value: 50.0,
                step_size: 5.0,
                bounds: (-100.0, 100.0),
            },
        },
    };
    
    let comsrv = start_comsrv_with_virtual(virt_config).await;
    
    // Collect multiple samples
    let mut samples = Vec::new();
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let value: f64 = test_env.redis_client()
            .hget("comsrv:1002:T", "1")
            .await
            .unwrap();
        samples.push(value);
    }
    
    // Verify sine wave characteristics
    assert!(samples.len() >= 10);
    assert!(samples.iter().all(|&x| x >= -100.0 && x <= 100.0));
}
```

**TC-COMSRV-003**: Multi-Channel Concurrent Operations
```rust
#[tokio::test]
async fn test_multi_channel_concurrent_operations() {
    let test_env = TestEnvironment::new().await;
    
    // Start multiple mock servers
    let modbus_server1 = MockModbusServer::start("127.0.0.1:5021").await;
    let modbus_server2 = MockModbusServer::start("127.0.0.1:5022").await;
    let modbus_server3 = MockModbusServer::start("127.0.0.1:5023").await;
    
    // Configure different data for each server
    modbus_server1.set_holding_register(1, 1111);
    modbus_server2.set_holding_register(1, 2222);
    modbus_server3.set_holding_register(1, 3333);
    
    let multi_channel_config = ComsrvConfig {
        channels: vec![
            create_test_channel(1001, "127.0.0.1:5021"),
            create_test_channel(1002, "127.0.0.1:5022"),
            create_test_channel(1003, "127.0.0.1:5023"),
        ],
    };
    
    let comsrv = start_comsrv_service(multi_channel_config).await;
    
    // Wait for concurrent data collection
    tokio::time::sleep(Duration::from_millis(1000)).await;
    
    // Verify all channels collected data independently
    let redis_client = test_env.redis_client();
    
    let value1: f64 = redis_client.hget("comsrv:1001:T", "1").await.unwrap();
    let value2: f64 = redis_client.hget("comsrv:1002:T", "1").await.unwrap();
    let value3: f64 = redis_client.hget("comsrv:1003:T", "1").await.unwrap();
    
    assert_eq!(value1, 1111.000000);
    assert_eq!(value2, 2222.000000);
    assert_eq!(value3, 3333.000000);
    
    // Verify no cross-contamination
    assert_ne!(value1, value2);
    assert_ne!(value2, value3);
}
```

### 1.2 modsrv Model Operations with Template Expansion

**Test Category**: Model Management Integration
**Purpose**: Verify model CRUD operations with Redis Lua Functions

#### Test Cases:

**TC-MODSRV-001**: Model Creation and Template Expansion
```rust
#[tokio::test]
async fn test_model_creation_with_template_expansion() {
    let test_env = TestEnvironment::new().await;
    test_env.load_redis_functions().await;
    
    let modsrv = start_modsrv_service().await;
    let client = TestHttpClient::new("http://127.0.0.1:6001");
    
    // Create model with template
    let model_request = ModelRequest {
        model_id: "model_001".to_string(),
        name: "Test Power Station".to_string(),
        template_id: "power_station_template".to_string(),
        parameters: json!({
            "capacity_mw": 100,
            "voltage_kv": 35,
            "location": "Site A"
        }),
    };
    
    let response = client
        .post("/models")
        .json(&model_request)
        .send()
        .await
        .expect("Model creation should succeed");
    
    assert_eq!(response.status(), 201);
    
    // Verify model stored in Redis via Lua function
    let redis_client = test_env.redis_client();
    let model_data: String = redis_client
        .fcall("model_get", vec!["model_001"], vec![])
        .await
        .expect("Model should exist in Redis");
    
    let model: serde_json::Value = serde_json::from_str(&model_data).unwrap();
    assert_eq!(model["name"], "Test Power Station");
    assert_eq!(model["parameters"]["capacity_mw"], 100);
    
    // Verify template expansion created points
    let points_data: String = redis_client
        .fcall("model_get_points", vec!["model_001"], vec![])
        .await
        .expect("Model points should exist");
    
    let points: Vec<serde_json::Value> = serde_json::from_str(&points_data).unwrap();
    assert!(!points.is_empty());
    assert!(points.iter().any(|p| p["name"].as_str().unwrap().contains("voltage")));
}
```

**TC-MODSRV-002**: Model Update Operations
```rust
#[tokio::test]
async fn test_model_update_operations() {
    let test_env = TestEnvironment::new().await;
    test_env.load_redis_functions().await;
    
    let modsrv = start_modsrv_service().await;
    let client = TestHttpClient::new("http://127.0.0.1:6001");
    
    // Create initial model
    let initial_model = create_test_model("model_002").await;
    
    // Update model
    let update_request = ModelUpdateRequest {
        name: Some("Updated Power Station".to_string()),
        parameters: Some(json!({
            "capacity_mw": 150,
            "voltage_kv": 35,
            "location": "Site B"
        })),
    };
    
    let response = client
        .put("/models/model_002")
        .json(&update_request)
        .send()
        .await
        .expect("Model update should succeed");
    
    assert_eq!(response.status(), 200);
    
    // Verify update in Redis
    let redis_client = test_env.redis_client();
    let updated_model: String = redis_client
        .fcall("model_get", vec!["model_002"], vec![])
        .await
        .expect("Updated model should exist");
    
    let model: serde_json::Value = serde_json::from_str(&updated_model).unwrap();
    assert_eq!(model["name"], "Updated Power Station");
    assert_eq!(model["parameters"]["capacity_mw"], 150);
    assert_eq!(model["parameters"]["location"], "Site B");
}
```

### 1.3 alarmsrv Alarm Lifecycle Tests

**Test Category**: Alarm Management Integration
**Purpose**: Verify complete alarm lifecycle from creation to resolution

#### Test Cases:

**TC-ALARMSRV-001**: Alarm Creation and Storage
```rust
#[tokio::test]
async fn test_alarm_creation_lifecycle() {
    let test_env = TestEnvironment::new().await;
    test_env.load_redis_functions().await;
    
    let alarmsrv = start_alarmsrv_service().await;
    let client = TestHttpClient::new("http://127.0.0.1:6002");
    
    // Create alarm
    let alarm_request = AlarmRequest {
        title: "High Temperature Alert".to_string(),
        description: "Temperature exceeded threshold".to_string(),
        level: AlarmLevel::Warning,
        source: "sensor_001".to_string(),
        metadata: json!({
            "current_value": 85.5,
            "threshold": 80.0,
            "unit": "°C"
        }),
    };
    
    let response = client
        .post("/alarms")
        .json(&alarm_request)
        .send()
        .await
        .expect("Alarm creation should succeed");
    
    assert_eq!(response.status(), 201);
    
    let alarm_response: AlarmResponse = response.json().await.unwrap();
    let alarm_id = alarm_response.id;
    
    // Verify alarm stored via Redis Lua function
    let redis_client = test_env.redis_client();
    let alarm_data: String = redis_client
        .fcall("alarm_get", vec![&alarm_id], vec![])
        .await
        .expect("Alarm should exist in Redis");
    
    let alarm: serde_json::Value = serde_json::from_str(&alarm_data).unwrap();
    assert_eq!(alarm["title"], "High Temperature Alert");
    assert_eq!(alarm["level"], "Warning");
    assert_eq!(alarm["status"], "Active");
}
```

**TC-ALARMSRV-002**: Alarm Acknowledgment Flow
```rust
#[tokio::test]
async fn test_alarm_acknowledgment_flow() {
    let test_env = TestEnvironment::new().await;
    test_env.load_redis_functions().await;
    
    let alarmsrv = start_alarmsrv_service().await;
    let client = TestHttpClient::new("http://127.0.0.1:6002");
    
    // Create alarm
    let alarm_id = create_test_alarm(&client).await;
    
    // Acknowledge alarm
    let ack_request = AcknowledgmentRequest {
        acknowledged_by: "operator_001".to_string(),
        notes: "Investigating temperature sensor".to_string(),
    };
    
    let response = client
        .post(&format!("/alarms/{}/acknowledge", alarm_id))
        .json(&ack_request)
        .send()
        .await
        .expect("Alarm acknowledgment should succeed");
    
    assert_eq!(response.status(), 200);
    
    // Verify acknowledgment in Redis
    let redis_client = test_env.redis_client();
    let alarm_data: String = redis_client
        .fcall("alarm_get", vec![&alarm_id], vec![])
        .await
        .expect("Acknowledged alarm should exist");
    
    let alarm: serde_json::Value = serde_json::from_str(&alarm_data).unwrap();
    assert_eq!(alarm["status"], "Acknowledged");
    assert_eq!(alarm["acknowledged_by"], "operator_001");
    assert!(alarm["acknowledged_at"].is_string());
}
```

### 1.4 rulesrv Rule Evaluation and Action Triggers

**Test Category**: Rule Engine Integration
**Purpose**: Verify rule evaluation triggers and actions

#### Test Cases:

**TC-RULESRV-001**: Rule Creation and Evaluation
```rust
#[tokio::test]
async fn test_rule_evaluation_trigger() {
    let test_env = TestEnvironment::new().await;
    test_env.load_redis_functions().await;
    
    // Prepare test data in Redis
    let redis_client = test_env.redis_client();
    redis_client.hset("comsrv:1001:T", "1", "75.500000").await.unwrap();
    
    let rulesrv = start_rulesrv_service().await;
    let client = TestHttpClient::new("http://127.0.0.1:6003");
    
    // Create temperature threshold rule
    let rule_request = RuleRequest {
        name: "Temperature Threshold Rule".to_string(),
        description: "Trigger when temperature > 80°C".to_string(),
        condition: RuleCondition {
            expression: "comsrv:1001:T:1 > 80.0".to_string(),
            data_sources: vec!["comsrv:1001:T:1".to_string()],
        },
        actions: vec![
            RuleAction {
                action_type: "create_alarm".to_string(),
                parameters: json!({
                    "title": "High Temperature",
                    "level": "Warning",
                    "description": "Temperature threshold exceeded"
                }),
            }
        ],
        enabled: true,
    };
    
    let response = client
        .post("/rules")
        .json(&rule_request)
        .send()
        .await
        .expect("Rule creation should succeed");
    
    let rule_response: RuleResponse = response.json().await.unwrap();
    let rule_id = rule_response.id;
    
    // Update temperature to trigger rule
    redis_client.hset("comsrv:1001:T", "1", "85.750000").await.unwrap();
    
    // Trigger rule evaluation
    let eval_response = client
        .post(&format!("/rules/{}/evaluate", rule_id))
        .send()
        .await
        .expect("Rule evaluation should succeed");
    
    assert_eq!(eval_response.status(), 200);
    
    // Verify rule execution result
    let execution_data: String = redis_client
        .fcall("rule_get_execution", vec![&rule_id], vec![])
        .await
        .expect("Rule execution should be recorded");
    
    let execution: serde_json::Value = serde_json::from_str(&execution_data).unwrap();
    assert_eq!(execution["triggered"], true);
    assert!(execution["executed_at"].is_string());
}
```

### 1.5 hissrv Historical Data Collection Pipeline

**Test Category**: Historical Data Integration
**Purpose**: Verify data collection and InfluxDB storage

#### Test Cases:

**TC-HISSRV-001**: Historical Data Collection Flow
```rust
#[tokio::test]
async fn test_historical_data_collection() {
    let test_env = TestEnvironment::new().await;
    test_env.load_redis_functions().await;
    
    // Setup InfluxDB test client
    let influx_client = test_env.influx_client();
    
    // Prepare real-time data in Redis
    let redis_client = test_env.redis_client();
    let test_points = vec![
        ("1", "100.500000"),
        ("2", "85.250000"),
        ("3", "1"),  // Boolean signal
    ];
    
    for (point_id, value) in &test_points {
        redis_client
            .hset("comsrv:1001:T", point_id, value)
            .await
            .unwrap();
    }
    
    let hissrv = start_hissrv_service().await;
    
    // Wait for collection cycle
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Verify data in InfluxDB
    let query = format!(
        "SELECT * FROM telemetry WHERE channel_id = '1001' AND time >= now() - 5m"
    );
    
    let query_result = influx_client
        .query(&query)
        .await
        .expect("InfluxDB query should succeed");
    
    // Verify collected points
    assert!(!query_result.is_empty());
    
    // Check specific values with 6-decimal precision
    let point_1_record = query_result.iter()
        .find(|r| r.get("point_id") == Some(&"1".to_string()))
        .expect("Point 1 should be collected");
    
    let collected_value: f64 = point_1_record
        .get("value")
        .and_then(|v| v.parse().ok())
        .expect("Value should be numeric");
    
    assert_eq!(collected_value, 100.500000);
}
```

### 1.6 API Gateway Routing and Aggregation

**Test Category**: Gateway Integration
**Purpose**: Verify service routing and request aggregation

#### Test Cases:

**TC-GATEWAY-001**: Service Routing Verification
```rust
#[tokio::test]
async fn test_api_gateway_service_routing() {
    let test_env = TestEnvironment::new().await;
    test_env.load_redis_functions().await;
    
    // Start all services
    let _comsrv = start_comsrv_service().await;
    let _modsrv = start_modsrv_service().await;
    let _alarmsrv = start_alarmsrv_service().await;
    let _rulesrv = start_rulesrv_service().await;
    let _hissrv = start_hissrv_service().await;
    
    let gateway = start_api_gateway().await;
    let client = TestHttpClient::new("http://127.0.0.1:6005");
    
    // Test routing to different services
    let test_routes = vec![
        ("/comsrv/health", 6000),
        ("/modsrv/health", 6001),
        ("/alarmsrv/health", 6002),
        ("/rulesrv/health", 6003),
        ("/hissrv/health", 6004),
    ];
    
    for (route, expected_port) in test_routes {
        let response = client
            .get(route)
            .send()
            .await
            .expect(&format!("Route {} should be accessible", route));
        
        assert_eq!(response.status(), 200);
        
        // Verify correct service responded
        let health_response: serde_json::Value = response.json().await.unwrap();
        assert_eq!(health_response["status"], "healthy");
        
        // Check if response contains service identifier
        if let Some(service_info) = health_response.get("service") {
            let port = service_info.get("port").and_then(|p| p.as_u64()).unwrap_or(0);
            assert_eq!(port, expected_port);
        }
    }
}
```

## 2. Data Flow Testing

### 2.1 End-to-End Data Flow Tests

**Test Category**: Complete Pipeline Integration
**Purpose**: Verify data flows through entire system

#### Test Cases:

**TC-E2E-001**: Device to Alarm Pipeline
```rust
#[tokio::test]
async fn test_device_to_alarm_pipeline() {
    let test_env = TestEnvironment::new().await;
    test_env.load_redis_functions().await;
    
    // Start all services
    let services = start_all_services().await;
    
    // Setup mock device with critical temperature
    let modbus_server = MockModbusServer::start("127.0.0.1:5030").await;
    modbus_server.set_holding_register(1, 9500); // 95.00°C scaled
    
    // Create temperature monitoring rule
    let client = TestHttpClient::new("http://127.0.0.1:6005");
    
    let rule_request = RuleRequest {
        name: "Critical Temperature Monitor".to_string(),
        condition: RuleCondition {
            expression: "comsrv:1001:T:1 > 90.0".to_string(),
            data_sources: vec!["comsrv:1001:T:1".to_string()],
        },
        actions: vec![
            RuleAction {
                action_type: "create_alarm".to_string(),
                parameters: json!({
                    "title": "Critical Temperature Alert",
                    "level": "Critical",
                    "description": "Temperature critically high"
                }),
            }
        ],
        enabled: true,
    };
    
    let rule_response = client
        .post("/rulesrv/rules")
        .json(&rule_request)
        .send()
        .await
        .expect("Rule creation should succeed");
    
    // Wait for data collection and rule evaluation
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Verify alarm was created
    let alarms_response = client
        .get("/alarmsrv/alarms?status=active")
        .send()
        .await
        .expect("Alarms query should succeed");
    
    let alarms: Vec<serde_json::Value> = alarms_response.json().await.unwrap();
    
    let critical_alarm = alarms.iter()
        .find(|a| a["title"].as_str().unwrap().contains("Critical Temperature"))
        .expect("Critical temperature alarm should exist");
    
    assert_eq!(critical_alarm["level"], "Critical");
    assert_eq!(critical_alarm["status"], "Active");
    
    // Verify historical data was collected
    let history_response = client
        .get("/hissrv/data/telemetry?channel_id=1001&point_id=1&duration=5m")
        .send()
        .await
        .expect("Historical data query should succeed");
    
    let history_data: Vec<serde_json::Value> = history_response.json().await.unwrap();
    assert!(!history_data.is_empty());
    
    let latest_reading = history_data.last().unwrap();
    let temperature: f64 = latest_reading["value"].as_f64().unwrap();
    assert!(temperature >= 95.0);
}
```

### 2.2 CSV Configuration Loading Tests

**Test Category**: Configuration Management
**Purpose**: Verify CSV-based configuration loading and hot-reload

#### Test Cases:

**TC-CONFIG-001**: CSV Hot-Reload Functionality
```rust
#[tokio::test]
async fn test_csv_hot_reload() {
    let test_env = TestEnvironment::new().await;
    
    // Create temporary CSV files
    let temp_dir = tempfile::tempdir().unwrap();
    let telemetry_file = temp_dir.path().join("telemetry.csv");
    
    // Write initial configuration
    let initial_csv = "point_id,signal_name,scale,offset,unit,reverse,data_type\n\
                      1,Temperature,1.0,0.0,℃,false,float\n\
                      2,Pressure,0.1,0.0,bar,false,float\n";
    
    std::fs::write(&telemetry_file, initial_csv).unwrap();
    
    // Start comsrv with file watching
    let config = ComsrvConfig {
        channels: vec![Channel {
            id: 1001,
            table_config: TableConfig {
                four_telemetry_files: FourTelemetryFiles {
                    telemetry_file: telemetry_file.to_string_lossy().to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..create_test_channel_config()
        }],
        file_watch_enabled: true,
    };
    
    let comsrv = start_comsrv_service(config).await;
    
    // Verify initial configuration loaded
    let redis_client = test_env.redis_client();
    let point_count: i64 = redis_client
        .hlen("comsrv:1001:config:telemetry")
        .await
        .unwrap();
    assert_eq!(point_count, 2);
    
    // Update CSV file
    let updated_csv = "point_id,signal_name,scale,offset,unit,reverse,data_type\n\
                      1,Temperature,1.0,0.0,℃,false,float\n\
                      2,Pressure,0.1,0.0,bar,false,float\n\
                      3,Flow Rate,0.01,0.0,L/min,false,float\n";
    
    std::fs::write(&telemetry_file, updated_csv).unwrap();
    
    // Wait for hot-reload
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Verify updated configuration
    let updated_point_count: i64 = redis_client
        .hlen("comsrv:1001:config:telemetry")
        .await
        .unwrap();
    assert_eq!(updated_point_count, 3);
    
    // Verify new point configuration
    let flow_config: String = redis_client
        .hget("comsrv:1001:config:telemetry", "3")
        .await
        .unwrap();
    
    let config: serde_json::Value = serde_json::from_str(&flow_config).unwrap();
    assert_eq!(config["signal_name"], "Flow Rate");
    assert_eq!(config["scale"], 0.01);
}
```

## 3. Protocol Testing

### 3.1 Modbus Protocol Testing

**Test Category**: Industrial Protocol Integration
**Purpose**: Verify Modbus TCP/RTU communication reliability

#### Test Cases:

**TC-MODBUS-001**: Modbus TCP Communication Robustness
```rust
#[tokio::test]
async fn test_modbus_tcp_robustness() {
    let test_env = TestEnvironment::new().await;
    
    let modbus_server = MockModbusServer::start("127.0.0.1:5040").await;
    
    // Setup various data types
    modbus_server.set_holding_register(1, 1234);      // Temperature
    modbus_server.set_holding_register(2, 5678);      // Pressure
    modbus_server.set_coil(1, true);                  // Pump Status
    modbus_server.set_discrete_input(1, false);       // Door Open
    modbus_server.set_input_register(1, 9999);        // Flow Rate
    
    let comsrv_config = create_comprehensive_modbus_config("127.0.0.1:5040");
    let comsrv = start_comsrv_service(comsrv_config).await;
    
    // Wait for multiple polling cycles
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    let redis_client = test_env.redis_client();
    
    // Verify all data types collected correctly
    let holding_reg_1: f64 = redis_client.hget("comsrv:1001:T", "1").await.unwrap();
    let holding_reg_2: f64 = redis_client.hget("comsrv:1001:T", "2").await.unwrap();
    let coil_1: i32 = redis_client.hget("comsrv:1001:S", "1").await.unwrap();
    let discrete_1: i32 = redis_client.hget("comsrv:1001:S", "2").await.unwrap();
    let input_reg_1: f64 = redis_client.hget("comsrv:1001:T", "3").await.unwrap();
    
    assert_eq!(holding_reg_1, 1234.000000);
    assert_eq!(holding_reg_2, 5678.000000);
    assert_eq!(coil_1, 1);
    assert_eq!(discrete_1, 0);
    assert_eq!(input_reg_1, 9999.000000);
}
```

**TC-MODBUS-002**: Modbus Error Handling
```rust
#[tokio::test]
async fn test_modbus_error_handling() {
    let test_env = TestEnvironment::new().await;
    
    // Start server that will be stopped mid-test
    let modbus_server = MockModbusServer::start("127.0.0.1:5041").await;
    modbus_server.set_holding_register(1, 1000);
    
    let comsrv_config = create_test_modbus_config("127.0.0.1:5041");
    let comsrv = start_comsrv_service(comsrv_config).await;
    
    // Wait for initial successful connection
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    let redis_client = test_env.redis_client();
    let initial_value: f64 = redis_client.hget("comsrv:1001:T", "1").await.unwrap();
    assert_eq!(initial_value, 1000.000000);
    
    // Stop the Modbus server to simulate network failure
    modbus_server.stop().await;
    
    // Wait through error period
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Restart server with different value
    let new_server = MockModbusServer::start("127.0.0.1:5041").await;
    new_server.set_holding_register(1, 2000);
    
    // Wait for reconnection and recovery
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // Verify service recovered and new data is collected
    let recovered_value: f64 = redis_client.hget("comsrv:1001:T", "1").await.unwrap();
    assert_eq!(recovered_value, 2000.000000);
    
    // Verify error states were logged
    let error_count: i64 = redis_client
        .hget("comsrv:1001:status", "error_count")
        .await
        .unwrap_or(0);
    assert!(error_count > 0);
}
```

### 3.2 Virtual Protocol Testing

**TC-VIRTUAL-001**: Virtual Protocol Simulation Patterns
```rust
#[tokio::test]
async fn test_virtual_protocol_patterns() {
    let test_env = TestEnvironment::new().await;
    
    let virtual_config = VirtualChannelConfig {
        id: 1002,
        protocol: "virtual",
        simulation_config: SimulationConfig {
            patterns: vec![
                SimulationPattern::SineWave {
                    point_id: 1,
                    amplitude: 100.0,
                    frequency: 0.1, // 0.1 Hz
                    phase: 0.0,
                    offset: 50.0,
                },
                SimulationPattern::RandomWalk {
                    point_id: 2,
                    initial_value: 0.0,
                    step_size: 1.0,
                    bounds: (-50.0, 50.0),
                },
                SimulationPattern::StepFunction {
                    point_id: 3,
                    values: vec![0.0, 100.0, 50.0, 75.0],
                    duration_ms: 500,
                },
            ],
        },
    };
    
    let comsrv = start_comsrv_with_virtual(virtual_config).await;
    
    let redis_client = test_env.redis_client();
    let mut samples = Vec::new();
    
    // Collect samples over time
    for i in 0..20 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let sine_value: f64 = redis_client.hget("comsrv:1002:T", "1").await.unwrap();
        let random_value: f64 = redis_client.hget("comsrv:1002:T", "2").await.unwrap();
        let step_value: f64 = redis_client.hget("comsrv:1002:T", "3").await.unwrap();
        
        samples.push((sine_value, random_value, step_value));
    }
    
    // Verify sine wave properties
    let sine_values: Vec<f64> = samples.iter().map(|(s, _, _)| *s).collect();
    assert!(sine_values.iter().all(|&x| x >= -50.0 && x <= 150.0)); // offset ± amplitude
    
    // Verify random walk stays within bounds
    let random_values: Vec<f64> = samples.iter().map(|(_, r, _)| *r).collect();
    assert!(random_values.iter().all(|&x| x >= -50.0 && x <= 50.0));
    
    // Verify step function values
    let step_values: Vec<f64> = samples.iter().map(|(_, _, s)| *s).collect();
    let unique_steps: HashSet<i32> = step_values.iter().map(|&x| x as i32).collect();
    assert!(unique_steps.contains(&0));
    assert!(unique_steps.contains(&100));
    assert!(unique_steps.contains(&50));
    assert!(unique_steps.contains(&75));
}
```

## 4. Performance Testing

### 4.1 Load Testing Framework

**Test Category**: Performance and Scalability
**Purpose**: Verify system performance under load

#### Test Cases:

**TC-PERF-001**: Concurrent Connection Load Test
```rust
#[tokio::test]
async fn test_concurrent_connection_load() {
    let test_env = TestEnvironment::new().await;
    test_env.load_redis_functions().await;
    
    // Setup multiple mock Modbus servers
    let server_count = 50;
    let mut servers = Vec::new();
    let mut channels = Vec::new();
    
    for i in 0..server_count {
        let port = 5100 + i;
        let address = format!("127.0.0.1:{}", port);
        
        let server = MockModbusServer::start(&address).await;
        server.set_holding_register(1, (i * 100) as u16);
        servers.push(server);
        
        channels.push(create_test_channel(1000 + i as u16, &address));
    }
    
    let multi_channel_config = ComsrvConfig {
        channels,
        global_config: GlobalConfig {
            max_concurrent_connections: 100,
            connection_timeout_ms: 5000,
            polling_interval_ms: 1000,
        },
    };
    
    // Measure startup time
    let start_time = Instant::now();
    let comsrv = start_comsrv_service(multi_channel_config).await;
    let startup_duration = start_time.elapsed();
    
    // Should start within reasonable time even with many channels
    assert!(startup_duration < Duration::from_secs(10));
    
    // Wait for all channels to collect at least one sample
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    let redis_client = test_env.redis_client();
    
    // Verify all channels are collecting data
    let mut successful_channels = 0;
    
    for i in 0..server_count {
        let channel_id = format!("{}", 1000 + i);
        match redis_client.hget::<_, _, f64>(
            &format!("comsrv:{}:T", channel_id), 
            "1"
        ).await {
            Ok(value) => {
                assert_eq!(value, ((i * 100) as f64));
                successful_channels += 1;
            },
            Err(_) => {
                // Channel might still be connecting
            }
        }
    }
    
    // At least 80% of channels should be operational
    assert!(successful_channels >= (server_count * 80) / 100);
    
    // Measure throughput
    let throughput_start = Instant::now();
    tokio::time::sleep(Duration::from_secs(10)).await;
    let throughput_duration = throughput_start.elapsed();
    
    // Count total operations
    let mut total_operations = 0;
    for i in 0..successful_channels {
        let channel_id = format!("{}", 1000 + i);
        let operations: i64 = redis_client
            .hget(&format!("comsrv:{}:stats", channel_id), "total_reads")
            .await
            .unwrap_or(0);
        total_operations += operations;
    }
    
    let ops_per_second = total_operations as f64 / throughput_duration.as_secs_f64();
    
    // Should achieve reasonable throughput (target: >100 ops/sec total)
    assert!(ops_per_second > 100.0, 
            "Throughput too low: {} ops/sec", ops_per_second);
}
```

### 4.2 Redis Performance Testing

**TC-REDIS-001**: Redis Lua Function Performance
```rust
#[tokio::test]
async fn test_redis_lua_function_performance() {
    let test_env = TestEnvironment::new().await;
    test_env.load_redis_functions().await;
    
    let redis_client = test_env.redis_client();
    
    // Benchmark model operations
    let model_data = json!({
        "name": "Performance Test Model",
        "template_id": "test_template",
        "parameters": {
            "capacity": 1000,
            "efficiency": 0.95
        }
    });
    
    let model_count = 1000;
    
    // Measure model creation performance
    let create_start = Instant::now();
    let mut create_tasks = Vec::new();
    
    for i in 0..model_count {
        let model_id = format!("perf_model_{:04}", i);
        let data = model_data.to_string();
        let client = redis_client.clone();
        
        let task = tokio::spawn(async move {
            client.fcall::<_, _, String>("model_upsert", vec![&model_id], vec![&data]).await
        });
        
        create_tasks.push(task);
    }
    
    // Wait for all creates to complete
    let create_results = futures::future::join_all(create_tasks).await;
    let create_duration = create_start.elapsed();
    
    let successful_creates = create_results.iter()
        .filter(|r| r.as_ref().unwrap().is_ok())
        .count();
    
    assert_eq!(successful_creates, model_count);
    
    let creates_per_second = model_count as f64 / create_duration.as_secs_f64();
    assert!(creates_per_second > 500.0, 
            "Model creation too slow: {} creates/sec", creates_per_second);
    
    // Measure model retrieval performance
    let read_start = Instant::now();
    let mut read_tasks = Vec::new();
    
    for i in 0..model_count {
        let model_id = format!("perf_model_{:04}", i);
        let client = redis_client.clone();
        
        let task = tokio::spawn(async move {
            client.fcall::<_, _, String>("model_get", vec![&model_id], vec![]).await
        });
        
        read_tasks.push(task);
    }
    
    let read_results = futures::future::join_all(read_tasks).await;
    let read_duration = read_start.elapsed();
    
    let successful_reads = read_results.iter()
        .filter(|r| r.as_ref().unwrap().is_ok())
        .count();
    
    assert_eq!(successful_reads, model_count);
    
    let reads_per_second = model_count as f64 / read_duration.as_secs_f64();
    assert!(reads_per_second > 1000.0, 
            "Model retrieval too slow: {} reads/sec", reads_per_second);
}
```

## 5. Fault Tolerance Testing

### 5.1 Service Failure Recovery

**Test Category**: Resilience and Recovery
**Purpose**: Verify system behavior during failures

#### Test Cases:

**TC-FAULT-001**: Redis Connection Resilience
```rust
#[tokio::test]
async fn test_redis_connection_resilience() {
    let test_env = TestEnvironment::new().await;
    
    // Start services
    let comsrv = start_comsrv_service().await;
    let alarmsrv = start_alarmsrv_service().await;
    
    // Verify initial functionality
    let client = TestHttpClient::new("http://127.0.0.1:6002");
    let response = client.get("/health").send().await.unwrap();
    assert_eq!(response.status(), 200);
    
    // Simulate Redis failure
    test_env.stop_redis().await;
    
    // Wait for connection failure detection
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Services should handle graceful degradation
    let response = client.get("/health").send().await.unwrap();
    // Should return 503 Service Unavailable during Redis outage
    assert_eq!(response.status(), 503);
    
    // Restart Redis
    test_env.start_redis().await;
    test_env.load_redis_functions().await;
    
    // Wait for reconnection
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Services should recover automatically
    let response = client.get("/health").send().await.unwrap();
    assert_eq!(response.status(), 200);
    
    // Verify functionality restored
    let alarm_request = AlarmRequest {
        title: "Recovery Test Alarm".to_string(),
        description: "Testing recovery".to_string(),
        level: AlarmLevel::Info,
        source: "test".to_string(),
        metadata: json!({}),
    };
    
    let response = client
        .post("/alarms")
        .json(&alarm_request)
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 201);
}
```

### 5.2 Network Interruption Handling

**TC-FAULT-002**: Network Interruption Recovery
```rust
#[tokio::test]
async fn test_network_interruption_recovery() {
    let test_env = TestEnvironment::new().await;
    
    // Create a controllable network proxy
    let proxy = NetworkProxy::new("127.0.0.1:5060", "127.0.0.1:5061").await;
    let modbus_server = MockModbusServer::start("127.0.0.1:5061").await;
    modbus_server.set_holding_register(1, 1000);
    
    // Configure comsrv to connect through proxy
    let config = create_test_modbus_config("127.0.0.1:5060");
    let comsrv = start_comsrv_service(config).await;
    
    // Wait for initial connection
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    let redis_client = test_env.redis_client();
    let initial_value: f64 = redis_client.hget("comsrv:1001:T", "1").await.unwrap();
    assert_eq!(initial_value, 1000.000000);
    
    // Simulate network interruption
    proxy.block_connections().await;
    
    // Change server value during outage
    modbus_server.set_holding_register(1, 2000);
    
    // Wait during network outage
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Value should remain at last known good value
    let outage_value: f64 = redis_client.hget("comsrv:1001:T", "1").await.unwrap();
    assert_eq!(outage_value, 1000.000000); // Should not have changed
    
    // Restore network
    proxy.allow_connections().await;
    
    // Wait for reconnection and new data
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // Should receive updated value
    let recovered_value: f64 = redis_client.hget("comsrv:1001:T", "1").await.unwrap();
    assert_eq!(recovered_value, 2000.000000);
    
    // Verify error statistics were recorded
    let connection_errors: i64 = redis_client
        .hget("comsrv:1001:stats", "connection_errors")
        .await
        .unwrap();
    assert!(connection_errors > 0);
    
    let reconnect_count: i64 = redis_client
        .hget("comsrv:1001:stats", "reconnect_count")
        .await
        .unwrap();
    assert!(reconnect_count > 0);
}
```

## 6. Test Implementation Framework

### 6.1 Test Environment Setup

```rust
// tests/common/test_environment.rs
use redis::Client as RedisClient;
use influxdb2::Client as InfluxClient;
use std::process::{Command, Stdio};
use tokio::time::{sleep, Duration};

pub struct TestEnvironment {
    redis_client: RedisClient,
    influx_client: InfluxClient,
    temp_dirs: Vec<tempfile::TempDir>,
    redis_process: Option<tokio::process::Child>,
}

impl TestEnvironment {
    pub async fn new() -> Self {
        let redis_client = RedisClient::open("redis://127.0.0.1:6379")
            .expect("Failed to create Redis client");
        
        let influx_client = InfluxClient::new(
            "http://localhost:8086",
            "test-org",
            "test-token"
        );
        
        Self {
            redis_client,
            influx_client,
            temp_dirs: Vec::new(),
            redis_process: None,
        }
    }
    
    pub async fn setup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Start Redis if not running
        self.start_redis().await?;
        
        // Clean existing data
        self.cleanup_redis().await?;
        
        // Load Redis functions
        self.load_redis_functions().await?;
        
        Ok(())
    }
    
    pub async fn start_redis(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Check if Redis is already running
        if self.redis_client.get_connection().is_ok() {
            return Ok(());
        }
        
        // Start Redis server
        let mut cmd = tokio::process::Command::new("redis-server")
            .arg("--port")
            .arg("6379")
            .arg("--save")
            .arg("")  // Disable persistence for tests
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        
        self.redis_process = Some(cmd);
        
        // Wait for Redis to start
        for _ in 0..30 {
            if self.redis_client.get_connection().is_ok() {
                return Ok(());
            }
            sleep(Duration::from_millis(100)).await;
        }
        
        Err("Redis failed to start within timeout".into())
    }
    
    pub async fn load_redis_functions(&self) -> Result<(), Box<dyn std::error::Error>> {
        let script_path = std::env::current_dir()?
            .join("scripts/redis-functions/load_functions.sh");
        
        let output = Command::new("bash")
            .arg(script_path)
            .output()?;
        
        if !output.status.success() {
            return Err(format!(
                "Failed to load Redis functions: {}",
                String::from_utf8_lossy(&output.stderr)
            ).into());
        }
        
        Ok(())
    }
    
    pub fn redis_client(&self) -> &RedisClient {
        &self.redis_client
    }
    
    pub fn influx_client(&self) -> &InfluxClient {
        &self.influx_client
    }
    
    pub fn create_temp_dir(&mut self) -> &tempfile::TempDir {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        self.temp_dirs.push(temp_dir);
        self.temp_dirs.last().unwrap()
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        if let Some(mut redis_process) = self.redis_process.take() {
            let _ = redis_process.kill();
        }
    }
}
```

### 6.2 Mock Services

```rust
// tests/common/mock_services.rs
use tokio::net::TcpListener;
use tokio_modbus::server::tcp::Server;

pub struct MockModbusServer {
    address: String,
    server_handle: tokio::task::JoinHandle<()>,
    data_store: Arc<Mutex<ModbusDataStore>>,
}

impl MockModbusServer {
    pub async fn start(address: &str) -> Self {
        let data_store = Arc::new(Mutex::new(ModbusDataStore::new()));
        let data_store_clone = data_store.clone();
        
        let listener = TcpListener::bind(address).await
            .expect("Failed to bind Modbus server");
        
        let server_handle = tokio::spawn(async move {
            let server = Server::new(listener);
            server.serve(data_store_clone).await;
        });
        
        // Wait for server to start
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        Self {
            address: address.to_string(),
            server_handle,
            data_store,
        }
    }
    
    pub fn set_holding_register(&self, address: u16, value: u16) {
        let mut store = self.data_store.lock().unwrap();
        store.holding_registers.insert(address, value);
    }
    
    pub fn set_coil(&self, address: u16, value: bool) {
        let mut store = self.data_store.lock().unwrap();
        store.coils.insert(address, value);
    }
    
    pub async fn stop(self) {
        self.server_handle.abort();
    }
}

#[derive(Default)]
struct ModbusDataStore {
    holding_registers: HashMap<u16, u16>,
    input_registers: HashMap<u16, u16>,
    coils: HashMap<u16, bool>,
    discrete_inputs: HashMap<u16, bool>,
}
```

### 6.3 CI/CD Integration

```yaml
# .github/workflows/integration-tests.yml
name: Integration Tests

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  integration-tests:
    runs-on: ubuntu-latest
    
    services:
      redis:
        image: redis:8-alpine
        ports:
          - 6379:6379
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
      
      influxdb:
        image: influxdb:2.7-alpine
        ports:
          - 8086:8086
        env:
          INFLUXDB_DB: testdb
          INFLUXDB_ADMIN_USER: admin
          INFLUXDB_ADMIN_PASSWORD: password
    
    steps:
    - uses: actions/checkout@v4
    
    - name: Setup Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        override: true
    
    - name: Cache dependencies
      uses: actions/cache@v3
      with:
        path: |
          ~/.cargo
          target/
        key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
    
    - name: Load Redis Functions
      run: |
        cd scripts/redis-functions
        ./load_functions.sh
    
    - name: Run Integration Tests
      run: |
        cargo test --test integration_tests --release -- --test-threads=1
      env:
        RUST_LOG: debug
        REDIS_URL: redis://localhost:6379
        INFLUX_URL: http://localhost:8086
        INFLUX_TOKEN: test-token
        INFLUX_ORG: test-org
        INFLUX_BUCKET: test-bucket
    
    - name: Upload Test Results
      uses: actions/upload-artifact@v3
      if: always()
      with:
        name: integration-test-results
        path: target/test-results/
```

### 6.4 Test Execution Scripts

```bash
#!/bin/bash
# scripts/run-integration-tests.sh

set -e

echo "=== VoltageEMS Integration Test Runner ==="

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m'

# Configuration
REDIS_PORT=${REDIS_PORT:-6379}
TEST_TIMEOUT=${TEST_TIMEOUT:-300}
PARALLEL_TESTS=${PARALLEL_TESTS:-1}

# Check prerequisites
check_prerequisites() {
    echo "Checking prerequisites..."
    
    # Check Redis
    if ! command -v redis-cli &> /dev/null; then
        echo -e "${RED}Redis CLI not found${NC}"
        exit 1
    fi
    
    # Check Redis connection
    if ! redis-cli -p $REDIS_PORT ping > /dev/null 2>&1; then
        echo -e "${RED}Redis not running on port $REDIS_PORT${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}Prerequisites OK${NC}"
}

# Load Redis functions
load_redis_functions() {
    echo "Loading Redis functions..."
    cd scripts/redis-functions
    if ./load_functions.sh; then
        echo -e "${GREEN}Redis functions loaded${NC}"
    else
        echo -e "${RED}Failed to load Redis functions${NC}"
        exit 1
    fi
    cd ../..
}

# Run test categories
run_service_integration_tests() {
    echo "Running service integration tests..."
    cargo test --test service_integration_tests \
        --release \
        -- --test-threads=$PARALLEL_TESTS \
        --timeout=$TEST_TIMEOUT
}

run_data_flow_tests() {
    echo "Running data flow tests..."
    cargo test --test data_flow_tests \
        --release \
        -- --test-threads=$PARALLEL_TESTS \
        --timeout=$TEST_TIMEOUT
}

run_protocol_tests() {
    echo "Running protocol tests..."
    cargo test --test protocol_tests \
        --release \
        -- --test-threads=$PARALLEL_TESTS \
        --timeout=$TEST_TIMEOUT
}

run_performance_tests() {
    echo "Running performance tests..."
    cargo test --test performance_tests \
        --release \
        -- --test-threads=1 \
        --timeout=600
}

run_fault_tolerance_tests() {
    echo "Running fault tolerance tests..."
    cargo test --test fault_tolerance_tests \
        --release \
        -- --test-threads=1 \
        --timeout=$TEST_TIMEOUT
}

# Main execution
main() {
    check_prerequisites
    load_redis_functions
    
    echo ""
    echo "=== Starting Integration Tests ==="
    
    TEST_START=$(date +%s)
    FAILED_TESTS=()
    
    # Run test categories
    test_categories=(
        "service_integration_tests"
        "data_flow_tests"
        "protocol_tests"
        "performance_tests"
        "fault_tolerance_tests"
    )
    
    for category in "${test_categories[@]}"; do
        echo ""
        echo -e "${YELLOW}Running $category...${NC}"
        
        if "run_${category}"; then
            echo -e "${GREEN}✓ $category passed${NC}"
        else
            echo -e "${RED}✗ $category failed${NC}"
            FAILED_TESTS+=("$category")
        fi
    done
    
    TEST_END=$(date +%s)
    TEST_DURATION=$((TEST_END - TEST_START))
    
    echo ""
    echo "=== Test Results ==="
    echo "Duration: ${TEST_DURATION}s"
    
    if [ ${#FAILED_TESTS[@]} -eq 0 ]; then
        echo -e "${GREEN}All integration tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}Failed test categories: ${FAILED_TESTS[*]}${NC}"
        exit 1
    fi
}

# Execute main function
main "$@"
```

## Test Success Criteria

### Functional Criteria
- **Service Integration**: All services successfully communicate with Redis and execute Lua functions
- **Data Flow**: End-to-end data flows complete within expected timeframes
- **Protocol Support**: All supported protocols (Modbus TCP/RTU, Virtual, gRPC) function correctly
- **Configuration Management**: CSV hot-reload works without service restart

### Performance Criteria
- **Throughput**: System handles >1000 concurrent device connections
- **Response Time**: API responses complete within <500ms under normal load
- **Redis Operations**: Lua function execution completes within <10ms per operation
- **Historical Data**: InfluxDB ingestion rate >10,000 points/second

### Reliability Criteria
- **Uptime**: Services maintain >99% uptime during load tests
- **Error Recovery**: Services recover from failures within 30 seconds
- **Data Integrity**: No data loss during network interruptions
- **Resource Usage**: Memory usage remains stable over 24-hour test runs

This comprehensive integration testing strategy ensures VoltageEMS maintains high quality, performance, and reliability across all system components and use cases.