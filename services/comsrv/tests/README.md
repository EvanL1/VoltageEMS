# ComsRV协议插件系统测试方案

## 测试架构概述

本测试框架为ComsRV协议插件系统提供全面的测试覆盖，包括单元测试、集成测试、性能测试和兼容性测试。

## 测试层级

### 1. 单元测试 (Unit Tests)
- 核心组件测试
- 插件接口测试  
- 配置管理测试
- 传输层测试

### 2. 集成测试 (Integration Tests)
- 协议插件集成测试
- 多协议并发测试
- Redis集成测试
- 配置中心集成测试

### 3. 性能测试 (Performance Tests)
- 吞吐量测试
- 延迟测试
- 内存使用测试
- 并发性能测试

### 4. 兼容性测试 (Compatibility Tests)
- 标准协议合规性测试
- 设备兼容性测试
- 版本兼容性测试

### 5. 端到端测试 (E2E Tests)
- 完整数据流测试
- 故障恢复测试
- 长时间稳定性测试

## 测试工具

### 协议模拟器
- Modbus模拟器
- IEC60870模拟器
- CAN总线模拟器
- GPIO模拟器

### 测试框架
- 内置测试框架 (`test_framework.rs`)
- 性能基准测试工具
- 协议一致性测试工具

### 自动化工具
- CI/CD集成脚本
- 测试报告生成器
- 代码覆盖率分析

## 使用指南

### 运行所有测试
```bash
cd services/comsrv
./scripts/run_all_tests.sh
```

### 运行特定测试
```bash
# 单元测试
cargo test --lib

# 集成测试  
cargo test --test '*'

# 性能测试
cargo bench

# 协议测试
./scripts/test_protocol.sh modbus_tcp
```

### 生成测试报告
```bash
./scripts/generate_test_report.sh
```

## 测试配置

测试配置文件位于 `tests/configs/` 目录下，每个协议都有对应的测试配置。

## 贡献指南

添加新测试时，请遵循以下规范：
1. 单元测试放在模块同目录的 `tests` 子模块中
2. 集成测试放在 `tests/integration/` 目录下
3. 性能测试放在 `benches/` 目录下
4. 测试数据和配置放在 `tests/fixtures/` 目录下