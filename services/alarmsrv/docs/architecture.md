# alarmsrv 架构设计

## 概述

alarmsrv（Alarm Service）是 VoltageEMS 的告警管理服务，负责实时监控系统数据，检测异常情况，生成告警事件，并通过多种渠道发送通知。

## 架构特点

1. **实时检测**：毫秒级告警触发
2. **规则引擎**：灵活的告警规则配置
3. **智能抑制**：防止告警风暴
4. **多渠道通知**：邮件、短信、Webhook
5. **告警生命周期**：完整的告警状态管理

## 系统架构图

```
┌────────────────────────────────────────────────────────────┐
│                        alarmsrv                             │
├────────────────────────────────────────────────────────────┤
│                   Detection Layer                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │Data Monitor  │  │Rule Engine   │  │Threshold Check│    │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘    │
│         └──────────────────┴──────────────────┘            │
│                            │                                │
│                   Processing Layer                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │Alarm Manager │  │Suppression   │  │Aggregation   │    │
│  └──────────────┘  └──────────────┘  └──────────────┘    │
│                            │                                │
│                  Notification Layer                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │Email Sender  │  │SMS Gateway   │  │Webhook Client│    │
│  └──────────────┘  └──────────────┘  └──────────────┘    │
└────────────────────────────────────────────────────────────┘
```

## 核心组件

### 1. Alarm Detection（告警检测）

#### 数据监控器
```rust
pub struct DataMonitor {
    redis_client: Arc<RedisClient>,
    rule_engine: Arc<RuleEngine>,
    alarm_queue: mpsc::Sender<AlarmEvent>,
}

impl DataMonitor {
    /// 监控实时数据
    pub async fn monitor_realtime_data(&self) -> Result<()> {
        let mut pubsub = self.redis_client.get_async_pubsub().await?;
        pubsub.psubscribe("point:update:*").await?;
        
        while let Some(msg) = pubsub.on_message().next().await {
            if let Ok(point_data) = self.parse_point_update(&msg) {
                // 检查告警规则
                if let Some(alarm) = self.rule_engine.check(&point_data).await? {
                    self.alarm_queue.send(alarm).await?;
                }
            }
        }
        
        Ok(())
    }
}
```

#### 规则引擎
```rust
pub struct RuleEngine {
    rules: Arc<RwLock<HashMap<String, AlarmRule>>>,
    expression_engine: ExpressionEngine,
}

#[derive(Debug, Clone)]
pub struct AlarmRule {
    /// 规则ID
    pub id: String,
    
    /// 规则名称
    pub name: String,
    
    /// 触发条件
    pub condition: Condition,
    
    /// 告警级别
    pub severity: AlarmSeverity,
    
    /// 告警内容模板
    pub message_template: String,
    
    /// 抑制策略
    pub suppression: Option<SuppressionPolicy>,
}

impl RuleEngine {
    /// 检查数据是否触发告警
    pub async fn check(&self, data: &PointData) -> Result<Option<AlarmEvent>> {
        let rules = self.rules.read().await;
        
        for rule in rules.values() {
            if self.evaluate_condition(&rule.condition, data).await? {
                return Ok(Some(self.create_alarm_event(rule, data)?));
            }
        }
        
        Ok(None)
    }
}
```

### 2. Alarm Processing（告警处理）

#### 告警管理器
```rust
pub struct AlarmManager {
    /// 活跃告警
    active_alarms: Arc<RwLock<HashMap<String, ActiveAlarm>>>,
    
    /// 告警历史
    alarm_history: Arc<AlarmHistory>,
    
    /// 通知管理器
    notifier: Arc<NotificationManager>,
}

impl AlarmManager {
    /// 处理新告警
    pub async fn process_alarm(&self, event: AlarmEvent) -> Result<()> {
        // 1. 检查是否需要抑制
        if self.should_suppress(&event).await? {
            return Ok(());
        }
        
        // 2. 创建或更新告警
        let alarm = self.create_or_update_alarm(event).await?;
        
        // 3. 发送通知
        if alarm.is_new || alarm.severity_changed {
            self.notifier.send_notification(&alarm).await?;
        }
        
        // 4. 记录历史
        self.alarm_history.record(&alarm).await?;
        
        Ok(())
    }
    
    /// 告警恢复
    pub async fn recover_alarm(&self, alarm_id: &str) -> Result<()> {
        let mut active = self.active_alarms.write().await;
        
        if let Some(alarm) = active.remove(alarm_id) {
            // 发送恢复通知
            self.notifier.send_recovery(&alarm).await?;
            
            // 更新历史记录
            self.alarm_history.mark_recovered(alarm_id).await?;
        }
        
        Ok(())
    }
}
```

