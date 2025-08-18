# VoltageEMS 服务架构优化方案

## 一、服务架构总览

### 1.1 当前服务矩阵

| 服务 | 端口 | 职责 | 技术栈 | 状态 |
|-----|------|------|--------|------|
| **comsrv** | 6000 | 通信协议采集 | Rust + Tokio | 已优化 |
| **modsrv** | 6001 | 数据建模 | Rust + Redis Lua | 待优化 |
| **alarmsrv** | 6002 | 告警管理 | Rust + Redis | 待优化 |
| **rulesrv** | 6003 | 规则引擎 | Rust + Redis Lua | 待优化 |
| **hissrv** | 6004 | 历史数据 | Rust + InfluxDB | 待优化 |
| **apigateway** | 6005 | API聚合 | Rust + Axum | 待优化 |
| **netsrv** | 6006 | 外部通信 | Rust + HTTP/MQTT | 待优化 |

### 1.2 数据流架构
```
设备 → ComSrv → Redis → ModSrv → 业务服务(Alarm/Rule/His) → API Gateway → 应用
                  ↓                           ↓
                NetSrv → 外部系统        InfluxDB(历史)
```

## 二、各服务功能分析与优化

### 2.1 AlarmSrv (告警服务)

#### 当前功能
- 告警配置管理（CRUD）
- 告警触发检测
- 告警状态管理
- 告警历史记录

#### 存在问题
1. **性能问题**
   - 逐条检测告警规则，效率低
   - 无告警抑制机制，可能产生告警风暴
   - 缺少告警聚合功能

2. **功能缺失**
   - 无告警升级机制
   - 缺少智能告警（基于AI/ML）
   - 无告警关联分析

#### 优化方案
```rust
// 1. 批量告警检测
async fn batch_check_alarms(data: Vec<DataPoint>) -> Vec<Alarm> {
    let rules = load_alarm_rules().await?;
    
    // 并行检测
    let futures = data.iter().map(|point| {
        let rules = rules.clone();
        tokio::spawn(async move {
            check_point_against_rules(point, rules).await
        })
    });
    
    futures::future::join_all(futures).await
}

// 2. 告警抑制
struct AlarmSuppressor {
    window: Duration,
    threshold: usize,
    cache: LRUCache<String, Vec<Instant>>,
}

impl AlarmSuppressor {
    fn should_suppress(&mut self, alarm_id: &str) -> bool {
        let now = Instant::now();
        let history = self.cache.get_mut(alarm_id);
        
        // 清理过期记录
        history.retain(|t| now.duration_since(*t) < self.window);
        
        // 检查是否超过阈值
        if history.len() >= self.threshold {
            return true; // 抑制告警
        }
        
        history.push(now);
        false
    }
}

// 3. 告警关联分析
async fn correlate_alarms(alarms: Vec<Alarm>) -> Vec<AlarmGroup> {
    // 基于时间窗口、设备关系、告警类型进行关联
    let mut groups = Vec::new();
    
    for alarm in alarms {
        if let Some(group) = find_related_group(&alarm, &groups) {
            group.add(alarm);
        } else {
            groups.push(AlarmGroup::new(alarm));
        }
    }
    
    groups
}
```

### 2.2 RuleSrv (规则引擎)

#### 当前功能
- 规则配置管理
- 定时规则执行
- 条件判断与动作触发
- 规则执行统计

#### 存在问题
1. **性能瓶颈**
   - 串行执行规则，无并发
   - 规则匹配算法简单，效率低
   - 缺少规则优化器

2. **功能限制**
   - 不支持复杂事件处理(CEP)
   - 无规则链和规则树
   - 缺少规则测试框架

#### 优化方案
```rust
// 1. 规则执行引擎优化
struct RuleEngine {
    rules: Arc<RwLock<RuleTree>>,
    executor: ThreadPool,
}

impl RuleEngine {
    // 使用规则树优化匹配
    async fn execute(&self, context: Context) -> Vec<Action> {
        let rules = self.rules.read().await;
        let matched = rules.match_rules(&context);
        
        // 并行执行匹配的规则
        let futures = matched.iter().map(|rule| {
            let ctx = context.clone();
            self.executor.spawn(async move {
                rule.execute(ctx).await
            })
        });
        
        futures::future::join_all(futures).await
    }
}

// 2. 复杂事件处理(CEP)
struct EventPattern {
    sequence: Vec<EventMatcher>,
    window: Duration,
}

impl EventPattern {
    fn match_sequence(&self, events: &[Event]) -> bool {
        // 使用状态机匹配事件序列
        let mut state = 0;
        let mut start_time = None;
        
        for event in events {
            if self.sequence[state].matches(event) {
                if state == 0 {
                    start_time = Some(event.timestamp);
                }
                state += 1;
                
                if state == self.sequence.len() {
                    // 检查时间窗口
                    if let Some(start) = start_time {
                        if event.timestamp - start <= self.window {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

// 3. 规则测试框架
#[cfg(test)]
mod rule_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_temperature_rule() {
        let rule = Rule::from_yaml("
            condition: temperature > 80
            action: send_alarm
        ");
        
        let context = Context {
            temperature: 85.0,
            ..Default::default()
        };
        
        let actions = rule.execute(context).await;
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].type, "send_alarm");
    }
}
```

