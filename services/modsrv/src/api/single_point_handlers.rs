//! Single Point Routing API Handlers
//!
//! Provides RESTful API endpoints for managing routing of individual points.
//! Supports separate paths for measurement points and action points.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use axum::{
    extract::{Path, State},
    response::Json,
};
use common::SuccessResponse;
use serde_json::json;
use std::sync::Arc;

use crate::app_state::AppState;
use crate::dto::{SinglePointRoutingRequest, ToggleRoutingRequest};
use crate::error::ModSrvError;

// ============================================================================
// Measurement Point Handlers
// ============================================================================

/// 读单条测量点位的完整信息（定义 + 路由 + 当前值）。
///
/// 一次性返回 `instance.measurement_points` 表的点位定义、关联的 C2M
/// 路由（指向哪个 channel 哪个点）、以及 `inst:{id}:M` 上的最新测量
/// 值。前端"点位详情对话框"用。404 表示 instance 或该 point_id 不存
/// 在。
#[utoipa::path(
    get,
    path = "/api/instances/{id}/measurements/{point_id}",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Measurement point ID")
    ),
    responses(
        (status = 200, description = "Measurement point with routing", body = crate::dto::InstanceMeasurementPoint),
        (status = 404, description = "Instance or point not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn get_measurement_point(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
) -> Result<Json<SuccessResponse<crate::dto::InstanceMeasurementPoint>>, ModSrvError> {
    match state
        .instance_manager
        .load_single_measurement_point(id, point_id)
        .await
    {
        Ok(point) => Ok(Json(SuccessResponse::new(point))),
        Err(e) => Err(ModSrvError::InternalError(format!(
            "Failed to load measurement point: {}",
            e
        ))),
    }
}

/// 给单条测量点位创建或修改 C2M 路由（UPSERT 语义）。
///
/// 把 `instance.measurement_point` 指向 `channel.{T|S}.point` —— 这之
/// 后该 channel 点位的新值会自动同步到 `inst:{id}:M`。已有路由会被覆
/// 盖。改动立即触发路由缓存 reload，下个 ShmRedisSync 周期开始走新映
/// 射。
#[utoipa::path(
    put,
    path = "/api/instances/{id}/measurements/{point_id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Measurement point ID")
    ),
    request_body = crate::dto::SinglePointRoutingRequest,
    responses(
        (status = 200, description = "Routing created/updated", body = serde_json::Value,
            example = json!({"message": "Routing updated for measurement point 101"})
        ),
        (status = 400, description = "Invalid routing configuration"),
        (status = 404, description = "Instance not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn upsert_measurement_routing(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
    Json(request): Json<SinglePointRoutingRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Upsert routing in database
    state
        .instance_manager
        .upsert_measurement_routing(id, point_id, request)
        .await
        .map_err(|e| ModSrvError::InvalidData(format!("Failed to upsert routing: {}", e)))?;

    state
        .instance_manager
        .refresh_routing()
        .await
        .map_err(|e| {
            ModSrvError::InternalError(format!(
                "Failed to refresh routing after upsert measurement routing: {}",
                e
            ))
        })?;

    Ok(Json(SuccessResponse::new(json!({
        "message": format!("Routing updated for measurement point {}", point_id)
    }))))
}

/// 删除单条测量点位的 C2M 路由。
///
/// 删除路由，但**保留点位定义** —— instance 的物模型不变。删后该测量
/// 点不再有数据流入，`inst:{id}:M` 上对应字段不再更新，保持最后一次
/// 已写入的值（last-known-good）。
#[utoipa::path(
    delete,
    path = "/api/instances/{id}/measurements/{point_id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Measurement point ID")
    ),
    responses(
        (status = 200, description = "Routing deleted", body = serde_json::Value,
            example = json!({"message": "Routing deleted for measurement point 101", "rows_affected": 1})
        ),
        (status = 404, description = "Instance or routing not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn delete_measurement_routing(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Delete routing from database
    let rows_affected = state
        .instance_manager
        .delete_measurement_routing(id, point_id)
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to delete routing: {}", e)))?;

    if rows_affected == 0 {
        return Err(ModSrvError::InternalError(format!(
            "No routing found for measurement point {} in instance {}",
            point_id, id
        )));
    }

    state
        .instance_manager
        .refresh_routing()
        .await
        .map_err(|e| {
            ModSrvError::InternalError(format!(
                "Failed to refresh routing after delete measurement routing: {}",
                e
            ))
        })?;

    Ok(Json(SuccessResponse::new(json!({
        "message": format!("Routing deleted for measurement point {}", point_id),
        "rows_affected": rows_affected
    }))))
}

/// 切换单条测量点位 C2M 路由的启用/禁用。
///
/// 比"删除路由"轻 —— 保留路由定义但暂停数据流。禁用后 `inst:{id}:M`
/// 上该字段不再更新（保留 last-known-good），启用恢复正常同步。常用于
/// 临时屏蔽某个故障点位的数据上行。
#[utoipa::path(
    patch,
    path = "/api/instances/{id}/measurements/{point_id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Measurement point ID")
    ),
    request_body = crate::dto::ToggleRoutingRequest,
    responses(
        (status = 200, description = "Routing enabled/disabled", body = serde_json::Value,
            example = json!({"message": "Routing enabled for measurement point 101", "rows_affected": 1})
        ),
        (status = 404, description = "Instance or routing not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn toggle_measurement_routing(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
    Json(request): Json<ToggleRoutingRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Toggle routing in database
    let rows_affected = state
        .instance_manager
        .toggle_measurement_routing(id, point_id, request.enabled)
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to toggle routing: {}", e)))?;

    if rows_affected == 0 {
        return Err(ModSrvError::InternalError(format!(
            "No routing found for measurement point {} in instance {}",
            point_id, id
        )));
    }

    state
        .instance_manager
        .refresh_routing()
        .await
        .map_err(|e| {
            ModSrvError::InternalError(format!(
                "Failed to refresh routing after toggle measurement routing: {}",
                e
            ))
        })?;

    let action = if request.enabled {
        "enabled"
    } else {
        "disabled"
    };
    Ok(Json(SuccessResponse::new(json!({
        "message": format!("Routing {} for measurement point {}", action, point_id),
        "rows_affected": rows_affected
    }))))
}

// ============================================================================
// Action Point Handlers
// ============================================================================

/// 读单条动作点位的完整信息（定义 + M2C 路由 + 最近一次写入值）。
///
/// 跟 `/measurement-point/{id}` 对应的动作点版本。返回 action_point 定
/// 义、关联的 M2C 路由（指向哪个 channel 的 C/A 点）、以及
/// `inst:{id}:A` 上最近一次写入的命令值。
#[utoipa::path(
    get,
    path = "/api/instances/{id}/actions/{point_id}",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Action point ID")
    ),
    responses(
        (status = 200, description = "Action point with routing", body = crate::dto::InstanceActionPoint),
        (status = 404, description = "Instance or point not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn get_action_point(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
) -> Result<Json<SuccessResponse<crate::dto::InstanceActionPoint>>, ModSrvError> {
    match state
        .instance_manager
        .load_single_action_point(id, point_id)
        .await
    {
        Ok(point) => Ok(Json(SuccessResponse::new(point))),
        Err(e) => Err(ModSrvError::InternalError(format!(
            "Failed to load action point: {}",
            e
        ))),
    }
}

/// 给单条动作点位创建或修改 M2C 路由（UPSERT 语义）。
///
/// 把 `instance.action_point` 指向 `channel.{C|A}.point` —— 这之后通
/// 过 `POST /api/instances/{id}/action` 或规则引擎发出的命令会经 SHM
/// + UDS 到达该 channel 下发设备。改动后路由缓存 reload，下一次
/// execute_action 走新路由。
#[utoipa::path(
    put,
    path = "/api/instances/{id}/actions/{point_id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Action point ID")
    ),
    request_body = crate::dto::SinglePointRoutingRequest,
    responses(
        (status = 200, description = "Routing created/updated", body = serde_json::Value,
            example = json!({"message": "Routing updated for action point 201"})
        ),
        (status = 400, description = "Invalid routing configuration"),
        (status = 404, description = "Instance not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn upsert_action_routing(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
    Json(request): Json<SinglePointRoutingRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Upsert routing in database
    state
        .instance_manager
        .upsert_action_routing(id, point_id, request)
        .await
        .map_err(|e| ModSrvError::InvalidData(format!("Failed to upsert routing: {}", e)))?;

    state
        .instance_manager
        .refresh_routing()
        .await
        .map_err(|e| {
            ModSrvError::InternalError(format!(
                "Failed to refresh routing after upsert action routing: {}",
                e
            ))
        })?;

    Ok(Json(SuccessResponse::new(json!({
        "message": format!("Routing updated for action point {}", point_id)
    }))))
}

/// 删除单条动作点位的 M2C 路由。
///
/// 删除后该 action 不再能下发到设备：execute_action() 走"无路由"分支
/// 只写 `inst:{id}:A` 本地存储但 dispatch=None。点位定义保留。
#[utoipa::path(
    delete,
    path = "/api/instances/{id}/actions/{point_id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Action point ID")
    ),
    responses(
        (status = 200, description = "Routing deleted", body = serde_json::Value,
            example = json!({"message": "Routing deleted for action point 201", "rows_affected": 1})
        ),
        (status = 404, description = "Instance or routing not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn delete_action_routing(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Delete routing from database
    let rows_affected = state
        .instance_manager
        .delete_action_routing(id, point_id)
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to delete routing: {}", e)))?;

    if rows_affected == 0 {
        return Err(ModSrvError::InternalError(format!(
            "No routing found for action point {} in instance {}",
            point_id, id
        )));
    }

    state
        .instance_manager
        .refresh_routing()
        .await
        .map_err(|e| {
            ModSrvError::InternalError(format!(
                "Failed to refresh routing after delete action routing: {}",
                e
            ))
        })?;

    Ok(Json(SuccessResponse::new(json!({
        "message": format!("Routing deleted for action point {}", point_id),
        "rows_affected": rows_affected
    }))))
}

/// 切换单条动作点位 M2C 路由的启用/禁用。
///
/// 禁用时 execute_action() 走无路由分支：写本地 inst:{id}:A 但**不下
/// 发**设备。用于临时屏蔽某个控制点（如调试期间防止误触发设备）。启
/// 用后下次 action 会正常下发。
#[utoipa::path(
    patch,
    path = "/api/instances/{id}/actions/{point_id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Action point ID")
    ),
    request_body = crate::dto::ToggleRoutingRequest,
    responses(
        (status = 200, description = "Routing enabled/disabled", body = serde_json::Value,
            example = json!({"message": "Routing enabled for action point 201", "rows_affected": 1})
        ),
        (status = 404, description = "Instance or routing not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn toggle_action_routing(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
    Json(request): Json<ToggleRoutingRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Toggle routing in database
    let rows_affected = state
        .instance_manager
        .toggle_action_routing(id, point_id, request.enabled)
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to toggle routing: {}", e)))?;

    if rows_affected == 0 {
        return Err(ModSrvError::InternalError(format!(
            "No routing found for action point {} in instance {}",
            point_id, id
        )));
    }

    state
        .instance_manager
        .refresh_routing()
        .await
        .map_err(|e| {
            ModSrvError::InternalError(format!(
                "Failed to refresh routing after toggle action routing: {}",
                e
            ))
        })?;

    let action = if request.enabled {
        "enabled"
    } else {
        "disabled"
    };
    Ok(Json(SuccessResponse::new(json!({
        "message": format!("Routing {} for action point {}", action, point_id),
        "rows_affected": rows_affected
    }))))
}
