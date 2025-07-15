# HisSrv 测试套件

本目录包含 HisSrv 服务的完整测试套件，包括单元测试、集成测试和性能基准测试。

## 测试结构

```
tests/
├── mod.rs                    # 测试模块声明
├── test_utils.rs            # 测试辅助工具
├── mock_storage.rs          # 模拟存储后端
├── batch_writer_test.rs     # 批量写入器测试
├── redis_subscriber_test.rs # Redis订阅器测试  
├── query_optimizer_test.rs  # 查询优化器测试
├── retention_policy_test.rs # 保留策略测试
├── integration_test.rs      # 端到端集成测试
└── api_test.rs             # REST API测试

benches/
├── batch_writer_bench.rs    # 批量写入器性能基准
└── query_optimizer_bench.rs # 查询优化器性能基准
```

## 运行测试

### 运行所有测试
```bash
cargo test
```

### 运行特定测试模块
```bash
# 批量写入器测试
cargo test batch_writer_test

# Redis订阅器测试
cargo test redis_subscriber_test

# API测试
cargo test api_test
```

### 运行单个测试
```bash
cargo test test_batch_writer_basic_functionality -- --exact
```

### 显示测试输出
```bash
cargo test -- --nocapture
```

### 运行测试脚本
```bash
./run_tests.sh          # 运行所有测试
./run_tests.sh --bench  # 包括性能基准测试
```

## 性能基准测试

### 运行所有基准测试
```bash
cargo bench
```

### 运行特定基准测试
```bash
cargo bench batch_writer
cargo bench query_optimizer
```

### 生成HTML报告
基准测试结果会自动生成HTML报告，位于 `target/criterion/` 目录。

## 测试覆盖率

### 安装 cargo-tarpaulin
```bash
cargo install cargo-tarpaulin
```

### 生成覆盖率报告
```bash
cargo tarpaulin --out Html --output-dir target/coverage
```

覆盖率报告将生成在 `target/coverage/tarpaulin-report.html`。

## 测试环境要求

- Redis 服务器（用于集成测试）
  - 默认连接：localhost:6379
  - 测试数据库：15

- InfluxDB（可选，用于完整集成测试）
  - 默认连接：http://localhost:8086

## 测试最佳实践

1. **单元测试**
   - 每个模块都应有对应的单元测试
   - 使用 mock 对象隔离外部依赖
   - 测试边界条件和错误情况

2. **集成测试**
   - 测试组件之间的交互
   - 使用测试专用的数据库和配置
   - 确保测试后清理数据

3. **性能测试**
   - 定期运行基准测试监控性能变化
   - 使用 criterion 进行统计分析
   - 测试不同负载下的性能表现

## 常见问题

### Redis 连接失败
确保 Redis 服务器正在运行：
```bash
docker run -d -p 6379:6379 redis:7-alpine
```

### 测试超时
某些集成测试可能需要较长时间，可以增加超时时间：
```bash
cargo test -- --test-threads=1
```

### 并发测试问题
如果测试之间有冲突，可以串行运行：
```bash
cargo test -- --test-threads=1
```