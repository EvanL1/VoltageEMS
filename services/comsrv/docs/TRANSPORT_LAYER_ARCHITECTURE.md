# VoltageEMS 传输层架构总结

## 概述

VoltageEMS comsrv 现在采用**分层传输架构**，将物理通信细节从协议逻辑中分离，实现了全面的工业边端接口支持。这种架构兼具微服务的部署灵活性和单体应用的数据一致性优势，非常适合工业实时系统。

## 架构图

```text
┌─────────────────────────────────────────────────────────┐
│                Protocol Layer                           │
│  (Modbus, IEC60870, CAN Protocol Logic)                 │
└─────────────────────────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────┐
│              Transport Interface (Trait)                │
│  connect(), disconnect(), send(), receive()             │
└─────────────────────────────────────────────────────────┘
                             │
     ┌───────────────────────┼───────────────────────┐
     ▼               ▼               ▼               ▼
┌─────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│   TCP   │ │   Serial    │ │    GPIO     │ │    CAN      │
│Transport│ │  Transport  │ │  Transport  │ │  Transport  │
│         │ │             │ │   (DI/DO)   │ │             │
└─────────┘ └─────────────┘ └─────────────┘ └─────────────┘
```

## 支持的工业接口

### 网络通信

- **TCP传输** - 以太网协议支持 (Modbus TCP, IEC60870-104等)
- **串口传输** - RS232/RS485通信 (Modbus RTU, IEC60870-101等)

### 现场I/O接口

- **GPIO传输** - 数字输入输出(DI/DO)、模拟I/O(AI/AO)
- **CAN总线传输** - 汽车/工业控制网络

### 测试支持

- **Mock传输** - 可控制的模拟传输，用于协议测试

## 核心特性

### 1. 统一接口

所有传输层都实现相同的 `Transport` trait：

```rust
#[async_trait]
pub trait Transport: Send + Sync + fmt::Debug {
    async fn connect(&mut self) -> Result<(), TransportError>;
    async fn disconnect(&mut self) -> Result<(), TransportError>;
    async fn send(&mut self, data: &[u8]) -> Result<usize, TransportError>;
    async fn receive(&mut self, buffer: &mut [u8], timeout: Option<Duration>) -> Result<usize, TransportError>;
    async fn is_connected(&self) -> bool;
    async fn stats(&self) -> TransportStats;
    async fn diagnostics(&self) -> HashMap<String, String>;
}
```

### 2. 工厂模式

通过 `TransportFactory` 统一创建和管理传输实例：

```rust
let factory = TransportFactory::new();
let transport = factory.create_tcp_transport(tcp_config).await?;
let transport = factory.create_gpio_transport(gpio_config).await?;
let transport = factory.create_can_transport(can_config).await?;
```

### 3. 配置验证

每种传输都有完整的配置验证：

- TCP: 地址格式、端口范围、超时参数
- 串口: 波特率、数据位、停止位、校验位
- GPIO: 引脚配置、模式验证、唯一性检查
- CAN: 接口名称、比特率、过滤器配置

### 4. 统计和诊断

内置完整的统计信息：

- 连接尝试/成功/失败次数
- 发送/接收字节数
- 连接状态和运行时间
- 传输特定的诊断信息

## 详细实现

### TCP传输

- 支持IPv4/IPv6地址
- TCP keep-alive和no-delay配置
- 连接超时和重试机制
- 缓冲区大小配置

### 串口传输

- 支持所有标准波特率
- 灵活的串口参数配置
- 跨平台串口支持
- 读写超时控制

### GPIO传输

- 数字输入/输出引脚配置
- 上拉/下拉电阻支持
- 引脚防抖功能
- 多引脚批量操作
- 支持Linux GPIO字符设备和树莓派GPIO

### CAN传输

- 标准和扩展帧支持
- 可配置比特率 (125K-1M bps)
- CAN过滤器配置
- CAN FD支持
- RTR帧支持

## 架构优势

### 1. 代码复用性

- 传输实现可在所有协议间共享
- 减少重复代码，提高维护效率

### 2. 测试便利性

- 传输层可独立模拟测试
- 协议测试与物理连接解耦

### 3. 可维护性

- 传输层bug修复一次，所有协议受益
- 清晰的责任分离

### 4. 可扩展性

- 新传输类型自动可用于所有协议
- 协议和传输可独立演进

### 5. 工业就绪

- 全面支持边端设备接口
- 适合实时工业控制系统

## 使用示例

### TCP连接示例

```rust
let config = TcpTransportConfig {
    host: "192.168.1.100".to_string(),
    port: 502,
    timeout: Duration::from_secs(10),
    ..Default::default()
};

let mut transport = TcpTransport::new(config)?;
transport.connect().await?;
```

### GPIO控制示例

```rust
let mut config = GpioTransportConfig::default();
config.pins.push(GpioPinConfig {
    pin: 18,
    mode: GpioPinMode::DigitalOutput,
    initial_value: Some(false),
    label: Some("LED".to_string()),
});

let mut transport = GpioTransport::new(config)?;
transport.connect().await?;
transport.set_digital_output(18, true).await?;
```

### CAN通信示例

```rust
let config = CanTransportConfig {
    interface: "can0".to_string(),
    bit_rate: CanBitRate::Kbps500,
    ..Default::default()
};

let mut transport = CanTransport::new(config)?;
transport.connect().await?;
let frame = CanFrame::new_standard(0x123, vec![1, 2, 3, 4])?;
transport.send_frame(frame).await?;
```

## 依赖和特性

### 核心依赖

- `tokio` - 异步运行时
- `async-trait` - trait异步支持
- `serde` - 配置序列化
- `socket2` - TCP套接字配置

### 可选特性

- `gpio` - GPIO支持 (rppal, gpio-cdev)
- `can` - CAN总线支持 (socketcan)
- `industrial-io` - 工业I/O支持 (i2cdev, spidev)

## 测试覆盖

实现了完整的单元测试覆盖：

- 配置验证测试
- 传输创建和连接测试
- 数据发送接收测试
- 错误处理测试
- 工厂模式测试
- 统计和诊断测试

## 总结

新的分层传输架构为VoltageEMS提供了：

1. **全面的工业接口支持** - TCP、串口、GPIO、CAN等
2. **优雅的架构设计** - 清晰的分层和统一接口
3. **优秀的可维护性** - 代码复用和独立测试
4. **强大的扩展能力** - 轻松添加新的传输类型
5. **工业级可靠性** - 完整的错误处理和统计监控

这种架构使VoltageEMS能够支持更广泛的工业应用场景，同时保持代码的简洁性和可维护性。
