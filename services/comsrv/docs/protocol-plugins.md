# 协议插件开发指南

## 概述

comsrv 采用插件化架构，支持通过实现标准接口来添加新的工业通信协议。本指南介绍如何开发自定义协议插件。

## 核心接口

### ProtocolPlugin Trait

所有协议插件必须实现 `ProtocolPlugin` trait：

```rust
use async_trait::async_trait;
use voltage_libs::types::{StandardFloat, PointData};

#[async_trait]
pub trait ProtocolPlugin: Send + Sync {
    /// 初始化插件
    async fn initialize(&mut self, config: PluginConfig) -> Result<()>;
    
    /// 启动数据采集
    async fn start(&mut self) -> Result<()>;
    
    /// 停止数据采集
    async fn stop(&mut self) -> Result<()>;
    
    /// 采集一次数据
    async fn collect_data(&self) -> Result<Vec<PointData>>;
    
    /// 发送控制命令
    async fn send_command(&self, command: ControlCommand) -> Result<()>;
    
    /// 获取插件信息
    fn get_info(&self) -> PluginInfo;
}
```

### 数据结构

```rust
pub struct PluginConfig {
    pub channel_id: u16,
    pub transport_config: TransportConfig,
    pub protocol_params: serde_json::Value,
    pub points_config: PointsConfig,
}

pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub supported_transports: Vec<String>,
}

pub struct ControlCommand {
    pub point_id: u32,
    pub value: StandardFloat,
    pub command_type: CommandType,
}

pub enum CommandType {
    Control,      // 遥控
    Adjustment,   // 遥调
}
```

## 开发步骤

### 1. 创建插件结构

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct MyProtocolPlugin {
    channel_id: u16,
    transport: Arc<Mutex<Box<dyn Transport>>>,
    config: ProtocolConfig,
    points: PointsManager,
}

#[derive(Debug, Deserialize)]
struct ProtocolConfig {
    device_address: u8,
    polling_interval: u64,
    timeout: u64,
    // 协议特定参数
}
```

### 2. 实现初始化

```rust
#[async_trait]
impl ProtocolPlugin for MyProtocolPlugin {
    async fn initialize(&mut self, config: PluginConfig) -> Result<()> {
        // 解析协议配置
        self.config = serde_json::from_value(config.protocol_params)?;
        
        // 加载点位配置
        self.points = PointsManager::from_config(config.points_config)?;
        
        // 初始化传输层
        self.transport.lock().await.connect().await?;
        
        info!("MyProtocol plugin initialized for channel {}", self.channel_id);
        Ok(())
    }
}
```

### 3. 实现数据采集

```rust
async fn collect_data(&self) -> Result<Vec<PointData>> {
    let mut results = Vec::new();
    let mut transport = self.transport.lock().await;
    
    // 采集遥测数据
    for point in self.points.get_telemetry_points() {
        let request = build_read_request(point);
        transport.send(&request).await?;
        
        let response = transport.receive().await?;
        let value = parse_response(&response, point)?;
        
        results.push(PointData {
            channel_id: self.channel_id,
            point_type: "m".to_string(),
            point_id: point.id,
            value: StandardFloat::new(value),
            timestamp: chrono::Utc::now().timestamp_millis(),
        });
    }
    
    // 采集遥信数据
    for point in self.points.get_signal_points() {
        // 类似处理...
    }
    
    Ok(results)
}
```

### 4. 实现控制命令

```rust
async fn send_command(&self, command: ControlCommand) -> Result<()> {
    let mut transport = self.transport.lock().await;
    
    // 查找点位配置
    let point = self.points.find_point(command.point_id)
        .ok_or_else(|| Error::PointNotFound(command.point_id))?;
    
    // 构建协议命令
    let request = match command.command_type {
        CommandType::Control => {
            build_control_request(point, command.value)
        }
        CommandType::Adjustment => {
            build_adjustment_request(point, command.value)
        }
    };
    
    // 发送命令
    transport.send(&request).await?;
    
    // 等待响应
    let response = transport.receive().await?;
    verify_response(&response)?;
    
    info!("Command sent successfully: point_id={}, value={}", 
          command.point_id, command.value.to_redis());
    
    Ok(())
}
```

## 点位管理

### PointsManager 实现

```rust
pub struct PointsManager {
    telemetry: HashMap<u32, PointConfig>,
    signals: HashMap<u32, PointConfig>,
    controls: HashMap<u32, PointConfig>,
    adjustments: HashMap<u32, PointConfig>,
}

impl PointsManager {
    pub fn from_config(config: PointsConfig) -> Result<Self> {
        let mut manager = Self::default();
        
        // 加载 CSV 文件
        manager.load_csv(&config.telemetry_path, PointType::Telemetry)?;
        manager.load_csv(&config.signal_path, PointType::Signal)?;
        manager.load_csv(&config.control_path, PointType::Control)?;
        manager.load_csv(&config.adjustment_path, PointType::Adjustment)?;
        
        Ok(manager)
    }
    
