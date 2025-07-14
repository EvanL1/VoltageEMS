# 设备模型系统

## 概述

modsrv 的设备模型系统提供了完整的物理设备抽象能力，支持属性定义、遥测映射、命令执行和实时计算。通过标准化的模型定义，实现了设备的统一管理和数据处理。

## 模型结构

### 完整模型定义

```rust
pub struct DeviceModel {
    /// 模型唯一标识
    pub id: String,
    
    /// 模型名称
    pub name: String,
    
    /// 版本号
    pub version: String,
    
    /// 模型描述
    pub description: String,
    
    /// 设备类型
    pub device_type: DeviceType,
    
    /// 属性定义（静态配置）
    pub properties: Vec<PropertyDefinition>,
    
    /// 遥测点定义（动态数据）
    pub telemetry: Vec<TelemetryDefinition>,
    
    /// 命令定义（控制操作）
    pub commands: Vec<CommandDefinition>,
    
    /// 事件定义（状态变化）
    pub events: Vec<EventDefinition>,
    
    /// 计算定义（衍生数据）
    pub calculations: Vec<CalculationDefinition>,
    
    /// 扩展元数据
    pub metadata: HashMap<String, String>,
}
```

## 组件详解

### 1. 属性定义（Properties）

设备的静态配置参数，如额定值、型号等。

```rust
pub struct PropertyDefinition {
    /// 属性标识符
    pub identifier: String,
    
    /// 显示名称
    pub name: String,
    
    /// 数据类型
    pub data_type: DataType,
    
    /// 是否必需
    pub required: bool,
    
    /// 默认值
    pub default_value: Option<Value>,
    
    /// 约束条件
    pub constraints: Option<Constraints>,
    
    /// 单位
    pub unit: Option<String>,
    
    /// 描述
    pub description: Option<String>,
}
```

#### 示例：变压器属性
```yaml
properties:
  - identifier: rated_capacity
    name: 额定容量
    data_type: float64
    required: true
    default_value: 1000
    unit: kVA
    constraints:
      min: 100
      max: 10000
    description: 变压器额定容量
    
  - identifier: voltage_ratio
    name: 变比
    data_type: string
    required: true
    default_value: "10/0.4"
    description: 高压侧/低压侧电压比
```

### 2. 遥测定义（Telemetry）

设备的实时测量数据。

```rust
pub struct TelemetryDefinition {
    /// 遥测标识符
    pub identifier: String,
    
    /// 显示名称
    pub name: String,
    
    /// 数据类型
    pub data_type: DataType,
    
    /// 采集方式
    pub collection_type: CollectionType,
    
    /// 数据源映射
    pub mapping: TelemetryMapping,
    
    /// 转换规则
    pub transform: Option<TransformRule>,
    
    /// 单位
    pub unit: Option<String>,
}
```

#### 采集类型
```rust
pub enum CollectionType {
    /// 周期采集
    Periodic { interval_ms: u64 },
    
    /// 变化采集
    OnChange { threshold: Option<f64> },
    
    /// 事件驱动
    EventDriven,
    
    /// 混合模式
    Hybrid {
        interval_ms: u64,
        change_threshold: Option<f64>,
    },
}
```

#### 数据映射
```rust
pub struct TelemetryMapping {
    /// 通道ID
    pub channel_id: u16,
    
    /// 点类型 (m/s/c/a)
    pub point_type: String,
    
    /// 点ID
    pub point_id: u32,
    
    /// 缩放因子
    pub scale: Option<f64>,
    
    /// 偏移量
    pub offset: Option<f64>,
}
```

### 3. 命令定义（Commands）

设备支持的控制操作。

```rust
pub struct CommandDefinition {
    /// 命令标识符
    pub identifier: String,
    
    /// 显示名称
    pub name: String,
    
    /// 命令类型
    pub command_type: CommandType,
    
    /// 输入参数
    pub input_params: Vec<ParamDefinition>,
    
    /// 输出参数
    pub output_params: Vec<ParamDefinition>,
    
    /// 命令映射
    pub mapping: CommandMapping,
}

pub enum CommandType {
    Control,    // 控制命令
    Setting,    // 设置命令
    Query,      // 查询命令
    Action,     // 动作命令
}
```

### 4. 事件定义（Events）

