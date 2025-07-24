# 设备模型系统

## 概述

modsrv 的设备模型系统提供了完整的物模型定义、实例管理和数据处理框架。通过标准化的模型定义，可以快速创建和管理各种工业设备的数字孪生。

## 模型定义

### 基本结构

设备模型使用 YAML 格式定义，包含以下核心要素：

```yaml
id: "power_meter_v1"
name: "智能电表"
version: "1.0.0"
description: "三相智能电表物模型"

# 属性定义（静态配置）
properties:
  rated_voltage:
    type: "float"
    unit: "V"
    default: 380.0
    description: "额定电压"
  
  rated_current:
    type: "float"
    unit: "A"
    default: 100.0
    description: "额定电流"

# 遥测定义（动态数据）
telemetry:
  voltage_a:
    type: "float"
    unit: "V"
    source: "comsrv:1001:m:10001"
    description: "A相电压"
  
  current_a:
    type: "float"
    unit: "A"
    source: "comsrv:1001:m:10002"
    description: "A相电流"
  
  power_factor:
    type: "float"
    unit: ""
    calculated: true
    description: "功率因数"

# 命令定义（控制操作）
commands:
  reset_energy:
    description: "复位电能计数"
    parameters: []
    target: "comsrv:1001:c:30001"
  
  set_limit:
    description: "设置功率限值"
    parameters:
      - name: "limit"
        type: "float"
        unit: "kW"
        min: 0
        max: 1000
    target: "comsrv:1001:a:40001"

# 事件定义（异常告警）
events:
  overload:
    description: "过载事件"
    severity: "warning"
    condition: "current_a > rated_current * 1.2"
  
  power_failure:
    description: "断电事件"
    severity: "critical"
    condition: "voltage_a < 50"

# 计算定义（衍生数据）
calculations:
  - id: "apparent_power"
    description: "视在功率"
    function: "multiply"
    inputs: ["voltage_a", "current_a"]
    output: "apparent_power"
  
  - id: "power_factor_calc"
    description: "功率因数计算"
    function: "custom_power_factor"
    inputs: ["real_power", "apparent_power"]
    output: "power_factor"
```

## 实例管理

### 创建实例

```rust
use voltage_libs::types::StandardFloat;

// 创建设备实例
let instance = DeviceInstance {
    id: "meter_001".to_string(),
    model_id: "power_meter_v1".to_string(),
    name: "1号楼总表".to_string(),
    properties: hashmap! {
        "rated_voltage" => Value::from(StandardFloat::new(380.0)),
        "rated_current" => Value::from(StandardFloat::new(100.0)),
    },
    status: InstanceStatus::Active,
    created_at: Utc::now(),
};

// 保存到 Redis
instance_manager.create_instance(instance).await?;
```

### 实例数据结构

```rust
pub struct DeviceInstance {
    pub id: String,
    pub model_id: String,
    pub name: String,
    pub properties: HashMap<String, Value>,
    pub telemetry_cache: Arc<RwLock<HashMap<String, TelemetryData>>>,
    pub status: InstanceStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct TelemetryData {
    pub value: StandardFloat,
    pub timestamp: i64,
    pub source: String,
}

pub enum InstanceStatus {
    Active,
    Inactive,
    Maintenance,
    Fault,
}
```

## 数据流处理

### 实时数据订阅

