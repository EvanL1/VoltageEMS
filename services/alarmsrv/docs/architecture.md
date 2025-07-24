# alarmsrv 架构设计

## 概述

alarmsrv 采用事件驱动架构，通过监控 Redis 数据流实现实时告警检测。服务使用简化的键值存储结构 `alarm:{id}`，配合多维度索引实现高效的告警管理和查询。

## 核心架构

```
┌─────────────────────────────────────────────────────────┐
│                      alarmsrv                           │
├─────────────────────────────────────────────────────────┤
│                   API Server                            │
│              (Alarms/Stats/Config)                      │
├─────────────────────────────────────────────────────────┤
│                 Monitor Engine                          │
│     ┌──────────────┬──────────────┬──────────────┐    │
│     │ Data         │ Threshold    │ Pattern      │    │
│     │ Subscriber   │ Evaluator    │ Matcher      │    │
│     └──────────────┴──────────────┴──────────────┘    │
├─────────────────────────────────────────────────────────┤
│                Alarm Processor                          │
│     ┌──────────┬──────────┬──────────┬──────────┐    │
│     │Detector  │Classifier │Creator   │Escalator │    │
│     └──────────┴──────────┴──────────┴──────────┘    │
├─────────────────────────────────────────────────────────┤
│                 Storage Layer                           │
│     ┌──────────────┬──────────────┬──────────────┐    │
│     │ Alarm Store  │ Index        │ Statistics   │    │
│     │ (alarm:id)   │ Manager      │ Collector    │    │
│     └──────────────┴──────────────┴──────────────┘    │
├─────────────────────────────────────────────────────────┤
│                  Redis Client                           │
│          ┌──────────────┬──────────────┐              │
│          │ Key-Value    │   Pub/Sub    │              │
│          └──────────────┴──────────────┘              │
└─────────────────────────────────────────────────────────┘
```

## 组件说明

### 1. Monitor Engine

负责监控数据源并检测告警条件：

```rust
pub struct MonitorEngine {
    subscribers: Vec<DataSubscriber>,
    evaluator: ThresholdEvaluator,
    pattern_matcher: PatternMatcher,
}

pub struct DataSubscriber {
    patterns: Vec<String>,
    redis_client: Arc<RedisClient>,
    handler: Arc<dyn DataHandler>,
}

impl DataSubscriber {
    pub async fn subscribe(&self) -> Result<()> {
        let mut pubsub = self.redis_client.get_async_pubsub().await?;
        
        // 订阅数据通道
        for pattern in &self.patterns {
            pubsub.psubscribe(pattern).await?;
        }
        
        // 处理消息
        while let Some(msg) = pubsub.on_message().next().await {
            self.handle_message(msg).await?;
        }
        
        Ok(())
    }
    
    async fn handle_message(&self, msg: PubSubMessage) -> Result<()> {
        let channel = msg.get_channel_name()?;
        let payload = msg.get_payload()?;
        
        // 解析消息格式: "pointID:value"
        if let Some((point_id, value)) = payload.split_once(':') {
            let data_point = DataPoint {
                channel: channel.to_string(),
                point_id: point_id.parse()?,
                value: value.parse::<f64>()?,
                timestamp: Utc::now(),
            };
            
            self.handler.handle_data(data_point).await?;
        }
        
        Ok(())
    }
}
```

### 2. Threshold Evaluator

评估数据是否触发告警阈值：