    fn load_csv(&mut self, path: &Path, point_type: PointType) -> Result<()> {
        let mut reader = csv::Reader::from_path(path)?;
        
        for record in reader.deserialize() {
            let point: PointConfig = record?;
            
            match point_type {
                PointType::Telemetry => self.telemetry.insert(point.id, point),
                PointType::Signal => self.signals.insert(point.id, point),
                PointType::Control => self.controls.insert(point.id, point),
                PointType::Adjustment => self.adjustments.insert(point.id, point),
            };
        }
        
        Ok(())
    }
}
```

### CSV 格式

```csv
id,name,address,scale,offset,unit
10001,"电压A相","1:3:0",0.1,0,"V"
10002,"电流A相","1:3:2",0.01,0,"A"
10003,"有功功率","1:3:4",1,0,"kW"
```

## 传输层集成

### 使用现有传输层

```rust
// TCP 传输
let transport = TcpTransport::new("192.168.1.100", 502);

// 串口传输
let transport = SerialTransport::new("/dev/ttyUSB0", 9600);

// Mock 传输（用于测试）
let transport = MockTransport::new();
```

### 自定义传输层

```rust
#[async_trait]
impl Transport for CustomTransport {
    async fn connect(&mut self) -> Result<()> {
        // 建立连接
    }
    
    async fn disconnect(&mut self) -> Result<()> {
        // 断开连接
    }
    
    async fn send(&mut self, data: &[u8]) -> Result<()> {
        // 发送数据
    }
    
    async fn receive(&mut self, timeout: Duration) -> Result<Vec<u8>> {
        // 接收数据
    }
}
```

## 错误处理

### 定义错误类型

```rust
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    #[error("Timeout waiting for response")]
    Timeout,
    
    #[error("Device error: {0}")]
    DeviceError(u8),
    
    #[error("Point not found: {0}")]
    PointNotFound(u32),
}
```

### 重试机制

```rust
async fn collect_with_retry(&self) -> Result<Vec<PointData>> {
    let mut attempts = 0;
    let max_attempts = 3;
    
    loop {
        match self.collect_data().await {
            Ok(data) => return Ok(data),
            Err(e) if attempts < max_attempts => {
                attempts += 1;
                warn!("Collection failed, retry {}/{}: {}", 
                      attempts, max_attempts, e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

## 注册插件

### 插件工厂

```rust
pub struct ProtocolFactory;

impl ProtocolFactory {
    pub fn create(
        protocol_type: &str,
        config: PluginConfig,
    ) -> Result<Box<dyn ProtocolPlugin>> {
        match protocol_type {
            "modbus_tcp" => Ok(Box::new(ModbusTcpPlugin::new(config)?)),
            "modbus_rtu" => Ok(Box::new(ModbusRtuPlugin::new(config)?)),
            "iec104" => Ok(Box::new(Iec104Plugin::new(config)?)),
            "my_protocol" => Ok(Box::new(MyProtocolPlugin::new(config)?)),
            _ => Err(Error::UnknownProtocol(protocol_type.to_string())),
        }
    }
}
```

## 测试

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_data_collection() {
        // 使用 Mock 传输层
        let transport = MockTransport::new();
        transport.add_response(vec![0x01, 0x03, 0x04, 0x00, 0x64]);
        
        let plugin = MyProtocolPlugin::new_with_transport(
            Box::new(transport)
        );
        
        let data = plugin.collect_data().await.unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].value.to_redis(), "10.000000");
    }
}
```

### 集成测试

```rust
#[tokio::test]
async fn test_with_redis() {
    // 启动测试 Redis
    let redis_client = setup_test_redis().await;
    
    // 创建插件
    let plugin = create_test_plugin();
    
    // 采集并发布数据
    let data = plugin.collect_data().await.unwrap();
    publish_to_redis(&redis_client, &data).await.unwrap();
    
    // 验证数据
    let stored = redis_client.hget("comsrv:1001:m", "10001").await.unwrap();
    assert_eq!(stored, "25.123456");
}
```

## 最佳实践

### 1. 性能优化

- 批量读取多个点位
- 使用连接池复用连接
- 实现本地缓存减少重复读取

### 2. 可靠性

- 实现自动重连机制
- 添加超时处理
- 记录详细日志便于调试

### 3. 可维护性

- 使用配置文件而非硬编码
- 实现完善的错误类型
- 编写充分的文档和测试

## 示例插件

完整的示例插件代码可以参考：

- `src/plugins/modbus/` - Modbus 协议实现
- `src/plugins/iec104/` - IEC 60870-5-104 实现
- `src/plugins/can/` - CAN 总线协议实现

## 常见问题

### Q: 如何处理不同数据类型的转换？

```rust
fn convert_to_standard_float(
    raw_value: u16,
    point_config: &PointConfig,
) -> StandardFloat {
    let scaled = raw_value as f64 * point_config.scale + point_config.offset;
    StandardFloat::new(scaled)
}
```

### Q: 如何实现协议特定的地址解析？

```rust
fn parse_address(address: &str) -> Result<(u8, u16)> {
    // 格式: "device_id:register"
    let parts: Vec<&str> = address.split(':').collect();
    if parts.len() != 2 {
        return Err(Error::InvalidAddress);
    }
    
    let device_id = parts[0].parse()?;
    let register = parts[1].parse()?;
    
    Ok((device_id, register))
}
```

### Q: 如何处理异步轮询？

```rust
pub async fn run_polling_loop(&self) {
    let interval = Duration::from_millis(self.config.polling_interval);
    let mut ticker = tokio::time::interval(interval);
    
    loop {
        ticker.tick().await;
        
        match self.collect_data().await {
            Ok(data) => {
                // 处理数据
                self.process_data(data).await;
            }
            Err(e) => {
                error!("Collection error: {}", e);
            }
        }
    }
}
```