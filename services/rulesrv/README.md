# rulesrv - 控制规则引擎

## 概述

rulesrv 是 VoltageEMS 的智能规则引擎服务，负责执行基于 DAG（有向无环图）的控制规则。服务专注于读取 modsrv 的计算结果，执行逻辑判断，并触发相应的控制动作。所有数据读写基于新的 Redis Hash 结构。

## 主要特性

- **DAG 规则引擎**: 支持复杂的有向无环图规则定义
- **专注控制逻辑**: 只读取 modsrv 数据，执行控制决策
- **实时触发**: 监听数据变化，自动执行相关规则
- **多种条件类型**: 阈值、范围、表达式、状态等
- **灵活的动作**: 控制命令、告警生成、通知发送
- **标准化精度**: 所有数值保持 6 位小数精度

## 快速开始

### 运行服务

```bash
cd services/rulesrv
cargo run
```

### 配置文件

主配置文件位于 `config/default.yml`：

```yaml
service:
  name: "rulesrv"
  host: "0.0.0.0"
  port: 8085
  
redis:
  url: "redis://localhost:6379"
  
rule_engine:
  max_rules: 1000
  execution_timeout: 30  # 秒
  max_parallel: 100
  
monitoring:
  sources:
    - "modsrv:*:measurement"  # 监控所有模型的测量值
    - "modsrv:*:control"      # 监控所有模型的控制值
    
logging:
  level: "info"
  file: "logs/rulesrv.log"
```

## 数据读取原则

rulesrv **只从 modsrv 读取数据**，不直接访问 comsrv 数据：

```rust
// 正确：读取 modsrv 计算结果
let value = redis_client.hget("modsrv:power_meter:measurement", "total_power").await?;

// 错误：不应直接读取 comsrv 原始数据
// let value = redis_client.hget("comsrv:1001:m", "10001").await?;
```

## DAG 规则定义

### 规则结构

```json
{
  "id": "power_optimization",
  "name": "功率优化控制",
  "description": "基于功率计算结果的优化控制",
  "enabled": true,
  "nodes": [
    {
      "id": "read_power",
      "type": "input",
      "config": {
        "source": "modsrv:power_meter:measurement",
        "field": "total_power"
      }
    },
    {
      "id": "read_limit",
      "type": "input",
      "config": {
        "source": "modsrv:power_meter:control",
        "field": "power_limit"
      }
    },
    {
      "id": "check_overload",
      "type": "condition",
      "config": {
        "expression": "$read_power > $read_limit * 0.9"
      }
    },
    {
      "id": "reduce_load",
      "type": "action",
      "config": {
        "type": "control",
        "channel": "cmd:1001:adjustment",
        "command": {
          "point_id": 40001,
          "value": "$read_limit * 0.8"
        }
      }
    }
  ],
  "edges": [
    {"from": "read_power", "to": "check_overload"},
    {"from": "read_limit", "to": "check_overload"},
    {"from": "check_overload", "to": "reduce_load", "condition": true}
  ]
}
```

### 节点类型

#### 1. Input 节点
```json
{
  "type": "input",
  "config": {
    "source": "modsrv:power_meter:measurement",
    "field": "total_power",
    "default": 0.0
  }
}
```

#### 2. Transform 节点
```json
{
  "type": "transform",
  "config": {
    "expression": "($input1 + $input2) / 2",
    "output_precision": 6
  }
}
```

#### 3. Condition 节点
```json
{
  "type": "condition",
  "config": {
    "type": "threshold",
    "operator": ">",
    "value": 1000.0,
    "duration": 60  // 持续时间（秒）
  }
}
```

#### 4. Action 节点
```json
{
  "type": "action",
  "config": {
    "type": "control",
    "delay": 0,
    "retry": 3
  }
}
```

## API 接口

### 规则管理

```bash
# 列出所有规则
GET /rules?enabled=true&tag=power

# 获取规则详情
GET /rules/{rule_id}

# 创建规则
POST /rules
Content-Type: application/json

# 更新规则
PUT /rules/{rule_id}

# 删除规则
DELETE /rules/{rule_id}

# 启用/禁用规则
POST /rules/{rule_id}/enable
POST /rules/{rule_id}/disable
```

### 规则执行

```bash
# 手动执行规则
POST /rules/{rule_id}/execute
{
  "context": {
    "key": "value"
  }
}

# 测试规则（不保存结果）
POST /rules/test
{
  "rule": { ... },
  "input": { ... }
}

# 获取执行历史
GET /rules/{rule_id}/history?limit=100
```

### 监控和统计

```bash
# 获取规则统计
GET /stats

# 获取执行指标
GET /metrics
```

## 条件类型

### 阈值条件

```yaml
conditions:
  - type: "threshold"
    source: "modsrv:power_meter:measurement"
    field: "total_power"
    operator: ">"  # >, <, >=, <=, ==, !=
    value: 1000.0
    duration: 60   # 持续时间（秒）
```

### 范围条件

```yaml
conditions:
  - type: "range"
    source: "modsrv:power_meter:measurement"
    field: "power_factor"
    min: 0.85
    max: 0.95
    inside: true  # true=在范围内触发，false=在范围外触发
```

### 表达式条件