```rust
pub struct ThresholdEvaluator {
    rules: HashMap<String, Vec<ThresholdRule>>,
}

pub struct ThresholdRule {
    pub field: String,
    pub operator: ComparisonOperator,
    pub value: f64,
    pub level: AlarmLevel,
    pub debounce: Option<Duration>,
}

pub enum ComparisonOperator {
    GreaterThan,
    LessThan,
    Equal,
    NotEqual,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

impl ThresholdEvaluator {
    pub fn evaluate(
        &self,
        data_point: &DataPoint,
    ) -> Option<AlarmTrigger> {
        let key = format!("{}:{}", data_point.channel, data_point.point_id);
        
        if let Some(rules) = self.rules.get(&key) {
            for rule in rules {
                if self.check_rule(rule, data_point.value) {
                    return Some(AlarmTrigger {
                        rule: rule.clone(),
                        data_point: data_point.clone(),
                        triggered_at: Utc::now(),
                    });
                }
            }
        }
        
        None
    }
    
    fn check_rule(&self, rule: &ThresholdRule, value: f64) -> bool {
        match rule.operator {
            ComparisonOperator::GreaterThan => value > rule.value,
            ComparisonOperator::LessThan => value < rule.value,
            ComparisonOperator::Equal => (value - rule.value).abs() < f64::EPSILON,
            ComparisonOperator::NotEqual => (value - rule.value).abs() >= f64::EPSILON,
            ComparisonOperator::GreaterThanOrEqual => value >= rule.value,
            ComparisonOperator::LessThanOrEqual => value <= rule.value,
        }
    }
}
```

### 3. Alarm Classifier

智能分类告警：

```rust
pub struct AlarmClassifier {
    category_rules: HashMap<AlarmCategory, CategoryRule>,
}

pub struct CategoryRule {
    pub patterns: Vec<String>,
    pub keywords: Vec<String>,
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlarmCategory {
    Environmental,
    Power,
    Communication,
    System,
    Security,
    Unknown,
}

impl AlarmClassifier {
    pub fn classify(&self, alarm_data: &AlarmData) -> AlarmCategory {
        let mut scores: HashMap<AlarmCategory, f64> = HashMap::new();
        
        // 检查标题和描述
        let text = format!("{} {}", alarm_data.title, alarm_data.description);
        
        for (category, rule) in &self.category_rules {
            let mut score = 0.0;
            
            // 模式匹配
            for pattern in &rule.patterns {
                if text.contains(pattern) {
                    score += rule.weight;
                }
            }
            
            // 关键词匹配
            for keyword in &rule.keywords {
                if text.to_lowercase().contains(&keyword.to_lowercase()) {
                    score += rule.weight * 0.5;
                }
            }
            
            if score > 0.0 {
                scores.insert(category.clone(), score);
            }
        }
        
        // 返回得分最高的分类
        scores.into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(category, _)| category)
            .unwrap_or(AlarmCategory::Unknown)
    }
}
```

### 4. Storage Layer

简化的存储结构：

```rust
pub struct AlarmStorage {
    redis_client: Arc<RedisClient>,
    index_manager: IndexManager,
}

impl AlarmStorage {
    /// 存储告警（简化的键结构）
    pub async fn store_alarm(&self, alarm: &Alarm) -> Result<()> {
        let key = format!("alarm:{}", alarm.id);
        let value = serde_json::to_string(alarm)?;
        
        // 存储告警数据
        self.redis_client.set(&key, value).await?;
        
        // 更新索引
        self.index_manager.index_alarm(alarm).await?;
        
        Ok(())
    }
    
    /// 获取告警
    pub async fn get_alarm(&self, alarm_id: &str) -> Result<Option<Alarm>> {
        let key = format!("alarm:{}", alarm_id);
        
        match self.redis_client.get(&key).await? {
            Some(data) => {
                let alarm: Alarm = serde_json::from_str(&data)?;
                Ok(Some(alarm))
            }
            None => Ok(None),
        }
    }
}

/// 索引管理器
pub struct IndexManager {
    redis_client: Arc<RedisClient>,
}

impl IndexManager {
    pub async fn index_alarm(&self, alarm: &Alarm) -> Result<()> {
        let alarm_id = &alarm.id;
        
        // 状态索引
        let status_key = format!("alarm:index:{}", alarm.status.to_string().to_lowercase());
        self.redis_client.sadd(&status_key, alarm_id).await?;
        
        // 级别索引
        let level_key = format!("alarm:index:level:{}", alarm.level.to_string().to_lowercase());
        self.redis_client.sadd(&level_key, alarm_id).await?;
        
        // 分类索引
        let category_key = format!("alarm:index:category:{}", alarm.category.to_string().to_lowercase());
        self.redis_client.sadd(&category_key, alarm_id).await?;
        
        // 日期索引
        let date_key = format!("alarm:index:date:{}", alarm.created_at.format("%Y-%m-%d"));
        self.redis_client.sadd(&date_key, alarm_id).await?;
        
        Ok(())
    }
    
    pub async fn remove_from_indexes(&self, alarm: &Alarm) -> Result<()> {
        let alarm_id = &alarm.id;
        
        // 从所有相关索引中移除
        let keys = vec![
            format!("alarm:index:{}", alarm.status.to_string().to_lowercase()),
            format!("alarm:index:level:{}", alarm.level.to_string().to_lowercase()),
            format!("alarm:index:category:{}", alarm.category.to_string().to_lowercase()),
            format!("alarm:index:date:{}", alarm.created_at.format("%Y-%m-%d")),
        ];
        
        for key in keys {
            self.redis_client.srem(&key, alarm_id).await?;
        }
        
        Ok(())
    }
}
```

