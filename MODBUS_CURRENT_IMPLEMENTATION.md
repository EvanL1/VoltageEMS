# 当前Modbus实现使用指南

## 🏗️ 架构概览

现在的Modbus实现已经完全简化和现代化，只有一个核心客户端和一套配置系统：

```
📦 Modbus模块结构
├── 🎯 ModbusClient        # 唯一的客户端实现
├── ⚡ ModbusProtocolEngine # 高性能协议引擎  
├── 🔄 EnhancedTransportBridge # 增强传输桥接
├── ⚙️  ConfigManager      # 现代化配置系统
└── 📊 BasicMonitoring     # 内置监控诊断
```

## 🚀 完整使用示例

### 1. 配置文件 (config.yaml)

```yaml
service:
  name: "VoltageEMS-ComSrv"
  port: 8080
  max_connections: 100

channels:
  - id: 1
    name: "PLC_01"
    protocol: "modbus_tcp"
    enabled: true
    connection:
      host: "192.168.1.100"
      port: 502
      timeout_ms: 5000
      max_retries: 3
      retry_delay_ms: 1000
    
    points:
      # 遥测点位（模拟量）
      - id: 1001
        name: "温度传感器1"
        type: "telemetry"
        enabled: true
        protocol_mapping:
          slave_id: 1
          function_code: 3           # 读保持寄存器
          address: 1000
          count: 2                   # float32需要2个寄存器
          data_type: "float32"
          byte_order: "ABCD"
        processing:
          scale: 0.1
          offset: -50.0
          unit: "°C"
          min_value: -100.0
          max_value: 100.0
          decimal_places: 1
        description: "主控制器温度传感器"
      
      # 遥信点位（数字量）
      - id: 2001
        name: "运行状态"
        type: "signaling"
        enabled: true
        protocol_mapping:
          slave_id: 1
          function_code: 1           # 读线圈
          address: 2000
          data_type: "bool"
          bit_position: 0
        processing:
          value_mapping:
            "0": "停止"
            "1": "运行"
        description: "设备运行状态指示"
      
      # 遥调点位（模拟量输出）
      - id: 3001
        name: "设定温度"
        type: "setpoint"
        enabled: true
        protocol_mapping:
          slave_id: 1
          function_code: 6           # 写单个寄存器
          address: 3000
          data_type: "uint16"
        processing:
          scale: 0.1
          unit: "°C"
          min_value: 0.0
          max_value: 100.0
        description: "温度设定值"
      
      # 遥控点位（数字量输出）
      - id: 4001
        name: "启停控制"
        type: "control"
        enabled: true
        protocol_mapping:
          slave_id: 1
          function_code: 5           # 写单个线圈
          address: 4000
          data_type: "bool"
        processing:
          value_mapping:
            "false": "停止"
            "true": "启动"
        description: "设备启停控制"

polling:
  interval_ms: 1000
  batch_enabled: true
  batch_size: 10
  priority: "normal"

logging:
  level: "info"
  max_file_size_mb: 10
  max_files: 5
```

### 2. Rust代码示例

