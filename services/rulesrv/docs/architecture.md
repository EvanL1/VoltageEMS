# rulesrv 架构设计

## 概述

rulesrv 是 VoltageEMS 的规则引擎服务，负责实时监控系统数据、执行业务规则、触发自动化动作。服务采用高性能的规则评估引擎，支持复杂的条件判断和灵活的动作执行。

## 架构原则

1. **实时响应**：毫秒级规则评估，快速响应数据变化
2. **灵活配置**：支持热更新规则，无需重启服务
3. **可扩展性**：插件化的条件和动作设计
4. **高可靠性**：规则执行隔离，单个规则失败不影响其他规则

## 系统架构

```
┌─────────────────────────────────────────────────────────────┐
│                      rulesrv 规则引擎                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    │
│  │  数据订阅器  │    │  规则管理器  │    │  动作执行器  │    │
│  │             │    │             │    │             │    │
│  │ - 点位订阅   │    │ - 规则加载   │    │ - 控制命令   │    │
│  │ - 事件订阅   │    │ - 热更新     │    │ - 告警生成   │    │
│  │ - 批量获取   │    │ - 规则缓存   │    │ - 通知发送   │    │
│  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘    │
│         │                  │                  │            │
│         └──────────────────┴──────────────────┘            │
│                           │                                │
│                    ┌──────┴──────┐                         │
│                    │  规则引擎    │                         │
│                    │             │                         │
│                    │ - 表达式评估 │                         │
│                    │ - 状态机管理 │                         │
│                    │ - 调度系统   │                         │
│                    └──────┬──────┘                         │
│                           │                                │
└───────────────────────────┼─────────────────────────────────┘
                            │
                     ┌──────┴──────┐
                     │    Redis    │
                     └─────────────┘
```

## 核心组件

### 1. 数据订阅器 (Data Subscriber)

负责从 Redis 获取实时数据和事件。

```rust
pub struct DataSubscriber {
    redis: RedisClient,
    subscriptions: HashMap<String, Subscription>,
    cache: DataCache,
}

pub struct Subscription {
    pub pattern: String,          // 订阅模式，如 "1001:m:*"
    pub callback: SubscriptionCallback,
    pub batch_config: Option<BatchConfig>,
}
```

**功能特点**：
- 支持通配符订阅
- 批量数据获取优化
- 本地缓存减少查询

### 2. 规则管理器 (Rule Manager)

管理所有规则的生命周期。

```rust
pub struct RuleManager {
    rules: HashMap<String, Rule>,
    rule_groups: HashMap<String, RuleGroup>,
    config_watcher: ConfigWatcher,
}

pub struct Rule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: u8,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    pub metadata: RuleMetadata,
}
```

**规则结构**：
- 多条件组合（AND/OR）
- 优先级控制
- 分组管理
- 元数据扩展

### 3. 规则引擎 (Rule Engine)

核心的规则评估和执行引擎。

```rust
pub struct RuleEngine {
    evaluator: ExpressionEvaluator,
    state_manager: StateManager,
    scheduler: RuleScheduler,
    executor: ActionExecutor,
}

impl RuleEngine {
    pub async fn evaluate(&self, rule: &Rule, context: &Context) -> Result<bool> {
        // 评估所有条件
        for condition in &rule.conditions {
            if !self.evaluator.evaluate(condition, context).await? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
```

### 4. 动作执行器 (Action Executor)

执行规则触发的动作。

```rust
pub struct ActionExecutor {
    handlers: HashMap<ActionType, Box<dyn ActionHandler>>,
    redis: RedisClient,
    rate_limiter: RateLimiter,
}

#[async_trait]
pub trait ActionHandler {
    async fn execute(&self, action: &Action, context: &Context) -> Result<()>;
}
```

## 规则定义

### 规则配置示例

```yaml
rules:
  - id: high_temperature_alarm
    name: 高温告警
    enabled: true
    priority: 10
    trigger:
      type: data_change
      sources:
        - pattern: "1001:m:10001"  # 温度传感器
    conditions:
      - type: threshold
        expression: "value > 80"
        duration: 60s  # 持续60秒
    actions:
      - type: alarm
        level: critical
        message: "温度超过80度，当前值: ${value}"
      - type: control
        target:
          channel_id: 1001
          point_type: "c"
          point_id: 30001
        value: 0  # 关闭加热器

  - id: power_optimization
    name: 功率优化
    enabled: true
    schedule:
      cron: "*/5 * * * *"  # 每5分钟执行
    conditions:
      - type: expression
        expression: |
          avg("1001:m:20001..20010") > 1000 AND 
          time.hour >= 9 AND time.hour <= 18
    actions:
      - type: calculate
        formula: "optimal_power = current_power * 0.95"
      - type: batch_control
        targets:
          - channel: 1001
            controls:
              - { type: "a", id: 40001, value: "${optimal_power}" }
```

## 条件类型

### 1. 阈值条件 (Threshold)

```rust
pub struct ThresholdCondition {
    pub operator: ComparisonOperator,  // >, <, >=, <=, ==, !=
    pub value: f64,
    pub duration: Option<Duration>,     // 持续时间
    pub hysteresis: Option<f64>,        // 滞后值
}
```

