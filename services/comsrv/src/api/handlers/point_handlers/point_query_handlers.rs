#![allow(clippy::disallowed_methods)]

//! Query handlers for point information, configuration, and unmapped points

use crate::api::routes::AppState;
use crate::dto::{AppError, SuccessResponse};
use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use voltage_model::{KeySpaceConfig, PointType};
use voltage_rtdb::Rtdb;

use super::point_helpers::{
    fetch_grouped_points, parse_protocol_mapping_json, point_type_to_table, validate_channel_exists,
};

/// 读单个点的实时值（value + ts + raw）。
///
/// 从 Redis hash 读 `comsrv:{channel_id}:{T|S|C|A}` 对应 field，返回工
/// 程量值（经过线性变换后）、时间戳、原始寄存器值。**实时性以 Redis
/// 为准**，跟 SHM 实时层有 ~100ms 滞后（ShmRedisSync 异步同步周期）。
/// 405/406 表示点位定义不存在；数据为 NaN（未首次成功 poll 或离线）时
/// 返回 value=null。
#[utoipa::path(
    get,
    path = "/api/channels/{channel_id}/{telemetry_type}/{point_id}",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("telemetry_type" = String, Path, description = "Point type: T, S, C, or A"),
        ("point_id" = u32, Path, description = "Point identifier")
    ),
    responses(
        (status = 200, description = "Point information", body = serde_json::Value,
            example = json!({
                "success": true,
                "data": {
                    "channel_id": 1,
                    "telemetry_type": "T",
                    "point_id": 101,
                    "value": "650.5",
                    "timestamp": "1729000815",
                    "raw": "6505"
                }
            })
        )
    ),
    tag = "comsrv"
)]
pub async fn get_point_info_handler<R: Rtdb>(
    State(state): State<AppState<R>>,
    Path((channel_id, telemetry_type, point_id)): Path<(u32, String, u32)>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, AppError> {
    let point_type = PointType::from_str(&telemetry_type).ok_or_else(|| {
        AppError::bad_request(format!(
            "Invalid telemetry type '{}'. Must be T, S, C, or A",
            telemetry_type
        ))
    })?;

    let keyspace = KeySpaceConfig::production_cached();
    let field = point_id.to_string();
    let data_key = keyspace.channel_key(channel_id, point_type);
    let ts_key = keyspace.channel_ts_key(channel_id, point_type);
    let raw_key = keyspace.channel_raw_key(channel_id, point_type);

    let rtdb = &state.rtdb;
    let value = rtdb
        .hash_get(&data_key, &field)
        .await
        .map(|opt| opt.map(|b| String::from_utf8_lossy(&b).to_string()))
        .map_err(|e| AppError::internal_error(format!("Failed to read value: {}", e)))?;

    let timestamp = rtdb
        .hash_get(&ts_key, &field)
        .await
        .map(|opt| opt.map(|b| String::from_utf8_lossy(&b).to_string()))
        .map_err(|e| AppError::internal_error(format!("Failed to read timestamp: {}", e)))?;

    let raw_value = rtdb
        .hash_get(&raw_key, &field)
        .await
        .map(|opt| opt.map(|b| String::from_utf8_lossy(&b).to_string()))
        .map_err(|e| AppError::internal_error(format!("Failed to read raw value: {}", e)))?;

    Ok(Json(SuccessResponse::new(serde_json::json!({
        "channel_id": channel_id,
        "telemetry_type": point_type.as_str(),
        "point_id": point_id,
        "value": value,
        "timestamp": timestamp,
        "raw": raw_value,
        "source": "redis"
    }))))
}

/// Get list of points for a channel, optionally filtered by type
///
/// Returns all point definitions for the specified channel.
/// Supports filtering by point type (T, S, C, A).
#[utoipa::path(
    get,
    path = "/api/channels/{id}/points",
    params(
        ("id" = u32, Path, description = "Channel identifier"),
        ("type" = Option<String>, Query, description = "Point type filter: T (telemetry), S (signal), C (control), A (adjustment)")
    ),
    responses(
        (status = 200, description = "Points retrieved (grouped)", body = crate::dto::GroupedPoints,
            example = json!({
                "success": true,
                "data": {
                    "telemetry": [
                        {
                            "point_id": 101,
                            "signal_name": "DC_Voltage",
                            "scale": 0.1,
                            "offset": 0.0,
                            "unit": "V",
                            "data_type": "uint16",
                            "reverse": false,
                            "description": "DC bus voltage",
                            "protocol_mapping": {
                                "slave_id": 1,
                                "function_code": 3,
                                "register_address": 100,
                                "data_type": "float32",
                                "byte_order": "ABCD",
                                "bit_position": 0
                            }
                        }
                    ],
                    "signal": [],
                    "control": [],
                    "adjustment": []
                }
            })
        )
    ),
    tag = "comsrv"
)]
pub async fn get_channel_points_handler<R: Rtdb>(
    Path(channel_id): Path<u32>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::GroupedPoints>>, AppError> {
    validate_channel_exists(&state.sqlite_pool, channel_id).await?;
    let type_filter = params.get("type").map(|s| s.as_str());
    let grouped = fetch_grouped_points(&state.sqlite_pool, channel_id, type_filter, false).await?;
    Ok(Json(SuccessResponse::new(grouped)))
}

/// Get mapping for a specific point with explicit four-remote type
///
/// Unique identifier: (channel_id, four_remote_type, point_id)
#[utoipa::path(
    get,
    path = "/api/channels/{channel_id}/{type}/points/{point_id}/mapping",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("type" = String, Path, description = "Four-remote type: T(Telemetry), S(Signal), C(Control), A(Adjustment)"),
        ("point_id" = u32, Path, description = "Point identifier")
    ),
    responses(
        (status = 200, description = "Mapping retrieved successfully", body = crate::dto::PointMappingDetail),
        (status = 400, description = "Invalid four-remote type (must be T, S, C, or A)"),
        (status = 404, description = "Channel or point not found in specified type")
    ),
    tag = "comsrv"
)]
pub async fn get_point_mapping_with_type_handler<R: Rtdb>(
    Path((channel_id, point_type, point_id)): Path<(u32, String, u32)>,
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::PointMappingDetail>>, AppError> {
    let table = point_type_to_table(&point_type)?;
    validate_channel_exists(&state.sqlite_pool, channel_id).await?;

    let query = format!(
        "SELECT signal_name, protocol_mappings FROM {} WHERE channel_id = ? AND point_id = ?",
        table
    );

    let result: Option<(String, Option<String>)> = sqlx::query_as(&query)
        .bind(channel_id as i64)
        .bind(point_id as i64)
        .fetch_optional(&state.sqlite_pool)
        .await
        .map_err(|e| {
            tracing::error!("Query {}: {}", table, e);
            AppError::internal_error("Database operation failed")
        })?;

    let (signal_name, protocol_mappings_json) = result.ok_or_else(|| {
        AppError::not_found(format!(
            "Point {} (type {}) not found in channel {}",
            point_id,
            point_type.to_uppercase(),
            channel_id
        ))
    })?;

    // For mapping endpoints, default to empty object instead of None
    let protocol_data = parse_protocol_mapping_json(protocol_mappings_json.as_deref())
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));

    Ok(Json(SuccessResponse::new(crate::dto::PointMappingDetail {
        point_id,
        signal_name,
        protocol_data,
    })))
}