设备状态变化和告警事件。

```rust
pub struct EventDefinition {
    /// 事件标识符
    pub identifier: String,
    
    /// 事件名称
    pub name: String,
    
    /// 事件类型
    pub event_type: EventType,
    
    /// 触发条件
    pub trigger: TriggerCondition,
    
    /// 事件参数
    pub params: Vec<ParamDefinition>,
}

pub enum TriggerCondition {
    /// 阈值触发
    Threshold {
        variable: String,
        operator: String,
        value: f64,
    },
    
    /// 表达式触发
    Expression(String),
    
    /// 状态变化
    StateChange {
        variable: String,
        from: Option<Value>,
        to: Option<Value>,
    },
}
```

### 5. 计算定义（Calculations）

基于原始数据的衍生计算。

```rust
pub struct CalculationDefinition {
    /// 计算标识符
    pub identifier: String,
    
    /// 计算名称
    pub name: String,
    
    /// 输入变量
    pub inputs: Vec<String>,
    
    /// 输出变量
    pub outputs: Vec<String>,
    
    /// 计算表达式
    pub expression: CalculationExpression,
    
    /// 执行条件
    pub condition: Option<String>,
}
```

## 模型实例化

### 实例管理器

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

### 创建实例

```rust
// 创建电表实例
let instance = instance_manager.create_instance(
    "power_meter_v1",           // 模型ID
    "meter_001",               // 实例ID
    "1号楼总电表",              // 实例名称
    Some(hashmap! {            // 初始属性
        "location" => json!("1号楼配电室"),
        "rated_voltage" => json!(380),
    }),
    None,                      // 配置参数
).await?;
```

### 实例数据结构

```rust
pub struct DeviceInstance {
    /// 实例唯一标识
    pub instance_id: String,
    
    /// 所属模型ID
    pub model_id: String,
    
    /// 实例名称
    pub name: String,
    
    /// 属性值
    pub properties: HashMap<String, Value>,
    
    /// 配置参数
    pub config: HashMap<String, Value>,
    
    /// 实例状态
    pub status: DeviceStatus,
    
    /// 创建时间
    pub created_at: i64,
    
    /// 更新时间
    pub updated_at: i64,
}
```

## 实时数据流

### 数据流处理器

```rust
pub struct DataFlowProcessor {
    redis_client: Arc<RedisHandler>,
    instance_manager: Arc<InstanceManager>,
    calculation_engine: Arc<CalculationEngine>,
    subscriptions: Arc<RwLock<HashMap<String, DataSubscription>>>,
    update_channel: mpsc::Sender<DataUpdate>,
}
```

### 数据订阅流程

```rust
// 1. 订阅实例数据
dataflow_processor.subscribe_instance(
    "meter_001".to_string(),
    hashmap! {
        "voltage_a" => "1001:m:10001",
        "current_a" => "1001:m:10002",
        "power_a" => "1001:m:10003",
    },
    Duration::from_secs(1),
).await?;

// 2. 处理数据更新
async fn process_update(&self, update: DataUpdate) -> Result<()> {
    // 更新实例遥测
    self.instance_manager.update_telemetry(
        &update.instance_id,
        &update.telemetry_name,
        update.value
    ).await?;
    
    // 触发相关计算
    self.trigger_calculations(&update).await?;
    
    // 检查事件触发
    self.check_events(&update).await?;
    
    Ok(())
}
```

## 模型示例

### 1. 智能电表模型

```yaml
id: smart_meter_v1
name: 智能电表
version: 1.0.0
device_type: energy

properties:
  - identifier: meter_type
    name: 电表类型
    data_type: string
    default_value: "三相四线"
    
  - identifier: accuracy_class
    name: 精度等级
    data_type: float64
    default_value: 0.5

telemetry:
  - identifier: voltage_a
    name: A相电压
    data_type: float64
    collection_type:
      periodic:
        interval_ms: 1000
    mapping:
      channel_id: 1001
      point_type: m
      point_id: 10001
    unit: V
    
  - identifier: current_a
    name: A相电流
    data_type: float64
    collection_type:
      periodic:
        interval_ms: 1000
    mapping:
      channel_id: 1001
      point_type: m
      point_id: 10002
    unit: A

calculations:
  - identifier: apparent_power_a
    name: A相视在功率
    inputs: [voltage_a, current_a]
    outputs: [apparent_power_a]
    expression:
      built_in:
        function: multiply
        args: []
    
  - identifier: daily_energy
    name: 日电能累计
    inputs: [total_power]
    outputs: [daily_energy]
    expression:
      built_in:
        function: integrate
        args: ["1d"]

events:
  - identifier: over_voltage
    name: 过压告警
    event_type: alarm
    trigger:
      threshold:
        variable: voltage_a
        operator: ">"
        value: 253
        
  - identifier: power_loss
    name: 失电事件
    event_type: fault
    trigger:
      expression: "voltage_a < 50 && voltage_b < 50 && voltage_c < 50"
```

