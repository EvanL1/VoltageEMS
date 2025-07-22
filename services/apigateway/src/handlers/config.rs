use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::auth::Claims;
use crate::data_access::{sync::ConfigChangeEvent, sync::ConfigChangeType, AccessOptions};
use crate::error::{ApiError, ApiResult};
use crate::response::success_response;
use crate::AppState;

/// 配置查询参数
#[derive(Debug, Deserialize)]
pub struct ConfigQuery {
    pub service: Option<String>,
    pub config_type: Option<String>,
    pub include_cache_stats: Option<bool>,
}

/// 配置项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigItem {
    pub key: String,
    pub value: Value,
    pub source: String, // "cache", "redis", "http"
    pub last_updated: Option<i64>,
    pub ttl: Option<u64>,
}

/// 配置更新请求
#[derive(Debug, Deserialize)]
pub struct ConfigUpdateRequest {
    pub value: Value,
    pub ttl: Option<u64>,
    pub sync_to_services: Option<bool>,
}

/// 配置统计信息
#[derive(Debug, Serialize)]
pub struct ConfigStats {
    pub total_configs: usize,
    pub cache_hit_rate: f64,
    pub services_synced: Vec<String>,
    pub last_sync_time: Option<i64>,
}

/// 获取配置列表
pub async fn list_configs(
    Query(query): Query<ConfigQuery>,
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    debug!("Getting config list with query: {:?}", query);

    let mut config_keys = Vec::new();

    // 根据查询参数构建键模式
    match (&query.service, &query.config_type) {
        (Some(service), Some(config_type)) => {
            let key = format!("cfg:{}:{}", service, config_type);
            config_keys.push(key);
        }
        (Some(service), None) => {
            // 获取某个服务的所有配置
            let pattern = format!("cfg:{}:*", service);
            match state.redis_client.keys(&pattern).await {
                Ok(keys) => config_keys.extend(keys),
                Err(e) => {
                    warn!("Failed to get config keys for service {}: {}", service, e);
                }
            }
        }
        (None, Some(config_type)) => {
            // 获取所有服务的某种类型配置
            let pattern = format!("cfg:*:{}", config_type);
            match state.redis_client.keys(&pattern).await {
                Ok(keys) => config_keys.extend(keys),
                Err(e) => {
                    warn!("Failed to get config keys for type {}: {}", config_type, e);
                }
            }
        }
        (None, None) => {
            // 获取所有配置
            let pattern = "cfg:*";
            match state.redis_client.keys(&pattern).await {
                Ok(keys) => config_keys.extend(keys),
                Err(e) => {
                    warn!("Failed to get all config keys: {}", e);
                }
            }
        }
    }

    // 批量获取配置
    let configs = state
        .data_access_layer
        .batch_get(config_keys.clone(), AccessOptions::cache_first())
        .await
        .map_err(|e| {
            warn!("Failed to get configs: {}", e);
            ApiError::ServiceError("Failed to get configurations".to_string())
        })?;

    // 构建配置项列表
    let mut config_items = Vec::new();
    for (i, config_opt) in configs.into_iter().enumerate() {
        if let Some(config_value) = config_opt {
            let config_item = ConfigItem {
                key: config_keys[i].clone(),
                value: config_value,
                source: "cache".to_string(), // 简化，实际可以跟踪来源
                last_updated: Some(chrono::Utc::now().timestamp()),
                ttl: Some(300), // 默认5分钟TTL
            };
            config_items.push(config_item);
        }
    }

    // 如果需要，添加缓存统计
    let mut response = serde_json::json!({
        "configs": config_items,
        "total": config_items.len()
    });

    if query.include_cache_stats.unwrap_or(false) {
        if let Ok(cache_stats) = state.data_access_layer.cache_stats().await {
            response["cache_stats"] = serde_json::json!({
                "hit_rate": cache_stats.hit_rate,
                "total_keys": cache_stats.total_keys,
                "hits": cache_stats.hits,
                "misses": cache_stats.misses
            });
        }
    }

    Ok(success_response(response))
}

