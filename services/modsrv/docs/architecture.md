# modsrv 架构设计

## 概述

modsrv（Model Service）是 VoltageEMS 的计算引擎服务，负责实时数据处理、物模型映射和规则执行。服务采用高性能的异步架构，支持复杂的计算图和设备建模。

## 架构特点

1. **物模型映射**：完整的设备抽象和实例管理
2. **DAG 计算引擎**：支持复杂依赖的计算图
3. **高性能缓存**：多级缓存减少 Redis 访问
4. **规则引擎**：灵活的条件判断和动作执行
5. **实时数据流**：订阅-处理-发布的流式架构

## 系统架构图

```
┌────────────────────────────────────────────────────────────────┐
│                            modsrv                               │
├────────────────────────────────────────────────────────────────┤
│                      Device Model System                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │Model Registry│  │Instance Mgr  │  │Data Flow Proc│        │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘        │
│         └──────────────────┴──────────────────┘                │
│                            │                                    │
│                    Calculation Engine                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │ Built-in Func│  │ Custom Calc  │  │ Rule Engine  │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
│                            │                                    │
│                      Storage Layer                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │Cache Manager │  │comsrv Reader │  │Control Sender│        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
└────────────────────────────────────────────────────────────────┘
                             │
                        Redis Pub/Sub
                             │
                    ┌────────┴────────┐
                    │   Data Flow     │
                    │ ┌─────┐ ┌─────┐│
                    │ │Input│→│Output││
                    │ └─────┘ └─────┘│
                    └─────────────────┘
```

## 核心组件

### 1. Device Model System（设备模型系统）

#### DeviceModel 结构
```rust
pub struct DeviceModel {
    /// 模型标识
    pub id: String,
    /// 模型名称
    pub name: String,
    /// 设备类型
    pub device_type: DeviceType,
    /// 属性定义
    pub properties: Vec<PropertyDefinition>,
    /// 遥测点定义
    pub telemetry: Vec<TelemetryDefinition>,
    /// 命令定义
    pub commands: Vec<CommandDefinition>,
    /// 事件定义
    pub events: Vec<EventDefinition>,
    /// 计算模型
    pub calculations: Vec<CalculationDefinition>,
}
```

#### 设备类型
```rust
pub enum DeviceType {
    Sensor,      // 传感器
    Actuator,    // 执行器
    Gateway,     // 网关
    Edge,        // 边缘设备
    Energy,      // 能源设备
    Industrial,  // 工业设备
    Custom(String),
}
```

#### 实例管理
```rust
pub struct InstanceManager {
    /// 模型注册表
    model_registry: Arc<ModelRegistry>,
    /// 实例存储
    instances: Arc<RwLock<HashMap<String, DeviceInstance>>>,
    /// 数据缓存
    data_cache: Arc<RwLock<HashMap<String, DeviceData>>>,
}
```

### 2. Calculation Engine（计算引擎）

#### 计算定义
```rust
pub struct CalculationDefinition {
    /// 计算标识
    pub identifier: String,
    /// 输入变量
    pub inputs: Vec<String>,
    /// 输出变量
    pub outputs: Vec<String>,
    /// 计算表达式
    pub expression: CalculationExpression,
    /// 执行条件
    pub condition: Option<String>,
}

pub enum CalculationExpression {
    Math(String),              // 数学表达式
    JavaScript(String),        // JS 代码
    Python(String),           // Python 代码
    BuiltIn {                 // 内置函数
        function: String,
        args: Vec<String>,
    },
}
```

#### 内置函数
```rust
impl CalculationEngine {
    pub fn new() -> Self {
        let mut engine = Self::default();
        
        // 注册内置函数
        engine.register_function("sum", sum_executor);
        engine.register_function("avg", avg_executor);
        engine.register_function("min", min_executor);
        engine.register_function("max", max_executor);
        engine.register_function("scale", scale_executor);
        
        engine
    }
}
```

### 3. Data Flow Processor（数据流处理器）

#### 实时数据订阅
```rust
pub struct DataFlowProcessor {
    redis_client: Arc<RedisHandler>,
    instance_manager: Arc<InstanceManager>,
    calculation_engine: Arc<CalculationEngine>,
    subscriptions: Arc<RwLock<HashMap<String, DataSubscription>>>,
}

impl DataFlowProcessor {
    /// 订阅实例数据更新
    pub async fn subscribe_instance(
        &self,
        instance_id: String,
        point_mappings: HashMap<String, String>,
        update_interval: Duration,
    ) -> Result<()>;
    
    /// 处理数据更新
    pub async fn process_update(&self, update: DataUpdate) -> Result<()>;
}
```

