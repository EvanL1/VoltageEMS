# ModSrv - 设备影子服务设计文档

## 概述

ModSrv 是 VoltageEMS 系统中的设备影子（Device Shadow）服务，负责维护物理设备在系统中的数字孪生。它是一个纯粹的状态映射服务，不包含任何业务逻辑或计算功能。

## 核心概念

### 设备影子（Device Shadow）

设备影子是物理设备的虚拟表示，包含：
- **Reported State**：设备上报的当前状态
- **Desired State**：系统期望设备达到的状态
- **Delta**：期望状态与当前状态的差异

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  Physical       │     │   Device Shadow  │     │  Applications   │
│  Device         │     │                  │     │                 │
│                 │     │ ┌──────────────┐ │     │                 │
│ Current State   │────▶│ │  Reported    │ │◀────│ Read State      │
│                 │     │ └──────────────┘ │     │                 │
│                 │     │                  │     │                 │
│ Execute Command │◀────│ ┌──────────────┐ │◀────│ Send Command    │
│                 │     │ │   Desired    │ │     │                 │
│                 │     │ └──────────────┘ │     │                 │
└─────────────────┘     └──────────────────┘     └─────────────────┘
```

## 架构设计

### 数据模型

```rust
// 设备影子核心结构
pub struct DeviceShadow {
    // 设备标识
    pub device_id: String,        // 设备唯一ID
    pub model_name: String,       // 设备模型名称

    // 状态数据
    pub reported: StateData,      // 设备上报的状态
    pub desired: StateData,       // 期望的设备状态
    pub delta: Option<StateData>, // 差异（desired - reported）

    // 元数据
    pub metadata: ShadowMetadata,
    pub version: u64,
}

// 状态数据
pub struct StateData {
    pub measurements: HashMap<String, f64>,  // 测量值
    pub signals: HashMap<String, bool>,      // 信号状态
    pub settings: HashMap<String, Value>,    // 设置参数
}

// 影子元数据
pub struct ShadowMetadata {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub reported_at: HashMap<String, DateTime<Utc>>,  // 各字段上报时间
    pub desired_at: HashMap<String, DateTime<Utc>>,   // 各字段设置时间
}
```

### Redis 存储结构

```
# 设备影子主体
modsrv:{model_name}:shadow → Hash {
    "reported": JSON(StateData),
    "desired": JSON(StateData),
    "delta": JSON(StateData),
    "metadata": JSON(ShadowMetadata),
    "version": "123"
}

# 快速访问的扁平化数据
modsrv:{model_name}:reported → Hash {
    "voltage": "230.5",
    "current": "10.2",
    "power": "2346.1"
}

modsrv:{model_name}:desired → Hash {
    "setpoint": "25.0",
    "mode": "auto"
}

