# Modbus 通信实现详解

## 1. 概述

comsrv中的Modbus实现采用了分层架构设计，支持Modbus TCP和RTU协议，完整实现了与四遥系统的映射。本文档详细说明了Modbus通信的实现原理、架构设计和使用方法。

## 2. 架构设计

### 2.1 分层架构

```
┌─────────────────────────────────────┐
│      应用层 (Application Layer)      │
│         ModbusClient API            │
├─────────────────────────────────────┤
│    协议引擎层 (Protocol Engine)      │
│      ModbusProtocolEngine           │
├─────────────────────────────────────┤
│   PDU/帧处理层 (PDU/Frame Layer)    │
│  ModbusPduProcessor/FrameProcessor  │
├─────────────────────────────────────┤
│    传输桥接层 (Transport Bridge)    │
│    UniversalTransportBridge         │
├─────────────────────────────────────┤
│      传输层 (Transport Layer)       │
│    TCP / Serial / CAN / Mock       │
└─────────────────────────────────────┘
```

### 2.2 核心组件

#### ModbusClient (`client.rs`)
主要职责：
- 提供高层API接口（read/write操作）
- 管理点位映射和配置
- 维护连接状态和统计信息
- 实现ComBase trait以保持系统兼容性

```rust
pub struct ModbusClient {
    transport_bridge: Arc<UniversalTransportBridge>,
    config: Arc<RwLock<ModbusClientConfig>>,
    protocol_engine: Arc<ModbusProtocolEngine>,
    stats: Arc<RwLock<ProtocolStats>>,
    channel_logger: Option<ChannelLogger>,
}
```

#### ModbusProtocolEngine (`protocol_engine.rs`)
主要职责：
- 协议逻辑处理
- 请求优化和缓存
- 并发控制
- 零拷贝数据处理

```rust
pub struct ModbusProtocolEngine {
    transport_bridge: Arc<UniversalTransportBridge>,
    pdu_processor: Arc<ModbusPduProcessor>,
    frame_processor: Arc<ModbusFrameProcessor>,
    request_cache: Arc<RequestCache>,
    semaphore: Arc<Semaphore>,
    stats: Arc<RwLock<EngineStats>>,
}
```

## 3. Modbus与四遥映射

### 3.1 功能码映射

| 四遥类型 | Modbus功能码 | 寄存器类型 | 数据类型 | 操作方向 |
|---------|-------------|-----------|---------|---------|
| 遥测(YC) | 0x03/0x04 | 保持/输入寄存器 | 模拟量 | 读取 |
| 遥信(YX) | 0x01/0x02 | 线圈/离散输入 | 开关量 | 读取 |
| 遥控(YK) | 0x05/0x0F | 线圈 | 开关量 | 写入 |
| 遥调(YT) | 0x06/0x10 | 保持寄存器 | 模拟量 | 写入 |

### 3.2 数据结构定义

```rust
/// 遥测映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusTelemetryMapping {
    pub point_id: u32,
    pub slave_id: u8,
    pub function_code: u8,      // 0x03 or 0x04
    pub address: u16,
    pub data_type: String,      // float32, int16, uint16
    pub scale: f64,
    pub offset: f64,
}

/// 遥信映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusSignalMapping {
    pub point_id: u32,
    pub slave_id: u8,
    pub function_code: u8,      // 0x01 or 0x02
    pub address: u16,
    pub bit_location: u8,       // 0-15 for bit position
}

/// 遥控映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusControlMapping {
    pub point_id: u32,
    pub slave_id: u8,
    pub function_code: u8,      // 0x05 or 0x0F
    pub address: u16,
}

/// 遥调映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusAdjustmentMapping {
    pub point_id: u32,
    pub slave_id: u8,
    pub function_code: u8,      // 0x06 or 0x10
    pub address: u16,
    pub data_type: String,
    pub scale: f64,
    pub offset: f64,
}
```

## 4. 配置体系

### 4.1 配置文件结构