#### 数据流处理
```rust
// 1. 接收数据更新
let update = DataUpdate {
    instance_id: "power_meter_01",
    telemetry_name: "voltage_a",
    value: json!(220.5),
    timestamp: Utc::now().timestamp_millis(),
};

// 2. 更新实例遥测
instance_manager.update_telemetry(
    &update.instance_id,
    &update.telemetry_name,
    update.value
).await?;

// 3. 触发相关计算
for calc in affected_calculations {
    calculation_engine.execute(
        &instance_id,
        &model,
        calc.identifier
    ).await?;
}

// 4. 发布计算结果
redis.publish("calc:update", &results).await?;
```

### 4. Cache Management（缓存管理）

#### 多级缓存架构
```rust
pub struct ModelCacheManager {
    /// L1: 点位数据缓存
    point_cache: Arc<RwLock<HashMap<String, CacheEntry<PointData>>>>,
    /// L2: 模型输出缓存
    model_output_cache: Arc<RwLock<HashMap<String, CacheEntry<Value>>>>,
    /// 默认 TTL
    default_ttl: Duration,
    /// 缓存统计
    stats: Arc<RwLock<CacheStats>>,
}

struct CacheEntry<T> {
    value: T,
    expires_at: Instant,
    access_count: AtomicU64,
    last_access: AtomicU64,
}
```

#### 缓存策略
```rust
impl ModelCacheManager {
    /// 获取或计算
    pub async fn get_or_compute<F, Fut>(
        &self,
        key: &str,
        compute_fn: F,
    ) -> Result<Value>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Value>>,
    {
        // 1. 尝试从缓存获取
        if let Some(cached) = self.get_from_cache(key).await {
            self.stats.write().await.hits += 1;
            return Ok(cached);
        }
        
        // 2. 缓存未命中，执行计算
        self.stats.write().await.misses += 1;
        let value = compute_fn().await?;
        
        // 3. 存入缓存
        self.put_to_cache(key, value.clone()).await;
        
        Ok(value)
    }
}
```

### 5. Storage Interface（存储接口）

#### comsrv 数据读取
```rust
pub struct DataReader {
    redis_client: Arc<RedisClient>,
    cache: Arc<ModelCacheManager>,
}

impl DataReader {
    /// 批量读取点位数据
    pub async fn batch_read_points(
        &self,
        requests: Vec<PointReadRequest>,
    ) -> Result<Vec<PointData>> {
        // 1. 从缓存读取
        let mut results = Vec::new();
        let mut cache_misses = Vec::new();
        
        for req in requests {
            if let Some(cached) = self.cache.get_point(&req.key()).await {
                results.push(cached);
            } else {
                cache_misses.push(req);
            }
        }
        
        // 2. 批量从 Redis 读取缓存未命中的数据
        if !cache_misses.is_empty() {
            let keys: Vec<String> = cache_misses.iter()
                .map(|r| format!("{}:{}:{}", r.channel_id, r.point_type, r.point_id))
                .collect();
            
            let values = self.redis_client.mget(&keys).await?;
            
            // 3. 更新缓存
            for (req, value) in cache_misses.iter().zip(values.iter()) {
                if let Some(data) = value {
                    let point_data = parse_point_data(data)?;
                    self.cache.put_point(&req.key(), point_data.clone()).await;
                    results.push(point_data);
                }
            }
        }
        
        Ok(results)
    }
}
```

#### 控制命令发送
```rust
pub struct ControlSender {
    redis_client: Arc<RedisClient>,
    pending_commands: Arc<RwLock<HashMap<String, PendingCommand>>>,
}

impl ControlSender {
    /// 发送控制命令
    pub async fn send_control(
        &self,
        channel_id: u16,
        point_id: u32,
        value: f64,
    ) -> Result<String> {
        let command_id = Uuid::new_v4().to_string();
        
        let command = ControlCommand {
            id: command_id.clone(),
            channel_id,
            point_id,
            value,
            timestamp: Utc::now().timestamp_millis(),
        };
        
        // 1. 记录待确认命令
        self.pending_commands.write().await.insert(
            command_id.clone(),
            PendingCommand {
                command: command.clone(),
                sent_at: Instant::now(),
                retry_count: 0,
            }
        );
        
        // 2. 发布到控制通道
        let channel = format!("cmd:{}:control", channel_id);
        self.redis_client.publish(&channel, &command).await?;
        
        // 3. 启动超时检查
        self.start_timeout_check(command_id.clone());
        
        Ok(command_id)
    }
}
```

