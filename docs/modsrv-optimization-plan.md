# ModSrv (模型服务) 功能说明与优化思路

## 一、当前功能概述

### 1.1 核心定位
ModSrv 是 VoltageEMS 的数据建模服务，负责将 ComSrv 采集的原始数据映射到逻辑模型，提供面向对象的数据访问接口。

### 1.2 主要功能

#### 数据模型管理
- **模板管理**: 创建、更新、删除可重用的设备模型模板
- **模型实例管理**: 基于模板创建具体设备模型实例
- **数据映射**: 将底层通道数据点映射到高层业务模型属性

#### 数据操作
- **测量数据同步** (`sync_measurement`): 从 ComSrv 通道同步测量数据到模型
- **动作执行** (`execute_action`): 将模型层动作转换为底层控制命令
- **数据查询**: 提供模型级别的数据访问接口

#### 架构特点
- **Measurement/Action 分离**: 测量数据和控制动作分开处理
- **模板化设计**: 支持批量创建相同类型设备
- **Redis Lua 函数**: 业务逻辑在 Redis 中执行，降低延迟

### 1.3 数据流架构

```
设备 → ComSrv (原始数据) → Redis → ModSrv (模型映射) → 应用层
                              ↑
                        Redis Lua Functions
                        (业务逻辑处理)
```

## 二、当前实现分析

### 2.1 优势
1. **轻量级设计**: 单文件实现，代码清晰简洁
2. **Redis 原生处理**: 利用 Lua 函数在数据层直接处理，减少网络开销
3. **灵活映射**: 支持通道到模型的灵活数据映射
4. **模板复用**: 通过模板机制提高配置效率

### 2.2 存在的问题

#### 性能瓶颈
1. **同步串行化**: 每次 API 调用都创建新的 Redis 连接
2. **缺少批量操作**: 数据同步逐个处理，效率低
3. **无连接池**: 未使用连接池，连接开销大

#### 功能缺失
1. **缺少计算映射**: 不支持基于表达式的计算属性
2. **无聚合功能**: 不支持多点聚合（如平均值、求和等）
3. **缺少数据验证**: 模型数据缺少类型和范围验证
4. **无历史查询**: 只能获取当前值，无历史数据访问

#### 可靠性问题
1. **错误处理简单**: 错误直接返回，缺少重试机制
2. **无健康检查**: 缺少对依赖服务的健康监控
3. **日志不完整**: 缺少详细的操作日志和审计记录

## 三、优化方案

### 3.1 性能优化

#### 1. 实现连接池管理
```rust
// 使用连接池管理器
struct ConnectionPool {
    pool: deadpool_redis::Pool,
    metrics: Arc<Mutex<PoolMetrics>>,
}

impl ConnectionPool {
    async fn get(&self) -> Result<PooledConnection> {
        // 自动管理连接生命周期
        self.pool.get().await
    }
}
```

#### 2. 批量数据处理
```rust
// 批量同步接口
async fn sync_batch_measurements(
    model_ids: Vec<String>,
) -> Vec<Result<SyncResult>> {
    // 使用 pipeline 批量执行
    let pipe = redis::pipe()
        .atomic()
        .fcall("modsrv_sync_measurement", &keys, &args)
        .query_async(&mut conn).await?;
}
```

#### 3. 异步并发处理
```rust
// 并发处理多个模型
async fn sync_all_models_concurrent() {
    let futures = models.iter().map(|model| {
        tokio::spawn(async move {
            sync_model(model).await
        })
    });
    
    let results = futures::future::join_all(futures).await;
}
```

### 3.2 功能增强

#### 1. 计算属性支持
```yaml
# 配置示例
models:
  - id: "transformer_01"
    computed_properties:
      total_power:
        expression: "power_a + power_b + power_c"
      efficiency:
        expression: "output_power / input_power * 100"
```

实现计算引擎：
```rust
use evalexpr::{eval_with_context, Context};

async fn calculate_property(
    expression: &str,
    context: HashMap<String, f64>,
) -> Result<f64> {
    let mut ctx = Context::new();
    for (key, value) in context {
        ctx.set_value(key, value)?;
    }
    eval_with_context(expression, &ctx)
}
```

#### 2. 数据聚合功能
```rust
enum AggregateFunction {
    Sum,
    Average,
    Min,
    Max,
    StdDev,
}

async fn aggregate_data(
    points: Vec<f64>,
    func: AggregateFunction,
) -> f64 {
    match func {
        AggregateFunction::Average => {
            points.iter().sum::<f64>() / points.len() as f64
        },
        // ... 其他聚合函数
    }
}
```

#### 3. 数据验证框架
```rust
#[derive(Debug, Deserialize)]
struct PropertyValidation {
    data_type: DataType,
    min: Option<f64>,
    max: Option<f64>,
    pattern: Option<String>,
}

fn validate_value(
    value: &Value,
    validation: &PropertyValidation,
) -> Result<(), ValidationError> {
    // 类型检查
    // 范围检查
    // 格式验证
}
```

