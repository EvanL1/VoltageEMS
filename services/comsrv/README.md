# Communication Service (comsrv)

通信服务模块位于 `services/comsrv/` 目录，提供统一的工业通信协议支持。
该服务使用 Rust 实现，架构可扩展、异步且具备高性能。
## 🏗️ 架构概览

### 核心组件

```
comsrv/
├── src/
│   ├── core/
│   │   ├── config/           # 配置管理
│   │   │   ├── config_manager.rs   # 通道配置
│   │   │   └── point_table.rs      # 点表管理
│   │   ├── protocols/        # 协议实现
│   │   │   ├── common.rs           # 通用接口定义
│   │   │   ├── factory.rs          # 协议工厂
│   │   │   ├── modbus/             # Modbus协议
│   │   │   │   ├── client.rs       # Modbus客户端
│   │   │   │   ├── common.rs       # Modbus数据类型
│   │   │   │   └── mod.rs
│   │   │   ├── can/                # CAN协议
│   │   │   │   ├── client.rs       # CAN客户端
│   │   │   │   ├── common.rs       # CAN数据类型
│   │   │   │   ├── frame.rs        # CAN帧处理
│   │   │   │   └── mod.rs
│   │   │   └── iec104/             # IEC 104协议（扩展）
│   │   └── service/          # 服务层
│   ├── utils/               # 工具函数
│   └── lib.rs
├── tests/                   # 集成测试
│   ├── modbus_integration_tests.rs
│   ├── modbus_error_scenarios.rs
│   ├── modbus_protocol_integration.rs
│   ├── modbus_rtu_tests.rs
│   └── can_integration_tests.rs
└── examples/               # 使用示例
    └── usage_example.rs
```

### 设计原则

1. **统一接口**：所有协议客户端实现 `ComBase` trait，提供一致的API
2. **工厂模式**：使用 `ProtocolFactory` 动态创建不同协议的客户端
3. **异步设计**：全面支持 async/await，高并发性能
4. **可扩展性**：通过 trait 系统轻松添加新协议
5. **配置驱动**：通过配置文件管理通道和点表
6. **类型安全**：使用强类型系统确保运行时安全

## 📡 支持的协议

### 1. Modbus TCP
- **端口**：默认 502
- **功能码**：支持 1, 2, 3, 4, 5, 6, 15, 16
- **数据类型**：Bool, UInt16, Int16, UInt32, Int32, UInt64, Int64, Float32, Float64
- **优化**：自动读取组优化，减少网络请求

### 2. Modbus RTU
- **串口**：支持 RS485/RS232
- **波特率**：1200-115200 bps
- **校验**：CRC-16/Modbus
- **时序**：严格遵循3.5字符时间间隔

### 3. CAN Bus
- **接口**：SocketCAN, Peak CAN, USB CAN
- **速率**：10K-1M bps
- **帧格式**：标准帧(11-bit)和扩展帧(29-bit)
- **数据提取**：支持位字段、多字节数据类型

### 4. IEC 104 (规划中)
- **传输**：TCP/IP
- **对象**：遥测、遥信、遥控
- **时间戳**：高精度时间同步

### 5. IEC 61850 (规划中)
- **协议栈**：MMS over TCP/IP
- **模型**：IED 数据模型
- **服务**：GOOSE, SV, MMS

## 🚀 快速开始

### 基本使用

```rust
use comsrv::core::protocols::factory::create_default_factory;
use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建协议工厂
    let factory = create_default_factory();
    
    // 2. 获取默认配置
    let mut config = factory.get_default_config(&ProtocolType::ModbusTcp).unwrap();
    config.parameters.get_mut("host").map(|h| *h = serde_yaml::Value::String("192.168.1.100".to_string()));
    
    // 3. 创建客户端
    let client = factory.create_client("PLC_001", config).await?;
    
    // 4. 启动客户端
    client.set_running(true).await;
    
    // 5. 获取状态
    let status = client.status().await;
    println!("Client status: {:?}", status);
    
    Ok(())
}
```

### Modbus TCP 示例

```rust
use comsrv::core::protocols::modbus::client::ModbusClientBase;
use comsrv::core::protocols::modbus::common::*;

// 创建 Modbus TCP 配置
let config = ChannelConfig {
    id: 1,
    name: "PLC_Main".to_string(),
    description: "主PLC通信通道".to_string(),
    protocol: ProtocolType::ModbusTcp,
    parameters: create_modbus_tcp_parameters(),
};

// 创建客户端
let client = ModbusClientBase::new("PLC_Main", config);

// 加载点表
let mappings = vec![
    ModbusRegisterMapping {
        name: "temperature".to_string(),
        display_name: Some("温度".to_string()),
        register_type: ModbusRegisterType::HoldingRegister,
        address: 1000,
        data_type: ModbusDataType::Float32,
        scale: 0.1,
        offset: 0.0,
        unit: Some("°C".to_string()),
        description: Some("环境温度".to_string()),
        access_mode: "read".to_string(),
        group: Some("environmental".to_string()),
        byte_order: ByteOrder::BigEndian,
    },
];

client.load_register_mappings(mappings).await;

// 启动采集
client.set_running(true).await;
```

