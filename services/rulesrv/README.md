# Rules Service (rulesrv)

规则引擎服务，负责实时监控系统数据、执行业务规则、触发自动化动作。

## 概述

rulesrv 是 VoltageEMS 的核心决策服务，提供：
- 灵活的规则定义和管理
- 实时数据订阅和处理
- 条件判断和表达式评估
- 自动化动作执行
- DAG（有向无环图）规则执行引擎

## 主要功能

### 1. 规则管理
- 规则的创建、更新、删除、查询
- 规则分组管理
- 规则优先级控制
- 规则启用/禁用

### 2. 触发类型
- **数据变化触发**：监控特定数据源的变化
- **定时触发**：按照 cron 表达式定时执行
- **手动触发**：通过 API 手动执行

### 3. 条件类型
- **阈值条件**：数值比较（>, <, >=, <=, ==, !=）
- **范围条件**：数值范围判断
- **表达式条件**：支持复杂表达式
- **状态条件**：设备或系统状态判断

### 4. 动作类型
- **控制动作**：发送控制命令到设备
- **告警动作**：生成告警事件
- **通知动作**：发送通知消息
- **计算动作**：执行计算并存储结果

## API 接口

### 规则管理

```bash
# 列出所有规则
GET /api/v1/rules?group_id={group_id}&enabled={true/false}

# 获取单个规则
GET /api/v1/rules/{rule_id}

# 创建规则
POST /api/v1/rules
{
  "rule": {
    "id": "temp_threshold_rule",
    "name": "温度阈值告警",
    "group_id": "energy_rules",
    "enabled": true,
    "trigger": {
      "type": "data_change",
      "sources": ["1001:m:10001"]
    },
    "conditions": [{
      "type": "threshold",
      "source": "1001:m:10001",
      "operator": ">",
      "value": 80,
      "duration": 60000
    }],
    "actions": [{
      "type": "alarm",
      "level": "warning",
      "message": "温度超过阈值"
    }]
  }
}

# 更新规则
PUT /api/v1/rules/{rule_id}

# 删除规则
DELETE /api/v1/rules/{rule_id}

# 执行规则
POST /api/v1/rules/{rule_id}/execute
{
  "input": {
    "temperature": 85.5
  }
}

# 测试规则（不保存）
POST /api/v1/rules/test
{
  "rule": { ... },
  "input": { ... }
}

# 获取执行历史
GET /api/v1/rules/{rule_id}/history?limit=100
```

### 规则组管理

```bash
# 列出所有规则组
GET /api/v1/groups

# 获取单个规则组
GET /api/v1/groups/{group_id}

# 创建规则组
POST /api/v1/groups
{
  "group": {
    "id": "energy_rules",
    "name": "能源管理规则",
    "description": "能源相关的自动化规则",
    "enabled": true
  }
}

# 删除规则组
DELETE /api/v1/groups/{group_id}

# 获取组内规则
GET /api/v1/groups/{group_id}/rules
```

### 健康检查

```bash
GET /health
```

## 配置说明

配置文件位于 `config/default.yml`：

```yaml
# Redis 配置
redis:
  url: "redis://localhost:6379"
  key_prefix: "rulesrv"

# API 服务配置
api:
  port: 8080

# 规则引擎配置
engine:
  max_rules: 1000
  execution_timeout: 30s
  max_parallel_executions: 100

# 订阅配置
subscription:
  channels:
    - "modsrv:outputs:*"
    - "alarm:event:*"
```

## 规则示例

### 1. 温度阈值告警

```json
{
  "id": "high_temp_alarm",
  "name": "高温告警",
  "trigger": {
    "type": "data_change",
    "sources": ["1001:m:10001"]
  },
  "conditions": [{
    "type": "threshold",
    "source": "1001:m:10001",
    "operator": ">",
    "value": 85.0,
    "duration": 60000
  }],
  "actions": [{
    "type": "alarm",
    "level": "critical",
    "message": "温度超过85度"
  }, {
    "type": "control",
    "channel_id": 1001,
    "point_type": "c",
    "point_id": 30001,
    "value": false
  }]
}
```

### 2. 功率优化规则

```json
{
  "id": "power_optimization",
  "name": "功率优化",
  "trigger": {
    "type": "schedule",
    "cron": "*/5 * * * *"
  },
  "conditions": [{
    "type": "expression",
    "expression": "avg(1001:m:20001..20010) > 1000 AND time.hour >= 9 AND time.hour <= 18"
  }],
  "actions": [{
    "type": "control",
    "channel_id": 1001,
    "point_type": "a",
    "point_id": 40001,
    "value": "current_power * 0.95"
  }]
}
```