```rust
pub struct DataFlowProcessor {
    instance_manager: Arc<InstanceManager>,
    calculation_engine: Arc<CalculationEngine>,
    redis_client: Arc<RedisClient>,
}

impl DataFlowProcessor {
    pub async fn start(&self) -> Result<()> {
        // 获取所有活跃实例的数据源
        let instances = self.instance_manager.get_active_instances().await?;
        let mut patterns = HashSet::new();
        
        for instance in instances {
            let model = self.get_model(&instance.model_id)?;
            for telemetry in model.telemetry.values() {
                if let Some(source) = &telemetry.source {
                    patterns.insert(source.clone());
                }
            }
        }
        
        // 订阅数据源
        for pattern in patterns {
            self.subscribe_pattern(pattern).await?;
        }
        
        Ok(())
    }
    
    async fn handle_data_update(&self, channel: &str, message: &str) -> Result<()> {
        // 解析数据
        let (point_id, value) = parse_message(message)?;
        
        // 查找受影响的实例
        let affected_instances = self.find_affected_instances(channel).await?;
        
        // 更新实例数据并触发计算
        for instance_id in affected_instances {
            self.update_instance_telemetry(&instance_id, channel, value).await?;
            self.trigger_calculations(&instance_id).await?;
        }
        
        Ok(())
    }
}
```

### 计算触发

```rust
impl DataFlowProcessor {
    async fn trigger_calculations(&self, instance_id: &str) -> Result<()> {
        let instance = self.instance_manager.get_instance(instance_id).await?;
        let model = self.get_model(&instance.model_id)?;
        
        // 执行所有计算
        for calc_def in &model.calculations {
            // 收集输入数据
            let inputs = self.collect_inputs(&instance, &calc_def.inputs).await?;
            
            // 检查输入是否完整
            if inputs.len() == calc_def.inputs.len() {
                // 执行计算
                let result = self.calculation_engine
                    .execute_calculation(&calc_def.function, inputs, &calc_def.params)
                    .await?;
                
                // 存储结果
                self.store_calculation_result(
                    &instance,
                    &calc_def.output,
                    result,
                ).await?;
            }
        }
        
        Ok(())
    }
    
    async fn store_calculation_result(
        &self,
        instance: &DeviceInstance,
        field_name: &str,
        value: StandardFloat,
    ) -> Result<()> {
        // 存储到 Redis Hash（无时间戳）
        let hash_key = format!("modsrv:{}:measurement", instance.id);
        self.redis_client
            .hset(&hash_key, field_name, value.to_redis())
            .await?;
        
        // 更新实例缓存
        instance.telemetry_cache.write().await.insert(
            field_name.to_string(),
            TelemetryData {
                value,
                timestamp: Utc::now().timestamp_millis(),
                source: "calculated".to_string(),
            },
        );
        
        Ok(())
    }
}
```

## 命令执行

### 命令处理流程

```rust
pub async fn execute_command(
    &self,
    instance_id: &str,
    command_id: &str,
    parameters: HashMap<String, Value>,
) -> Result<CommandResult> {
    // 获取实例和模型
    let instance = self.instance_manager.get_instance(instance_id).await?;
    let model = self.get_model(&instance.model_id)?;
    
    // 查找命令定义
    let command_def = model.commands.get(command_id)
        .ok_or_else(|| Error::CommandNotFound(command_id.to_string()))?;
    
    // 验证参数
    self.validate_command_parameters(&command_def, &parameters)?;
    
    // 构建控制消息
    let control_message = self.build_control_message(
        &command_def,
        &parameters,
    )?;
    
    // 发布到 Redis
    if let Some(target) = &command_def.target {
        let parts: Vec<&str> = target.split(':').collect();
        if parts.len() >= 3 {
            let channel = format!("cmd:{}:{}", parts[1], parts[2]);
            self.redis_client
                .publish(&channel, serde_json::to_string(&control_message)?)
                .await?;
        }
    }
    
    Ok(CommandResult {
        success: true,
        message: format!("Command {} executed", command_id),
        timestamp: Utc::now(),
    })
}
```

## 事件处理

### 事件检测

```rust
pub struct EventDetector {
    event_rules: HashMap<String, CompiledRule>,
}

impl EventDetector {
    pub async fn check_events(
        &self,
        instance: &DeviceInstance,
        model: &DeviceModel,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        
        for (event_id, event_def) in &model.events {
            // 评估条件
            if let Ok(triggered) = self.evaluate_condition(
                &event_def.condition,
                instance,
            ).await {
                if triggered {
                    events.push(Event {
                        id: Uuid::new_v4().to_string(),
                        instance_id: instance.id.clone(),
                        event_type: event_id.clone(),
                        severity: event_def.severity.clone(),
                        description: event_def.description.clone(),
                        timestamp: Utc::now(),
                        data: self.collect_event_data(instance).await,
                    });
                }
            }
        }
        
        events
    }
}
```