### CAN Bus 示例

```rust
use comsrv::core::protocols::can::client::CanClientBase;
use comsrv::core::protocols::can::common::*;

// 创建 CAN 配置
let config = ChannelConfig {
    id: 2,
    name: "CAN_Engine".to_string(),
    description: "发动机CAN总线".to_string(),
    protocol: ProtocolType::Can,
    parameters: create_can_parameters(),
};

let client = CanClientBase::new("CAN_Engine", config);

// 加载消息映射
let mappings = vec![
    CanMessageMapping {
        name: "engine_rpm".to_string(),
        display_name: Some("发动机转速".to_string()),
        can_id: 0x123,
        frame_format: CanFrameFormat::Standard,
        data_config: CanDataConfig {
            data_type: CanDataType::UInt16,
            start_byte: 0,
            bit_offset: 0,
            bit_length: 16,
            byte_order: CanByteOrder::BigEndian,
        },
        scale: 0.25,
        offset: 0.0,
        unit: Some("RPM".to_string()),
        description: Some("发动机转速".to_string()),
        access_mode: "read".to_string(),
        transmission_rate: 10.0,
    },
];

client.load_message_mappings(mappings).await;
```

## 🔧 配置管理

### 通道配置

```yaml
# channels.yaml
channels:
  - id: 1
    name: "主PLC"
    description: "西门子S7-1200"
    protocol: ModbusTcp
    parameters:
      host: "192.168.1.100"
      port: 502
      slave_id: 1
      timeout: 1000
      retry_count: 3

  - id: 2
    name: "RTU设备"
    description: "温湿度传感器"
    protocol: ModbusRtu
    parameters:
      port: "/dev/ttyUSB0"
      baud_rate: 9600
      data_bits: 8
      stop_bits: 1
      parity: "None"
      slave_id: 1

  - id: 3
    name: "CAN总线"
    description: "车辆CAN网络"
    protocol: Can
    parameters:
      interface: "socketcan:can0"
      bit_rate: 500000
      timeout: 1000
```

### 点表配置

```csv
# modbus_points.csv
name,display_name,register_type,address,data_type,scale,offset,unit,description,access_mode,group
temp_01,温度1,holding_register,1000,float32,0.1,0,°C,环境温度,read,environmental
pressure_01,压力1,input_register,2000,uint16,0.01,0,kPa,系统压力,read,pressure
status_01,状态1,coil,3000,bool,1,0,,运行状态,read_write,status
```

## 🧪 测试

### 运行所有测试

```bash
# 运行单元测试
cargo test

# 运行集成测试
cargo test --test modbus_integration_tests
cargo test --test modbus_rtu_tests
cargo test --test can_integration_tests

# 运行性能测试
cargo test --release test_performance

# 运行特定协议测试
cargo test modbus
cargo test can
```

### 测试覆盖

- **功能测试**：协议基本功能、数据转换、错误处理
- **性能测试**：大量点表、高频采集、内存使用
- **可靠性测试**：网络中断、设备故障、超时处理
- **兼容性测试**：不同厂商设备、协议变种

## 🔌 扩展新协议

### 1. 创建协议模块

```rust
// src/core/protocols/iec104/mod.rs
pub mod common;
pub mod client;

pub use common::*;
pub use client::*;
```

### 2. 定义数据类型

```rust
// src/core/protocols/iec104/common.rs
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Iec104ObjectType {
    SinglePointInformation,
    DoublePointInformation,
    MeasuredValueShort,
    MeasuredValueFloat,
    // ... 其他对象类型
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iec104ObjectMapping {
    pub name: String,
    pub object_address: u32,
    pub object_type: Iec104ObjectType,
    pub scale: f64,
    pub offset: f64,
    // ... 其他字段
}
```

### 3. 实现客户端

```rust
// src/core/protocols/iec104/client.rs
use async_trait::async_trait;
use crate::core::protocols::common::{ComBase, ComBaseImpl, ChannelStatus, PointData};

#[async_trait]
pub trait Iec104Client: ComBase {
    async fn send_command(&self, address: u32, value: f64) -> Result<()>;
    async fn interrogation(&self) -> Result<Vec<PointData>>;
    // ... 其他IEC 104特定方法
}

pub struct Iec104ClientBase {
    pub base: ComBaseImpl,
    // ... IEC 104特定字段
}

#[async_trait]
impl Iec104Client for Iec104ClientBase {
    async fn send_command(&self, address: u32, value: f64) -> Result<()> {
        // 实现IEC 104命令发送
        Ok(())
    }

    async fn interrogation(&self) -> Result<Vec<PointData>> {
        // 实现总召唤
        Ok(vec![])
    }
}

#[async_trait]
impl ComBase for Iec104ClientBase {
    // 实现通用接口
}
```

