# modsrv 架构设计

## 概述

modsrv 采用基于 DAG（有向无环图）的计算引擎架构，结合物模型系统，实现灵活的实时数据处理和设备管理。服务通过 Redis Hash 结构存储计算结果，并订阅 comsrv 的数据变化自动触发计算。

## 核心架构

```
┌─────────────────────────────────────────────────────────┐
│                      modsrv                             │
├─────────────────────────────────────────────────────────┤
│                   API Server                            │
│              (Models/Instances/Metrics)                 │
├─────────────────────────────────────────────────────────┤
│                Device Model System                      │
│     ┌──────────────┬──────────────┬──────────────┐    │
│     │DeviceModel   │Instance      │DataFlow      │    │
│     │Definition    │Manager       │Processor     │    │
│     └──────────────┴──────────────┴──────────────┘    │
├─────────────────────────────────────────────────────────┤
│                 DAG Calculation Engine                  │
│     ┌──────────┬──────────┬──────────┬──────────┐    │
│     │Parser    │Validator │Executor  │Functions │    │
│     └──────────┴──────────┴──────────┴──────────┘    │
├─────────────────────────────────────────────────────────┤
│                  Data Pipeline                          │
│     ┌──────────┬──────────┬──────────┬──────────┐    │
│     │Subscribe │Transform │Calculate │Publish   │    │
│     └──────────┴──────────┴──────────┴──────────┘    │
├─────────────────────────────────────────────────────────┤
│                  Redis Interface                        │
│          ┌──────────────┬──────────────┐              │
│          │ Hash Storage │   Pub/Sub    │              │
│          └──────────────┴──────────────┘              │
└─────────────────────────────────────────────────────────┘
```

## 组件说明

### 1. Device Model System

物模型系统提供完整的设备建模和管理能力：

```rust
pub struct DeviceModelSystem {
    models: HashMap<String, DeviceModel>,
    instance_manager: InstanceManager,
    calculation_engine: CalculationEngine,
    data_flow_processor: DataFlowProcessor,
}
```

#### DeviceModel Definition

```rust
pub struct DeviceModel {
    pub id: String,
    pub name: String,
    pub version: String,
    pub properties: HashMap<String, PropertyDef>,
    pub telemetry: HashMap<String, TelemetryDef>,
    pub commands: HashMap<String, CommandDef>,
    pub calculations: Vec<CalculationDef>,
}
```

#### Instance Manager

```rust
pub struct InstanceManager {
    instances: Arc<RwLock<HashMap<String, DeviceInstance>>>,
    redis_client: Arc<RedisClient>,
}

impl InstanceManager {
    pub async fn create_instance(
        &self,
        model_id: &str,
        instance_id: String,
        properties: HashMap<String, Value>,
    ) -> Result<String> {
        // 创建实例并存储到 Redis
    }
}
```

### 2. DAG Calculation Engine

计算引擎支持复杂的数据流计算：

```rust
pub struct CalculationEngine {
    functions: HashMap<String, CalculationFunction>,
    dag_executor: DagExecutor,
}

// 计算函数类型
type CalculationFunction = Box<dyn Fn(&[f64], &HashMap<String, f64>) -> Result<f64>>;
```

#### 内置函数实现

```rust
impl CalculationEngine {
    pub fn new() -> Self {
        let mut engine = Self::default();
        
        // 注册内置函数
        engine.register_function("sum", |inputs, _| {
            Ok(inputs.iter().sum())
        });
        
        engine.register_function("avg", |inputs, _| {
            Ok(inputs.iter().sum::<f64>() / inputs.len() as f64)
        });
        
        engine.register_function("scale", |inputs, params| {
            let factor = params.get("factor").unwrap_or(&1.0);
            Ok(inputs[0] * factor)
        });
        
        engine
    }
}
```

#### DAG 执行器

```rust
pub struct DagExecutor {
    graph: DiGraph<CalculationNode, ()>,
    topological_order: Vec<NodeIndex>,
}

impl DagExecutor {
    pub async fn execute(
        &self,
        inputs: HashMap<String, StandardFloat>,
    ) -> Result<HashMap<String, StandardFloat>> {
        let mut results = HashMap::new();
        
        // 按拓扑顺序执行节点
        for node_idx in &self.topological_order {
            let node = &self.graph[*node_idx];
            let result = self.execute_node(node, &inputs, &results).await?;
            results.insert(node.id.clone(), result);
        }
        
        Ok(results)
    }
}
```

### 3. Data Pipeline

数据处理流水线负责实时数据流的处理：

#### 订阅管理

```rust
pub struct DataSubscriptionManager {
    subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
    redis_client: Arc<RedisClient>,
}

impl DataSubscriptionManager {
    pub async fn subscribe_to_comsrv(
        &self,
        channel_patterns: Vec<String>,
        handler: Box<dyn DataHandler>,
    ) -> Result<String> {
        let sub_id = Uuid::new_v4().to_string();
        
        // 启动订阅任务
        let manager = self.clone();
        tokio::spawn(async move {
            manager.run_subscription(sub_id, channel_patterns, handler).await
        });
        
        Ok(sub_id)
    }
}
```

#### 数据转换

