# AlarmSrv 改进方案

## 当前状态
- 基础的阈值告警功能
- 简单的告警管理

## 改进目标
告警检测服务，主要从 ModSrv 获取设备影子数据，检测异常并生成告警。

## 核心设计

### 1. 数据结构

```rust
/// 告警定义（极简）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmDef {
    pub id: String,        // 告警ID
    pub name: String,      // 告警名称  
    pub level: AlarmLevel, // 告警级别
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlarmLevel {
    Info,
    Warning,
    Critical,
}

/// 告警状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmStatus {
    pub triggered: bool,           // 触发状态
    pub first_triggered_at: i64,   // 首次触发时间戳
}

/// 告警服务
pub struct AlarmService {
    // 告警定义
    alarm_defs: HashMap<String, AlarmDef>,
    
    // 告警状态："{device_id}:{alarm_id}" -> 状态
    alarm_status: HashMap<String, AlarmStatus>,
    
    // Redis连接
    redis: Arc<Mutex<RedisClient>>,
}
```

### 2. 告警检测逻辑

```rust
impl AlarmService {
    /// 从 ModSrv 获取数据并检测告警
    pub async fn check_device(&mut self, device_id: &str) -> Result<()> {
        // 从 ModSrv 获取设备影子数据
        let shadow_key = format!("modsrv:{}:reported", device_id);
        let data: HashMap<String, String> = self.redis.lock().await
            .hgetall(&shadow_key)
            .await?;
        
        // 检查每个告警定义
        for (alarm_id, alarm_def) in &self.alarm_defs {
            let key = format!("{}:{}", device_id, alarm_id);
            
            // 根据 alarm_id 检查不同的条件
            let should_trigger = self.check_alarm_condition(alarm_id, &data);
            
            match self.alarm_status.get_mut(&key) {
                Some(status) => {
                    if status.triggered != should_trigger {
                        status.triggered = should_trigger;
                        self.publish_alarm_event(device_id, alarm_id, should_trigger).await?;
                    }
                }
                None if should_trigger => {
                    let status = AlarmStatus {
                        triggered: true,
                        first_triggered_at: Utc::now().timestamp(),
                    };
                    self.alarm_status.insert(key, status);
                    self.publish_alarm_event(device_id, alarm_id, true).await?;
                }
                _ => {}
            }
        }
        
        Ok(())
    }
    
    /// 根据告警ID检查条件（硬编码业务逻辑）
    fn check_alarm_condition(&self, alarm_id: &str, data: &HashMap<String, String>) -> bool {
        match alarm_id {
            "voltage_high" => {
                data.get("voltage")
                    .and_then(|v| v.parse::<f64>().ok())
                    .map(|v| v > 245.0)
                    .unwrap_or(false)
            }
            "voltage_critical" => {
                data.get("voltage")
                    .and_then(|v| v.parse::<f64>().ok())
                    .map(|v| v > 260.0)
                    .unwrap_or(false)
            }
            "temp_high" => {
                data.get("temperature")
                    .and_then(|v| v.parse::<f64>().ok())
                    .map(|v| v > 85.0)
                    .unwrap_or(false)
            }
            "current_low" => {
                data.get("current")
                    .and_then(|v| v.parse::<f64>().ok())
                    .map(|v| v < 0.1)
                    .unwrap_or(false)
            }
            _ => false,
        }
    }
}
```

### 3. 主循环

```rust
impl AlarmService {
    pub async fn run(&mut self) -> Result<()> {
        // 订阅 ModSrv 的设备更新
        let mut sub = self.redis.lock().await
            .psubscribe("modsrv:*:update")
            .await?;
            
        while let Ok(msg) = sub.on_message().await {
            // 解析设备ID
            if let Some(device_id) = self.extract_device_id(&msg.channel) {
                // 检查设备告警
                self.check_device(&device_id).await?;
            }
        }
        
        Ok(())
    }
    
    /// 发布告警事件
    async fn publish_alarm_event(
        &self,
        device_id: &str,
        alarm_id: &str,
        triggered: bool,
    ) -> Result<()> {
        // 获取告警定义
        let alarm_def = self.alarm_defs.get(alarm_id)
            .ok_or_else(|| anyhow!("Unknown alarm: {}", alarm_id))?;
        
        let event = json!({
            "device_id": device_id,
            "alarm_id": alarm_id,
            "alarm_name": alarm_def.name,
            "level": alarm_def.level,
            "triggered": triggered,
            "timestamp": Utc::now().timestamp(),
        });
        
        // 发布事件供 RuleSrv 订阅
        self.redis.lock().await
            .publish("alarm:event", event.to_string())
            .await?;
            
        Ok(())
    }
}
```