### 4. 创建协议工厂

```rust
// 在 src/core/protocols/factory.rs 中添加
pub struct Iec104Factory;

#[async_trait]
impl ProtocolClientFactory for Iec104Factory {
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::Iec104
    }
    
    async fn create_client(&self, name: &str, config: ChannelConfig) -> Result<DynComClient> {
        use crate::core::protocols::iec104::client::Iec104ClientBase;
        let client = Iec104ClientBase::new(name, config);
        Ok(Arc::new(client) as DynComClient)
    }
    
    fn validate_config(&self, config: &ChannelConfig) -> Result<()> {
        // 验证IEC 104配置
        Ok(())
    }
    
    fn default_config(&self) -> ChannelConfig {
        // 返回默认IEC 104配置
    }
    
    fn config_schema(&self) -> serde_json::Value {
        // 返回配置架构
    }
}
```

### 5. 注册协议

```rust
// 在 create_default_factory() 中添加
factory.register_factory(Iec104Factory);
```

### 6. 编写测试

```rust
// tests/iec104_integration_tests.rs
#[tokio::test]
async fn test_iec104_client_creation() {
    let factory = create_default_factory();
    let config = factory.get_default_config(&ProtocolType::Iec104).unwrap();
    let client = factory.create_client("TestIEC104", config).await;
    assert!(client.is_ok());
}
```

## 📊 性能优化

### Modbus 优化

- **读取组优化**：自动合并连续寄存器读取
- **连接复用**：TCP连接池管理
- **并发控制**：限制同时请求数量

### CAN 优化

- **消息过滤**：硬件级别消息过滤
- **批量处理**：批量处理接收的消息
- **零拷贝**：减少内存拷贝开销

### 通用优化

- **异步I/O**：全异步网络和串口操作
- **内存池**：复用内存分配
- **无锁设计**：最小化锁争用

## 🔍 监控和诊断

### 状态监控

```rust
// 获取通道状态
let status = client.status().await;
println!("连接状态: {}", if status.connected { "已连接" } else { "未连接" });
println!("最后响应: {:?}", status.last_response_time);
println!("最后错误: {:?}", status.last_error);
```

### 统计信息

```rust
// 对于支持统计的协议
if let Some(stats) = client.get_statistics().await {
    println!("发送消息: {}", stats.messages_sent);
    println!("接收消息: {}", stats.messages_received);
    println!("错误消息: {}", stats.error_messages);
}
```

### 日志记录

```rust
use log::{info, warn, error, debug};

// 在代码中使用结构化日志
info!("客户端 {} 已启动", client.name());
warn!("连接超时，正在重试...");
error!("通信错误: {}", error);
debug!("接收到数据: {:?}", data);
```

## 🛡️ 错误处理

### 错误类型

```rust
use crate::utils::ComSrvError;

match result {
    Ok(data) => println!("成功: {:?}", data),
    Err(ComSrvError::ConnectionTimeout) => {
        println!("连接超时，请检查网络");
    },
    Err(ComSrvError::InvalidData(msg)) => {
        println!("数据格式错误: {}", msg);
    },
    Err(ComSrvError::UnsupportedProtocol(protocol)) => {
        println!("不支持的协议: {}", protocol);
    },
    Err(e) => println!("其他错误: {:?}", e),
}
```

### 重试机制

```rust
// 自动重试配置
config.parameters.insert("retry_count", 3);
config.parameters.insert("retry_delay", 1000); // 毫秒
```

## 📈 路线图

### 短期目标 (3个月)
- [ ] 完善 CAN FD 支持
- [ ] 添加 Modbus ASCII 协议
- [ ] 实现配置热重载
- [ ] 性能优化和内存使用改进

### 中期目标 (6个月)
- [ ] IEC 104 协议完整实现
- [ ] OPC UA 客户端支持
- [ ] 分布式部署支持
- [ ] Web 管理界面

### 长期目标 (12个月)
- [ ] IEC 61850 协议支持
- [ ] DNP3 协议支持
- [ ] 实时数据库集成
- [ ] 云平台连接器

## 🤝 贡献指南

### 提交代码

1. Fork 项目
2. 创建特性分支：`git checkout -b feature/new-protocol`
3. 提交更改：`git commit -am 'feat(protocol): add IEC 104 support'`
4. 推送分支：`git push origin feature/new-protocol`
5. 提交 Pull Request

### 代码规范

- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 检查代码质量
- 遵循 [Conventional Commits](https://www.conventionalcommits.org/) 规范
- 添加必要的单元测试和集成测试
- 更新相关文档

### 问题报告

请使用 GitHub Issues 报告问题，包含：
- 环境信息（操作系统、Rust版本）
- 复现步骤
- 期望行为和实际行为
- 相关日志和错误信息

## 📄 许可证

本项目采用 MIT 许可证。详情请参阅 [LICENSE](LICENSE) 文件。

---

**注意**：本文档持续更新中，如有疑问请查看代码注释或提交 Issue。 