### 2.3 HisSrv (历史服务)

#### 当前功能
- 数据采集与存储
- 历史查询接口
- 数据聚合统计
- 定时数据归档

#### 存在问题
1. **存储效率**
   - 未实现数据压缩
   - 缺少分层存储策略
   - 无数据降采样

2. **查询性能**
   - 缺少查询缓存
   - 无预聚合数据
   - 查询优化不足

#### 优化方案
```rust
// 1. 数据压缩与降采样
struct DataCompressor {
    algorithm: CompressionAlgorithm,
    downsampling_rules: Vec<DownsamplingRule>,
}

impl DataCompressor {
    async fn process(&self, data: Vec<DataPoint>) -> CompressedData {
        // 先降采样
        let downsampled = self.downsample(data);
        
        // 再压缩
        match self.algorithm {
            CompressionAlgorithm::Gorilla => {
                // Facebook's Gorilla时序压缩
                compress_gorilla(downsampled)
            },
            CompressionAlgorithm::Delta => {
                // Delta编码
                compress_delta(downsampled)
            },
        }
    }
    
    fn downsample(&self, data: Vec<DataPoint>) -> Vec<DataPoint> {
        // 根据数据年龄应用不同的降采样策略
        // 1小时内: 原始数据
        // 1天内: 1分钟平均
        // 1周内: 5分钟平均
        // 1月内: 1小时平均
        // 更久: 1天平均
    }
}

// 2. 分层存储
struct TieredStorage {
    hot: RedisStorage,      // 热数据：最近1小时
    warm: InfluxDB,         // 温数据：最近1个月
    cold: S3Storage,        // 冷数据：历史归档
}

impl TieredStorage {
    async fn query(&self, range: TimeRange) -> Vec<DataPoint> {
        let now = Utc::now();
        
        if range.end > now - Duration::hours(1) {
            // 查询热数据
            self.hot.query(range).await
        } else if range.end > now - Duration::days(30) {
            // 查询温数据
            self.warm.query(range).await
        } else {
            // 查询冷数据
            self.cold.query(range).await
        }
    }
}

// 3. 查询优化
struct QueryOptimizer {
    cache: Arc<RwLock<LRUCache<String, CachedResult>>>,
    pre_aggregations: HashMap<String, PreAggregation>,
}

impl QueryOptimizer {
    async fn query(&self, request: QueryRequest) -> QueryResult {
        // 检查缓存
        let cache_key = request.cache_key();
        if let Some(cached) = self.cache.read().await.get(&cache_key) {
            if cached.is_valid() {
                return cached.result.clone();
            }
        }
        
        // 检查是否可以使用预聚合
        if let Some(pre_agg) = self.can_use_pre_aggregation(&request) {
            return self.query_pre_aggregation(pre_agg, request).await;
        }
        
        // 执行原始查询
        let result = self.execute_query(request).await?;
        
        // 更新缓存
        self.cache.write().await.put(cache_key, CachedResult::new(result.clone()));
        
        result
    }
}
```

### 2.4 NetSrv (网络服务)

#### 当前功能
- HTTP/MQTT协议支持
- 数据格式转换
- 外部系统集成
- 连接管理

#### 存在问题
1. **协议支持**
   - 协议支持有限
   - 缺少协议转换
   - 无协议适配器框架

2. **可靠性**
   - 缺少断线重连
   - 无消息队列缓冲
   - 缺少流量控制

