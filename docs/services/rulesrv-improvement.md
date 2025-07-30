# RuleSrv 改进方案

## 当前状态
- DAG 规则引擎，支持条件判断和动作执行
- 基于 JSON 的规则定义
- 支持简单规则和复杂 DAG 规则
- Redis 存储规则配置

## 改进目标
业务规则引擎，负责复杂的逻辑判断和控制决策。可以基于设备数据或告警事件触发。

## 核心设计

### 1. 规则触发机制

```rust
/// 规则触发类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleTrigger {
    /// 基于设备数据触发
    DeviceData {
        device_pattern: String,  // 如 "power_meter_*"
        check_interval: u64,     // 检查间隔（秒）
    },
    
    /// 基于告警事件触发
    AlarmEvent {
        alarm_ids: Vec<String>,  // 监听的告警ID列表
    },
    
    /// 基于时间触发
    Schedule {
        cron: String,            // cron 表达式
    },
    
    /// 组合触发
    Combined {
        triggers: Vec<RuleTrigger>,
        logic: LogicOp,          // AND/OR
    },
}

/// 规则定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub trigger: RuleTrigger,
    pub condition: Option<String>,  // 额外的条件判断
    pub actions: Vec<Action>,
    pub enabled: bool,
}
```

### 2. 基于告警的规则处理

```rust
impl RuleService {
    /// 订阅告警事件
    pub async fn subscribe_alarm_events(&mut self) -> Result<()> {
        let mut sub = self.redis.lock().await
            .subscribe("alarm:event")
            .await?;
            
        while let Ok(msg) = sub.on_message().await {
            let event: AlarmEvent = serde_json::from_str(&msg.payload)?;
            
            // 查找匹配的规则
            let triggered_rules = self.find_alarm_triggered_rules(&event);
            
            // 执行规则
            for rule in triggered_rules {
                self.execute_rule(rule, RuleContext::from_alarm(event.clone())).await?;
            }
        }
        
        Ok(())
    }
    
    /// 查找告警触发的规则
    fn find_alarm_triggered_rules(&self, event: &AlarmEvent) -> Vec<&Rule> {
        self.rules
            .values()
            .filter(|rule| {
                if !rule.enabled {
                    return false;
                }
                
                match &rule.trigger {
                    RuleTrigger::AlarmEvent { alarm_ids } => {
                        alarm_ids.contains(&event.alarm_id)
                    }
                    RuleTrigger::Combined { triggers, logic } => {
                        // 检查组合触发条件
                        self.check_combined_trigger(triggers, logic, Some(event))
                    }
                    _ => false,
                }
            })
            .collect()
    }
}
```

### 3. 基于设备数据的规则处理

```rust
impl RuleService {
    /// 定期检查设备数据规则
    pub async fn check_device_rules(&mut self) -> Result<()> {
        let device_rules: Vec<_> = self.rules
            .values()
            .filter(|r| matches!(r.trigger, RuleTrigger::DeviceData { .. }))
            .collect();
            
        for rule in device_rules {
            if let RuleTrigger::DeviceData { device_pattern, .. } = &rule.trigger {
                // 获取匹配的设备
                let devices = self.get_matching_devices(device_pattern).await?;
                
                for device_id in devices {
                    // 从 ModSrv 获取设备数据
                    let data = self.get_device_data(&device_id).await?;
                    
                    // 检查条件
                    if self.evaluate_condition(&rule.condition, &data) {
                        self.execute_rule(
                            rule.clone(), 
                            RuleContext::from_device(&device_id, data)
                        ).await?;
                    }
                }
            }
        }
        
        Ok(())
    }
}
```

### 4. 规则动作执行

```rust
/// 规则动作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    /// 设备控制
    DeviceControl {
        device_pattern: String,
        field: String,
        value: f64,
    },
    
    /// 创建派生告警
    CreateAlarm {
        alarm_id: String,
        message: String,
    },
    
    /// 执行脚本
    RunScript {
        script_id: String,
        params: HashMap<String, Value>,
    },
    
    /// 数据聚合
    Aggregate {
        operation: AggregateOp,
        target: String,
    },
    
    /// 通知
    Notify {
        channel: String,
        message: String,
    },
}

impl RuleService {
    /// 执行规则动作
    async fn execute_actions(&self, actions: &[Action], context: &RuleContext) -> Result<()> {
        for action in actions {
            match action {
                Action::DeviceControl { device_pattern, field, value } => {
                    // 通过 ModSrv 更新设备 desired 状态
                    let devices = self.resolve_device_pattern(device_pattern, context)?;
                    for device_id in devices {
                        self.update_device_desired(&device_id, field, *value).await?;
                    }
                }
                
                Action::CreateAlarm { alarm_id, message } => {
                    // 创建派生告警
                    self.create_derived_alarm(alarm_id, message, context).await?;
                }
                
                Action::RunScript { script_id, params } => {
                    // 执行 Lua 脚本
                    self.run_lua_script(script_id, params, context).await?;
                }
                
                _ => {
                    // 其他动作类型
                }
            }
        }
        
        Ok(())
    }
}
```

