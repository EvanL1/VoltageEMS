# ComsRV 测试指南

## 概述

本指南提供了ComsRV协议插件系统的完整测试方案，包括测试策略、工具使用和最佳实践。

## 测试架构

```
tests/
├── unit/                    # 单元测试
│   ├── plugin_interface_test.rs
│   ├── plugin_registry_test.rs
│   └── config_validation_test.rs
├── integration/            # 集成测试
│   ├── multi_protocol_test.rs
│   └── protocol_compatibility_test.rs
├── performance/           # 性能测试
│   └── benchmark_tests.rs
├── e2e/                   # 端到端测试
│   └── full_system_test.rs
├── simulators/            # 协议模拟器
│   └── modbus_simulator.rs
└── configs/               # 测试配置
    ├── modbus_test.yaml
    └── iec60870_test.yaml
```

## 测试层级

### 1. 单元测试

单元测试验证单个组件的功能正确性。

#### 运行单元测试
```bash
# 运行所有单元测试
cargo test --lib

# 运行特定模块的测试
cargo test plugin_interface

# 运行带输出的测试
cargo test -- --nocapture
```

#### 测试覆盖的组件
- 插件接口（ProtocolPlugin trait）
- 插件注册表（PluginRegistry）
- 配置验证（ConfigValidator）
- 传输层（Transport trait）
- 数据类型转换

### 2. 集成测试

集成测试验证多个组件协同工作的正确性。

#### 多协议并发测试
```bash
cargo test --test multi_protocol_test
```

测试场景：
- 单协议多实例
- 多协议并发运行
- 高并发压力测试
- 持续负载测试

#### 协议兼容性测试
```bash
cargo test --test protocol_compatibility_test
```

验证内容：
- 标准协议规范符合性
- 帧格式正确性
- 功能码支持完整性
- 错误处理规范性

### 3. 性能测试

性能测试评估系统的性能指标。

#### 运行基准测试
```bash
cargo bench

# 或使用测试脚本
RUN_BENCHMARKS=true ./scripts/run_all_tests.sh
```

#### 性能指标
- **吞吐量**：操作/秒
- **延迟**：平均、最小、最大、P50、P95、P99
- **资源使用**：内存、CPU
- **并发能力**：最大并发连接数

### 4. 端到端测试

E2E测试验证完整的系统功能。

#### 运行E2E测试
```bash
# 需要Redis运行
cargo test --test full_system_test -- --ignored
```

#### 测试内容
- 完整数据流：设备 → 协议插件 → Redis → 前端
- 控制命令执行
- 故障恢复机制
- 数据一致性
- 长期稳定性

## 测试工具

### 1. 协议模拟器

#### Modbus模拟器
```rust
// 启动Modbus TCP模拟器
let addr = "127.0.0.1:5502".parse().unwrap();
let simulator = ModbusTcpSimulator::new(addr);
simulator.start().await?;

// 设置测试数据
simulator.set_register(1, 100, 1234).await;
simulator.set_coil(1, 0, true).await;
```

#### 使用Python模拟器
```bash
# 启动Python Modbus模拟器
python tests/modbus_server_simulator.py
```

### 2. 测试脚本

#### 运行所有测试
```bash
./scripts/run_all_tests.sh
```

#### 测试特定协议
```bash
./scripts/test_protocol.sh modbus_tcp
./scripts/test_protocol.sh modbus_tcp tests/configs/modbus_test.yaml
```

#### 生成测试报告
```bash
./scripts/generate_test_report.sh
```

### 3. CLI测试工具

#### 协议测试
```bash
# 使用内置测试框架
cargo run --bin comsrv-cli -- test-protocol modbus_tcp

# 指定测试配置
cargo run --bin comsrv-cli -- test-protocol modbus_tcp --config tests/configs/modbus_test.yaml

# 运行所有测试
cargo run --bin comsrv-cli -- test-protocol modbus_tcp --all
```

#### 性能基准测试
```bash
cargo run --bin comsrv-cli -- benchmark-protocol modbus_tcp --duration 60
```

## 测试配置

### YAML配置格式

测试配置使用YAML格式，包含以下部分：

```yaml
protocol: modbus_tcp
name: "Test Configuration"

connection:
  host: "127.0.0.1"
  port: 5502
  # 协议特定参数

test_scenarios:
  - name: "Scenario Name"
    operations:
      - type: operation_type
        # 操作参数

validations:
  - name: "Validation Name"
    # 验证规则
```

### 环境变量

```bash
# 启用基准测试
export RUN_BENCHMARKS=true

# 设置Redis URL
export REDIS_URL=redis://127.0.0.1:6379

# 设置日志级别
export RUST_LOG=debug
```

## 持续集成

### GitHub Actions配置

```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      redis:
        image: redis:latest
        ports:
          - 6379:6379
    
    steps:
      - uses: actions/checkout@v2
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Run tests
        run: ./scripts/run_all_tests.sh
      
      - name: Generate report
        run: ./scripts/generate_test_report.sh
      
      - name: Upload report
        uses: actions/upload-artifact@v2
        with:
          name: test-report
          path: test_reports/
```

## 测试最佳实践

### 1. 测试命名
- 使用描述性名称
- 遵循 `test_<what>_<condition>_<expected>` 格式
- 例如：`test_modbus_read_coils_invalid_address_returns_error`

### 2. 测试组织
- 相关测试放在同一模块
- 使用 `#[cfg(test)]` 标记测试模块
- 共享测试工具放在 `test_helpers` 模块

### 3. 测试数据
- 使用固定的测试数据
- 避免随机数据（除非测试随机性）
- 清理测试产生的副作用

### 4. 异步测试
```rust
#[tokio::test]
async fn test_async_operation() {
    // 使用 tokio::test 宏
}
```

### 5. 超时处理
```rust
use tokio::time::timeout;

let result = timeout(Duration::from_secs(5), async_operation()).await;
```

### 6. 模拟和桩
```rust
// 使用 MockTransport 进行测试
let transport = MockTransport::new();
transport.add_response(vec![0x01, 0x03, ...]);
```

## 故障排查

### 常见问题

1. **测试超时**
   - 增加超时时间
   - 检查网络连接
   - 验证服务是否启动

2. **Redis连接失败**
   - 确保Redis运行：`redis-cli ping`
   - 检查连接URL
   - 验证防火墙设置

3. **并发测试失败**
   - 检查资源限制
   - 调整并发数量
   - 使用互斥锁保护共享资源

### 调试技巧

1. **启用详细日志**
   ```bash
   RUST_LOG=debug cargo test
   ```

2. **单独运行失败的测试**
   ```bash
   cargo test test_name -- --exact
   ```

3. **使用调试器**
   ```bash
   rust-gdb target/debug/test_binary
   ```

## 性能优化建议

1. **减少锁竞争**
   - 使用读写锁（RwLock）
   - 细粒度锁
   - 无锁数据结构

2. **批量操作**
   - 批量读取数据点
   - 批量写入Redis
   - 使用Pipeline模式

3. **连接池**
   - 复用TCP连接
   - 限制最大连接数
   - 实现连接健康检查

4. **内存优化**
   - 使用对象池
   - 及时释放资源
   - 避免内存泄漏

## 测试报告

测试完成后会生成HTML格式的测试报告，包含：

- 测试总结（通过/失败/跳过）
- 详细测试结果
- 代码覆盖率
- 性能指标
- 测试日志

报告位置：`test_reports/test_report_YYYYMMDD_HHMMSS.html`