/// 获取单个配置
pub async fn get_config(
    Path(config_key): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    debug!("Getting config for key: {}", config_key);

    // 确保键以cfg:开头
    let full_key = if config_key.starts_with("cfg:") {
        config_key
    } else {
        format!("cfg:{}", config_key)
    };

    match state
        .data_access_layer
        .get_data(&full_key, AccessOptions::cache_with_fallback())
        .await
    {
        Ok(config_value) => {
            let config_item = ConfigItem {
                key: full_key,
                value: config_value,
                source: "cache".to_string(),
                last_updated: Some(chrono::Utc::now().timestamp()),
                ttl: Some(300),
            };
            Ok(success_response(config_item))
        }
        Err(e) => {
            warn!("Failed to get config {}: {}", full_key, e);
            Err(ApiError::NotFound(format!("Config not found: {}", full_key)))
        }
    }
}

/// 更新配置
pub async fn update_config(
    Path(config_key): Path<String>,
    State(state): State<AppState>,
    claims: Option<axum::extract::Extension<Claims>>,
    Json(request): Json<ConfigUpdateRequest>,
) -> ApiResult<impl IntoResponse> {
    debug!("Updating config for key: {}", config_key);

    // 检查权限
    if let Some(axum::extract::Extension(ref claims)) = claims {
        if !claims.has_permission("config:write") {
            return Err(ApiError::Forbidden);
        }
    } else {
        return Err(ApiError::Unauthorized);
    }

    // 确保键以cfg:开头
    let full_key = if config_key.starts_with("cfg:") {
        config_key
    } else {
        format!("cfg:{}", config_key)
    };

    // 更新配置
    let options = AccessOptions {
        use_cache: true,
        cache_ttl: request.ttl,
        fallback_http: false,
        timeout: std::time::Duration::from_secs(10),
        data_type: crate::data_access::DataType::Config,
    };

    state
        .data_access_layer
        .set_data(&full_key, request.value.clone(), options)
        .await
        .map_err(|e| {
            warn!("Failed to update config {}: {}", full_key, e);
            ApiError::ServiceError(format!("Failed to update config: {}", e))
        })?;

    // 发布配置变更事件
    let change_event = ConfigChangeEvent {
        event_type: ConfigChangeType::Updated,
        service: extract_service_from_key(&full_key),
        config_key: full_key.clone(),
        timestamp: chrono::Utc::now().timestamp(),
        user: claims.map(|ext| ext.0.username),
        changes: Some(request.value.clone()),
    };

    if let Err(e) = state.config_sync_service.publish_config_change(change_event).await {
        warn!("Failed to publish config change event: {}", e);
    }

    // 如果需要，触发服务同步
    if request.sync_to_services.unwrap_or(false) {
        let service_name = extract_service_from_key(&full_key);
        if let Err(e) = state.config_sync_service.trigger_sync(&service_name).await {
            warn!("Failed to trigger sync for service {}: {}", service_name, e);
        }
    }

    info!("Config updated: {}", full_key);
    Ok(success_response(serde_json::json!({
        "success": true,
        "message": format!("Config {} updated successfully", full_key),
        "key": full_key,
        "timestamp": chrono::Utc::now().timestamp()
    })))
}

/// 删除配置
pub async fn delete_config(
    Path(config_key): Path<String>,
    State(state): State<AppState>,
    claims: Option<axum::extract::Extension<Claims>>,
) -> ApiResult<impl IntoResponse> {
    debug!("Deleting config for key: {}", config_key);

    // 检查权限
    if let Some(axum::extract::Extension(ref claims)) = claims {
        if !claims.has_permission("config:write") {
            return Err(ApiError::Forbidden);
        }
    } else {
        return Err(ApiError::Unauthorized);
    }

    // 确保键以cfg:开头
    let full_key = if config_key.starts_with("cfg:") {
        config_key
    } else {
        format!("cfg:{}", config_key)
    };

    // 删除配置
    state
        .data_access_layer
        .delete(&full_key)
        .await
        .map_err(|e| {
            warn!("Failed to delete config {}: {}", full_key, e);
            ApiError::ServiceError(format!("Failed to delete config: {}", e))
        })?;

    // 发布配置变更事件
    let change_event = ConfigChangeEvent {
        event_type: ConfigChangeType::Deleted,
        service: extract_service_from_key(&full_key),
        config_key: full_key.clone(),
        timestamp: chrono::Utc::now().timestamp(),
        user: claims.map(|ext| ext.0.username),
        changes: None,
    };

    if let Err(e) = state.config_sync_service.publish_config_change(change_event).await {
        warn!("Failed to publish config change event: {}", e);
    }

    info!("Config deleted: {}", full_key);
    Ok(success_response(serde_json::json!({
        "success": true,
        "message": format!("Config {} deleted successfully", full_key),
        "key": full_key,
        "timestamp": chrono::Utc::now().timestamp()
    })))
}

