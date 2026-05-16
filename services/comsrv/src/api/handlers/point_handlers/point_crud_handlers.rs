#![allow(clippy::disallowed_methods)]

//! Single-point CRUD handlers (Create, Update, Delete)

use crate::api::routes::AppState;
use crate::core::config::TelemetryPoint;
use crate::dto::{AppError, SuccessResponse};
use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use voltage_model::{KeySpaceConfig, PointType};
use voltage_rtdb::Rtdb;

use super::point_helpers::{
    point_type_to_table, trigger_channel_reload_if_needed, validate_channel_exists,
    validate_point_uniqueness,
};
use super::point_types::{PointCrudResult, PointUpdateRequest};

// ----------------------------------------------------------------------------
// Helper: Extract common fields from point creation payload
// ----------------------------------------------------------------------------

/// Common fields for S/C/A point creation
struct CreatePointFields {
    signal_name: String,
    scale: f64,
    offset: f64,
    unit: String,
    reverse: bool,
    data_type: String,
    description: String,
}

/// Extract and validate common fields from a JSON payload for point creation
fn extract_create_fields(
    payload: &serde_json::Value,
    point_id: u32,
    default_data_type: &str,
) -> Result<CreatePointFields, AppError> {
    let payload_point_id = payload
        .get("point_id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| AppError::bad_request("Missing field: point_id"))?;
    let payload_point_id = u32::try_from(payload_point_id).map_err(|_| {
        AppError::bad_request(format!("point_id {} out of range", payload_point_id))
    })?;

    if payload_point_id != point_id {
        return Err(AppError::bad_request(format!(
            "Point ID mismatch: path has {}, body has {}",
            point_id, payload_point_id
        )));
    }

    let signal_name = payload
        .get("signal_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::bad_request("Missing field: signal_name"))?
        .to_string();

    Ok(CreatePointFields {
        signal_name,
        scale: payload.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0),
        offset: payload
            .get("offset")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        unit: payload
            .get("unit")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        reverse: payload
            .get("reverse")
            .and_then(|v| {
                v.as_str()
                    .and_then(|s| s.parse::<bool>().ok())
                    .or_else(|| v.as_bool())
            })
            .unwrap_or(false),
        data_type: payload
            .get("data_type")
            .and_then(|v| v.as_str())
            .unwrap_or(default_data_type)
            .to_string(),
        description: payload
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

// ----------------------------------------------------------------------------
// Create Point Handlers
// ----------------------------------------------------------------------------

/// 新建一个遥测点（Telemetry / "T" 类型）。
///
/// T 是只读的浮点测量量（电压、电流、温度、SOC 等），周期性从设备读
/// 出。写入 `telemetry_points` 表 + 注册对应 SHM 槽位（如果 channel 已
/// 启动）。寄存器地址 / 字节序 / 缩放线性变换 / 单位等都在 request 里。
/// 单 channel 内 point_id 必须唯一。
#[utoipa::path(
    post,
    path = "/api/channels/{channel_id}/T/points/{point_id}",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("point_id" = u32, Path, description = "Point identifier"),
        ("auto_reload" = bool, Query, description = "Auto-reload channel after creation (default: true)")
    ),
    responses(
        (status = 201, description = "Point created", body = PointCrudResult),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Channel not found"),
        (status = 409, description = "Point ID already exists")
    ),
    tag = "comsrv"
)]
pub async fn create_telemetry_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    let point: TelemetryPoint = serde_json::from_value(payload)
        .map_err(|e| AppError::bad_request(format!("Invalid request body: {}", e)))?;
    validate_channel_exists(&state.sqlite_pool, channel_id).await?;

    if point.base.point_id != point_id {
        return Err(AppError::bad_request(format!(
            "Point ID mismatch: path has {}, body has {}",
            point_id, point.base.point_id
        )));
    }

    validate_point_uniqueness(&state.sqlite_pool, channel_id, "telemetry_points", point_id).await?;

    sqlx::query(
        "INSERT INTO telemetry_points
         (channel_id, point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
    )
    .bind(channel_id as i64)
    .bind(point.base.point_id as i64)
    .bind(&point.base.signal_name)
    .bind(point.scale)
    .bind(point.offset)
    .bind(&point.base.unit)
    .bind(&point.data_type)
    .bind(point.reverse)
    .bind(&point.base.description)
    .execute(&state.sqlite_pool)
    .await
    .map_err(|e| {
        tracing::error!("Create T point: {}", e);
        AppError::internal_error("Failed to create point")
    })?;

    tracing::debug!("Ch{}:T:{} created", channel_id, point_id);
    trigger_channel_reload_if_needed(channel_id, &state, reload_query.auto_reload).await;

    Ok(Json(SuccessResponse::new(PointCrudResult {
        channel_id,
        point_type: "T".to_string(),
        point_id,
        signal_name: point.base.signal_name.clone(),
        message: "Telemetry point created successfully".to_string(),
    })))
}

/// 新建一个信号点（Signal / "S" 类型）。
///
/// S 是只读的开关量 / 状态位（断路器 on/off、运行/故障旗标、报警位
/// 等），从设备的离散输入读取。比 T 多一个 `normal_state` 字段表示
/// "正常态"是 0 还是 1 —— 后续告警规则用它判断"翻转"。其余流程同 T。
#[utoipa::path(
    post,
    path = "/api/channels/{channel_id}/S/points/{point_id}",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("point_id" = u32, Path, description = "Point identifier"),
        ("auto_reload" = bool, Query, description = "Auto-reload channel after creation (default: true)")
    ),
    responses(
        (status = 201, description = "Point created", body = PointCrudResult),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Channel not found"),
        (status = 409, description = "Point ID already exists")
    ),
    tag = "comsrv"
)]
pub async fn create_signal_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    let fields = extract_create_fields(&payload, point_id, "bool")?;
    validate_channel_exists(&state.sqlite_pool, channel_id).await?;
    validate_point_uniqueness(&state.sqlite_pool, channel_id, "signal_points", point_id).await?;

    let normal_state = payload
        .get("normal_state")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    sqlx::query(
        "INSERT INTO signal_points
         (channel_id, point_id, signal_name, scale, offset, unit, reverse, normal_state, data_type, description, protocol_mappings)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
    )
    .bind(channel_id as i64)
    .bind(point_id as i64)
    .bind(&fields.signal_name)
    .bind(fields.scale)
    .bind(fields.offset)
    .bind(&fields.unit)
    .bind(fields.reverse)
    .bind(normal_state)
    .bind(&fields.data_type)
    .bind(&fields.description)
    .execute(&state.sqlite_pool)
    .await
    .map_err(|e| {
        tracing::error!("Create S point: {}", e);
        AppError::internal_error("Failed to create point")
    })?;

    tracing::debug!("Ch{}:S:{} created", channel_id, point_id);
    trigger_channel_reload_if_needed(channel_id, &state, reload_query.auto_reload).await;

    Ok(Json(SuccessResponse::new(PointCrudResult {
        channel_id,
        point_type: "S".to_string(),
        point_id,
        signal_name: fields.signal_name,
        message: "Signal point created successfully".to_string(),
    })))
}