```rust
pub struct DataTransformer {
    pub fn transform_comsrv_data(
        &self,
        channel: &str,
        message: &str,
    ) -> Result<DataPoint> {
        // 解析消息格式: "pointID:value"
        let parts: Vec<&str> = message.split(':').collect();
        if parts.len() != 2 {
            return Err(Error::InvalidFormat);
        }
        
        let point_id = parts[0].parse::<u32>()?;
        let value = parts[1].parse::<f64>()?;
        
        Ok(DataPoint {
            source: channel.to_string(),
            point_id,
            value: StandardFloat::new(value),
            timestamp: chrono::Utc::now().timestamp_millis(),
        })
    }
}
```

### 4. Redis Interface

#### Hash 存储操作

```rust
impl RedisStorage {
    /// 存储计算结果（无时间戳）
    pub async fn store_calculation_result(
        &mut self,
        model_name: &str,
        result_type: &str,
        field: &str,
        value: StandardFloat,
    ) -> Result<()> {
        let hash_key = format!("modsrv:{}:{}", model_name, result_type);
        self.redis_client
            .hset(&hash_key, field, value.to_redis())
            .await?;
        Ok(())
    }
    
    /// 批量存储结果
    pub async fn batch_store_results(
        &mut self,
        model_name: &str,
        result_type: &str,
        results: HashMap<String, StandardFloat>,
    ) -> Result<()> {
        let hash_key = format!("modsrv:{}:{}", model_name, result_type);
        
        let mut pipe = redis::pipe();
        for (field, value) in results {
            pipe.hset(&hash_key, field, value.to_redis());
        }
        
        pipe.query_async(&mut self.conn).await?;
        Ok(())
    }
}
```

## 数据流

### 实时计算流程

1. **订阅数据变化**
   ```
   comsrv:1001:m → "10001:25.123456"
   ```

2. **触发计算**
   - 查找依赖该数据的模型
   - 收集所有输入数据
   - 执行 DAG 计算

3. **存储结果**
   ```
   modsrv:power_meter:measurement → {
       "total_power": "1200.500000"
   }
   ```

4. **发布变化**（可选）
   ```
   通道: modsrv:power_meter:update
   消息: "total_power:1200.500000"
   ```

### 批量处理优化

```rust
pub struct BatchProcessor {
    buffer: Arc<Mutex<Vec<CalculationTask>>>,
    flush_interval: Duration,
    batch_size: usize,
}

impl BatchProcessor {
    pub async fn process(&self) {
        let mut interval = tokio::time::interval(self.flush_interval);
        
        loop {
            interval.tick().await;
            
            let tasks = {
                let mut buffer = self.buffer.lock().await;
                if buffer.is_empty() {
                    continue;
                }
                std::mem::take(&mut *buffer)
            };
            
            // 并行执行无依赖的计算
            let results = self.execute_parallel(tasks).await;
            
            // 批量存储结果
            self.store_results(results).await;
        }
    }
}
```

## 性能优化

### 1. 计算缓存

```rust
pub struct CalculationCache {
    cache: Arc<RwLock<HashMap<String, CachedResult>>>,
    ttl: Duration,
}

struct CachedResult {
    value: StandardFloat,
    inputs_hash: u64,
    timestamp: Instant,
}

impl CalculationCache {
    pub async fn get_or_compute<F>(
        &self,
        key: &str,
        inputs: &HashMap<String, f64>,
        compute_fn: F,
    ) -> Result<StandardFloat>
    where
        F: FnOnce() -> Result<StandardFloat>,
    {
        let inputs_hash = calculate_hash(inputs);
        
        // 检查缓存
        if let Some(cached) = self.get_cached(key, inputs_hash).await {
            return Ok(cached);
        }
        
        // 计算并缓存
        let result = compute_fn()?;
        self.cache_result(key, result, inputs_hash).await;
        
        Ok(result)
    }
}
```

### 2. 并行执行

```rust
pub async fn execute_parallel_calculations(
    tasks: Vec<CalculationTask>,
) -> Vec<Result<CalculationResult>> {
    let mut handles = Vec::new();
    
    for task in tasks {
        let handle = tokio::spawn(async move {
            execute_calculation(task).await
        });
        handles.push(handle);
    }
    
    futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect()
}
```

### 3. 内存优化

- 使用对象池复用计算缓冲区
- 压缩存储中间结果
- 定期清理过期缓存

## 监控指标

```rust
pub struct Metrics {
    calculations_total: IntCounter,
    calculation_duration: Histogram,
    cache_hits: IntCounter,
    cache_misses: IntCounter,
    active_models: IntGauge,
}

impl Metrics {
    pub fn record_calculation(&self, model: &str, duration: Duration) {
        self.calculations_total.inc();
        self.calculation_duration
            .with_label_values(&[model])
            .observe(duration.as_secs_f64());
    }
}
```

## 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum ModelSrvError {
    #[error("Calculation error: {0}")]
    Calculation(String),
    
    #[error("Model not found: {0}")]
    ModelNotFound(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
}
```

## 扩展性设计

### 添加自定义函数

```rust
// 注册自定义函数
engine.register_function("power_factor", |inputs, params| {
    let real_power = inputs[0];
    let apparent_power = inputs[1];
    let min_pf = params.get("min").unwrap_or(&0.0);
    
    let pf = real_power / apparent_power;
    Ok(pf.max(*min_pf))
});
```

### 添加新的数据源

```rust
// 订阅其他服务的数据
subscription_manager.subscribe(
    vec!["alarmsrv:*:status".to_string()],
    Box::new(AlarmDataHandler::new()),
).await?;
```