```
config/
├── comsrv.yaml                 # 主配置文件
└── test_points/
    └── ModbusTCP_Demo/
        ├── telemetry.csv       # 遥测点定义
        ├── signal.csv          # 遥信点定义
        ├── control.csv         # 遥控点定义
        ├── adjustment.csv      # 遥调点定义
        ├── mapping_telemetry.csv    # 遥测协议映射
        ├── mapping_signal.csv       # 遥信协议映射
        ├── mapping_control.csv      # 遥控协议映射
        └── mapping_adjustment.csv   # 遥调协议映射
```

### 4.2 通道配置示例

```yaml
channels:
  - id: 1001
    name: "Modbus_Test_5020"
    protocol: "ModbusTcp"
    enabled: true
    parameters:
      host: "127.0.0.1"
      port: 5020
      timeout: 5000
      retry: 3
    table_config:
      four_telemetry_route: "test_points/ModbusTCP_Demo"
      protocol_mapping_route: "test_points/ModbusTCP_Demo"
```

### 4.3 CSV配置格式

**telemetry.csv** (遥测点定义):
```csv
point_id,name,description,unit,data_type,scale,offset
1001,电压A相,A相线电压,V,float,1.0,0
1002,电流A相,A相线电流,A,float,0.1,0
```

**mapping_telemetry.csv** (遥测映射):
```csv
point_id,signal_name,address,data_type,data_format,number_of_bytes
1001,Voltage_A,40001,uint16,AB,2
1002,Current_A,40003,int16,AB,2
```

## 5. 数据读写流程

### 5.1 读取流程

```rust
// 1. 应用层调用
let value = client.read_telemetry_point(1001).await?;

// 2. 协议引擎处理
async fn read_telemetry_point(&self, point_id: u32) -> Result<f64> {
    // 查找映射
    let mapping = self.find_telemetry_mapping(point_id)?;
    
    // 检查缓存
    if let Some(cached) = self.check_cache(point_id).await {
        return Ok(cached);
    }
    
    // 构建PDU
    let pdu = self.pdu_processor.build_read_holding_registers(
        mapping.address, 
        mapping.number_of_registers()
    )?;
    
    // 构建帧
    let frame = self.frame_processor.build_tcp_frame(
        mapping.slave_id,
        pdu
    )?;
    
    // 发送请求
    let response = self.transport_bridge.send_request(frame).await?;
    
    // 解析响应
    let raw_value = self.parse_response(response, &mapping)?;
    
    // 应用转换
    let value = raw_value * mapping.scale + mapping.offset;
    
    // 更新缓存
    self.update_cache(point_id, value).await;
    
    Ok(value)
}
```

### 5.2 写入流程

```rust
// 1. 应用层调用
client.write_adjustment_point(4001, 100.0).await?;

// 2. 协议引擎处理
async fn write_adjustment_point(&self, point_id: u32, value: f64) -> Result<()> {
    // 查找映射
    let mapping = self.find_adjustment_mapping(point_id)?;
    
    // 逆向转换
    let raw_value = (value - mapping.offset) / mapping.scale;
    
    // 构建PDU
    let pdu = self.pdu_processor.build_write_single_register(
        mapping.address,
        raw_value as u16
    )?;
    
    // 构建帧
    let frame = self.frame_processor.build_tcp_frame(
        mapping.slave_id,
        pdu
    )?;
    
    // 发送请求
    self.transport_bridge.send_request(frame).await?;
    
    Ok(())
}
```

## 6. 传输层集成

### 6.1 UniversalTransportBridge

传输桥接层提供了协议无关的传输接口：

```rust
pub struct UniversalTransportBridge {
    transport: Arc<Mutex<Box<dyn Transport>>>,
    config: ProtocolBridgeConfig,
    stats: Arc<RwLock<BridgeStats>>,
    reconnect_notify: Arc<Notify>,
}
```

### 6.2 支持的传输方式

- **TCP**: Modbus TCP协议
- **Serial**: Modbus RTU协议（RS232/RS485）
- **CAN**: 通过CAN总线传输
- **Mock**: 用于测试的模拟传输

### 6.3 连接管理