```yaml
conditions:
  - type: "expression"
    expression: |
      power > 1000 AND 
      power_factor < 0.9 AND 
      time.hour >= 8 AND 
      time.hour <= 18
```

### 状态变化条件

```yaml
conditions:
  - type: "state_change"
    source: "modsrv:device:status"
    field: "online"
    from: 1
    to: 0
    debounce: 30  # 防抖时间（秒）
```

## 动作类型

### 控制动作

```yaml
actions:
  - type: "control"
    channel: "cmd:1001:control"
    command:
      point_id: 30001
      value: 1.0
    confirm_timeout: 5000  # 毫秒
```

### 调节动作

```yaml
actions:
  - type: "adjustment"
    channel: "cmd:1001:adjustment"
    command:
      point_id: 40001
      value: "$calculated_value * 0.95"
```

### 告警动作

```yaml
actions:
  - type: "alarm"
    severity: "major"
    title: "功率超限"
    description: "当前功率 {$power} 超过限值 {$limit}"
    category: "power"
```

### 通知动作

```yaml
actions:
  - type: "notification"
    channel: "notification:email"
    template: "power_alert"
    recipients: ["operator@example.com"]
    data:
      power: "$current_power"
      limit: "$power_limit"
```

## 规则示例

### 示例 1：功率因数优化

```json
{
  "id": "power_factor_optimization",
  "name": "功率因数优化",
  "trigger": {
    "type": "data_change",
    "sources": ["modsrv:power_meter:measurement"]
  },
  "conditions": [{
    "type": "threshold",
    "source": "modsrv:power_meter:measurement",
    "field": "power_factor",
    "operator": "<",
    "value": 0.90
  }],
  "actions": [{
    "type": "control",
    "channel": "cmd:1001:control",
    "command": {
      "point_id": 30010,
      "value": 1.0,
      "description": "启动功率因数补偿"
    }
  }]
}
```

### 示例 2：温度联动控制

```json
{
  "id": "temperature_control",
  "name": "温度联动控制",
  "nodes": [
    {
      "id": "temp_avg",
      "type": "input",
      "config": {
        "source": "modsrv:env_monitor:measurement",
        "field": "avg_temperature"
      }
    },
    {
      "id": "temp_high",
      "type": "condition",
      "config": {
        "expression": "$temp_avg > 28.0"
      }
    },
    {
      "id": "temp_low",
      "type": "condition",
      "config": {
        "expression": "$temp_avg < 20.0"
      }
    },
    {
      "id": "start_cooling",
      "type": "action",
      "config": {
        "type": "control",
        "channel": "cmd:2001:control",
        "command": {"point_id": 30001, "value": 1.0}
      }
    },
    {
      "id": "start_heating",
      "type": "action",
      "config": {
        "type": "control",
        "channel": "cmd:2001:control",
        "command": {"point_id": 30002, "value": 1.0}
      }
    }
  ],
  "edges": [
    {"from": "temp_avg", "to": "temp_high"},
    {"from": "temp_avg", "to": "temp_low"},
    {"from": "temp_high", "to": "start_cooling", "condition": true},
    {"from": "temp_low", "to": "start_heating", "condition": true}
  ]
}
```

## 表达式语法

### 支持的操作符

- 算术: `+`, `-`, `*`, `/`, `%`, `^`
- 比较: `>`, `<`, `>=`, `<=`, `==`, `!=`
- 逻辑: `AND`, `OR`, `NOT`
- 函数: `abs()`, `min()`, `max()`, `avg()`, `sum()`

### 内置变量

- `$node_id` - 引用其他节点的输出
- `time.hour` - 当前小时（0-23）
- `time.minute` - 当前分钟（0-59）
- `time.weekday` - 星期几（1-7）

### 示例表达式

```javascript
// 简单比较
temperature > 30.0

// 复合条件
power > 1000 AND power_factor < 0.9

// 使用函数
abs(voltage - 220) > 10

// 时间条件
time.hour >= 8 AND time.hour <= 18 AND time.weekday <= 5
```

## 性能优化

### 1. 规则缓存

规则引擎自动缓存已编译的规则，避免重复解析。

### 2. 批量数据读取

```rust
// 批量读取多个字段
let fields = vec!["total_power", "power_factor", "voltage"];
let values = redis_client.hmget("modsrv:power_meter:measurement", &fields).await?;
```

### 3. 并行执行

独立的规则可以并行执行，通过 `max_parallel` 配置控制并发度。

## 监控指标

通过 `/metrics` 端点暴露 Prometheus 指标：

- `rulesrv_rules_total` - 规则总数
- `rulesrv_rule_executions_total` - 规则执行次数
- `rulesrv_rule_execution_duration_seconds` - 执行耗时
- `rulesrv_rule_failures_total` - 执行失败次数

## 故障排查

### 规则未触发

1. 检查规则是否启用
2. 验证数据源是否有更新
3. 查看条件是否满足
4. 检查日志中的错误信息

### 动作执行失败

1. 验证目标通道是否存在
2. 检查命令格式是否正确
3. 确认权限是否足够
4. 查看重试日志

## 环境变量

- `RUST_LOG` - 日志级别
- `RULESRV_CONFIG` - 配置文件路径
- `REDIS_URL` - Redis 连接地址

## 相关文档

- [架构设计](docs/architecture.md)
- [规则配置指南](docs/rule-configuration.md)