### 2. 范围条件 (Range)

```rust
pub struct RangeCondition {
    pub min: f64,
    pub max: f64,
    pub inclusive: bool,
}
```

### 3. 表达式条件 (Expression)

```rust
pub struct ExpressionCondition {
    pub expression: String,  // 支持复杂表达式
    pub variables: HashMap<String, String>,
}
```

### 4. 状态条件 (State)

```rust
pub struct StateCondition {
    pub state_name: String,
    pub expected_state: String,
    pub timeout: Option<Duration>,
}
```

## 动作类型

### 1. 控制动作

发送控制命令到设备。

```rust
pub struct ControlAction {
    pub channel_id: u16,
    pub point_type: String,
    pub point_id: u32,
    pub value: f64,
    pub confirm_timeout: Duration,
}
```

### 2. 告警动作

生成告警事件。

```rust
pub struct AlarmAction {
    pub level: AlarmLevel,
    pub category: String,
    pub message: String,
    pub auto_resolve: bool,
}
```

### 3. 通知动作

发送通知消息。

```rust
pub struct NotificationAction {
    pub channels: Vec<NotificationChannel>,
    pub template: String,
    pub recipients: Vec<String>,
}
```

### 4. 计算动作

执行计算并存储结果。

```rust
pub struct CalculationAction {
    pub formula: String,
    pub output_key: String,
    pub cache_duration: Duration,
}
```

## 表达式引擎

支持丰富的表达式语法：

```rust
// 基本运算
value + 10
value * 0.9
(value1 + value2) / 2

// 函数调用
avg("1001:m:10001..10010")
max(value1, value2, value3)
min_in_window("1001:m:10001", "5m")

// 条件判断
value > 100 ? "high" : "normal"
if(state == "running", power * 0.8, 0)

// 时间函数
time.hour >= 8 AND time.hour <= 20
time.dayOfWeek != "Sunday"
```

## 状态管理

### 规则状态机

```rust
pub enum RuleState {
    Idle,           // 空闲
    Evaluating,     // 评估中
    Triggered,      // 已触发
    Executing,      // 执行中
    Completed,      // 已完成
    Failed,         // 失败
}

pub struct RuleStateMachine {
    current_state: RuleState,
    transitions: HashMap<(RuleState, Event), RuleState>,
    history: Vec<StateTransition>,
}
```

### 持久化状态

```rust
// Redis 键格式
rule:state:{rule_id} -> RuleState
rule:history:{rule_id} -> List<StateTransition>
rule:context:{rule_id} -> Context
```

## 性能优化

### 1. 批量评估

```rust
pub struct BatchEvaluator {
    pub batch_size: usize,
    pub parallel_workers: usize,
}

impl BatchEvaluator {
    pub async fn evaluate_rules(&self, rules: Vec<Rule>) -> Vec<RuleResult> {
        let chunks = rules.chunks(self.batch_size);
        let mut handles = vec![];
        
        for chunk in chunks {
            let handle = tokio::spawn(async move {
                // 并行评估规则
            });
            handles.push(handle);
        }
        
        futures::future::join_all(handles).await
    }
}
```

### 2. 缓存策略

- 表达式编译缓存
- 数据预取缓存
- 规则结果缓存

### 3. 优先级队列

```rust
pub struct PriorityExecutor {
    high_priority: Queue<Rule>,
    normal_priority: Queue<Rule>,
    low_priority: Queue<Rule>,
}
```

## 错误处理

### 规则隔离

每个规则在独立的上下文中执行，失败不影响其他规则。

```rust
pub async fn execute_with_isolation(&self, rule: &Rule) -> Result<()> {
    let result = tokio::time::timeout(
        rule.timeout,
        self.execute_rule(rule)
    ).await;
    
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => {
            self.handle_rule_error(rule, e).await;
            Err(e)
        }
        Err(_) => {
            self.handle_rule_timeout(rule).await;
            Err(Error::Timeout)
        }
    }
}
```

### 重试机制

```rust
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub exponential_base: f64,
}
```

## 监控指标

```rust
pub struct RuleMetrics {
    pub evaluation_count: Counter,
    pub trigger_count: Counter,
    pub execution_duration: Histogram,
    pub failure_count: Counter,
    pub active_rules: Gauge,
}
```

## 配置示例

```yaml
service:
  name: rulesrv
  redis:
    url: redis://localhost:6379
    pool_size: 20

engine:
  max_parallel_rules: 100
  evaluation_timeout: 5s
  batch_size: 50

data_subscription:
  buffer_size: 10000
  batch_interval: 100ms

action_execution:
  rate_limit:
    max_per_minute: 1000
  retry:
    max_attempts: 3
    initial_delay: 1s

monitoring:
  metrics_port: 9093
  health_check_interval: 30s
```

## 部署注意事项

1. **高可用部署**
   - 支持多实例部署
   - 使用 Redis 分布式锁避免重复执行
   - 规则分片负载均衡

2. **性能调优**
   - 根据规则数量调整并行度
   - 优化 Redis 连接池大小
   - 合理设置缓存策略

3. **安全考虑**
   - 规则执行沙箱
   - 表达式注入防护
   - 动作权限控制