/// Internal: create control or adjustment point (identical schema)
async fn create_ca_point_inner<R: Rtdb>(
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    state: AppState<R>,
    reload_query: crate::dto::AutoReloadQuery,
    payload: serde_json::Value,
    default_data_type: &str,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    let table = point_type_to_table(point_type)?;
    let fields = extract_create_fields(&payload, point_id, default_data_type)?;
    validate_channel_exists(&state.sqlite_pool, channel_id).await?;
    validate_point_uniqueness(&state.sqlite_pool, channel_id, table, point_id).await?;

    let query = format!(
        "INSERT INTO {}
         (channel_id, point_id, signal_name, scale, offset, unit, reverse, data_type, description, protocol_mappings)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
        table
    );
    sqlx::query(&query)
        .bind(channel_id as i64)
        .bind(point_id as i64)
        .bind(&fields.signal_name)
        .bind(fields.scale)
        .bind(fields.offset)
        .bind(&fields.unit)
        .bind(fields.reverse)
        .bind(&fields.data_type)
        .bind(&fields.description)
        .execute(&state.sqlite_pool)
        .await
        .map_err(|e| {
            tracing::error!("Create {} point: {}", point_type, e);
            AppError::internal_error("Failed to create point")
        })?;

    let type_name = match point_type {
        "C" => "Control",
        "A" => "Adjustment",
        _ => point_type,
    };
    tracing::debug!("Ch{}:{}:{} created", channel_id, point_type, point_id);
    trigger_channel_reload_if_needed(channel_id, &state, reload_query.auto_reload).await;

    Ok(Json(SuccessResponse::new(PointCrudResult {
        channel_id,
        point_type: point_type.to_string(),
        point_id,
        signal_name: fields.signal_name,
        message: format!("{} point created successfully", type_name),
    })))
}

