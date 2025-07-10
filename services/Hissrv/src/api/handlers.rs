use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;
use utoipa::{IntoParams, ToSchema};

use crate::api::{models::*, models_history::HistoryQueryFilter, AppState};

#[derive(Deserialize, ToSchema, IntoParams)]
pub struct KeysQuery {
    /// 键匹配模式
    pattern: Option<String>,
}

#[derive(Deserialize, ToSchema, IntoParams)]
pub struct DeleteQuery {
    /// 键匹配模式
    key_pattern: Option<String>,
    /// 开始时间
    start_time: Option<chrono::DateTime<Utc>>,
    /// 结束时间
    end_time: Option<chrono::DateTime<Utc>>,
}

/// 查询历史数据
#[utoipa::path(
    get,
    path = "/history",
    tag = "history",
    params(HistoryQueryFilter),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<HistoryQueryResult>),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    )
)]
pub async fn query_history(
    State(state): State<AppState>,
    Query(filter): Query<ApiQueryFilter>,
) -> Result<Json<ApiResponse<ApiQueryResult>>, (StatusCode, Json<ErrorResponse>)> {
    let storage_manager = state.storage_manager.read().await;
    let query_filter = filter.into();

    // Use default storage backend for querying
    if let Some(backend) = storage_manager.get_backend_readonly(None) {
        match backend.query_data_points(&query_filter).await {
            Ok(result) => {
                let api_result: ApiQueryResult = result.into();
                Ok(Json(ApiResponse::success(api_result)))
            }
            Err(e) => {
                tracing::error!("Failed to query data points: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "查询数据失败".to_string(),
                        code: "QUERY_ERROR".to_string(),
                        timestamp: Utc::now(),
                    }),
                ))
            }
        }
    } else {
        Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "未找到存储后端".to_string(),
                code: "NO_BACKEND".to_string(),
                timestamp: Utc::now(),
            }),
        ))
    }
}

/// 存储数据点
#[utoipa::path(
    post,
    path = "/data",
    tag = "data",
    request_body = ApiDataPoint,
    responses(
        (status = 201, description = "存储成功", body = ApiResponse<String>),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    )
)]
pub async fn store_data_point(
    State(state): State<AppState>,
    Json(api_data_point): Json<ApiDataPoint>,
) -> Result<Json<ApiResponse<String>>, (StatusCode, Json<ErrorResponse>)> {
    let mut storage_manager = state.storage_manager.write().await;
    let data_point: crate::storage::DataPoint = api_data_point.into();

    // Determine which backend to use based on the key
    let backend_name = determine_storage_backend(&data_point.key);

    if let Some(backend) = storage_manager.get_backend(Some(&backend_name)) {
        match backend.store_data_point(&data_point).await {
            Ok(_) => Ok(Json(ApiResponse::success_with_message(
                "数据存储成功".to_string(),
                format!("数据点已存储到 {} 后端", backend_name),
            ))),
            Err(e) => {
                tracing::error!("Failed to store data point: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "存储数据失败".to_string(),
                        code: "STORE_ERROR".to_string(),
                        timestamp: Utc::now(),
                    }),
                ))
            }
        }
    } else {
        Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("未找到存储后端: {}", backend_name),
                code: "NO_BACKEND".to_string(),
                timestamp: Utc::now(),
            }),
        ))
    }
}

/// 删除数据点
#[utoipa::path(
    delete,
    path = "/data",
    tag = "data",
    params(DeleteQuery),
    responses(
        (status = 200, description = "删除成功", body = ApiResponse<u64>),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    )
)]
pub async fn delete_data_points(
    State(state): State<AppState>,
    Query(query): Query<DeleteQuery>,
) -> Result<Json<ApiResponse<u64>>, (StatusCode, Json<ErrorResponse>)> {
    let mut storage_manager = state.storage_manager.write().await;

    let filter = crate::storage::QueryFilter {
        key_pattern: query.key_pattern,
        start_time: query.start_time,
        end_time: query.end_time,
        tags: HashMap::new(),
        limit: None,
        offset: None,
    };

    // Use default storage backend for deletion
    if let Some(backend) = storage_manager.get_backend(None) {
        match backend.delete_data_points(&filter).await {
            Ok(deleted_count) => Ok(Json(ApiResponse::success_with_message(
                deleted_count,
                format!("已删除 {} 个数据点", deleted_count),
            ))),
            Err(e) => {
                tracing::error!("Failed to delete data points: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "删除数据失败".to_string(),
                        code: "DELETE_ERROR".to_string(),
                        timestamp: Utc::now(),
                    }),
                ))
            }
        }
    } else {
        Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "未找到存储后端".to_string(),
                code: "NO_BACKEND".to_string(),
                timestamp: Utc::now(),
            }),
        ))
    }
}