```rust
use std::time::Duration;
use tracing::{info, error};

// 导入核心组件
use comsrv::core::config::{ConfigManager, NewChannelConfig};
use comsrv::core::protocols::modbus::{
    ModbusClient, ModbusChannelConfig, ProtocolMappingTable
};
use comsrv::core::protocols::common::combase::{
    BasicMonitoring, TelemetryType
};
use comsrv::core::transport::tcp::TcpTransport;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt::init();
    
    // 1. 🔧 加载配置
    let config_manager = ConfigManager::from_file("config.yaml").await?;
    let channel_config = config_manager.get_channel_config(1)
        .ok_or("通道1未找到")?;
    
    info!("加载配置完成: {}", channel_config.name);
    
    // 2. 🚀 创建Modbus客户端
    let modbus_config = config_manager.to_modbus_channel_config(channel_config);
    let transport = create_transport(&channel_config.connection).await?;
    let client = ModbusClient::new(modbus_config, transport).await?;
    
    // 3. 📊 设置监控
    let monitoring = BasicMonitoring::new("modbus_client".to_string());
    
    // 4. 🔗 连接设备
    client.connect().await?;
    info!("已连接到Modbus设备");
    
    // 5. 📖 读取数据示例
    demo_read_operations(&client, &monitoring).await?;
    
    // 6. ✍️ 写入数据示例
    demo_write_operations(&client, &monitoring).await?;
    
    // 7. 📈 显示统计信息
    display_statistics(&client, &monitoring).await;
    
    // 8. 🔌 断开连接
    client.disconnect().await?;
    info!("已断开连接");
    
    Ok(())
}

/// 创建传输层
async fn create_transport(config: &ConnectionConfig) -> Result<Box<dyn Transport>, Box<dyn std::error::Error>> {
    let transport = TcpTransport::new(
        config.host.as_ref().unwrap(),
        config.port.unwrap(),
        Duration::from_millis(config.timeout_ms as u64)
    ).await?;
    
    Ok(Box::new(transport))
}

/// 演示读取操作
async fn demo_read_operations(
    client: &ModbusClient, 
    monitoring: &BasicMonitoring
) -> Result<(), Box<dyn std::error::Error>> {
    info!("=== 读取操作演示 ===");
    
    // 读取遥测点位（温度传感器）
    let start_time = std::time::Instant::now();
    match client.read_point(1001, TelemetryType::Telemetry).await {
        Ok(point_data) => {
            let response_time = start_time.elapsed().as_millis() as u64;
            info!("✅ 遥测读取成功: {} = {} {}", 
                  point_data.name, point_data.value, point_data.unit);
            monitoring.record_request(true, response_time).await;
        }
        Err(e) => {
            let response_time = start_time.elapsed().as_millis() as u64;
            error!("❌ 遥测读取失败: {}", e);
            monitoring.record_request(false, response_time).await;
        }
    }
    
    // 读取遥信点位（运行状态）
    let start_time = std::time::Instant::now();
    match client.read_point(2001, TelemetryType::Signaling).await {
        Ok(point_data) => {
            let response_time = start_time.elapsed().as_millis() as u64;
            info!("✅ 遥信读取成功: {} = {}", 
                  point_data.name, point_data.value);
            monitoring.record_request(true, response_time).await;
        }
        Err(e) => {
            let response_time = start_time.elapsed().as_millis() as u64;
            error!("❌ 遥信读取失败: {}", e);
            monitoring.record_request(false, response_time).await;
        }
    }
    
    // 批量读取所有点位
    let all_points = client.get_all_points().await;
    info!("📊 批量读取完成，共 {} 个点位", all_points.len());
    for point in &all_points {
        info!("  📍 {}: {} {}", point.name, point.value, point.unit);
    }
    
    Ok(())
}

/// 演示写入操作
async fn demo_write_operations(
    client: &ModbusClient,
    monitoring: &BasicMonitoring
) -> Result<(), Box<dyn std::error::Error>> {
    info!("=== 写入操作演示 ===");
    
    // 写入遥调点位（设定温度）
    let start_time = std::time::Instant::now();
    match client.write_point("3001", "25.5").await {
        Ok(_) => {
            let response_time = start_time.elapsed().as_millis() as u64;
            info!("✅ 遥调写入成功: 设定温度 = 25.5°C");
            monitoring.record_request(true, response_time).await;
        }
        Err(e) => {
            let response_time = start_time.elapsed().as_millis() as u64;
            error!("❌ 遥调写入失败: {}", e);
            monitoring.record_request(false, response_time).await;
        }
    }
    
    // 写入遥控点位（启动设备）
    let start_time = std::time::Instant::now();
    match client.write_point("4001", "true").await {
        Ok(_) => {
            let response_time = start_time.elapsed().as_millis() as u64;
            info!("✅ 遥控执行成功: 设备启动");
            monitoring.record_request(true, response_time).await;
        }
        Err(e) => {
            let response_time = start_time.elapsed().as_millis() as u64;
            error!("❌ 遥控执行失败: {}", e);
            monitoring.record_request(false, response_time).await;
        }
    }
    
    Ok(())
}

/// 显示统计信息
async fn display_statistics(client: &ModbusClient, monitoring: &BasicMonitoring) {
    info!("=== 性能统计 ===");
    
    // 客户端统计
    let stats = client.get_statistics().await;
    info!("📊 客户端统计:");
    info!("  总请求数: {}", stats.total_requests);
    info!("  成功请求: {}", stats.successful_requests);
    info!("  失败请求: {}", stats.failed_requests);
    info!("  平均响应时间: {:.1}ms", stats.average_response_time_ms);
    
    // 监控统计
    let metrics = monitoring.get_performance_metrics().await;
    info!("📈 性能指标:");
    info!("  请求速率: {:.2} req/s", metrics.request_rate);
    info!("  成功率: {:.1}%", metrics.success_rate);
    info!("  错误率: {:.1}%", metrics.error_rate);
    info!("  P95响应时间: {:.1}ms", metrics.p95_response_time_ms);
    info!("  运行时间: {}s", metrics.uptime_seconds);
    
    // 健康检查
    if let Ok(health) = client.health_check().await {
        info!("🏥 健康状态:");
        for (key, value) in health {
            info!("  {}: {}", key, value);
        }
    }
    
    // 连接状态
    let connection_state = client.get_connection_state().await;
    info!("🔗 连接信息:");
    info!("  连接状态: {}", if connection_state.connected { "已连接" } else { "未连接" });
    info!("  重试次数: {}", connection_state.retry_count);
    if let Some(last_error) = connection_state.last_error {
        info!("  最后错误: {}", last_error);
    }
}
```