#### 告警抑制
```rust
pub struct SuppressionManager {
    /// 抑制规则
    rules: Vec<SuppressionRule>,
    
    /// 告警计数器
    counters: Arc<RwLock<HashMap<String, AlarmCounter>>>,
}

#[derive(Debug, Clone)]
pub struct SuppressionRule {
    /// 时间窗口
    pub window: Duration,
    
    /// 最大告警数
    pub max_count: u32,
    
    /// 抑制时长
    pub suppress_duration: Duration,
}

impl SuppressionManager {
    /// 检查是否应该抑制
    pub async fn should_suppress(&self, alarm: &AlarmEvent) -> bool {
        let mut counters = self.counters.write().await;
        let counter = counters.entry(alarm.rule_id.clone())
            .or_insert_with(|| AlarmCounter::new());
        
        // 更新计数
        counter.increment();
        
        // 检查抑制条件
        for rule in &self.rules {
            if counter.count_in_window(rule.window) > rule.max_count {
                counter.suppress_until = Some(Instant::now() + rule.suppress_duration);
                return true;
            }
        }
        
        // 检查是否在抑制期
        if let Some(until) = counter.suppress_until {
            return Instant::now() < until;
        }
        
        false
    }
}
```

### 3. Notification（通知系统）

#### 通知管理器
```rust
pub struct NotificationManager {
    channels: Vec<Box<dyn NotificationChannel>>,
    router: NotificationRouter,
    template_engine: TemplateEngine,
}

#[async_trait]
pub trait NotificationChannel: Send + Sync {
    /// 发送通知
    async fn send(&self, notification: &Notification) -> Result<()>;
    
    /// 检查可用性
    async fn is_available(&self) -> bool;
}
```

#### 邮件通知
```rust
pub struct EmailChannel {
    smtp_client: SmtpClient,
    config: EmailConfig,
}

#[async_trait]
impl NotificationChannel for EmailChannel {
    async fn send(&self, notification: &Notification) -> Result<()> {
        let email = Message::builder()
            .from(self.config.from.parse()?)
            .to(notification.recipient.parse()?)
            .subject(&notification.subject)
            .body(notification.content.clone())?;
        
        self.smtp_client.send(email).await?;
        Ok(())
    }
}
```

#### Webhook 通知
```rust
pub struct WebhookChannel {
    http_client: reqwest::Client,
    endpoints: Vec<WebhookEndpoint>,
}

impl WebhookChannel {
    async fn send_webhook(&self, alarm: &AlarmEvent) -> Result<()> {
        let payload = json!({
            "alarm_id": alarm.id,
            "severity": alarm.severity,
            "message": alarm.message,
            "timestamp": alarm.timestamp,
            "tags": alarm.tags,
            "data": alarm.data,
        });
        
        for endpoint in &self.endpoints {
            let response = self.http_client
                .post(&endpoint.url)
                .header("X-Alarm-Token", &endpoint.token)
                .json(&payload)
                .timeout(Duration::from_secs(5))
                .send()
                .await?;
            
            if !response.status().is_success() {
                error!("Webhook failed: {}", response.status());
            }
        }
        
        Ok(())
    }
}
```

## 告警规则配置

