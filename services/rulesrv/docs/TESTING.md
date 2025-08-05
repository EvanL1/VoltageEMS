# rulesrv 测试指南

## 概述

本文档描述了rulesrv服务的测试策略、测试类型和测试执行方法。

## 测试架构

```
单元测试
├── rule_engine_test.rs    # 规则引擎核心功能测试
└── action_test.rs         # 动作执行测试

集成测试
├── api_test.rs            # REST API端点测试
└── rule_execution_test.rs # 规则执行流程测试

示例规则
├── battery_management.json # 电池管理规则
├── voltage_monitoring.json # 电压监控规则
└── alarm_rules.json       # 告警规则
```

## 测试环境准备

### 1. 启动Redis

```bash
# Docker方式
docker run -d --name redis-test -p 6379:6379 redis:8-alpine

# 或使用本地Redis
redis-server
```

### 2. 准备测试数据

```bash
# 运行数据准备脚本
./scripts/prepare-test-data.sh
```

## 运行测试

### 快速测试

```bash
# 运行所有测试
./run_tests.sh
```

### 分类测试

```bash
# 仅运行单元测试
cargo test --lib

# 仅运行集成测试
cargo test --test '*'

# 运行特定测试
cargo test test_condition_evaluation
```

### API测试

```bash
# 启动服务
cargo run --bin rulesrv service

# 运行API测试脚本
./test-api.sh
```

### 规则执行测试

```bash
# 加载示例规则
./scripts/load-test-rules.sh

# 测试规则执行
./test-rules.sh
```

### Docker测试

```bash
# 运行完整的Docker测试
./test-docker.sh
```

## 测试用例说明

### 单元测试

#### rule_engine_test.rs
- **条件评估测试**
  - 等于/不等于比较
  - 大于/小于比较
  - 包含操作
  - AND/OR逻辑组合
  - Hash字段访问

- **规则管理测试**
  - 规则存储和加载
  - 禁用规则处理
  - 不存在规则处理
  - 缺失数据源处理

- **冷却期测试**
  - 冷却期内阻止执行
  - 冷却期后允许执行

#### action_test.rs
- **动作类型测试**
  - 设备控制动作
  - 发布消息动作
  - 设置值动作
  - 通知动作

- **复合测试**
  - 多动作执行
  - 动作执行顺序
  - 错误处理

### 集成测试

#### api_test.rs
- **CRUD操作**
  - 创建规则
  - 列出规则
  - 获取规则
  - 更新规则
  - 删除规则

- **规则执行**
  - 执行规则API
  - 测试规则API
  - 获取执行统计
  - 获取执行历史

#### rule_execution_test.rs
- **完整流程测试**
  - 电池管理场景
  - 复杂条件评估
  - 并发执行
  - 错误处理

## 测试数据

### 基础测试数据

```javascript
// 电池数据
battery.soc: 75
battery.voltage: 48.5
battery.temperature: 28.5

// 发电机数据
generator.status: "stopped"
generator.fuel: 85

// 电压数据
comsrv:1001:T.1: 230.5  // A相电压
comsrv:1001:T.2: 231.2  // B相电压
comsrv:1001:T.3: 229.8  // C相电压
```

### 测试规则示例

```json
{
  "id": "test_rule",
  "name": "测试规则",
  "conditions": {
    "operator": "AND",
    "conditions": [
      {
        "source": "battery.soc",
        "operator": "<=",
        "value": 20.0
      }
    ]
  },
  "actions": [
    {
      "action_type": "notify",
      "config": {
        "level": "warning",
        "message": "电池电量低"
      }
    }
  ],
  "enabled": true,
  "priority": 1,
  "cooldown_seconds": 300
}
```

## 性能测试

### 并发执行测试

```bash
# 使用Apache Bench
ab -n 1000 -c 10 -p rule.json -T application/json \
   http://localhost:6003/rules/test_rule/execute
```

### 负载测试

```bash
# 创建多个规则
for i in {1..100}; do
  curl -X POST http://localhost:6003/rules \
    -H "Content-Type: application/json" \
    -d "{\"rule\": {...}}"
done
```

## 测试覆盖率

```bash
# 生成测试覆盖率报告
cargo tarpaulin --out Html

# 查看报告
open tarpaulin-report.html
```

## 故障排查

### 常见问题

1. **Redis连接失败**
   ```bash
   # 检查Redis是否运行
   redis-cli ping
   ```

2. **规则未触发**
   ```bash
   # 检查条件数据
   redis-cli GET battery.soc
   
   # 检查规则状态
   curl http://localhost:6003/rules/{rule_id}
   ```

3. **冷却期问题**
   ```bash
   # 检查规则统计
   curl http://localhost:6003/rules/{rule_id}/stats
   ```

### 调试技巧

1. **启用详细日志**
   ```bash
   RUST_LOG=rulesrv=trace cargo run
   ```

2. **监控Redis操作**
   ```bash
   redis-cli monitor | grep rulesrv
   ```

3. **查看执行结果**
   ```bash
   redis-cli --scan --pattern "rulesrv:execution:*"
   ```

## CI/CD集成

### GitHub Actions示例

```yaml
name: rulesrv Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      redis:
        image: redis:8-alpine
        ports:
          - 6379:6379
    
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
      - name: Run tests
        run: |
          cd services/rulesrv
          cargo test --all-features
```

## 最佳实践

1. **测试数据隔离**
   - 使用不同的Redis数据库或key前缀
   - 测试后清理数据

2. **并发测试**
   - 使用`--test-threads=1`避免竞争条件
   - 或使用不同的测试数据

3. **Mock外部依赖**
   - 使用测试专用的Redis实例
   - Mock外部服务调用

4. **持续监控**
   - 监控测试执行时间
   - 跟踪测试失败率