#### 优化方案
```rust
// 1. 协议适配器框架
trait ProtocolAdapter: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn send(&self, data: Message) -> Result<()>;
    async fn receive(&mut self) -> Result<Message>;
    fn protocol_type(&self) -> ProtocolType;
}

// 实现各种协议适配器
struct MqttAdapter { /* ... */ }
struct KafkaAdapter { /* ... */ }
struct WebSocketAdapter { /* ... */ }
struct OpcUaAdapter { /* ... */ }

// 2. 消息缓冲队列
struct MessageBuffer {
    queue: Arc<RwLock<VecDeque<Message>>>,
    max_size: usize,
    overflow_strategy: OverflowStrategy,
}

impl MessageBuffer {
    async fn push(&self, message: Message) -> Result<()> {
        let mut queue = self.queue.write().await;
        
        if queue.len() >= self.max_size {
            match self.overflow_strategy {
                OverflowStrategy::DropOldest => {
                    queue.pop_front();
                },
                OverflowStrategy::DropNewest => {
                    return Err(Error::BufferFull);
                },
                OverflowStrategy::Block => {
                    // 等待空间
                    while queue.len() >= self.max_size {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                },
            }
        }
        
        queue.push_back(message);
        Ok(())
    }
}

// 3. 流量控制
struct RateLimiter {
    tokens: Arc<Mutex<f64>>,
    rate: f64,
    capacity: f64,
    last_update: Arc<Mutex<Instant>>,
}

impl RateLimiter {
    async fn acquire(&self, tokens: f64) -> Result<()> {
        loop {
            let now = Instant::now();
            let mut last = self.last_update.lock().await;
            let mut available = self.tokens.lock().await;
            
            // 补充令牌
            let elapsed = now.duration_since(*last).as_secs_f64();
            *available = (*available + elapsed * self.rate).min(self.capacity);
            *last = now;
            
            if *available >= tokens {
                *available -= tokens;
                return Ok(());
            }
            
            // 等待令牌
            let wait_time = (tokens - *available) / self.rate;
            tokio::time::sleep(Duration::from_secs_f64(wait_time)).await;
        }
    }
}
```

### 2.5 API Gateway (API网关)

#### 当前功能
- API路由分发
- 健康检查
- CORS支持
- 基础认证

#### 存在问题
1. **功能缺失**
   - 无API限流
   - 缺少请求缓存
   - 无API版本管理
   - 缺少API文档

2. **性能问题**
   - 无负载均衡
   - 缺少熔断器
   - 无请求合并

#### 优化方案
```rust
// 1. API限流
struct ApiRateLimiter {
    limiters: Arc<RwLock<HashMap<String, RateLimiter>>>,
    default_rate: f64,
}

impl ApiRateLimiter {
    async fn check_rate_limit(&self, client_id: &str) -> Result<()> {
        let mut limiters = self.limiters.write().await;
        let limiter = limiters.entry(client_id.to_string())
            .or_insert_with(|| RateLimiter::new(self.default_rate));
        
        limiter.acquire(1.0).await
    }
}

// 2. 请求缓存
struct RequestCache {
    cache: Arc<RwLock<LRUCache<String, CachedResponse>>>,
    ttl: Duration,
}

impl RequestCache {
    async fn get_or_fetch<F, Fut>(&self, key: String, fetch: F) -> Response
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Response>,
    {
        // 检查缓存
        if let Some(cached) = self.cache.read().await.get(&key) {
            if cached.is_valid() {
                return cached.response.clone();
            }
        }
        
        // 获取新数据
        let response = fetch().await;
        
        // 更新缓存
        self.cache.write().await.put(
            key,
            CachedResponse::new(response.clone(), self.ttl)
        );
        
        response
    }
}

// 3. 熔断器
struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_threshold: usize,
    success_threshold: usize,
    timeout: Duration,
}

enum CircuitState {
    Closed,
    Open(Instant),
    HalfOpen,
}

impl CircuitBreaker {
    async fn call<F, Fut, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let state = self.state.read().await.clone();
        
        match state {
            CircuitState::Open(since) => {
                if Instant::now().duration_since(since) > self.timeout {
                    // 尝试半开
                    *self.state.write().await = CircuitState::HalfOpen;
                } else {
                    return Err(Error::CircuitOpen);
                }
            },
            _ => {},
        }
        
        match f().await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            },
            Err(e) => {
                self.on_failure().await;
                Err(e)
            },
        }
    }
}

// 4. API版本管理
#[derive(Clone)]
struct ApiVersionManager {
    versions: HashMap<String, Router>,
    default_version: String,
}

impl ApiVersionManager {
    fn route(&self, version: Option<String>) -> Router {
        let v = version.unwrap_or(self.default_version.clone());
        self.versions.get(&v)
            .cloned()
            .unwrap_or_else(|| self.versions.get(&self.default_version).cloned().unwrap())
    }
}
```

## 三、横切关注点优化