### 规则定义
```yaml
rules:
  - id: "high_temperature"
    name: "高温告警"
    condition:
      type: threshold
      field: value
      operator: ">"
      threshold: 85
      duration: 60s  # 持续60秒触发
    severity: warning
    message: "设备 {device_name} 温度过高: {value}°C"
    tags:
      - temperature
      - safety
      
  - id: "power_failure"
    name: "电源故障"
    condition:
      type: expression
      expression: "voltage_a < 50 && voltage_b < 50 && voltage_c < 50"
    severity: critical
    message: "设备 {device_name} 电源故障"
    actions:
      - type: webhook
        url: "https://api.example.com/critical-alerts"
```

### 条件类型

#### 阈值条件
```rust
pub enum ThresholdCondition {
    /// 简单阈值
    Simple {
        field: String,
        operator: ComparisonOp,
        threshold: f64,
    },
    
    /// 范围条件
    Range {
        field: String,
        min: f64,
        max: f64,
        inclusive: bool,
    },
    
    /// 持续时间
    Duration {
        condition: Box<ThresholdCondition>,
        duration: Duration,
    },
}
```

#### 表达式条件
```rust
pub struct ExpressionCondition {
    /// 表达式字符串
    expression: String,
    
    /// 变量映射
    variables: HashMap<String, String>,
    
    /// 计算引擎
    evaluator: ExpressionEvaluator,
}
```

## 告警状态管理

### 告警生命周期
```
Created → Active → Acknowledged → Recovering → Recovered → Closed
   │         │           │            │            │
   └─────────┴───────────┴────────────┴────────────┴─→ Suppressed
```

### 状态转换
```rust
pub struct AlarmStateMachine {
    transitions: HashMap<(AlarmState, AlarmEvent), AlarmState>,
}

impl AlarmStateMachine {
    pub fn transition(
        &self,
        current: AlarmState,
        event: AlarmEvent,
    ) -> Result<AlarmState> {
        self.transitions
            .get(&(current, event))
            .cloned()
            .ok_or_else(|| Error::InvalidTransition)
    }
}
```

## 性能优化

### 1. 规则缓存
```rust
pub struct RuleCache {
    /// 编译后的规则
    compiled_rules: Arc<RwLock<HashMap<String, CompiledRule>>>,
    
    /// 规则索引
    point_index: Arc<RwLock<HashMap<String, Vec<String>>>>,
}
```

### 2. 批量处理
```rust
pub struct BatchProcessor {
    batch_size: usize,
    flush_interval: Duration,
    buffer: Arc<Mutex<Vec<AlarmEvent>>>,
}
```

### 3. 异步通知
```rust
pub struct AsyncNotifier {
    /// 通知队列
    queue: Arc<SegQueue<Notification>>,
    
    /// 工作线程数
    worker_count: usize,
}
```

## 监控指标

- 告警触发率
- 规则评估耗时
- 通知发送成功率
- 活跃告警数量
- 抑制告警数量

## 配置示例

```yaml
# alarmsrv 配置
redis:
  url: "redis://localhost:6379"
  
engine:
  max_rules: 1000
  evaluation_threads: 4
  
suppression:
  default_window: 5m
  default_max_count: 10
  
notification:
  channels:
    email:
      enabled: true
      smtp_host: "smtp.example.com"
      smtp_port: 587
      from: "alerts@example.com"
      
    webhook:
      enabled: true
      endpoints:
        - url: "https://api.example.com/alerts"
          token: "${WEBHOOK_TOKEN}"
          
history:
  retention_days: 30
  archive_path: "/var/lib/alarmsrv/archive"
```

## 集成示例

### 钉钉通知
```rust
pub struct DingTalkChannel {
    webhook_url: String,
    secret: String,
}

impl DingTalkChannel {
    async fn send_dingtalk(&self, alarm: &AlarmEvent) -> Result<()> {
        let timestamp = Utc::now().timestamp_millis();
        let sign = self.calculate_sign(timestamp)?;
        
        let message = json!({
            "msgtype": "markdown",
            "markdown": {
                "title": format!("[{}] {}", alarm.severity, alarm.name),
                "text": self.format_markdown(alarm),
            },
            "at": {
                "isAtAll": alarm.severity == AlarmSeverity::Critical,
            }
        });
        
        // 发送请求...
    }
}
```