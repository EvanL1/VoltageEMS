# Modbus RTU 测试指南

本文档详细描述了 Modbus RTU 协议实现的完整测试方案，包括单元测试、集成测试和性能测试。

## 目录

1. [测试架构概述](#测试架构概述)
2. [单元测试](#单元测试)
3. [集成测试](#集成测试)
4. [串口模拟器](#串口模拟器)
5. [性能测试](#性能测试)
6. [测试运行指南](#测试运行指南)
7. [故障排查](#故障排查)
8. [测试覆盖率](#测试覆盖率)

## 测试架构概述

Modbus RTU 测试采用分层架构，确保协议实现的可靠性：

```
┌─────────────────────────────────────────┐
│           集成测试                        │
│  - 协议工厂集成测试                       │
│  - 端到端通信测试                         │
│  - 错误处理测试                           │
│  - 性能基准测试                           │
└─────────────────────────────────────────┘
┌─────────────────────────────────────────┐
│           单元测试                        │
│  - 帧编码/解码测试                        │
│  - CRC 计算测试                          │
│  - 时序计算测试                           │
│  - 配置验证测试                           │
└─────────────────────────────────────────┘
┌─────────────────────────────────────────┐
│         模拟器和支持工具                   │
│  - 串口模拟器                            │
│  - RTU 服务器模拟器                       │
│  - 测试数据生成器                         │
└─────────────────────────────────────────┘
```

## 单元测试

### 1. 客户端创建测试

验证 RTU 客户端在不同配置下的正确创建：

```rust
// 文件: tests/modbus_rtu_unit_tests.rs

#[test]
fn test_rtu_client_creation_default() {
    let config = create_test_rtu_config("/dev/ttyUSB0", 9600, 1);
    let client = ModbusRtuClient::new(config);
    
    assert_eq!(client.port_path, "/dev/ttyUSB0");
    assert_eq!(client.baud_rate, 9600);
    assert_eq!(client.base.slave_id(), 1);
}
```

**测试覆盖的场景：**
- 默认配置验证
- 高速通信配置（>19200 波特率）
- 不同奇偶校验设置
- 不同数据位配置
- 不同停止位配置

### 2. 帧编码/解码测试

验证 Modbus RTU 帧的正确编码和解码：

```rust
#[test]
fn test_rtu_frame_encoding() {
    let client = ModbusRtuClient::new(config);
    let frame = client.encode_request(1, 0x03, &[0x00, 0x64, 0x00, 0x0A]);
    
    assert_eq!(frame[0], 1);      // 从站 ID
    assert_eq!(frame[1], 0x03);   // 功能码
    assert_eq!(frame.len(), 8);   // 包含 CRC 的总长度
}

#[test]
fn test_rtu_frame_decoding_success() {
    let response_data = vec![0x01, 0x03, 0x04, 0x12, 0x34, 0x56, 0x78];
    let (slave_id, function_code, data) = client.decode_response(&frame).unwrap();
    
    assert_eq!(slave_id, 1);
    assert_eq!(function_code, 0x03);
    assert_eq!(data, vec![0x04, 0x12, 0x34, 0x56, 0x78]);
}
```

**测试覆盖的场景：**
- 成功的帧解码
- CRC 错误处理
- 异常响应处理
- 帧长度不足错误
- 不同功能码的帧格式

### 3. CRC 计算测试

验证 CRC-16 校验和的正确计算：

```rust
#[test]
fn test_crc16_calculation() {
    let test_vectors = vec![
        (vec![0x01, 0x04, 0x02, 0xFF, 0xFF], 0x80B8),
        (vec![0x11, 0x03, 0x00, 0x6B, 0x00, 0x03], 0x7687),
        (vec![0x01, 0x03, 0x00, 0x00, 0x00, 0x0A], 0xC554),
    ];
    
    for (data, expected_crc) in test_vectors {
        let calculated_crc = client.calculate_crc16(&data);
        assert_eq!(calculated_crc, expected_crc);
    }
}
```

### 4. 时序计算测试

验证字符间隔和帧间隔的正确计算：

```rust
#[test]
fn test_timing_calculations() {
    let baud_rates_and_expected = vec![
        (9600, 1562, 3645),     // 1.5 和 3.5 字符时间
        (19200, 781, 1822),     // 1.5 和 3.5 字符时间
        (115200, 750, 1750),    // 高速固定时序
    ];
    
    for (baud_rate, expected_char, expected_frame) in baud_rates_and_expected {
        let client = ModbusRtuClient::new(create_config(baud_rate));
        assert_approx_eq!(client.char_timeout_us, expected_char, 50);
        assert_approx_eq!(client.frame_timeout_us, expected_frame, 50);
    }
}
```

### 5. 数据转换测试

验证原始寄存器值到 JSON 的转换：

```rust
#[test]
fn test_convert_raw_value() {
    let mapping = ModbusRegisterMapping {
        data_type: ModbusDataType::Float32,
        scale: 0.1,
        offset: 10.0,
        // ... 其他字段
    };
    
    let result = client.convert_raw_value(1000, &mapping);
    assert_eq!(result, serde_json::json!(110.0)); // (1000 * 0.1) + 10.0
}
```

## 集成测试

### 1. 协议工厂集成测试

验证 RTU 协议与协议工厂的完整集成：

```rust
// 文件: tests/modbus_rtu_integration_tests.rs

#[tokio::test]
async fn test_rtu_protocol_factory_integration() {
    let factory = ProtocolFactory::new();
    
    // 验证 RTU 协议已注册
    assert!(factory.is_protocol_supported(&ProtocolType::ModbusRtu));
    
    // 验证配置验证
    let config = create_rtu_config(1, "/dev/ttyUSB0", 1);
    assert!(factory.validate_config(&config).is_ok());
    
    // 验证默认配置
    let default_config = factory.get_default_config(&ProtocolType::ModbusRtu);
    assert!(default_config.is_some());
}
```

### 2. 配置验证测试

验证各种配置参数的验证逻辑：

```rust
#[test]
fn test_rtu_configuration_validation() {
    let factory = ModbusRtuFactory;
    
    // 测试有效波特率
    let valid_baud_rates = [300, 600, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
    for &baud_rate in &valid_baud_rates {
        let config = create_config_with_baud_rate(baud_rate);
        assert!(factory.validate_config(&config).is_ok());
    }
    
    // 测试无效从站 ID
    for &invalid_slave_id in &[0, 248, 255, 1000] {
        let config = create_config_with_slave_id(invalid_slave_id);
        assert!(factory.validate_config(&config).is_err());
    }
}
```

### 3. 通道管理测试

验证 RTU 通道的创建和管理：

```rust
#[tokio::test]
async fn test_rtu_channel_management() {
    let factory = ProtocolFactory::new();
    
    // 创建多个 RTU 通道
    let configs = vec![
        create_rtu_config(100, "/dev/ttyUSB0", 1),
        create_rtu_config(101, "/dev/ttyUSB1", 2),
        create_high_speed_rtu_config(102, "/dev/ttyS0"),
    ];
    
    for config in configs {
        assert!(factory.create_channel(config).is_ok());
    }
    
    assert_eq!(factory.channel_count(), 3);
    
    // 验证通道统计
    let stats = factory.get_channel_stats();
    assert_eq!(stats.protocol_counts.get("ModbusRtu"), Some(&3));
}
```

### 4. 寄存器映射测试

验证寄存器映射的加载和使用：

```rust
#[tokio::test]
async fn test_rtu_with_register_mappings() {
    let client = ModbusRtuClient::new(config);
    
    // 创建测试映射
    let mappings = create_test_mappings();
    client.base.load_register_mappings(mappings.clone()).await;
    
    // 验证映射已加载
    let loaded_mappings = client.base.get_register_mappings().await;
    assert_eq!(loaded_mappings.len(), mappings.len());
    
    // 验证点查找
    let point = client.base.find_mapping("HoldingReg_000").await;
    assert!(point.is_some());
    assert_eq!(point.unwrap().address, 100);
}
```

### 5. 错误处理测试

验证各种错误情况的处理：

```rust
#[tokio::test]
async fn test_rtu_error_handling() {
    let client = ModbusRtuClient::new(config);
    
    // 测试未连接时的错误
    let result = client.read_holding_registers(100, 10).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not connected"));
    
    // 测试配置信息
    let config_info = client.get_config_info();
    assert_eq!(config_info, "RTU[/dev/ttyUSB0:9600N1]");
}
```

## 串口模拟器

### 1. 模拟串口实现

创建用于测试的模拟串口：

```rust
// 文件: tests/support/serial_mock.rs

#[derive(Debug, Clone)]
pub struct MockSerialPort {
    read_buffer: Arc<Mutex<VecDeque<u8>>>,
    write_buffer: Arc<Mutex<VecDeque<u8>>>,
    connected: Arc<Mutex<bool>>,
}

impl MockSerialPort {
    pub fn new() -> Self {
        Self {
            read_buffer: Arc::new(Mutex::new(VecDeque::new())),
            write_buffer: Arc::new(Mutex::new(VecDeque::new())),
            connected: Arc::new(Mutex::new(true)),
        }
    }
    
    pub async fn add_response_data(&self, data: &[u8]) {
        let mut buffer = self.read_buffer.lock().await;
        buffer.extend(data);
    }
    
    pub async fn get_written_data(&self) -> Vec<u8> {
        let mut buffer = self.write_buffer.lock().await;
        let data: Vec<u8> = buffer.drain(..).collect();
        data
    }
}
```

### 2. RTU 服务器模拟器

模拟 Modbus RTU 服务器响应：

```rust
// 文件: tests/support/rtu_server_mock.rs

pub struct MockModbusRtuServer {
    slave_id: u8,
    coils: HashMap<u16, bool>,
    holding_registers: HashMap<u16, u16>,
    should_respond: bool,
}

impl MockModbusRtuServer {
    pub fn process_request(&self, request_frame: &[u8]) -> Option<Vec<u8>> {
        if !self.should_respond {
            return None;
        }
        
        // 验证 CRC
        let data_len = request_frame.len() - 2;
        let calculated_crc = ModbusClientBase::crc16_modbus(&request_frame[..data_len]);
        
        // 解析并处理请求
        match request_frame[1] {
            0x03 => self.handle_read_holding_registers(&request_frame[2..data_len]),
            0x01 => self.handle_read_coils(&request_frame[2..data_len]),
            _ => self.create_exception_response(request_frame[1], 0x01),
        }
    }
}
```

## 性能测试

### 1. 时序性能测试

验证 RTU 通信时序要求：

```rust
#[tokio::test]
async fn test_rtu_timing_and_performance() {
    let timing_test_cases = vec![
        (9600, 1041),   // 每字符约 1041μs
        (19200, 520),   // 每字符约 520μs  
        (115200, 86),   // 每字符约 86μs
    ];
    
    for (baud_rate, expected_char_time_us) in timing_test_cases {
        let client = create_client_with_baud_rate(baud_rate);
        
        let expected_char_timeout = (expected_char_time_us * 15) / 10; // 1.5 字符时间
        let tolerance = expected_char_timeout / 10; // 10% 容差
        
        assert!(
            (client.char_timeout_us as i64 - expected_char_timeout as i64).abs() <= tolerance,
            "字符超时时间不匹配：{} 波特率",
            baud_rate
        );
    }
}
```

### 2. 并发性能测试

验证并发协议创建的性能：

```rust
#[tokio::test]
async fn test_rtu_batch_protocol_creation() {
    let factory = ProtocolFactory::new();
    
    let configs = vec![
        create_rtu_config(200, "/dev/ttyUSB0", 1),
        create_rtu_config(201, "/dev/ttyUSB1", 2),
        create_high_speed_rtu_config(202, "/dev/ttyS0"),
        create_rtu_config(203, "/dev/ttyUSB2", 3),
    ];
    
    let start_time = Instant::now();
    let results = factory.create_protocols_parallel(configs).await;
    let duration = start_time.elapsed();
    
    assert_eq!(results.len(), 4);
    assert!(duration < Duration::from_millis(100)); // 应在 100ms 内完成
    
    for (i, result) in results.iter().enumerate() {
        assert!(result.is_ok(), "协议创建 {} 应该成功", i);
    }
}
```

## 测试运行指南

### 1. 运行所有测试

```bash
# 运行所有 RTU 相关测试
cargo test modbus_rtu

# 运行单元测试
cargo test modbus_rtu_unit_tests

# 运行集成测试  
cargo test modbus_rtu_integration_tests

# 运行协议工厂测试
cargo test protocol_factory
```

### 2. 运行特定测试

```bash
# 运行帧编码测试
cargo test test_rtu_frame_encoding

# 运行配置验证测试
cargo test test_rtu_configuration_validation

# 运行性能测试
cargo test test_rtu_timing_and_performance
```

### 3. 详细输出测试

```bash
# 显示详细测试输出
cargo test modbus_rtu -- --nocapture

# 显示忽略的测试
cargo test modbus_rtu -- --ignored

# 运行单个测试并显示输出
cargo test test_rtu_client_creation_default -- --exact --nocapture
```

### 4. 测试覆盖率

```bash
# 安装 tarpaulin 覆盖率工具
cargo install cargo-tarpaulin

# 生成测试覆盖率报告
cargo tarpaulin --out Html --output-dir target/coverage

# 只测试 RTU 模块的覆盖率
cargo tarpaulin --packages comsrv --include-tests --timeout 300 \
    --exclude-files "*/tests/*" "*/examples/*" \
    --out Html --output-dir target/coverage/rtu
```

## 故障排查

### 1. 常见测试问题

**问题：串口权限错误**
```bash
# Linux 系统添加用户到 dialout 组
sudo usermod -a -G dialout $USER

# 或者使用 sudo 运行测试
sudo cargo test modbus_rtu
```

**问题：CRC 计算错误**
```rust
// 验证 CRC 计算实现
let data = vec![0x01, 0x03, 0x00, 0x00, 0x00, 0x0A];
let expected_crc = 0xC554;
let calculated_crc = ModbusClientBase::crc16_modbus(&data);
assert_eq!(calculated_crc, expected_crc);
```

**问题：时序计算不准确**
```rust
// 验证时序计算逻辑
let bits_per_char = 10; // 8N1 格式
let baud_rate = 9600;
let char_time_us = (bits_per_char * 1_000_000) / baud_rate;
assert_eq!(char_time_us, 1041); // 约 1041 微秒
```

### 2. 调试技巧

**启用详细日志：**
```rust
// 在测试中启用日志
use env_logger;

#[test]
fn test_with_logging() {
    env_logger::init();
    // 测试代码
}
```

**使用断点调试：**
```bash
# 使用 rust-gdb 调试
rust-gdb target/debug/deps/modbus_rtu_unit_tests-*
```

**检查内存使用：**
```bash
# 使用 valgrind 检查内存泄漏
valgrind --tool=memcheck cargo test modbus_rtu
```

## 测试覆盖率

### 目标覆盖率

- **单元测试覆盖率**: ≥ 95%
- **集成测试覆盖率**: ≥ 85%
- **错误路径覆盖率**: ≥ 90%
- **边界条件覆盖率**: ≥ 100%

### 覆盖率报告

生成的覆盖率报告包含：

1. **函数覆盖率**：每个函数的测试覆盖情况
2. **行覆盖率**：代码行的执行覆盖情况  
3. **分支覆盖率**：条件分支的测试覆盖情况
4. **未覆盖代码**：需要补充测试的代码段

### 持续集成

在 CI/CD 管道中集成测试：

```yaml
# .github/workflows/test.yml
name: RTU Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          
      - name: Run RTU Tests
        run: cargo test modbus_rtu
        
      - name: Generate Coverage
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --out Xml
          
      - name: Upload Coverage
        uses: codecov/codecov-action@v1
```

## 总结

Modbus RTU 测试方案提供了完整的测试覆盖，确保协议实现的可靠性和性能。通过单元测试验证核心功能，通过集成测试验证系统集成，通过性能测试验证时序要求，形成了完整的测试体系。

测试的重点包括：
- 帧编码解码的正确性
- CRC 校验的准确性
- 时序计算的精确性
- 错误处理的完整性
- 配置验证的严格性
- 性能要求的满足性

这些测试确保 Modbus RTU 实现能够在实际工业环境中稳定可靠地运行。 