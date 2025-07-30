# ModSrv 改进方案

## 当前状态
- 模型元数据管理服务
- 管理设备类型定义和点位配置
- 通过 Lua 脚本（sync.lua）与 ComSrv 自动同步
- 缺少设备影子（Device Shadow）功能

## 改进目标
实现标准的设备影子架构，支持设备状态的双向同步和控制确认。

## 核心改进：实现设备影子

### 1. 影子数据结构设计

```rust
/// 设备影子核心结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceShadow {
    /// 设备唯一标识
    pub device_id: String,
    
    /// 关联的模型类型
    pub model_id: String,
    
    /// 设备上报的当前状态
    pub reported: StateDocument,
    
    /// 系统期望的设备状态
    pub desired: StateDocument,
    
    /// 元数据
    pub metadata: ShadowMetadata,
    
    /// 影子版本号（乐观锁）
    pub version: u64,
}

/// 状态文档
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateDocument {
    /// 测量值（只读）
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub measurements: HashMap<String, f64>,
    
    /// 控制值（可读写）
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub controls: HashMap<String, Value>,
    
    /// 状态标志
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub status: HashMap<String, bool>,
}

/// 影子元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowMetadata {
    /// 创建时间
    pub created: i64,
    
    /// 最后更新时间
    pub updated: i64,
    
    /// 各字段的更新时间戳
    pub field_timestamps: HashMap<String, i64>,
    
    /// 是否在线
    pub connected: bool,
    
    /// 最后上线时间
    pub last_connected: Option<i64>,
}
```

### 2. 影子服务实现

```rust
pub struct ShadowService {
    redis: Arc<Mutex<RedisClient>>,
    shadows: Arc<RwLock<HashMap<String, DeviceShadow>>>,
}

impl ShadowService {
    /// 更新设备上报状态（由 Lua 脚本调用）
    pub async fn update_reported(&self, device_id: &str, update: StateUpdate) -> Result<Delta> {
        let mut shadow = self.get_or_create_shadow(device_id).await?;
        
        // 合并更新到 reported
        shadow.merge_reported(&update);
        shadow.metadata.updated = Utc::now().timestamp();
        
        // 计算新的 delta
        let delta = self.calculate_delta(&shadow)?;
        
        // 保存到 Redis（供 Lua 脚本和其他服务使用）
        self.save_shadow(&shadow).await?;
        
        // 发布更新事件
        self.publish_update(device_id, "reported", &update).await?;
        
        Ok(delta)
    }
    
    /// 更新期望状态（由 API 或规则引擎调用）
    pub async fn update_desired(&self, device_id: &str, update: StateUpdate) -> Result<Delta> {
        let mut shadow = self.get_shadow(device_id).await?;
        
        // 版本检查（乐观锁）
        if let Some(expected_version) = update.expected_version {
            if shadow.version != expected_version {
                return Err(ModelSrvError::VersionConflict);
            }
        }
        
        // 合并更新到 desired
        shadow.merge_desired(&update);
        shadow.version += 1;
        
        // 计算新的 delta
        let delta = self.calculate_delta(&shadow)?;
        
        // 保存到 Redis
        self.save_shadow(&shadow).await?;
        
        // Lua 脚本会自动处理 delta 同步到 ComSrv
        
        Ok(delta)
    }
    
    /// 计算 Delta（差异）
    fn calculate_delta(&self, shadow: &DeviceShadow) -> Result<Delta> {
        let mut delta = Delta::new();
        
        // 只比较控制值（measurements 是只读的）
        for (key, desired_value) in &shadow.desired.controls {
            match shadow.reported.controls.get(key) {
                Some(reported_value) if reported_value == desired_value => {
                    // 已同步，跳过
                }
                _ => {
                    // 未同步，加入 delta
                    delta.controls.insert(key.clone(), desired_value.clone());
                }
            }
        }
        
        Ok(delta)
    }
}
```

### 3. Redis 存储结构

```
# 影子主文档
modsrv:{device_id}:shadow → Hash {
    "reported": JSON(StateDocument),
    "desired": JSON(StateDocument), 
    "metadata": JSON(ShadowMetadata),
    "version": "123"
}

# 独立的 reported/desired（供 Lua 脚本快速访问）
modsrv:{device_id}:reported → Hash { field: value, ... }
modsrv:{device_id}:desired → Hash { field: value, ... }

# Delta（供 Lua 脚本读取并同步）
modsrv:{device_id}:delta → Hash { field: value, ... }

# 设备索引
modsrv:devices:{model_id} → Set [device_id1, device_id2, ...]
modsrv:devices:online → Set [在线设备ID列表]
```