## 告警生命周期

### 状态流转

```
Created → Active → Acknowledged → Resolved → Archived
           ↓         ↓
       Escalated  Escalated
```

### 生命周期管理

```rust
pub struct AlarmLifecycle {
    storage: Arc<AlarmStorage>,
    notifier: Arc<NotificationService>,
}

impl AlarmLifecycle {
    pub async fn create_alarm(&self, alarm_data: AlarmData) -> Result<Alarm> {
        let alarm = Alarm {
            id: Uuid::new_v4().to_string(),
            title: alarm_data.title,
            description: alarm_data.description,
            category: alarm_data.category,
            level: alarm_data.level,
            status: AlarmStatus::Active,
            source: alarm_data.source,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            acknowledged_at: None,
            resolved_at: None,
        };
        
        // 存储告警
        self.storage.store_alarm(&alarm).await?;
        
        // 发送通知
        self.notifier.notify_alarm_created(&alarm).await?;
        
        Ok(alarm)
    }
    
    pub async fn acknowledge_alarm(
        &self,
        alarm_id: &str,
        acknowledged_by: &str,
        notes: Option<&str>,
    ) -> Result<()> {
        let mut alarm = self.storage.get_alarm(alarm_id).await?
            .ok_or_else(|| Error::AlarmNotFound)?;
        
        // 更新状态
        alarm.status = AlarmStatus::Acknowledged;
        alarm.acknowledged_at = Some(Utc::now());
        alarm.updated_at = Utc::now();
        
        // 保存更新
        self.storage.update_alarm(&alarm).await?;
        
        // 发送通知
        self.notifier.notify_alarm_acknowledged(&alarm).await?;
        
        Ok(())
    }
}
```

## 自动升级机制

### 升级引擎

```rust
pub struct EscalationEngine {
    rules: Vec<EscalationRule>,
    storage: Arc<AlarmStorage>,
}

pub struct EscalationRule {
    pub from_level: AlarmLevel,
    pub to_level: AlarmLevel,
    pub after: Duration,
    pub condition: EscalationCondition,
}

pub enum EscalationCondition {
    NotAcknowledged,
    NotResolved,
    Always,
}

impl EscalationEngine {
    pub async fn check_escalations(&self) -> Result<()> {
        // 获取活跃告警
        let active_alarms = self.storage.get_alarms_by_status(
            AlarmStatus::Active
        ).await?;
        
        for alarm in active_alarms {
            for rule in &self.rules {
                if self.should_escalate(&alarm, rule) {
                    self.escalate_alarm(&alarm, &rule.to_level).await?;
                }
            }
        }
        
        Ok(())
    }
    
    fn should_escalate(&self, alarm: &Alarm, rule: &EscalationRule) -> bool {
        // 检查级别匹配
        if alarm.level != rule.from_level {
            return false;
        }
        
        // 检查时间条件
        let elapsed = Utc::now() - alarm.created_at;
        if elapsed < rule.after {
            return false;
        }
        
        // 检查升级条件
        match rule.condition {
            EscalationCondition::NotAcknowledged => {
                alarm.acknowledged_at.is_none()
            }
            EscalationCondition::NotResolved => {
                alarm.resolved_at.is_none()
            }
            EscalationCondition::Always => true,
        }
    }
}
```