## 物模型示例

### 电力仪表模型
```rust
let power_meter = DeviceModel {
    id: "power_meter_v1".to_string(),
    name: "三相电力仪表".to_string(),
    device_type: DeviceType::Energy,
    
    properties: vec![
        PropertyDefinition {
            identifier: "rated_voltage",
            name: "额定电压",
            data_type: DataType::Float64,
            default_value: Some(json!(380)),
            unit: Some("V".to_string()),
        },
    ],
    
    telemetry: vec![
        TelemetryDefinition {
            identifier: "voltage_a",
            name: "A相电压",
            data_type: DataType::Float64,
            mapping: TelemetryMapping {
                channel_id: 1,
                point_type: "m",
                point_id: 10001,
            },
            unit: Some("V".to_string()),
        },
        // ... 其他相电压、电流等
    ],
    
    calculations: vec![
        CalculationDefinition {
            identifier: "total_power",
            name: "总功率",
            inputs: vec!["power_a", "power_b", "power_c"],
            outputs: vec!["total_power"],
            expression: CalculationExpression::BuiltIn {
                function: "sum".to_string(),
                args: vec![],
            },
        },
    ],
};
```

## 性能优化

### 1. 并发执行
```rust
pub struct OptimizedModelEngine {
    /// 最大并发数
    max_concurrency: usize,
    /// 执行信号量
    semaphore: Arc<Semaphore>,
}

impl OptimizedModelEngine {
    pub async fn execute_batch(
        &self,
        models: Vec<ModelDefinition>,
    ) -> Vec<Result<ModelOutput>> {
        let tasks: Vec<_> = models.into_iter()
            .map(|model| {
                let sem = self.semaphore.clone();
                tokio::spawn(async move {
                    let _permit = sem.acquire().await?;
                    self.execute_single(model).await
                })
            })
            .collect();
        
        futures::future::join_all(tasks).await
    }
}
```

### 2. 缓存优化
- **预热策略**：启动时加载热点数据
- **淘汰算法**：LRU + TTL 组合
- **分片缓存**：减少锁竞争

### 3. 批处理优化
- **聚合小请求**：减少 Redis 往返
- **Pipeline 执行**：批量读写操作
- **异步并行**：充分利用 I/O 等待

## 监控指标

### 性能指标
```rust
// 计算延迟
metrics::histogram!("modsrv.calculation.duration", duration);

// 缓存命中率
metrics::gauge!("modsrv.cache.hit_rate", hit_rate);

// 并发度
metrics::gauge!("modsrv.concurrency.active", active_tasks);
```

### 业务指标
- 活跃模型实例数
- 计算执行频率
- 规则触发次数
- 数据更新延迟

## 配置管理

### 服务配置
```yaml
# modsrv 配置
redis:
  url: "redis://localhost:6379"
  key_prefix: "modsrv"

engine:
  max_concurrency: 100
  batch_size: 1000
  calculation_timeout: 5s

cache:
  max_size: 10000
  default_ttl: 300s
  cleanup_interval: 60s

models:
  config_path: "./config/models"
  auto_reload: true
```

### 模型配置
```yaml
# 设备模型定义
models:
  - id: "power_meter_v1"
    name: "三相电力仪表"
    file: "models/power_meter.yaml"
    
  - id: "transformer_v1"
    name: "变压器监测"
    file: "models/transformer.yaml"
```

## 扩展指南

### 1. 添加新的计算函数
```rust
// 1. 实现计算执行器
fn my_function_executor(
    inputs: HashMap<String, Value>,
    params: HashMap<String, Value>,
) -> Result<Value> {
    // 计算逻辑
    Ok(result)
}

// 2. 注册到计算引擎
engine.register_function("my_function", my_function_executor);
```

### 2. 自定义设备模型
1. 定义模型 YAML 文件
2. 实现特定的计算逻辑
3. 配置数据映射
4. 注册到模型库

### 3. 集成外部系统
- 实现数据源接口
- 添加协议转换
- 配置数据同步策略