### 4. 与现有 Lua 脚本的配合

现有的 `sync.lua` 脚本已经处理了：
- ComSrv → ModSrv 的数据同步（更新 reported）
- ModSrv → ComSrv 的控制同步（读取 delta）

ModSrv 只需要：
1. 维护影子数据结构
2. 计算 delta
3. 将数据存储在 Redis 中正确的位置

```lua
-- sync.lua 会自动处理这些 Redis 键
-- 当 comsrv:1001:m 更新时 → 更新 modsrv:device_001:reported
-- 当 modsrv:device_001:delta 有值时 → 发送控制命令到 comsrv
```

### 5. API 接口

```rust
/// 获取设备影子
#[axum::debug_handler]
pub async fn get_shadow(
    Path(device_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<ShadowResponse>> {
    let shadow = state.shadow_service.get_shadow(&device_id).await?;
    
    // 计算当前 delta
    let delta = state.shadow_service.calculate_delta(&shadow)?;
    
    Ok(Json(ShadowResponse {
        device_id: shadow.device_id,
        model_id: shadow.model_id,
        state: ShadowState {
            reported: shadow.reported,
            desired: shadow.desired,
            delta: if delta.is_empty() { None } else { Some(delta) },
        },
        metadata: shadow.metadata,
        version: shadow.version,
    }))
}

/// 更新期望状态
#[axum::debug_handler]
pub async fn update_desired(
    Path(device_id): Path<String>,
    State(state): State<AppState>,
    Json(update): Json<DesiredUpdate>,
) -> Result<Json<UpdateResponse>> {
    // 验证更新的字段是否为可控制字段
    state.validate_controllable_fields(&device_id, &update).await?;
    
    // 更新影子
    let delta = state.shadow_service.update_desired(&device_id, update.into()).await?;
    
    Ok(Json(UpdateResponse {
        accepted: true,
        version: state.shadow_service.get_version(&device_id).await?,
        delta: if delta.is_empty() { None } else { Some(delta) },
    }))
}

/// 获取设备列表
#[axum::debug_handler]
pub async fn list_devices(
    Query(params): Query<ListParams>,
    State(state): State<AppState>,
) -> Result<Json<DeviceList>> {
    let devices = state.shadow_service.list_devices(params).await?;
    Ok(Json(devices))
}
```

### 6. WebSocket 实时推送

```rust
/// 影子变更通知
impl ShadowService {
    /// 监听影子变更并推送
    pub async fn start_change_stream(&self, ws_manager: Arc<WebSocketManager>) {
        let mut pubsub = self.redis.lock().await.subscribe("modsrv:*:update").await.unwrap();
        
        while let Ok(msg) = pubsub.on_message().await {
            let channel = msg.get_channel();
            let payload = msg.get_payload::<String>().unwrap();
            
            // 解析设备ID和更新类型
            if let Some(device_id) = parse_device_id(channel) {
                let update = ShadowUpdate {
                    device_id,
                    update_type: parse_update_type(channel),
                    data: serde_json::from_str(&payload).ok(),
                    timestamp: Utc::now().timestamp(),
                };
                
                // 推送给订阅的客户端
                ws_manager.broadcast_shadow_update(&update).await;
            }
        }
    }
}
```

## 实施步骤

### 第一步：基础影子结构（1周）
1. 定义影子数据结构
2. 实现基础的 CRUD 操作
3. 实现 Delta 计算逻辑

### 第二步：Redis 集成（3天）
1. 设计 Redis 存储结构
2. 实现影子持久化
3. 确保与 Lua 脚本的兼容性

### 第三步：API 和 WebSocket（3天）
1. 实现 REST API
2. 添加 WebSocket 推送
3. 完善错误处理

### 第四步：测试和优化（1周）
1. 功能测试
2. 性能优化
3. 与 sync.lua 的集成测试

## 配置示例

```yaml
# ModSrv 配置
shadow:
  # 默认超时设置
  sync_timeout: 30s
  
  # 缓存设置
  cache:
    enabled: true
    size: 10000
    ttl: 300s
    
  # 版本控制
  versioning:
    enabled: true
    max_conflicts_retry: 3
```

## 注意事项

1. **与 Lua 脚本协同**：ModSrv 只负责影子管理，数据同步由 sync.lua 处理
2. **保持简单**：第一版只实现核心功能，避免过度设计
3. **向后兼容**：保留现有的模型管理功能
4. **性能考虑**：使用缓存减少 Redis 访问

## 预期效果

1. 完整的设备状态管理
2. 支持设备控制确认
3. 符合 IoT 标准的影子实现
4. 与现有 Lua 脚本无缝集成