### 2. 变压器监测模型

```yaml
id: transformer_monitor_v1
name: 变压器监测
version: 1.0.0
device_type: energy

properties:
  - identifier: rated_capacity
    name: 额定容量
    data_type: float64
    default_value: 1000
    unit: kVA
    
  - identifier: cooling_type
    name: 冷却方式
    data_type: string
    default_value: "ONAN"

telemetry:
  - identifier: oil_temp
    name: 油温
    data_type: float64
    collection_type:
      periodic:
        interval_ms: 5000
    mapping:
      channel_id: 2001
      point_type: m
      point_id: 20001
    unit: °C
    
  - identifier: winding_temp_h
    name: 高压侧绕组温度
    data_type: float64
    collection_type:
      hybrid:
        interval_ms: 5000
        change_threshold: 2.0
    mapping:
      channel_id: 2001
      point_type: m
      point_id: 20002
    unit: °C

calculations:
  - identifier: load_rate
    name: 负载率
    inputs: [current_power, rated_capacity]
    outputs: [load_rate]
    expression:
      math: "(current_power / rated_capacity) * 100"
      
  - identifier: temp_rise
    name: 温升
    inputs: [oil_temp, ambient_temp]
    outputs: [temp_rise]
    expression:
      math: "oil_temp - ambient_temp"

events:
  - identifier: high_temp_alarm
    name: 高温告警
    event_type: alarm
    trigger:
      threshold:
        variable: oil_temp
        operator: ">"
        value: 85
        
  - identifier: overload_alarm
    name: 过载告警
    event_type: alarm
    trigger:
      threshold:
        variable: load_rate
        operator: ">"
        value: 110
```

## 最佳实践

### 1. 模型设计原则
- **单一职责**：每个模型专注一类设备
- **可扩展性**：预留扩展字段
- **版本管理**：支持模型升级
- **标准化**：遵循行业标准

### 2. 性能优化
- **批量处理**：聚合相关计算
- **缓存策略**：热点数据缓存
- **异步执行**：非阻塞计算
- **懒加载**：按需加载模型

### 3. 数据质量
- **数据验证**：类型和范围检查
- **异常处理**：降级和默认值
- **质量标记**：Good/Bad/Uncertain
- **时间戳**：精确到毫秒

### 4. 运维考虑
- **模型热更新**：无需重启服务
- **实例监控**：状态和性能指标
- **日志追踪**：完整的数据流日志
- **故障隔离**：实例级别的错误处理

## 扩展开发

### 自定义计算函数

```rust
// 1. 定义计算函数
async fn custom_calculation(
    inputs: HashMap<String, Value>,
    params: HashMap<String, Value>,
) -> Result<Value> {
    // 自定义计算逻辑
    let result = complex_calculation(inputs, params)?;
    Ok(json!(result))
}

// 2. 注册到计算引擎
calculation_engine.register_function(
    "custom_calc",
    Arc::new(custom_calculation)
);

// 3. 在模型中使用
calculations:
  - identifier: special_metric
    expression:
      built_in:
        function: custom_calc
        args: ["param1", "param2"]
```

### 协议适配

```rust
// 实现数据源适配器
#[async_trait]
impl DataSourceAdapter for CustomProtocolAdapter {
    async fn read_telemetry(
        &self,
        mapping: &TelemetryMapping,
    ) -> Result<Value> {
        // 协议特定的数据读取
        let raw_data = self.protocol_read(mapping).await?;
        
        // 数据转换
        let value = self.transform_data(raw_data, mapping)?;
        
        Ok(value)
    }
}
```