### 4. 配置和初始化

```yaml
# 告警定义配置
alarms:
  - id: "voltage_high"
    name: "电压偏高"
    level: "warning"
    
  - id: "voltage_critical"
    name: "电压严重超标"
    level: "critical"
    
  - id: "temp_high"
    name: "温度过高"
    level: "warning"
    
  - id: "current_low"
    name: "电流过低"
    level: "info"
```

```rust
impl AlarmService {
    /// 初始化告警定义
    pub fn init_alarms(&mut self, config: Vec<AlarmDef>) {
        for alarm in config {
            self.alarm_defs.insert(alarm.id.clone(), alarm);
        }
    }
    
    /// 从 Redis 恢复告警状态
    pub async fn restore_state(&mut self) -> Result<()> {
        let data: HashMap<String, String> = self.redis.lock().await
            .hgetall("alarm:status")
            .await?;
            
        for (key, value) in data {
            if let Ok(status) = serde_json::from_str::<AlarmStatus>(&value) {
                self.alarm_status.insert(key, status);
            }
        }
        
        Ok(())
    }
}
```

### 5. API 接口

```rust
/// 获取所有活动告警
#[axum::debug_handler]
pub async fn get_active_alarms(
    State(state): State<AppState>,
) -> Result<Json<Vec<ActiveAlarmInfo>>> {
    let mut result = Vec::new();
    
    for (key, status) in &state.alarm_service.alarm_status {
        if status.triggered {
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() == 2 {
                let device_id = parts[0];
                let alarm_id = parts[1];
                
                if let Some(alarm_def) = state.alarm_service.alarm_defs.get(alarm_id) {
                    result.push(ActiveAlarmInfo {
                        device_id: device_id.to_string(),
                        alarm_id: alarm_id.to_string(),
                        alarm_name: alarm_def.name.clone(),
                        level: alarm_def.level.clone(),
                        first_triggered_at: status.first_triggered_at,
                    });
                }
            }
        }
    }
    
    Ok(Json(result))
}
```

### 6. 与其他服务的交互

```rust
/// 数据流：
/// 1. ModSrv (主要)
///    - 订阅设备影子更新
///    - 读取 reported 状态
/// 
/// 2. RuleSrv (输出)
///    - 发布告警事件
///    - RuleSrv 可基于告警触发规则
/// 
/// 3. NetSrv (查询)
///    - 提供活动告警列表
///    - 用于上送云端

impl AlarmService {
    /// 为 NetSrv 提供批量告警数据
    pub async fn get_alarms_for_upload(&self) -> Vec<AlarmUploadData> {
        self.alarm_status
            .iter()
            .filter(|(_, status)| status.triggered)
            .map(|(key, status)| {
                let parts: Vec<&str> = key.split(':').collect();
                AlarmUploadData {
                    device_id: parts[0].to_string(),
                    alarm_id: parts[1].to_string(),
                    timestamp: status.first_triggered_at,
                }
            })
            .collect()
    }
}
```

## 优势

1. **极简设计**：告警定义只有 ID、名称、级别
2. **业务逻辑集中**：告警条件在代码中明确定义
3. **高效存储**：HashMap 维护状态
4. **清晰的数据流**：以 ModSrv 为主要数据源

## 总结

简化后的 AlarmSrv：
- 告警定义只包含：ID、名称、级别
- 告警条件在代码中实现（易于维护和测试）
- 主要从 ModSrv 获取数据
- 通过事件通知 RuleSrv
- 为 NetSrv 提供上送数据