/// 获取数据键列表
#[utoipa::path(
    get,
    path = "/data/keys",
    tag = "data",
    params(KeysQuery),
    responses(
        (status = 200, description = "获取成功", body = ApiResponse<Vec<String>>),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    )
)]
pub async fn get_keys(
    State(state): State<AppState>,
    Query(query): Query<KeysQuery>,
) -> Result<Json<ApiResponse<Vec<String>>>, (StatusCode, Json<ErrorResponse>)> {
    let storage_manager = state.storage_manager.read().await;

    // Use default storage backend
    if let Some(backend) = storage_manager.get_backend_readonly(None) {
        match backend.get_keys(query.pattern.as_deref()).await {
            Ok(keys) => Ok(Json(ApiResponse::success_with_message(
                keys.clone(),
                format!("找到 {} 个数据键", keys.len()),
            ))),
            Err(e) => {
                tracing::error!("Failed to get keys: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "获取数据键失败".to_string(),
                        code: "KEYS_ERROR".to_string(),
                        timestamp: Utc::now(),
                    }),
                ))
            }
        }
    } else {
        Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "未找到存储后端".to_string(),
                code: "NO_BACKEND".to_string(),
                timestamp: Utc::now(),
            }),
        ))
    }
}

/// 获取存储统计信息
#[utoipa::path(
    get,
    path = "/admin/statistics",
    tag = "admin",
    responses(
        (status = 200, description = "获取成功", body = ApiResponse<Vec<StorageStatistics>>),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    )
)]
pub async fn get_statistics(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<StorageStatistics>>>, (StatusCode, Json<ErrorResponse>)> {
    let storage_manager = state.storage_manager.read().await;

    let all_stats = storage_manager.get_all_statistics().await;
    let mut statistics = Vec::new();

    for (backend_name, stats) in all_stats {
        let mut api_stats: StorageStatistics = stats.into();
        api_stats.backend_name = backend_name;
        statistics.push(api_stats);
    }

    Ok(Json(ApiResponse::success_with_message(
        statistics,
        "统计信息获取成功".to_string(),
    )))
}

/// 获取服务配置
#[utoipa::path(
    get,
    path = "/admin/config",
    tag = "admin",
    responses(
        (status = 200, description = "获取成功", body = ApiResponse<serde_json::Value>),
    )
)]
pub async fn get_config(State(state): State<AppState>) -> Json<ApiResponse<serde_json::Value>> {
    // Return sanitized config (without sensitive data)
    let mut config_json = serde_json::to_value(&*state.config).unwrap_or_default();

    // Remove sensitive information
    if let Some(redis) = config_json.get_mut("redis") {
        if let Some(connection) = redis.get_mut("connection") {
            if let Some(password) = connection.get_mut("password") {
                *password = serde_json::Value::String("***".to_string());
            }
        }
    }

    if let Some(storage) = config_json.get_mut("storage") {
        if let Some(backends) = storage.get_mut("backends") {
            if let Some(influxdb) = backends.get_mut("influxdb") {
                if let Some(password) = influxdb.get_mut("password") {
                    *password = serde_json::Value::String("***".to_string());
                }
            }
            if let Some(postgresql) = backends.get_mut("postgresql") {
                if let Some(password) = postgresql.get_mut("password") {
                    *password = serde_json::Value::String("***".to_string());
                }
            }
        }
    }

    Json(ApiResponse::success_with_message(
        config_json,
        "配置信息获取成功".to_string(),
    ))
}

/// 健康检查
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "服务健康", body = ApiResponse<HealthStatus>),
    )
)]
pub async fn health_check(State(state): State<AppState>) -> Json<ApiResponse<HealthStatus>> {
    let storage_manager = state.storage_manager.read().await;
    let all_stats = storage_manager.get_all_statistics().await;

    let mut storage_backends = HashMap::new();
    for (backend_name, stats) in all_stats {
        storage_backends.insert(backend_name, stats.connection_status);
    }

    let health_status = HealthStatus {
        status: "healthy".to_string(),
        version: state.config.service.version.clone(),
        uptime: 0, // TODO: Calculate actual uptime
        storage_backends,
    };

    Json(ApiResponse::success(health_status))
}

fn determine_storage_backend(key: &str) -> String {
    // Simple logic - can be made configurable based on config rules
    if key.starts_with("temp:") || key.starts_with("sensor:") {
        "influxdb".to_string()
    } else if key.starts_with("logs:") || key.starts_with("events:") {
        "redis".to_string()
    } else {
        "influxdb".to_string() // Default
    }
}