### 5. 复杂事件处理（CEP）

```rust
/// 事件模式
#[derive(Debug, Clone)]
pub struct EventPattern {
    pub name: String,
    pub pattern: String,  // 如 "A->B->C within 5m"
    pub window: Duration,
}

/// CEP 引擎
pub struct CepEngine {
    patterns: Vec<CompiledPattern>,
    event_buffer: TimeWindowBuffer,
}

impl CepEngine {
    /// 处理新事件
    pub fn process_event(&mut self, event: Event) -> Vec<ComplexEvent> {
        // 添加到缓冲区
        self.event_buffer.add(event.clone());
        
        // 检查所有模式
        let mut matches = Vec::new();
        for pattern in &self.patterns {
            if let Some(complex_event) = pattern.match_in_buffer(&self.event_buffer) {
                matches.push(complex_event);
            }
        }
        
        // 清理过期事件
        self.event_buffer.cleanup();
        
        matches
    }
}
```

### 6. 规则配置示例

```yaml
rules:
  # 基于告警的规则
  - id: "voltage_protection"
    name: "电压保护"
    trigger:
      type: "alarm_event"
      alarm_ids: ["voltage_critical"]
    condition: "context.alarm.level == 'critical'"
    actions:
      - type: "device_control"
        device_pattern: "{context.device_id}"
        field: "relay_status"
        value: 0  # 断开
      - type: "notify"
        channel: "ops_team"
        message: "紧急：设备 {context.device_id} 电压超标，已自动断开"
        
  # 基于数据的规则
  - id: "load_balancing"
    name: "负载均衡"
    trigger:
      type: "device_data"
      device_pattern: "transformer_*"
      check_interval: 60
    condition: "data.load_percentage > 80"
    actions:
      - type: "run_script"
        script_id: "load_balance"
        params:
          threshold: 80
          
  # 组合触发规则
  - id: "cascade_protection"
    name: "级联保护"
    trigger:
      type: "combined"
      logic: "AND"
      triggers:
        - type: "alarm_event"
          alarm_ids: ["temp_high"]
        - type: "device_data"
          device_pattern: "{context.device_id}"
          condition: "data.current > 100"
    actions:
      - type: "device_control"
        device_pattern: "{context.device_id}"
        field: "cooling_mode"
        value: 2  # 强制冷却
```

### 7. 与其他服务的交互

```rust
/// 数据流：
/// 1. AlarmSrv → RuleSrv
///    - 订阅告警事件
///    - 基于告警触发规则
/// 
/// 2. ModSrv ← → RuleSrv  
///    - 读取设备数据
///    - 更新 desired 状态
/// 
/// 3. RuleSrv → 其他服务
///    - 可以触发 Lua 脚本
///    - 可以创建派生告警
///    - 可以发送通知

impl RuleService {
    /// 通过 ModSrv 更新设备
    async fn update_device_desired(
        &self,
        device_id: &str,
        field: &str,
        value: f64,
    ) -> Result<()> {
        let key = format!("modsrv:{}:desired", device_id);
        self.redis.lock().await
            .hset(&key, field, value.to_string())
            .await?;
            
        // 发布更新事件
        self.redis.lock().await
            .publish(
                format!("modsrv:{}:desired:update", device_id),
                json!({ "field": field, "value": value }).to_string()
            )
            .await?;
            
        Ok(())
    }
}
```

## 优势

1. **灵活的触发机制**：支持告警、数据、时间等多种触发方式
2. **与 AlarmSrv 解耦**：通过事件订阅，不需要重复告警逻辑
3. **强大的动作系统**：支持控制、脚本、通知等
4. **可扩展性**：易于添加新的触发类型和动作

## 配置

```yaml
rulesrv:
  port: 8004
  redis_url: "redis://localhost:6379"
  
  # 规则引擎配置
  engine:
    max_concurrent: 100
    timeout: 30s
    
  # CEP 配置
  cep:
    enabled: true
    max_events: 10000
    window_size: 3600s  # 1小时窗口
    
  # 脚本执行
  scripts:
    path: "/etc/voltage/scripts"
    timeout: 10s
```

## 总结

改进后的 RuleSrv：
- 可以基于告警事件触发（订阅 AlarmSrv）
- 可以基于设备数据触发（查询 ModSrv）
- 支持复杂的动作执行
- 与其他服务通过事件解耦
- 保持了 DAG 规则引擎的灵活性