# 设备注册表
modsrv:registry → Hash {
    "{device_id}": "{model_name}"
}
```

## 核心功能

### 1. 状态上报（Report State）

```rust
impl DeviceShadowService {
    /// 更新设备上报的状态
    pub async fn update_reported_state(
        &self,
        device_id: &str,
        state: StateUpdate,
    ) -> Result<()> {
        // 获取或创建设备影子
        let mut shadow = self.get_or_create_shadow(device_id).await?;

        // 更新 reported 状态（直接覆盖，不做任何处理）
        shadow.merge_reported(state);

        // 更新时间戳
        shadow.metadata.updated_at = Utc::now();

        // 重新计算 delta
        shadow.calculate_delta();

        // 保存到 Redis
        self.save_shadow(&shadow).await?;

        // 发布状态变更事件
        self.publish_event(ShadowEvent::ReportedUpdated {
            device_id: device_id.to_string(),
            changed_fields: state.get_fields(),
        }).await?;

        Ok(())
    }
}
```

### 2. 期望状态设置（Set Desired State）

```rust
impl DeviceShadowService {
    /// 设置设备的期望状态
    pub async fn update_desired_state(
        &self,
        device_id: &str,
        state: StateUpdate,
    ) -> Result<()> {
        let mut shadow = self.get_shadow(device_id).await?;

        // 更新 desired 状态
        shadow.merge_desired(state);

        // 增加版本号
        shadow.version += 1;

        // 重新计算 delta
        shadow.calculate_delta();

        // 保存到 Redis
        self.save_shadow(&shadow).await?;

        // 如果有 delta，通知设备更新
        if let Some(delta) = &shadow.delta {
            self.notify_device_update(device_id, delta).await?;
        }

        // 发布状态变更事件
        self.publish_event(ShadowEvent::DesiredUpdated {
            device_id: device_id.to_string(),
            version: shadow.version,
        }).await?;

        Ok(())
    }
}
```

### 3. 影子同步（Shadow Sync）

```rust
impl DeviceShadow {
    /// 计算 reported 和 desired 的差异
    fn calculate_delta(&mut self) {
        let mut delta = StateData::default();
        let mut has_delta = false;

        // 比较测量值
        for (key, desired_value) in &self.desired.measurements {
            if let Some(reported_value) = self.reported.measurements.get(key) {
                if (desired_value - reported_value).abs() > EPSILON {
                    delta.measurements.insert(key.clone(), *desired_value);
                    has_delta = true;
                }
            } else {
                // Reported 中没有这个字段，加入 delta
                delta.measurements.insert(key.clone(), *desired_value);
                has_delta = true;
            }
        }

        // 比较信号和设置（类似逻辑）
        // ...

        self.delta = if has_delta { Some(delta) } else { None };
    }
}
```

### 4. 设备注册与发现

```rust
impl DeviceShadowService {
    /// 注册新设备
    pub async fn register_device(
        &self,
        device_id: &str,
        model_name: &str,
        initial_state: Option<StateData>,
    ) -> Result<()> {
        // 创建设备影子
        let shadow = DeviceShadow::new(device_id, model_name, initial_state);

        // 保存到注册表
        self.redis.hset(
            "modsrv:registry",
            device_id,
            model_name,
        ).await?;

        // 保存影子
        self.save_shadow(&shadow).await?;

        Ok(())
    }

    /// 发现设备
    pub async fn discover_devices(&self, model_name: Option<&str>) -> Result<Vec<DeviceInfo>> {
        // 从注册表查询设备
        let devices = if let Some(model) = model_name {
            self.get_devices_by_model(model).await?
        } else {
            self.get_all_devices().await?
        };

        Ok(devices)
    }
}
```

## API 接口

### REST API

```yaml
# 获取设备影子
GET /api/shadows/{device_id}
Response:
{
  "deviceId": "device123",
  "modelName": "power_meter",
  "state": {
    "reported": {
      "voltage": 230.5,
      "current": 10.2
    },
    "desired": {
      "mode": "auto"
    },
    "delta": {
      "mode": "auto"
    }
  },
  "metadata": {
    "version": 123,
    "updatedAt": "2024-01-01T12:00:00Z"
  }
}

# 更新 desired 状态
PATCH /api/shadows/{device_id}/desired
Body:
{
  "state": {
    "mode": "manual",
    "setpoint": 25.0
  }
}

# 获取设备列表
GET /api/devices?model={model_name}
```

### WebSocket 订阅

```javascript
// 订阅设备影子变化
ws.subscribe(`shadow/${deviceId}/update`);

// 接收更新通知
ws.on('message', (data) => {
  const update = JSON.parse(data);
  console.log(`Device ${update.deviceId} state changed:`, update.state);
});
```

## 事件系统

### 事件类型

```rust
pub enum ShadowEvent {
    // 设备注册
    DeviceRegistered { device_id: String, model_name: String },

    // 状态更新
    ReportedUpdated { device_id: String, changed_fields: Vec<String> },
    DesiredUpdated { device_id: String, version: u64 },

    // Delta 变化
    DeltaChanged { device_id: String, delta: StateData },
    DeltaResolved { device_id: String },

    // 设备离线/在线
    DeviceOnline { device_id: String },
    DeviceOffline { device_id: String },
}
```

### Redis Pub/Sub 通道

```
# 状态更新通知
modsrv:{device_id}:reported → 设备上报更新
modsrv:{device_id}:desired → 期望状态更新
modsrv:{device_id}:delta → Delta 变化