/// 新建一个控制点（Control / "C" 类型）。
///
/// C 是可写的开关量（FC05 写线圈），用于"启动/停止"、"打开/关闭"等离散
/// 控制命令。modsrv → SHM C 槽 → UDS notify → comsrv 下发设备的链路终
/// 点。新建后该点立刻可写，但下发到设备前要先有 M2C 路由配置（指向某
/// 个 instance.action_point）。
#[utoipa::path(
    post,
    path = "/api/channels/{channel_id}/C/points/{point_id}",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("point_id" = u32, Path, description = "Point identifier"),
        ("auto_reload" = bool, Query, description = "Auto-reload channel after creation (default: true)")
    ),
    responses(
        (status = 201, description = "Point created", body = PointCrudResult),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Channel not found"),
        (status = 409, description = "Point ID already exists")
    ),
    tag = "comsrv"
)]
pub async fn create_control_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    create_ca_point_inner(
        channel_id,
        "C",
        point_id,
        state,
        reload_query,
        payload,
        "bool",
    )
    .await
}

/// 新建一个调节点（Adjustment / "A" 类型）。
///
/// A 是可写的浮点量（FC06 写寄存器 / FC16 多寄存器），用于"功率设定"、
/// "频率调节"、"电压设定"等连续值控制。跟 C 是平行的可写类型，区别只
/// 是值域：C 是 0/1，A 是浮点。其余规则相同（需 M2C 路由才下发设备）。
#[utoipa::path(
    post,
    path = "/api/channels/{channel_id}/A/points/{point_id}",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("point_id" = u32, Path, description = "Point identifier"),
        ("auto_reload" = bool, Query, description = "Auto-reload channel after creation (default: true)")
    ),
    responses(
        (status = 201, description = "Point created", body = PointCrudResult),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Channel not found"),
        (status = 409, description = "Point ID already exists")
    ),
    tag = "comsrv"
)]
pub async fn create_adjustment_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    create_ca_point_inner(
        channel_id,
        "A",
        point_id,
        state,
        reload_query,
        payload,
        "int16",
    )
    .await
}

// ----------------------------------------------------------------------------
// Update Point Handler (Universal for all types)
// ----------------------------------------------------------------------------