```rust
// 自动重连机制
async fn ensure_connected(&self) -> Result<()> {
    let mut transport = self.transport.lock().await;
    
    if !transport.is_connected() {
        for attempt in 0..self.config.retry_count {
            match transport.connect().await {
                Ok(_) => return Ok(()),
                Err(e) if attempt < self.config.retry_count - 1 => {
                    tokio::time::sleep(self.config.retry_interval).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
    Ok(())
}
```

## 7. 性能优化

### 7.1 缓存机制

```rust
pub struct RequestCache {
    cache: Arc<RwLock<HashMap<CacheKey, CachedValue>>>,
    config: CacheConfig,
}

pub struct CacheConfig {
    pub max_size: usize,        // 最大缓存条目
    pub ttl: Duration,          // 缓存过期时间
    pub enable_cache: bool,     // 是否启用缓存
}
```

### 7.2 批量操作

```rust
// 批量读取多个寄存器
pub async fn batch_read_registers(
    &self, 
    slave_id: u8,
    start_address: u16,
    count: u16
) -> Result<Vec<u16>> {
    // 优化：如果地址连续，合并为一个请求
    let optimized_requests = self.optimize_requests(requests)?;
    
    // 并发发送请求
    let futures: Vec<_> = optimized_requests
        .into_iter()
        .map(|req| self.send_request(req))
        .collect();
    
    let results = futures::future::join_all(futures).await;
    
    // 合并结果
    self.merge_results(results)
}
```

### 7.3 并发控制

```rust
// 使用Semaphore限制并发请求数
pub struct ModbusProtocolEngine {
    semaphore: Arc<Semaphore>,  // 默认10个许可
    // ...
}

async fn send_request(&self, request: ModbusRequest) -> Result<ModbusResponse> {
    let _permit = self.semaphore.acquire().await?;
    // 执行请求...
}
```

## 8. 错误处理

### 8.1 错误类型

```rust
#[derive(Debug, thiserror::Error)]
pub enum ModbusError {
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    #[error("Timeout")]
    Timeout,
    
    #[error("Modbus exception: {0:?}")]
    ModbusException(ExceptionCode),
}
```

### 8.2 异常处理

```rust
// Modbus异常码处理
pub enum ExceptionCode {
    IllegalFunction = 0x01,
    IllegalDataAddress = 0x02,
    IllegalDataValue = 0x03,
    SlaveDeviceFailure = 0x04,
    Acknowledge = 0x05,
    SlaveDeviceBusy = 0x06,
    MemoryParityError = 0x08,
    GatewayPathUnavailable = 0x0A,
    GatewayTargetDeviceFailedToRespond = 0x0B,
}
```

## 9. 监控与诊断

### 9.1 统计信息

```rust
pub struct ProtocolStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time_ms: f64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_error: Option<String>,
    pub connection_state: ConnectionState,
}
```

### 9.2 诊断接口

```rust
// 获取诊断信息
pub async fn get_diagnostics(&self) -> ModbusDiagnostics {
    ModbusDiagnostics {
        protocol_stats: self.stats.read().await.clone(),
        engine_stats: self.protocol_engine.get_stats().await,
        transport_stats: self.transport_bridge.get_stats().await,
        cache_stats: self.protocol_engine.get_cache_stats().await,
    }
}
```

## 10. 使用示例

### 10.1 基本使用

```rust
// 创建Modbus客户端
let config = ModbusClientConfig {
    name: "modbus_client".to_string(),
    transport_type: TransportType::Tcp,
    host: "192.168.1.100".to_string(),
    port: 502,
    timeout: Duration::from_secs(5),
    retry_count: 3,
};

let client = ModbusClient::new(config, transport_bridge).await?;

// 读取遥测点
let voltage = client.read_telemetry_point(1001).await?;
println!("电压: {} V", voltage);

// 写入遥调点
client.write_adjustment_point(4001, 100.0).await?;

// 批量读取
let values = client.batch_read_telemetry_points(vec![1001, 1002, 1003]).await?;
```

### 10.2 高级功能