## 数据查询

### 查询接口

```rust
impl DeviceModelSystem {
    /// 获取实例当前状态
    pub async fn get_instance_state(
        &self,
        instance_id: &str,
    ) -> Result<InstanceState> {
        let instance = self.instance_manager.get_instance(instance_id).await?;
        
        // 从 Redis 读取最新数据
        let measurements = self.read_measurements(&instance.id).await?;
        let controls = self.read_controls(&instance.id).await?;
        
        Ok(InstanceState {
            instance_id: instance.id.clone(),
            model_id: instance.model_id.clone(),
            properties: instance.properties.clone(),
            telemetry: measurements,
            controls,
            status: instance.status.clone(),
            last_update: instance.updated_at,
        })
    }
    
    /// 批量查询遥测数据
    pub async fn get_telemetry_batch(
        &self,
        instance_id: &str,
        telemetry_names: Vec<&str>,
    ) -> Result<HashMap<String, StandardFloat>> {
        let hash_key = format!("modsrv:{}:measurement", instance_id);
        
        // 批量获取
        let values: Vec<Option<String>> = self.redis_client
            .hmget(&hash_key, &telemetry_names)
            .await?;
        
        // 构建结果
        let mut result = HashMap::new();
        for (name, value) in telemetry_names.iter().zip(values.iter()) {
            if let Some(val) = value {
                if let Ok(parsed) = val.parse::<f64>() {
                    result.insert(
                        name.to_string(),
                        StandardFloat::new(parsed),
                    );
                }
            }
        }
        
        Ok(result)
    }
}
```

## 模型版本管理

### 版本升级

```rust
pub async fn upgrade_instance_model(
    &self,
    instance_id: &str,
    new_model_id: &str,
) -> Result<()> {
    let instance = self.instance_manager.get_instance(instance_id).await?;
    let old_model = self.get_model(&instance.model_id)?;
    let new_model = self.get_model(new_model_id)?;
    
    // 验证兼容性
    self.validate_model_compatibility(&old_model, &new_model)?;
    
    // 迁移数据
    let migrated_properties = self.migrate_properties(
        &instance.properties,
        &old_model,
        &new_model,
    )?;
    
    // 更新实例
    self.instance_manager.update_instance_model(
        instance_id,
        new_model_id,
        migrated_properties,
    ).await?;
    
    Ok(())
}
```

## 最佳实践

### 1. 模型设计原则

- 保持模型简洁，避免过度复杂
- 使用有意义的命名
- 提供完整的单位和描述信息
- 合理设置默认值和范围限制

### 2. 性能优化

- 使用计算缓存避免重复计算
- 批量处理数据更新
- 合理设置订阅粒度

### 3. 错误处理

- 实现优雅降级
- 记录详细的错误日志
- 提供有意义的错误消息

## 示例模型

### 温度传感器

```yaml
id: "temperature_sensor_v1"
name: "温度传感器"
properties:
  location:
    type: "string"
    description: "安装位置"
    
telemetry:
  temperature:
    type: "float"
    unit: "°C"
    source: "comsrv:2001:m:20001"
    
events:
  high_temperature:
    condition: "temperature > 40"
    severity: "warning"
```

### 开关控制器

```yaml
id: "switch_controller_v1"
name: "开关控制器"

telemetry:
  status:
    type: "boolean"
    source: "comsrv:3001:s:30001"
    
commands:
  turn_on:
    target: "comsrv:3001:c:30001"
    parameters: []
    
  turn_off:
    target: "comsrv:3001:c:30002"
    parameters: []
```