// ----------------------------------------------------------------------------
// Get Point Configuration Handler
// ----------------------------------------------------------------------------

/// 读点位的**配置**（不是运行时值）。
///
/// 从 SQLite 读点位定义：寄存器地址、字节序、缩放系数、单位、报警上下
/// 限等。不查 Redis、不查 SHM，只读静态配置。前端"编辑点位"对话框预填
/// 用；实时值走 `/api/channels/{id}/points/{point_id}`。
#[utoipa::path(
    get,
    path = "/api/channels/{channel_id}/{type}/points/{point_id}/config",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("type" = String, Path, description = "Point type: T, S, C, or A"),
        ("point_id" = u32, Path, description = "Point identifier")
    ),
    responses(
        (status = 200, description = "Point configuration", body = crate::dto::PointDefinition),
        (status = 400, description = "Invalid point type"),
        (status = 404, description = "Channel or point not found")
    ),
    tag = "comsrv"
)]
async fn get_point_config_handler_inner<R: Rtdb>(
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    state: AppState<R>,
) -> Result<Json<SuccessResponse<crate::dto::PointDefinition>>, AppError> {
    let table = point_type_to_table(point_type)?;
    validate_channel_exists(&state.sqlite_pool, channel_id).await?;

    let query = format!(
        "SELECT point_id, signal_name, scale, offset, unit, data_type, reverse, \
         description, protocol_mappings \
         FROM {} WHERE channel_id = ? AND point_id = ?",
        table
    );

    #[allow(clippy::type_complexity)]
    let result: Option<(
        u32,
        String,
        f64,
        f64,
        String,
        String,
        bool,
        String,
        Option<String>,
    )> = sqlx::query_as(&query)
        .bind(channel_id as i64)
        .bind(point_id as i64)
        .fetch_optional(&state.sqlite_pool)
        .await
        .map_err(|e| {
            tracing::error!("Query point config: {}", e);
            AppError::internal_error("Database operation failed")
        })?;

    let (pt_id, signal_name, scale, offset, unit, data_type, reverse, description, pm_json) =
        result.ok_or_else(|| {
            AppError::not_found(format!(
                "Point {} (type {}) not found in channel {}",
                point_id,
                point_type.to_ascii_uppercase(),
                channel_id
            ))
        })?;

    Ok(Json(SuccessResponse::new(crate::dto::PointDefinition {
        point_id: pt_id,
        signal_name,
        scale,
        offset,
        unit,
        data_type,
        reverse,
        description,
        protocol_mapping: parse_protocol_mapping_json(pm_json.as_deref()),
    })))
}

// ============================================================================
// Unmapped Points Query Handler
// ============================================================================