### 3.1 统一连接池管理
```rust
// 所有服务共享的连接池管理器
pub struct ConnectionPoolManager {
    redis_pool: deadpool_redis::Pool,
    influx_pool: Option<InfluxPool>,
    http_client: reqwest::Client,
}

impl ConnectionPoolManager {
    pub async fn new(config: &Config) -> Result<Self> {
        let redis_config = deadpool_redis::Config::from_url(&config.redis_url);
        let redis_pool = redis_config.create_pool(Some(Runtime::Tokio1))?;
        
        let influx_pool = if config.influx_enabled {
            Some(InfluxPool::new(&config.influx_config)?)
        } else {
            None
        };
        
        let http_client = reqwest::Client::builder()
            .pool_max_idle_per_host(10)
            .timeout(Duration::from_secs(30))
            .build()?;
        
        Ok(Self {
            redis_pool,
            influx_pool,
            http_client,
        })
    }
}
```

### 3.2 统一监控指标
```rust
use prometheus::{Counter, Gauge, Histogram, Registry};

pub struct ServiceMetrics {
    pub request_count: Counter,
    pub request_duration: Histogram,
    pub active_connections: Gauge,
    pub error_count: Counter,
}

impl ServiceMetrics {
    pub fn new(service_name: &str, registry: &Registry) -> Result<Self> {
        let request_count = Counter::new(
            format!("{}_requests_total", service_name),
            "Total number of requests"
        )?;
        
        let request_duration = Histogram::new(
            format!("{}_request_duration_seconds", service_name),
            "Request duration in seconds"
        )?;
        
        let active_connections = Gauge::new(
            format!("{}_active_connections", service_name),
            "Number of active connections"
        )?;
        
        let error_count = Counter::new(
            format!("{}_errors_total", service_name),
            "Total number of errors"
        )?;
        
        registry.register(Box::new(request_count.clone()))?;
        registry.register(Box::new(request_duration.clone()))?;
        registry.register(Box::new(active_connections.clone()))?;
        registry.register(Box::new(error_count.clone()))?;
        
        Ok(Self {
            request_count,
            request_duration,
            active_connections,
            error_count,
        })
    }
}
```

### 3.3 统一错误处理
```rust
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    
    #[error("InfluxDB error: {0}")]
    InfluxDB(String),
    
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Business logic error: {0}")]
    Business(String),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Circuit breaker open")]
    CircuitBreakerOpen,
}

// 统一错误响应格式
impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ServiceError::RateLimitExceeded => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            ServiceError::Config(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Configuration error".to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        
        let body = Json(json!({
            "error": error_message,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }));
        
        (status, body).into_response()
    }
}
```

## 四、实施计划

### 第一阶段：基础设施（2周）
1. 实现统一连接池管理
2. 添加统一监控指标
3. 完善错误处理框架
4. 升级日志系统

### 第二阶段：核心服务优化（4周）
1. AlarmSrv: 批量检测、告警抑制
2. RuleSrv: 并行执行、规则树
3. HisSrv: 数据压缩、分层存储
4. ModSrv: 按已有方案优化

### 第三阶段：网关和外部通信（2周）
1. API Gateway: 限流、缓存、熔断
2. NetSrv: 协议适配、消息缓冲

### 第四阶段：高级功能（3周）
1. 告警关联分析
2. 复杂事件处理
3. 智能降采样
4. API版本管理

### 第五阶段：测试和调优（2周）
1. 性能测试
2. 压力测试
3. 故障注入测试
4. 性能调优

## 五、性能目标

| 服务 | 指标 | 当前值 | 目标值 | 提升倍数 |
|-----|------|-------|--------|---------|
| AlarmSrv | 告警检测延迟 | 500ms | 50ms | 10x |
| RuleSrv | 规则执行吞吐 | 100/s | 1000/s | 10x |
| HisSrv | 查询响应时间 | 2s | 200ms | 10x |
| NetSrv | 消息吞吐量 | 1000/s | 10000/s | 10x |
| API Gateway | 请求延迟 | 100ms | 20ms | 5x |

## 六、风险管理

### 技术风险
1. **并发复杂性**: 使用成熟的并发框架(Tokio)
2. **数据一致性**: 使用事务和分布式锁
3. **性能退化**: 逐步优化，持续监控

### 业务风险
1. **服务中断**: 灰度发布，回滚机制
2. **数据丢失**: 完善备份策略
3. **兼容性问题**: 版本化API

## 七、总结

通过本优化方案，VoltageEMS 将实现：

1. **性能提升**: 所有服务性能提升5-10倍
2. **功能完善**: 补充关键功能缺失
3. **可靠性增强**: 添加熔断、限流、重试机制
4. **可维护性提高**: 统一框架、监控、错误处理
5. **可扩展性改善**: 模块化设计、插件化架构

预计总工期13周，投资回报率(ROI)预计在6个月内实现。