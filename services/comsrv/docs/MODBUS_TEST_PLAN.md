# Modbus 协议测试计划

## 概述

本测试计划涵盖了 Modbus 协议实现的全面测试，从单元测试到集成测试，从少量点位到大规模性能测试。

## 测试架构

```
services/comsrv/src/core/protocols/modbus/tests/
├── mod.rs              # 测试模块入口
├── mock_transport.rs   # Mock 传输层实现
├── test_helpers.rs     # 测试辅助函数
├── pdu_tests.rs        # PDU 处理单元测试
├── frame_tests.rs      # Frame 处理单元测试
├── client_tests.rs     # Client 功能测试
├── polling_tests.rs    # 轮询引擎测试
└── integration_tests.rs # 集成测试
```

## 测试层级

### 1. 单元测试（Unit Tests）

#### PDU 处理测试 (`pdu_tests.rs`)
- **测试范围**: 1-10 个点位
- **测试内容**:
  - 所有功能码的请求构建和响应解析
  - 异常响应处理
  - 数据验证和边界检查
  - 错误处理
- **关键测试用例**:
  - `test_build_read_coils_request` - 读线圈请求
  - `test_parse_read_holding_registers_response` - 解析保持寄存器响应
  - `test_modbus_exception_handling` - 异常处理
  - `test_coil_bit_packing` - 位打包算法

#### Frame 处理测试 (`frame_tests.rs`)
- **测试范围**: 单个帧处理
- **测试内容**:
  - TCP 模式 MBAP 头部处理
  - RTU 模式 CRC 计算
  - 帧验证和错误检测
  - 模式切换
- **关键测试用例**:
  - `test_tcp_frame_encoding` - TCP 帧编码
  - `test_rtu_crc_validation` - RTU CRC 验证
  - `test_partial_frame_handling` - 部分帧处理
  - `test_frame_size_limits` - 帧大小限制

### 2. 功能测试（Functional Tests）

#### Client 功能测试 (`client_tests.rs`)
- **测试范围**: 1-100 个点位
- **测试内容**:
  - 连接管理和重试机制
  - 读写操作（四遥）
  - 批量操作优化
  - 统计信息跟踪
- **关键测试用例**:
  - `test_read_holding_registers` - 读保持寄存器
  - `test_write_single_coil` - 写单个线圈
  - `test_batch_read` - 批量读取
  - `test_float32_data_type` - 浮点数据处理

#### 轮询引擎测试 (`polling_tests.rs`)
- **测试范围**: 1-1000 个点位
- **测试内容**:
  - 单从站和多从站轮询
  - 批量读取优化
  - 不同轮询间隔配置
  - 错误恢复
- **关键测试用例**:
  - `test_batch_reading_optimization` - 批量优化
  - `test_multi_slave_polling` - 多从站轮询
  - `test_large_point_count_performance` - 1000点位性能
  - `test_polling_with_errors` - 错误处理

### 3. 集成测试（Integration Tests）

#### 端到端测试 (`integration_tests.rs`)
- **测试范围**: 1-100 个点位，真实网络通信
- **测试内容**:
  - 与 Modbus 模拟器的真实通信
  - 四遥完整功能验证
  - 并发操作测试
  - 性能基准测试
- **关键测试用例**:
  - `test_read_all_telemetry_types` - 四遥类型测试
  - `test_concurrent_operations` - 并发操作
  - `test_batch_reading_performance` - 批量性能
  - `test_polling_engine_with_simulator` - 轮询集成

## 测试数据规模

### 点位数量级别

1. **小规模测试** (1-10 点位)
   - PDU 单元测试
   - 基本功能验证
   - 错误处理测试

2. **中等规模测试** (10-100 点位)
   - Client 功能测试
   - 批量操作测试
   - 数据类型测试

3. **大规模测试** (100-1000 点位)
   - 轮询引擎性能测试
   - 多从站管理测试
   - 内存和CPU使用测试

4. **压力测试** (1000+ 点位)
   - 最大负载测试
   - 长时间运行测试
   - 资源泄漏检测

## 测试执行

### 运行单元测试
```bash
cd services/comsrv
cargo test modbus::tests::pdu_tests
cargo test modbus::tests::frame_tests
cargo test modbus::tests::client_tests
cargo test modbus::tests::polling_tests
```

### 运行集成测试
```bash
# 先启动 Modbus 模拟器
./scripts/start_modbus_simulator.sh

# 运行集成测试
cargo test modbus::tests::integration_tests
```

### 运行所有测试
```bash
cargo test modbus::tests
```

### 运行性能测试
```bash
cargo test modbus::tests --release -- --nocapture
```

## 测试覆盖目标

- **代码覆盖率**: > 80%
- **功能覆盖**: 100% 公共 API
- **错误路径**: 100% 错误处理代码
- **性能基准**: 
  - 单点读取 < 10ms
  - 100点批量读取 < 100ms
  - 1000点轮询周期 < 1s

## Mock 工具

### MockTransport
- 模拟网络传输
- 支持预设响应
- 错误注入
- 延迟模拟
- 操作历史记录

### 测试辅助函数
- PDU/Frame 构建器
- CRC 计算
- 数据生成器
- 性能测量工具

## 持续集成

建议在 CI/CD 管道中包含：

1. **代码检查**
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   ```

2. **单元测试**
   ```bash
   cargo test --lib
   ```

3. **集成测试**（如果有模拟器环境）
   ```bash
   cargo test --test '*'
   ```

4. **代码覆盖率**
   ```bash
   cargo tarpaulin --out Xml
   ```

## 测试维护

1. **新功能**: 添加对应的单元测试和集成测试
2. **Bug 修复**: 添加回归测试
3. **性能优化**: 更新性能基准测试
4. **定期审查**: 每季度审查测试覆盖率和有效性

## 总结

这个测试计划提供了全面的 Modbus 协议测试覆盖，从基础的 PDU 处理到复杂的多从站轮询场景。通过分层测试策略，我们可以快速定位问题，确保代码质量，并为未来的优化提供性能基准。