/// Get unmapped points for a channel (points without protocol_mappings)
///
/// **Unmapped Definition**: Points where `protocol_mappings IS NULL OR '' OR '{}' OR 'null'`
#[utoipa::path(
    get,
    path = "/api/channels/{id}/unmapped-points",
    params(
        ("id" = u32, Path, description = "Channel identifier"),
        ("type" = Option<String>, Query, description = "Point type filter: T (telemetry), S (signal), C (control), A (adjustment)")
    ),
    responses(
        (status = 200, description = "Unmapped points retrieved (grouped by type)", body = crate::dto::GroupedPoints,
            example = json!({
                "success": true,
                "data": {
                    "telemetry": [
                        {
                            "point_id": 101,
                            "signal_name": "DC_Voltage",
                            "scale": 0.1,
                            "offset": 0.0,
                            "unit": "V",
                            "data_type": "uint16",
                            "reverse": false,
                            "description": "DC bus voltage",
                            "protocol_mapping": null
                        }
                    ],
                    "signal": [],
                    "control": [],
                    "adjustment": []
                }
            })
        )
    ),
    tag = "comsrv"
)]
pub async fn get_unmapped_points_handler<R: Rtdb>(
    Path(channel_id): Path<u32>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::GroupedPoints>>, AppError> {
    validate_channel_exists(&state.sqlite_pool, channel_id).await?;
    let type_filter = params.get("type").map(|s| s.as_str());
    let grouped = fetch_grouped_points(&state.sqlite_pool, channel_id, type_filter, true).await?;
    Ok(Json(SuccessResponse::new(grouped)))
}

// ============================================================================
// Type-specific GET wrapper handlers (delegate to *_inner functions)
// ============================================================================

/// Get telemetry point configuration
pub async fn get_telemetry_point_config_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::PointDefinition>>, AppError> {
    get_point_config_handler_inner(channel_id, "T", point_id, state).await
}

/// Get signal point configuration
pub async fn get_signal_point_config_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::PointDefinition>>, AppError> {
    get_point_config_handler_inner(channel_id, "S", point_id, state).await
}

/// Get control point configuration
pub async fn get_control_point_config_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::PointDefinition>>, AppError> {
    get_point_config_handler_inner(channel_id, "C", point_id, state).await
}

/// Get adjustment point configuration
pub async fn get_adjustment_point_config_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::PointDefinition>>, AppError> {
    get_point_config_handler_inner(channel_id, "A", point_id, state).await
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod cache_tests {
    use super::*;
    use axum::extract::{Path, State};
    use std::sync::Arc;
    use voltage_rtdb::{Bytes, MemoryRtdb};

    use crate::api::routes::AppState;
    use crate::core::channels::ChannelManager;
    use voltage_routing::RoutingCache;

    async fn create_test_sqlite_pool() -> sqlx::SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        common::test_utils::schema::init_comsrv_schema(&pool)
            .await
            .unwrap();

        pool
    }

    async fn create_test_state(rtdb: Arc<MemoryRtdb>) -> AppState<MemoryRtdb> {
        let sqlite_pool = create_test_sqlite_pool().await;
        let routing_cache = Arc::new(RoutingCache::default());
        let channel_manager = Arc::new(ChannelManager::new(rtdb.clone(), routing_cache));
        let command_tx_cache = Arc::new(crate::api::command_cache::CommandTxCache::new());

        AppState {
            channel_manager,
            rtdb,
            sqlite_pool,
            command_tx_cache,
            warning_stats: None,
        }
    }

    #[tokio::test]
    async fn test_get_point_info_from_redis() {
        let rtdb = Arc::new(MemoryRtdb::new());
        let channel_id: u32 = 1;
        let point_id: u32 = 102;

        let keyspace = KeySpaceConfig::production_cached();
        let data_key = keyspace.channel_key(channel_id, PointType::Telemetry);
        let ts_key = keyspace.channel_ts_key(channel_id, PointType::Telemetry);
        let raw_key = keyspace.channel_raw_key(channel_id, PointType::Telemetry);
        let field = point_id.to_string();

        rtdb.hash_set(&data_key, &field, Bytes::from("750.0"))
            .await
            .unwrap();
        rtdb.hash_set(&ts_key, &field, Bytes::from("1729001000"))
            .await
            .unwrap();
        rtdb.hash_set(&raw_key, &field, Bytes::from("7500"))
            .await
            .unwrap();

        let state = create_test_state(rtdb).await;

        let result =
            get_point_info_handler(State(state), Path((channel_id, "T".to_string(), point_id)))
                .await;

        let response = result.expect("Handler should succeed");
        let data = &response.0.data;
        assert_eq!(data["source"], "redis");
        assert_eq!(data["value"], "750.0");
    }

    #[tokio::test]
    async fn test_get_point_info_invalid_type() {
        let rtdb = Arc::new(MemoryRtdb::new());
        let state = create_test_state(rtdb).await;

        let result = get_point_info_handler(
            State(state),
            Path((1, "X".to_string(), 100)), // "X" is invalid
        )
        .await;

        let err = result.expect_err("Should return error for invalid type");
        assert!(
            format!("{:?}", err).contains("Invalid telemetry type"),
            "Error should mention invalid type"
        );
    }
}