```rust
// 配置缓存
let cache_config = CacheConfig {
    max_size: 1000,
    ttl: Duration::from_secs(10),
    enable_cache: true,
};
client.configure_cache(cache_config).await;

// 获取诊断信息
let diagnostics = client.get_diagnostics().await;
println!("成功率: {:.2}%", diagnostics.success_rate() * 100.0);

// 订阅状态变化
let mut rx = client.subscribe_status_changes().await;
while let Some(status) = rx.recv().await {
    println!("连接状态: {:?}", status);
}
```

## 11. 测试支持

### 11.1 Mock传输层

```rust
// 创建模拟传输用于测试
let mock_transport = MockTransport::new();
mock_transport.add_response(
    vec![0x01, 0x03, 0x02, 0x00, 0x64],  // 请求
    vec![0x01, 0x03, 0x02, 0x12, 0x34],  // 响应
);

let client = ModbusClient::with_transport(Box::new(mock_transport));
```

### 11.2 协议测试

```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_read_holding_registers() {
        let client = create_test_client();
        let result = client.read_holding_registers(1, 40001, 2).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![0x1234, 0x5678]);
    }
}
```

## 12. 最佳实践

1. **合理配置缓存**: 对于频繁读取的数据启用缓存
2. **批量操作优化**: 尽量使用批量读取减少网络往返
3. **错误重试策略**: 配置合理的重试次数和间隔
4. **监控诊断**: 定期检查统计信息，及时发现问题
5. **资源管理**: 正确关闭连接，避免资源泄漏

## 13. 批量读取功能

### 13.1 当前支持情况

系统已经具备批量读取的基础设施：

1. **轮询引擎框架** (`polling.rs`)
   - `PollingEngine` trait定义了标准轮询接口
   - `UniversalPollingEngine` 提供通用实现
   - 支持配置驱动的定时轮询

2. **批量读取方法** (`client.rs`)
   ```rust
   pub async fn read_points_batch(&self, point_ids: &[u32]) -> Result<Vec<PointData>>
   ```

3. **优化机制**
   - 支持按从站ID分组
   - 支持地址连续性优化
   - 并发请求控制

### 13.2 配置文件驱动的批量读取

要实现基于配置文件的批量读取，需要：

1. **实现PointReader接口**
   ```rust
   pub struct ModbusPointReader {
       client: Arc<ModbusClient>,
   }
   
   impl PointReader for ModbusPointReader {
       async fn read_points_batch(&self, point_ids: &[u32]) -> Result<Vec<PointData>>
   }
   ```

2. **配置轮询参数**
   ```yaml
   polling_config:
     enabled: true
     interval_ms: 1000        # 轮询间隔
     batch_size: 100          # 批量大小
     optimize_by_address: true # 地址优化
   ```

3. **启动轮询引擎**
   ```rust
   let engine = UniversalPollingEngine::new("modbus", reader);
   engine.start_polling(config, polling_points).await?;
   ```

### 13.3 地址连续性优化

批量读取可以通过地址连续性优化：

```
原始请求：读取 40001, 40002, 40003, 40010, 40011
优化后：
  - 请求1: 读取 40001-40003 (3个寄存器)
  - 请求2: 读取 40010-40011 (2个寄存器)
```

### 13.4 使用建议

1. **适合批量读取的场景**
   - 大量遥测点定期更新
   - 地址连续的寄存器组
   - 同一从站的多个点位

2. **批量读取配置优化**
   - 合理设置批量大小（建议50-200）
   - 启用地址连续性优化
   - 按从站ID分组请求

3. **注意事项**
   - 某些设备对批量请求大小有限制
   - 网络延迟会影响批量效率
   - 需要权衡实时性和效率

## 14. 性能指标

- 单点读取延迟: < 10ms (局域网)
- 批量读取(100点): < 50ms
- 地址优化后批量读取: 减少50-80%请求数
- 并发请求数: 10-50 (可配置)
- 缓存命中率: > 80% (稳定负载下)
- 内存占用: < 10MB (10000点配置)