## 查询优化

### 多条件查询

```rust
pub async fn query_alarms(
    &self,
    filters: AlarmFilters,
) -> Result<Vec<Alarm>> {
    let mut alarm_ids = HashSet::new();
    let mut first_filter = true;
    
    // 状态过滤
    if let Some(status) = filters.status {
        let key = format!("alarm:index:{}", status.to_lowercase());
        let ids = self.redis_client.smembers(&key).await?;
        if first_filter {
            alarm_ids = ids.into_iter().collect();
            first_filter = false;
        } else {
            alarm_ids = alarm_ids.intersection(&ids.into_iter().collect()).cloned().collect();
        }
    }
    
    // 级别过滤
    if let Some(level) = filters.level {
        let key = format!("alarm:index:level:{}", level.to_lowercase());
        let ids = self.redis_client.smembers(&key).await?;
        if first_filter {
            alarm_ids = ids.into_iter().collect();
            first_filter = false;
        } else {
            alarm_ids = alarm_ids.intersection(&ids.into_iter().collect()).cloned().collect();
        }
    }
    
    // 批量获取告警数据
    let mut alarms = Vec::new();
    for id in alarm_ids.iter().take(filters.limit.unwrap_or(100)) {
        if let Some(alarm) = self.get_alarm(id).await? {
            alarms.push(alarm);
        }
    }
    
    // 排序
    alarms.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    
    Ok(alarms)
}
```

## 性能优化

### 1. 批量处理

```rust
pub struct BatchProcessor {
    buffer: Arc<Mutex<Vec<AlarmTrigger>>>,
    processor: Arc<AlarmProcessor>,
    config: BatchConfig,
}

impl BatchProcessor {
    pub async fn process_batch(&self) {
        let triggers = {
            let mut buffer = self.buffer.lock().await;
            std::mem::take(&mut *buffer)
        };
        
        if triggers.is_empty() {
            return;
        }
        
        // 去重
        let unique_triggers = self.deduplicate(triggers);
        
        // 批量创建告警
        for trigger in unique_triggers {
            self.processor.create_alarm_from_trigger(trigger).await.ok();
        }
    }
}
```

### 2. 缓存策略

```rust
pub struct AlarmCache {
    active_alarms: Arc<RwLock<HashMap<String, Alarm>>>,
    ttl: Duration,
}

impl AlarmCache {
    pub async fn get_or_load(
        &self,
        alarm_id: &str,
        loader: impl Future<Output = Result<Option<Alarm>>>,
    ) -> Result<Option<Alarm>> {
        // 检查缓存
        if let Some(alarm) = self.active_alarms.read().await.get(alarm_id) {
            return Ok(Some(alarm.clone()));
        }
        
        // 从存储加载
        let alarm = loader.await?;
        
        // 更新缓存
        if let Some(ref a) = alarm {
            if a.status == AlarmStatus::Active {
                self.active_alarms.write().await.insert(alarm_id.to_string(), a.clone());
            }
        }
        
        Ok(alarm)
    }
}
```

## 监控指标

```rust
pub struct Metrics {
    alarms_created: IntCounter,
    alarms_acknowledged: IntCounter,
    alarms_resolved: IntCounter,
    alarms_escalated: IntCounter,
    active_alarms: IntGauge,
    processing_duration: Histogram,
}
```

## 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum AlarmError {
    #[error("Alarm not found: {0}")]
    AlarmNotFound(String),
    
    #[error("Invalid alarm state transition")]
    InvalidStateTransition,
    
    #[error("Threshold configuration error: {0}")]
    ThresholdConfig(String),
    
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
}
```