/// 修改任意类型点位的定义（统一入口）。
///
/// 跟 4 个 create 端点配对的统一 update：path 里带 `point_type` 判定改
/// 哪张表。可改寄存器地址、缩放系数、单位、报警限等；改 point_id 或
/// channel_id 不允许（要删了重建，避免破坏 SHM 槽映射）。改完后下次
/// poll 就用新配置，无需重启 channel。
#[utoipa::path(
    put,
    path = "/api/channels/{channel_id}/{type}/points/{point_id}",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("type" = String, Path, description = "Point type: T, S, C, or A"),
        ("point_id" = u32, Path, description = "Point identifier"),
        ("auto_reload" = bool, Query, description = "Auto-reload channel after update (default: true)")
    ),
    request_body(
        content = PointUpdateRequest,
        description = "Update request for point fields (supports partial updates). Only provide fields you want to update.",
        examples(
            ("Telemetry (T)" = (
                summary = "Update telemetry point",
                description = "Common fields: signal_name, description, unit, scale, offset, data_type, reverse",
                value = json!({
                    "signal_name": "DC_Voltage",
                    "description": "DC bus voltage",
                    "unit": "V",
                    "scale": 0.1,
                    "offset": 0.0,
                    "data_type": "float32",
                    "reverse": false
                })
            )),
            ("Signal (S)" = (
                summary = "Update signal point",
                description = "Common fields: signal_name, description, reverse",
                value = json!({
                    "signal_name": "Grid_Connected",
                    "description": "Grid connection status",
                    "reverse": false
                })
            )),
            ("Control (C)" = (
                summary = "Update control point",
                description = "Control fields: signal_name, description, reverse, control_type, on_value, off_value, pulse_duration_ms",
                value = json!({
                    "signal_name": "Main_Breaker",
                    "description": "Main breaker control",
                    "control_type": "momentary",
                    "on_value": 1,
                    "off_value": 0,
                    "pulse_duration_ms": 500,
                    "reverse": false
                })
            )),
            ("Adjustment (A)" = (
                summary = "Update adjustment point",
                description = "Adjustment fields: signal_name, description, unit, scale, offset, data_type, reverse (same as Telemetry)",
                value = json!({
                    "signal_name": "Target_Power",
                    "description": "Target power setpoint",
                    "unit": "kW",
                    "scale": 1.0,
                    "offset": 0.0,
                    "data_type": "float32",
                    "reverse": false
                })
            ))
        )
    ),
    responses(
        (status = 200, description = "Point updated", body = PointCrudResult),
        (status = 400, description = "Invalid point type"),
        (status = 404, description = "Channel or point not found")
    ),
    tag = "comsrv"
)]
/// All four point tables share the same updatable columns,
/// so a single parameterized query works for all types.
pub(super) async fn update_point_handler_inner<R: Rtdb>(
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    state: AppState<R>,
    reload_query: crate::dto::AutoReloadQuery,
    update: PointUpdateRequest,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    let point_type_upper = point_type.to_ascii_uppercase();
    let table = point_type_to_table(point_type)?;
    validate_channel_exists(&state.sqlite_pool, channel_id).await?;

    let has_update = update.signal_name.is_some()
        || update.description.is_some()
        || update.unit.is_some()
        || update.scale.is_some()
        || update.offset.is_some()
        || update.data_type.is_some()
        || update.reverse.is_some();

    if !has_update {
        return Err(AppError::bad_request("No fields provided for update"));
    }

    let query = format!(
        "UPDATE {} SET
            signal_name = COALESCE(?, signal_name),
            description = COALESCE(?, description),
            unit = COALESCE(?, unit),
            scale = COALESCE(?, scale),
            offset = COALESCE(?, offset),
            data_type = COALESCE(?, data_type),
            reverse = COALESCE(?, reverse)
        WHERE channel_id = ? AND point_id = ?
        RETURNING signal_name",
        table
    );

    let signal_name = sqlx::query_scalar::<_, String>(&query)
        .bind(update.signal_name.as_deref())
        .bind(update.description.as_deref())
        .bind(update.unit.as_deref())
        .bind(update.scale)
        .bind(update.offset)
        .bind(update.data_type.as_deref())
        .bind(update.reverse)
        .bind(channel_id as i64)
        .bind(point_id as i64)
        .fetch_optional(&state.sqlite_pool)
        .await
        .map_err(|e| {
            tracing::error!("Update {} point: {}", table, e);
            AppError::internal_error("Failed to update point")
        })?
        .ok_or_else(|| {
            AppError::not_found(format!(
                "Point {} (type {}) not found in channel {}",
                point_id, point_type_upper, channel_id
            ))
        })?;

    tracing::debug!("Ch{}:{}:{} updated", channel_id, point_type_upper, point_id);
    trigger_channel_reload_if_needed(channel_id, &state, reload_query.auto_reload).await;

    Ok(Json(SuccessResponse::new(PointCrudResult {
        channel_id,
        point_type: point_type_upper,
        point_id,
        signal_name,
        message: "Point updated successfully".to_string(),
    })))
}

// ----------------------------------------------------------------------------
// Delete Point Handler
// ----------------------------------------------------------------------------