#### 4. 历史数据查询
```rust
async fn get_model_history(
    model_id: &str,
    property: &str,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
) -> Vec<DataPoint> {
    // 从时序数据库查询
    // 或从 Redis 时间序列查询
}
```

### 3.3 架构优化

#### 1. 分层架构重构
```
API Layer (HTTP/gRPC)
    ↓
Service Layer (业务逻辑)
    ↓
Repository Layer (数据访问)
    ↓
Redis/InfluxDB
```

#### 2. 事件驱动架构
```rust
// 发布模型变更事件
async fn publish_model_event(event: ModelEvent) {
    redis::cmd("PUBLISH")
        .arg("model:events")
        .arg(serde_json::to_string(&event)?)
        .query_async(&mut conn).await?;
}

// 订阅处理
async fn subscribe_model_events() {
    let mut pubsub = conn.as_pubsub();
    pubsub.subscribe("model:events").await?;
    
    while let Some(msg) = pubsub.on_message().next().await {
        handle_model_event(msg.get_payload()?).await;
    }
}
```

#### 3. 缓存策略优化
```rust
struct ModelCache {
    data: Arc<RwLock<HashMap<String, CachedModel>>>,
    ttl: Duration,
}

impl ModelCache {
    async fn get_or_load(
        &self,
        model_id: &str,
    ) -> Result<Model> {
        // 先从缓存读取
        if let Some(cached) = self.data.read().await.get(model_id) {
            if cached.is_valid() {
                return Ok(cached.model.clone());
            }
        }
        
        // 缓存未命中，从数据库加载
        let model = self.load_from_db(model_id).await?;
        self.data.write().await.insert(
            model_id.to_string(),
            CachedModel::new(model.clone(), self.ttl),
        );
        Ok(model)
    }
}
```

### 3.4 可靠性增强

#### 1. 健康检查机制
```rust
async fn health_check() -> HealthStatus {
    let mut status = HealthStatus::default();
    
    // Redis 健康检查
    status.redis = check_redis_health().await;
    
    // 依赖服务检查
    status.comsrv = check_comsrv_health().await;
    
    // 内存和性能指标
    status.memory = get_memory_usage();
    status.cpu = get_cpu_usage();
    
    status
}
```

#### 2. 重试机制
```rust
use backoff::{ExponentialBackoff, future::retry};

async fn sync_with_retry(model_id: &str) -> Result<()> {
    let op = || async {
        sync_measurement(model_id).await
            .map_err(|e| {
                if e.is_transient() {
                    backoff::Error::transient(e)
                } else {
                    backoff::Error::permanent(e)
                }
            })
    };
    
    retry(ExponentialBackoff::default(), op).await
}
```

#### 3. 审计日志
```rust
#[derive(Debug, Serialize)]
struct AuditLog {
    timestamp: DateTime<Utc>,
    user: String,
    action: String,
    model_id: String,
    details: Value,
    result: String,
}

async fn log_audit(log: AuditLog) {
    // 写入审计日志
    redis::cmd("XADD")
        .arg("audit:stream")
        .arg("*")
        .arg(serde_json::to_value(log)?)
        .query_async(&mut conn).await?;
}
```

## 四、实施计划

### 第一阶段：性能优化（1-2周）
1. 实现 Redis 连接池
2. 添加批量操作接口
3. 实现并发处理

### 第二阶段：功能增强（2-3周）
1. 实现计算属性
2. 添加数据聚合
3. 集成数据验证

### 第三阶段：架构优化（3-4周）
1. 重构为分层架构
2. 实现事件驱动
3. 优化缓存策略

### 第四阶段：可靠性提升（1-2周）
1. 添加健康检查
2. 实现重试机制
3. 完善审计日志

## 五、性能指标目标

| 指标 | 当前值 | 目标值 | 提升 |
|-----|-------|-------|-----|
| 单模型同步延迟 | ~50ms | <10ms | 5x |
| 批量同步吞吐量 | 100/s | 1000/s | 10x |
| 并发连接数 | 10 | 100 | 10x |
| 内存使用 | 100MB | 50MB | 2x |
| CPU 使用率 | 20% | 10% | 2x |

## 六、风险与对策

### 风险点
1. **Redis Lua 函数限制**: 复杂计算可能超出 Lua 脚本执行时间限制
2. **向后兼容性**: 架构调整可能影响现有 API
3. **数据一致性**: 缓存和批量操作可能导致数据不一致

### 对策
1. **分离计算逻辑**: 将复杂计算移到服务层
2. **版本化 API**: 保留旧版 API，新功能使用 v2 接口
3. **事务保证**: 使用 Redis 事务和乐观锁确保一致性

## 七、总结

ModSrv 的优化重点在于：
1. **性能提升**: 通过连接池、批量操作、并发处理大幅提升性能
2. **功能完善**: 增加计算属性、数据聚合、验证等核心功能
3. **架构升级**: 采用分层架构和事件驱动提高可扩展性
4. **可靠性增强**: 通过健康检查、重试、审计提高系统稳定性

通过这些优化，ModSrv 将成为一个高性能、功能完善、可靠稳定的工业物联网数据建模服务。