/// 触发配置同步
pub async fn trigger_sync(
    Path(service_name): Path<String>,
    State(state): State<AppState>,
    claims: Option<axum::extract::Extension<Claims>>,
) -> ApiResult<impl IntoResponse> {
    debug!("Triggering sync for service: {}", service_name);

    // 检查权限
    if let Some(axum::extract::Extension(claims)) = claims {
        if !claims.has_permission("config:admin") {
            return Err(ApiError::Forbidden);
        }
    } else {
        return Err(ApiError::Unauthorized);
    }

    // 触发同步
    state
        .config_sync_service
        .trigger_sync(&service_name)
        .await
        .map_err(|e| {
            warn!("Failed to trigger sync for service {}: {}", service_name, e);
            ApiError::ServiceError(format!("Failed to trigger sync: {}", e))
        })?;

    info!("Sync triggered for service: {}", service_name);
    Ok(success_response(serde_json::json!({
        "success": true,
        "message": format!("Sync triggered for service {}", service_name),
        "service": service_name,
        "timestamp": chrono::Utc::now().timestamp()
    })))
}

/// 获取同步状态
pub async fn get_sync_status(
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    debug!("Getting sync status");

    let sync_stats = state.config_sync_service.get_sync_stats().await;
    let services = state.config_sync_service.get_services().await;

    let status = serde_json::json!({
        "sync_stats": {
            "total_syncs": sync_stats.total_syncs,
            "successful_syncs": sync_stats.successful_syncs,
            "failed_syncs": sync_stats.failed_syncs,
            "last_sync_time": sync_stats.last_sync_time.map(|t| t.elapsed().as_secs()),
            "avg_sync_duration_ms": sync_stats.avg_sync_duration.as_millis()
        },
        "services": services.iter().map(|s| serde_json::json!({
            "name": s.service_name,
            "url": s.url,
            "sync_enabled": s.sync_enabled,
            "last_sync": s.last_sync.map(|t| t.elapsed().as_secs()),
            "sync_interval_secs": s.sync_interval.as_secs()
        })).collect::<Vec<_>>()
    });

    Ok(success_response(status))
}

/// 清理缓存
pub async fn clear_cache(
    Query(query): Query<ConfigQuery>,
    State(state): State<AppState>,
    claims: Option<axum::extract::Extension<Claims>>,
) -> ApiResult<impl IntoResponse> {
    debug!("Clearing cache with query: {:?}", query);

    // 检查权限
    if let Some(axum::extract::Extension(claims)) = claims {
        if !claims.has_permission("config:admin") {
            return Err(ApiError::Forbidden);
        }
    } else {
        return Err(ApiError::Unauthorized);
    }

    // 构建清理模式
    let pattern = match (&query.service, &query.config_type) {
        (Some(service), Some(config_type)) => format!("cfg:{}:{}", service, config_type),
        (Some(service), None) => format!("cfg:{}:*", service),
        (None, Some(config_type)) => format!("cfg:*:{}", config_type),
        (None, None) => "cfg:*".to_string(),
    };

    let cleared_count = state
        .data_access_layer
        .clear_cache(&pattern)
        .await
        .map_err(|e| {
            warn!("Failed to clear cache with pattern {}: {}", pattern, e);
            ApiError::ServiceError(format!("Failed to clear cache: {}", e))
        })?;

    info!("Cache cleared: {} items removed with pattern {}", cleared_count, pattern);
    Ok(success_response(serde_json::json!({
        "success": true,
        "message": format!("Cache cleared: {} items removed", cleared_count),
        "pattern": pattern,
        "cleared_count": cleared_count,
        "timestamp": chrono::Utc::now().timestamp()
    })))
}

/// 从配置键中提取服务名
fn extract_service_from_key(key: &str) -> String {
    if let Some(stripped) = key.strip_prefix("cfg:") {
        if let Some(colon_pos) = stripped.find(':') {
            stripped[..colon_pos].to_string()
        } else {
            stripped.to_string()
        }
    } else {
        "unknown".to_string()
    }
}