/// 删除一个点位（任意类型）。
///
/// 从对应 `{type}_points` 表删行，关联的 protocol_mappings 也清理。
/// **会让该点对应的 SHM 槽闲置**（不立刻回收，保持 routing_hash 稳定，
/// 减少 modsrv 端 rebuild 风暴）。如果该点是某个 M2C 路由的目标，路由
/// 失效但不级联删 —— 需要单独清理孤儿路由。
#[utoipa::path(
    delete,
    path = "/api/channels/{channel_id}/{type}/points/{point_id}",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("type" = String, Path, description = "Point type: T, S, C, or A"),
        ("point_id" = u32, Path, description = "Point identifier"),
        ("auto_reload" = bool, Query, description = "Auto-reload channel after deletion (default: true)")
    ),
    responses(
        (status = 200, description = "Point deleted", body = PointCrudResult),
        (status = 400, description = "Invalid point type"),
        (status = 404, description = "Channel or point not found")
    ),
    tag = "comsrv"
)]
pub(super) async fn delete_point_handler_inner<R: Rtdb>(
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    state: AppState<R>,
    reload_query: crate::dto::AutoReloadQuery,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    let point_type_upper = point_type.to_ascii_uppercase();
    let table = point_type_to_table(point_type)?;
    validate_channel_exists(&state.sqlite_pool, channel_id).await?;

    // Get point info before deletion (for response)
    let query = format!(
        "SELECT signal_name FROM {} WHERE channel_id = ? AND point_id = ?",
        table
    );
    let existing: Option<(String,)> = sqlx::query_as(&query)
        .bind(channel_id as i64)
        .bind(point_id as i64)
        .fetch_optional(&state.sqlite_pool)
        .await
        .map_err(|e| {
            tracing::error!("Point check: {}", e);
            AppError::internal_error("Database operation failed")
        })?;

    let signal_name = existing
        .ok_or_else(|| {
            AppError::not_found(format!(
                "Point {} (type {}) not found in channel {}",
                point_id, point_type_upper, channel_id
            ))
        })?
        .0;

    // Delete point
    let delete_sql = format!(
        "DELETE FROM {} WHERE channel_id = ? AND point_id = ?",
        table
    );
    sqlx::query(&delete_sql)
        .bind(channel_id as i64)
        .bind(point_id as i64)
        .execute(&state.sqlite_pool)
        .await
        .map_err(|e| {
            tracing::error!("Delete point: {}", e);
            AppError::internal_error("Failed to delete point")
        })?;

    tracing::debug!("Ch{}:{}:{} deleted", channel_id, point_type_upper, point_id);

    // Clear Redis data for the deleted point
    let pt = PointType::from_str(&point_type_upper)
        .ok_or_else(|| AppError::internal_error("Invalid point type after validation"))?;
    let keyspace = KeySpaceConfig::production_cached();
    let redis_key = keyspace.channel_key(channel_id, pt);
    let fields_to_delete = vec![
        point_id.to_string(),
        format!("{}:ts", point_id),
        format!("{}:raw", point_id),
    ];

    for field in &fields_to_delete {
        match state.rtdb.hash_del(&redis_key, field).await {
            Ok(deleted) => {
                if deleted {
                    tracing::debug!(
                        "Cleared Redis field {} from {} for point {}",
                        field,
                        redis_key,
                        point_id
                    );
                }
            },
            Err(e) => {
                tracing::warn!("Redis del {}:{}: {}", redis_key, field, e);
            },
        }
    }

    tracing::debug!(
        "Ch{}:{}:{} Redis cleared",
        channel_id,
        point_type_upper,
        point_id
    );

    trigger_channel_reload_if_needed(channel_id, &state, reload_query.auto_reload).await;

    Ok(Json(SuccessResponse::new(PointCrudResult {
        channel_id,
        point_type: point_type_upper,
        point_id,
        signal_name,
        message: "Point deleted successfully".to_string(),
    })))
}

// ============================================================================
// Type-specific wrapper handlers (delegate to *_inner functions)
// ============================================================================

// --- PUT wrappers ---

/// Update telemetry point
pub async fn update_telemetry_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(update): Json<PointUpdateRequest>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    update_point_handler_inner(channel_id, "T", point_id, state, reload_query, update).await
}

/// Update signal point
pub async fn update_signal_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(update): Json<PointUpdateRequest>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    update_point_handler_inner(channel_id, "S", point_id, state, reload_query, update).await
}

/// Update control point
pub async fn update_control_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(update): Json<PointUpdateRequest>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    update_point_handler_inner(channel_id, "C", point_id, state, reload_query, update).await
}

/// Update adjustment point
pub async fn update_adjustment_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(update): Json<PointUpdateRequest>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    update_point_handler_inner(channel_id, "A", point_id, state, reload_query, update).await
}

// --- DELETE wrappers ---

/// Delete telemetry point
pub async fn delete_telemetry_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    delete_point_handler_inner(channel_id, "T", point_id, state, reload_query).await
}

/// Delete signal point
pub async fn delete_signal_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    delete_point_handler_inner(channel_id, "S", point_id, state, reload_query).await
}

/// Delete control point
pub async fn delete_control_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    delete_point_handler_inner(channel_id, "C", point_id, state, reload_query).await
}

/// Delete adjustment point
pub async fn delete_adjustment_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    delete_point_handler_inner(channel_id, "A", point_id, state, reload_query).await
}
