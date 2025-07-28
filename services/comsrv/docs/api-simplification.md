# Comsrv API 简化方案

## 背景

API Gateway 已经实现了直接从 Redis 读取实时数据的功能，comsrv 不再需要提供复杂的读取 API。

## 简化原则

1. **删除所有读取 API** - API Gateway 直接从 Redis 读取
2. **保留控制 API** - 写操作仍需要通过 comsrv
3. **保留管理 API** - 通道启停、状态查询等管理功能
4. **专注核心职责** - comsrv 只负责协议转换和数据采集

## 需要删除的 API

以下 API 将被删除，因为 API Gateway 可以直接从 Redis 读取：

```rust
// 删除 - API Gateway 直接读 Redis
GET /api/channels/{channel_id}/points/{point_table}/{point_name}  // 读取单个点
GET /api/channels/{channel_id}/points                             // 读取所有点
GET /api/channels/{channel_id}/telemetry_tables                   // 读取遥测表
```

## 需要保留的 API

### 1. 服务管理 API

```rust
GET  /api/status                    // 服务状态
GET  /api/health                    // 健康检查
```

### 2. 通道管理 API

```rust
GET  /api/channels                  // 列出所有通道
GET  /api/channels/{id}/status      // 通道状态（连接状态、错误等）
POST /api/channels/{id}/control     // 通道控制（启动/停止/重启）
```

### 3. 控制写入 API（简化版）

```rust
POST /api/control/{channel_id}      // 遥控命令
{
    "point_id": 1,
    "value": 1                       // 0 或 1
}

POST /api/adjustment/{channel_id}   // 遥调命令
{
    "point_id": 1,
    "value": 123.45                  // 浮点数
}
```

## 简化后的 API 实现

```rust
// services/comsrv/src/api/routes.rs

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};

/// 简化后的路由定义
pub fn create_api_routes(factory: Arc<RwLock<ProtocolFactory>>) -> Router {
    let state = AppState::new(factory);

    Router::new()
        // 服务管理
        .route("/api/status", get(get_service_status))
        .route("/api/health", get(health_check))
        
        // 通道管理
        .route("/api/channels", get(get_all_channels))
        .route("/api/channels/:id/status", get(get_channel_status))
        .route("/api/channels/:id/control", post(control_channel))
        
        // 控制命令（简化版）
        .route("/api/control/:channel_id", post(send_control))
        .route("/api/adjustment/:channel_id", post(send_adjustment))
        
        .with_state(state)
}

/// 发送控制命令（遥控）
async fn send_control(
    State(state): State<AppState>,
    Path(channel_id): Path<u16>,
    Json(cmd): Json<ControlCommand>,
) -> Result<Json<ApiResponse<bool>>, StatusCode> {
    let factory = state.factory.read().await;
    
    if let Some(channel) = factory.get_channel(channel_id).await {
        let mut channel_guard = channel.write().await;
        let redis_value = RedisValue::Integer(cmd.value as i64);
        
        match channel_guard.control(vec![(cmd.point_id, redis_value)]).await {
            Ok(results) => {
                let success = results.iter().any(|(_, s)| *s);
                Ok(Json(ApiResponse::success(success)))
            }
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// 发送调节命令（遥调）
async fn send_adjustment(
    State(state): State<AppState>,
    Path(channel_id): Path<u16>,
    Json(cmd): Json<AdjustmentCommand>,
) -> Result<Json<ApiResponse<bool>>, StatusCode> {
    let factory = state.factory.read().await;
    
    if let Some(channel) = factory.get_channel(channel_id).await {
        let mut channel_guard = channel.write().await;
        let redis_value = RedisValue::Float(cmd.value);
        
        match channel_guard.adjustment(vec![(cmd.point_id, redis_value)]).await {
            Ok(results) => {
                let success = results.iter().any(|(_, s)| *s);
                Ok(Json(ApiResponse::success(success)))
            }
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
```

## 数据模型简化

```rust
// 删除复杂的模型
// - PointValue
// - TelemetryPoint
// - TelemetryTableView
// - 各种 Mapping 结构

// 保留简单的模型
#[derive(Serialize, Deserialize)]
pub struct ControlCommand {
    pub point_id: u32,
    pub value: u8,  // 0 或 1
}

#[derive(Serialize, Deserialize)]
pub struct AdjustmentCommand {
    pub point_id: u32,
    pub value: f64,
}
```

## 实施步骤

### 第 1 步：标记废弃 API（1 天）
```rust
#[deprecated(note = "Use API Gateway direct Redis read instead")]
pub async fn read_point(...) { ... }
```

### 第 2 步：实现新的控制 API（2 天）
- 简化控制命令接口
- 统一错误处理
- 添加请求验证

### 第 3 步：删除废弃代码（1 天）
- 删除读取相关的 handler
- 删除复杂的数据模型
- 清理未使用的依赖

### 第 4 步：更新文档（1 天）
- 更新 API 文档
- 添加迁移指南
- 更新集成示例

## 预期收益

1. **代码量减少 50%** - 删除大量读取逻辑
2. **维护成本降低** - 只需维护核心功能
3. **性能提升** - 减少 JSON 序列化开销
4. **职责清晰** - comsrv 专注数据采集

## 迁移注意事项

1. **API Gateway 需要先部署** - 确保直读功能可用
2. **客户端需要更新** - 读取请求改为调用 API Gateway
3. **监控需要调整** - 关注 Redis 直接访问的指标

## 总结

通过这次简化，comsrv 将回归其核心职责：
- 工业协议转换
- 实时数据采集
- 控制命令下发
- 通道生命周期管理

而数据读取的职责完全交给 API Gateway，实现了更好的关注点分离。