### 3. DAG 规则示例

```json
{
  "id": "complex_dag_rule",
  "name": "复杂DAG规则",
  "nodes": [
    {
      "id": "temp_input",
      "type": "input",
      "config": {
        "source": "modsrv:calc:env_monitor"
      }
    },
    {
      "id": "humidity_input",
      "type": "input",
      "config": {
        "source": "1001:s:20001"
      }
    },
    {
      "id": "comfort_calc",
      "type": "transform",
      "config": {
        "expression": "temp * 0.7 + humidity * 0.3"
      }
    },
    {
      "id": "check_comfort",
      "type": "condition",
      "config": {
        "expression": "$comfort_calc > 75"
      }
    },
    {
      "id": "adjust_ac",
      "type": "action",
      "config": {
        "action_type": "control",
        "channel_id": 1001,
        "point_type": "a",
        "point_id": 50001,
        "value": "$comfort_calc"
      }
    }
  ],
  "edges": [
    { "from": "temp_input", "to": "comfort_calc" },
    { "from": "humidity_input", "to": "comfort_calc" },
    { "from": "comfort_calc", "to": "check_comfort" },
    { "from": "check_comfort", "to": "adjust_ac", "condition": "$check_comfort == true" }
  ]
}
```

## 开发指南

### 添加新的条件类型

1. 在 `models/rule.rs` 中添加新的条件枚举：

```rust
pub enum ConditionType {
    // ... 现有条件
    MyNewCondition {
        param1: String,
        param2: f64,
    },
}
```

2. 在 `engine/evaluator.rs` 中实现条件评估逻辑：

```rust
match condition {
    ConditionType::MyNewCondition { param1, param2 } => {
        // 实现条件评估逻辑
    }
}
```

### 添加新的动作处理器

1. 实现 `ActionHandler` trait：

```rust
pub struct MyActionHandler {
    // 处理器状态
}

#[async_trait]
impl ActionHandler for MyActionHandler {
    fn can_handle(&self, action_type: &str) -> bool {
        action_type == "my_action"
    }
    
    fn name(&self) -> &str {
        "MyActionHandler"
    }
    
    async fn execute_action(
        &self,
        action_type: &str,
        config: &Value,
    ) -> Result<String> {
        // 实现动作执行逻辑
    }
}
```

2. 注册处理器：

```rust
let handler = MyActionHandler::new();
rule_executor.register_action_handler(handler).await?;
```

## 性能优化

### 1. 批量数据获取

使用 `BatchDataFetcher` 批量获取多个点位数据：

```rust
let fetcher = BatchDataFetcher::new(&redis_url)?;
let points = vec!["1001:m:10001", "1001:m:10002", "1001:m:10003"];
let values = fetcher.fetch_points(&points).await?;
```

### 2. 规则缓存

规则引擎自动缓存已加载的规则，减少 Redis 访问。

### 3. 并行执行

规则引擎支持并行执行多个独立的规则，通过 `max_parallel_executions` 配置控制并发度。

## 监控和调试

### 1. 日志

服务使用结构化日志，支持按级别过滤：

```bash
RUST_LOG=rulesrv=debug cargo run
```

### 2. 指标

通过 Prometheus 端点暴露指标：

- `rulesrv_rules_total`：规则总数
- `rulesrv_rule_executions_total`：规则执行次数
- `rulesrv_rule_execution_duration`：规则执行耗时
- `rulesrv_rule_failures_total`：规则执行失败次数

### 3. 健康检查

```bash
curl http://localhost:8080/health
```

## 故障处理

### 1. 规则执行失败

- 检查规则定义是否正确
- 查看执行历史了解失败原因
- 使用测试端点验证规则逻辑

### 2. Redis 连接问题

- 检查 Redis 服务状态
- 验证连接配置
- 查看网络连接

### 3. 性能问题

- 减少规则复杂度
- 优化表达式
- 增加并行执行数
- 使用批量数据获取

## 部署

### Docker

```bash
docker build -t rulesrv .
docker run -d \
  --name rulesrv \
  -p 8080:8080 \
  -e REDIS_URL=redis://redis:6379 \
  rulesrv
```

### Kubernetes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rulesrv
spec:
  replicas: 2
  selector:
    matchLabels:
      app: rulesrv
  template:
    metadata:
      labels:
        app: rulesrv
    spec:
      containers:
      - name: rulesrv
        image: rulesrv:latest
        ports:
        - containerPort: 8080
        env:
        - name: REDIS_URL
          value: redis://redis-service:6379
```

## 许可证

MIT License