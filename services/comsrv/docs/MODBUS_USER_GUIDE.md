# Modbus 使用指南

## 目录

1. [快速开始](#快速开始)
2. [配置说明](#配置说明)
3. [使用示例](#使用示例)
4. [性能优化](#性能优化)
5. [故障排查](#故障排查)
6. [API参考](#api参考)

## 快速开始

### 1. 基本概念

VoltageEMS的Modbus实现支持：
- **Modbus TCP**: 基于以太网的Modbus协议
- **Modbus RTU**: 基于串口的Modbus协议
- **四遥映射**: 完整支持遥测(YC)、遥信(YX)、遥控(YK)、遥调(YT)

### 2. 最小配置示例

```yaml
# config/comsrv.yaml
channels:
  - id: 1001
    name: "ModbusDemo"
    protocol: "ModbusTcp"
    enabled: true
    parameters:
      host: "192.168.1.100"
      port: 502
      timeout_ms: 5000
      retry: 3
    table_config:
      four_telemetry_route: "test_points/ModbusDemo"
      protocol_mapping_route: "test_points/ModbusDemo"
```

### 3. 创建点表配置

创建CSV文件定义测量点：

```csv
# telemetry.csv - 遥测点定义
point_id,name,description,unit,data_type,scale,offset
1001,Voltage_A,A相电压,V,float,0.1,0
1002,Current_A,A相电流,A,float,0.01,0

# mapping_telemetry.csv - Modbus地址映射
point_id,signal_name,address,data_type,data_format,number_of_bytes,scale,offset
1001,Voltage_A,1:3:40001,uint16,AB,2,0.1,0
1002,Current_A,1:3:40003,uint16,AB,2,0.01,0
```

地址格式：`slave_id:function_code:register_address`

### 4. 运行测试

```bash
# 启动Modbus模拟器
python3 tests/modbus_csv_simulator.py --csv-dir config/test_points/ModbusDemo

# 运行comsrv
cargo run --release

# 检查Redis中的数据
redis-cli get point:1001
```

## 配置说明

### 通道参数

| 参数 | 类型 | 说明 | 默认值 |
|-----|------|------|--------|
| host | string | Modbus服务器地址 | 必填 |
| port | u16 | 端口号 | 502 |
| timeout_ms | u64 | 超时时间(毫秒) | 5000 |
| retry | u32 | 重试次数 | 3 |
| polling | object | 轮询配置 | 见下表 |

### 轮询配置

```yaml
polling:
  default_interval_ms: 1000    # 默认轮询间隔
  enable_batch_reading: true   # 启用批量读取优化
  max_batch_size: 125         # 最大批量大小
  read_timeout_ms: 5000       # 读取超时
  slave_configs:              # 从站特定配置
    1:
      interval_ms: 500        # 从站1的轮询间隔
      max_concurrent_requests: 1
```

### CSV映射文件格式

#### 遥测映射 (mapping_telemetry.csv)

| 字段 | 说明 | 示例 |
|-----|------|------|
| point_id | 点位ID | 1001 |
| signal_name | 信号名称 | Voltage_A |
| address | Modbus地址 | 1:3:40001 |
| data_type | 数据类型 | uint16/int16/float32 |
| data_format | 字节序 | AB/BA/ABCD/DCBA |
| number_of_bytes | 字节数 | 2/4 |
| scale | 缩放因子 | 0.1 |
| offset | 偏移量 | 0 |

#### 遥信映射 (mapping_signal.csv)

| 字段 | 说明 | 示例 |
|-----|------|------|
| point_id | 点位ID | 2001 |
| signal_name | 信号名称 | Status_1 |
| address | Modbus地址 | 1:1:10001 |
| data_type | 数据类型 | bool |

## 使用示例

### 示例1: 读取单个点位

```rust
use comsrv::core::protocols::modbus::client::ModbusClient;

// 创建客户端
let client = ModbusClient::new(config, transport).await?;

// 连接
client.connect().await?;

// 读取遥测点
let value = client.read_telemetry_point(1001).await?;
println!("电压: {} V", value);

// 读取遥信点
let status = client.read_signal_point(2001).await?;
println!("状态: {}", if status { "ON" } else { "OFF" });
```

### 示例2: 批量读取

```rust
// 批量读取多个遥测点
let point_ids = vec![1001, 1002, 1003];
let values = client.batch_read_telemetry_points(point_ids).await?;

for (id, value) in values {
    println!("Point {}: {}", id, value);
}
```

### 示例3: 写入操作

```rust
// 遥控操作（写单个线圈）
client.write_control_point(3001, true).await?;

// 遥调操作（写寄存器）
client.write_adjustment_point(4001, 100.0).await?;
```

### 示例4: 使用轮询引擎

```rust
use comsrv::core::protocols::modbus::modbus_polling::ModbusPollingEngine;

// 创建轮询引擎
let mut engine = ModbusPollingEngine::new(polling_config);

// 添加监控点
engine.add_points(points);

// 启动轮询
engine.start(read_callback).await?;

// 获取统计信息
let stats = engine.get_stats().await;
println!("成功率: {:.2}%", stats.success_rate() * 100.0);
```

### 示例5: RTU模式配置

```yaml
channels:
  - id: 2001
    name: "ModbusRTU"
    protocol: "ModbusRtu"
    parameters:
      port: "/dev/ttyUSB0"
      baud_rate: 9600
      data_bits: 8
      parity: "None"
      stop_bits: 1
      timeout_ms: 3000
```

## 性能优化

### 1. 批量读取优化

系统自动优化连续寄存器的读取：

```yaml
# 启用批量优化
polling:
  enable_batch_reading: true
  max_batch_size: 125
  
# 批量配置
batch_config:
  max_gap: 10          # 最大地址间隔
  max_batch_size: 50   # 设备限制
  device_limits:
    1:                 # 从站1的限制
      max_registers_per_read: 50
```

优化效果：
- 原始请求：读取40001, 40002, 40003 (3个请求)
- 优化后：读取40001-40003 (1个请求)

### 2. 并发控制

```rust
// 设置并发限制
let engine = ModbusProtocolEngine::with_concurrency_limit(10);

// 从站级别的并发控制
slave_configs:
  1:
    max_concurrent_requests: 1  # 串行访问从站1
  2:
    max_concurrent_requests: 5  # 从站2允许5个并发
```

### 3. 缓存策略

```rust
// 配置缓存
let cache_config = CacheConfig {
    max_size: 1000,
    ttl: Duration::from_secs(10),
    enable_cache: true,
};

client.configure_cache(cache_config).await;
```

### 4. 性能测试结果

在标准硬件上的测试结果：

| 测试场景 | 点位数量 | 优化前 | 优化后 | 提升比例 |
|---------|---------|--------|--------|----------|
| 顺序读取 | 100 | 50 req/s | 50 req/s | 1x |
| 批量读取 | 100 | 50 req/s | 150 req/s | 3x |
| 并发读取 | 1000 | 100 req/s | 800 req/s | 8x |
| 混合场景 | 1500 | 80 req/s | 600 req/s | 7.5x |

## 故障排查

### 常见问题

#### 1. 连接失败

```
Error: Connection refused
```

**解决方案**：
- 检查IP地址和端口
- 确认防火墙设置
- 验证Modbus服务器运行状态

#### 2. 超时错误

```
Error: Operation timed out
```

**解决方案**：
- 增加timeout_ms配置
- 检查网络延迟
- 减少批量读取大小

#### 3. 地址错误

```
Error: Illegal data address
```

**解决方案**：
- 检查CSV映射文件中的地址
- 确认设备支持的地址范围
- 验证功能码是否正确

### 调试工具

#### 1. 启用详细日志

```bash
RUST_LOG=debug cargo run
```

#### 2. 使用测试工具

```bash
# 测试单个点读取
cargo run --example modbus_test_client

# 运行端到端测试
./scripts/run_e2e_csv_test.sh

# 压力测试
./scripts/run_stress_test.sh
```

#### 3. 监控Redis数据

```bash
# 监控所有点位更新
redis-cli --scan --pattern "point:*"

# 实时监控
redis-cli monitor | grep point:
```

## API参考

### ModbusClient

主要客户端接口，提供高层API。

```rust
pub struct ModbusClient {
    // ...
}

impl ModbusClient {
    /// 创建新客户端
    pub async fn new(config: ModbusClientConfig, transport: Box<dyn Transport>) -> Result<Self>
    
    /// 连接到Modbus服务器
    pub async fn connect(&mut self) -> Result<()>
    
    /// 断开连接
    pub async fn disconnect(&mut self) -> Result<()>
    
    /// 读取遥测点
    pub async fn read_telemetry_point(&self, point_id: u32) -> Result<f64>
    
    /// 读取遥信点
    pub async fn read_signal_point(&self, point_id: u32) -> Result<bool>
    
    /// 写入遥控点
    pub async fn write_control_point(&self, point_id: u32, value: bool) -> Result<()>
    
    /// 写入遥调点
    pub async fn write_adjustment_point(&self, point_id: u32, value: f64) -> Result<()>
    
    /// 批量读取
    pub async fn batch_read_points(&self, point_ids: &[u32]) -> Result<Vec<PointData>>
}
```

### ModbusPollingEngine

轮询引擎，支持定时采集。

```rust
pub struct ModbusPollingEngine {
    // ...
}

impl ModbusPollingEngine {
    /// 创建轮询引擎
    pub fn new(config: ModbusPollingConfig) -> Self
    
    /// 添加监控点
    pub fn add_points(&mut self, points: Vec<ModbusPoint>)
    
    /// 启动轮询
    pub async fn start<F>(&self, read_callback: F) -> Result<()>
    where F: Fn(u8, u8, u16, u16) -> BoxFuture<'static, Result<Vec<u16>>>
    
    /// 停止轮询
    pub async fn stop(&self)
    
    /// 获取统计信息
    pub async fn get_stats(&self) -> ModbusPollingStats
}
```

### 数据结构

```rust
/// Modbus点位定义
pub struct ModbusPoint {
    pub point_id: String,
    pub telemetry_type: TelemetryType,
    pub slave_id: u8,
    pub function_code: u8,
    pub register_address: u16,
    pub scale_factor: Option<f64>,
}

/// 批量优化配置
pub struct ModbusBatchConfig {
    pub max_gap: u16,              // 最大地址间隔
    pub max_batch_size: u16,       // 批量大小限制
    pub merge_function_codes: bool, // 是否合并不同功能码
    pub device_limits: HashMap<u8, DeviceLimit>,
}

/// 轮询统计
pub struct ModbusPollingStats {
    pub total_polls: u64,
    pub successful_polls: u64,
    pub failed_polls: u64,
    pub average_poll_time_ms: f64,
    pub slave_stats: HashMap<u8, SlavePollingStats>,
}
```

## 最佳实践

1. **合理设置轮询间隔**
   - 关键数据: 100-500ms
   - 普通数据: 1000-5000ms
   - 状态数据: 5000-10000ms

2. **优化点位分组**
   - 将相邻地址的点位分配到同一从站
   - 按数据更新频率分组
   - 考虑设备的响应能力

3. **错误处理**
   - 实现重试机制
   - 记录错误日志
   - 设置合理的超时时间

4. **性能监控**
   - 定期检查轮询统计
   - 监控网络延迟
   - 跟踪失败率

5. **安全考虑**
   - 限制写操作权限
   - 验证数据范围
   - 使用加密传输（如需要）

## 扩展阅读

- [Modbus协议规范](https://modbus.org/specs.php)
- [实现细节文档](./MODBUS_IMPLEMENTATION.md)
- [性能优化指南](./MODBUS_OPTIMIZATION.md)