## 🎯 核心特性

### 1. **单一客户端** - 简洁统一
```rust
// 只需要一个客户端类型
let client = ModbusClient::new(config, transport).await?;

// 支持所有标准操作
client.read_point(1001, TelemetryType::Telemetry).await?;  // 遥测
client.read_point(2001, TelemetryType::Signaling).await?;  // 遥信
client.write_point("3001", "25.5").await?;                // 遥调
client.write_point("4001", "true").await?;                // 遥控
```

### 2. **智能缓存** - 性能优化
```rust
// 自动缓存机制（500ms TTL）
// 相同请求会从缓存返回，减少网络开销
let value1 = client.read_point(1001, TelemetryType::Telemetry).await?; // 网络请求
let value2 = client.read_point(1001, TelemetryType::Telemetry).await?; // 缓存命中
```

### 3. **批量操作** - 高效通信
```rust
// 批量读取多个点位
let point_ids = vec![1001, 1002, 1003, 2001, 2002];
let results = client.read_points_batch(&point_ids).await?;

// 或者读取所有点位
let all_points = client.get_all_points().await;
```

### 4. **智能重试** - 高可靠性
```rust
// 自动重试配置
let retry_config = RetryConfig {
    max_retries: 3,
    initial_delay: Duration::from_millis(100),
    max_delay: Duration::from_secs(5),
    backoff_multiplier: 2.0,
    jitter: true,  // 随机抖动避免同时重试
};
```

### 5. **内置监控** - 运维友好
```rust
// 实时性能指标
let metrics = monitoring.get_performance_metrics().await;
println!("成功率: {:.1}%", metrics.success_rate);
println!("P95响应时间: {:.1}ms", metrics.p95_response_time_ms);

// 健康检查
let health = client.health_check().await?;
println!("连接状态: {}", health.get("connected").unwrap());
```

## 📊 支持的数据类型

| 数据类型 | 描述 | 寄存器数量 | 字节序支持 |
|---------|------|-----------|-----------|
| `bool` | 布尔值 | 1 | - |
| `uint16` | 16位无符号整数 | 1 | ABCD |
| `int16` | 16位有符号整数 | 1 | ABCD |
| `uint32` | 32位无符号整数 | 2 | ABCD, DCBA, BADC, CDAB |
| `float32` | 32位浮点数 | 2 | ABCD, DCBA, BADC, CDAB |

## 🔧 配置参数说明

### 连接配置
- `host`: TCP主机地址
- `port`: TCP端口号
- `timeout_ms`: 请求超时时间（毫秒）
- `max_retries`: 最大重试次数
- `retry_delay_ms`: 重试延迟（毫秒）

### 点位配置
- `slave_id`: Modbus从站ID
- `function_code`: Modbus功能码
  - `1`: 读线圈
  - `2`: 读离散输入
  - `3`: 读保持寄存器
  - `4`: 读输入寄存器
  - `5`: 写单个线圈
  - `6`: 写单个寄存器
  - `15`: 写多个线圈
  - `16`: 写多个寄存器
- `address`: 寄存器地址
- `data_type`: 数据类型
- `byte_order`: 字节序（32位数据）

### 数据处理
- `scale`: 缩放因子
- `offset`: 偏移量
- `unit`: 单位
- `min_value`/`max_value`: 取值范围
- `decimal_places`: 小数位数
- `value_mapping`: 值映射（数字量）

## 🚀 性能特性

1. **零拷贝处理**: 数据处理过程中避免不必要的内存拷贝
2. **连接池管理**: 复用连接，减少连接开销
3. **智能缓存**: 自动缓存常用数据，减少网络请求
4. **并发控制**: 支持最多10个并发请求
5. **批量优化**: 自动合并相邻寄存器读取
6. **响应时间跟踪**: 实时监控P95/P99响应时间

这就是现在的Modbus实现 - **简洁、高效、功能完整**！