# 全局事件
modsrv:events → 所有影子事件
```

## 与其他服务的集成

### ComSrv 集成

```rust
// ComSrv 上报数据
comsrv.on_data_received(|channel_id, data| {
    // 通过映射找到对应的设备
    let device_id = mapping.get_device_id(channel_id)?;

    // 更新影子的 reported 状态
    modsrv.update_reported_state(device_id, data).await?;
});

// ComSrv 接收控制命令
modsrv.on_delta_changed(|device_id, delta| {
    // 通过映射找到对应的通道
    let channel_id = mapping.get_channel_id(device_id)?;

    // 发送控制命令
    comsrv.send_command(channel_id, delta).await?;
});
```

### RuleSrv 集成

```rust
// RuleSrv 读取设备状态
let shadow = modsrv.get_shadow(device_id).await?;
let voltage = shadow.reported.measurements.get("voltage");

// RuleSrv 设置期望状态
if voltage > threshold {
    modsrv.update_desired_state(device_id, StateUpdate {
        measurements: hashmap! {
            "power_limit" => 80.0,
        },
    }).await?;
}
```

### API Gateway 集成

```rust
// 直接读取影子数据
let shadow_data = redis.hgetall(format!("modsrv:{}:reported", model_name)).await?;

// 设置期望状态
modsrv_client.update_desired_state(device_id, desired_state).await?;
```

## 性能优化

### 1. 缓存策略

```rust
pub struct ShadowCache {
    // LRU 缓存最近访问的影子
    shadows: LruCache<String, Arc<DeviceShadow>>,

    // 缓存过期时间
    ttl: Duration,
}
```

### 2. 批量操作

```rust
// 批量更新多个设备
pub async fn batch_update_reported(
    &self,
    updates: Vec<(String, StateUpdate)>,
) -> Result<()> {
    let mut pipe = self.redis.pipeline();

    for (device_id, state) in updates {
        // 批量构建 Redis 命令
        pipe.hset(format!("modsrv:{}:reported", device_id), state);
    }

    pipe.execute().await?;
    Ok(())
}
```

### 3. 变更检测优化

```rust
// 只发送真正变化的字段
impl StateData {
    pub fn diff(&self, other: &StateData) -> Option<StateData> {
        let mut changed = StateData::default();
        let mut has_changes = false;

        // 智能比较，忽略微小变化
        for (key, value) in &self.measurements {
            if let Some(other_value) = other.measurements.get(key) {
                if (value - other_value).abs() > THRESHOLD {
                    changed.measurements.insert(key.clone(), *value);
                    has_changes = true;
                }
            }
        }

        if has_changes { Some(changed) } else { None }
    }
}
```

## 配置示例

```yaml
# modsrv 配置
service:
  name: "modsrv"
  port: 8002

redis:
  url: "redis://localhost:6379"

shadow:
  # 变化阈值
  change_threshold: 0.01

  # 缓存配置
  cache:
    size: 10000
    ttl: 300s

  # 离线检测
  offline_timeout: 600s

# 设备模型定义
models:
  - name: "power_meter"
    description: "智能电表"
    reported_fields:
      - name: "voltage"
        type: "float"
        unit: "V"
      - name: "current"
        type: "float"
        unit: "A"
    desired_fields:
      - name: "mode"
        type: "enum"
        values: ["auto", "manual"]
      - name: "power_limit"
        type: "float"
        unit: "kW"
```

## 部署考虑

1. **高可用**：支持多实例部署，通过 Redis 共享状态
2. **水平扩展**：无状态设计，可根据设备数量扩展
3. **监控指标**：
   - 影子更新频率
   - Delta 解决时间
   - 缓存命中率
   - API 响应时间

## 总结

ModSrv 作为设备影子服务，专注于：
1. 维护设备的数字孪生
2. 管理 reported/desired/delta 三态
3. 提供状态查询和订阅接口
4. 不包含任何业务逻辑

这种设计确保了服务的单一职责，使系统架构更加清晰和可维护。
