use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde_json;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;

use crate::api::models::{
    AdjustmentCommand, ApiResponse, ChannelOperation, ChannelStatus, ChannelStatusResponse,
    ControlCommand, HealthStatus, ServiceStatus,
};
use crate::core::combase::factory::ProtocolFactory;

/// Global service start time storage
static SERVICE_START_TIME: OnceLock<DateTime<Utc>> = OnceLock::new();

/// Set the service start time (should be called once at startup)
pub fn set_service_start_time(start_time: DateTime<Utc>) {
    let _ = SERVICE_START_TIME.set(start_time);
}

/// Get the service start time
pub fn get_service_start_time() -> DateTime<Utc> {
    *SERVICE_START_TIME.get().unwrap_or(&Utc::now())
}

// OpenAPI removed

/// Application state containing the protocol factory
#[derive(Clone, Debug)]
pub struct AppState {
    pub factory: Arc<RwLock<ProtocolFactory>>,
}

impl AppState {
    pub fn new(factory: Arc<RwLock<ProtocolFactory>>) -> Self {
        Self { factory }
    }
}

/// Get service status endpoint
pub async fn get_service_status(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<ServiceStatus>>, StatusCode> {
    let factory = state.factory.read().await;
    let total_channels = factory.channel_count();
    let active_channels = factory.running_channel_count().await;

    // Get actual service start time and calculate uptime
    let start_time = get_service_start_time();
    let uptime_duration = Utc::now() - start_time;
    let uptime_seconds = uptime_duration.num_seconds().max(0).try_into().unwrap_or(0);

    let status = ServiceStatus {
        name: "Communication Service".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime: uptime_seconds,
        start_time,
        channels: total_channels as u32,
        active_channels: active_channels as u32,
    };

    Ok(Json(ApiResponse::success(status)))
}

/// Health check endpoint
// OpenAPI path annotation removed
pub async fn health_check() -> Result<Json<ApiResponse<HealthStatus>>, StatusCode> {
    let health = HealthStatus {
        status: "healthy".to_string(),
        uptime: 3600,
        memory_usage: 1024 * 1024 * 100, // 100MB
        cpu_usage: 15.5,
    };

    Ok(Json(ApiResponse::success(health)))
}

/// List all channels
// OpenAPI path annotation removed
pub async fn get_all_channels(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ChannelStatusResponse>>>, StatusCode> {
    let factory = state.factory.read().await;

    // Get all channel IDs
    let channel_ids = factory.get_channel_ids();
    let mut channels = Vec::new();

    for channel_id in channel_ids {
        if let Some(channel) = factory.get_channel(channel_id).await {
            // Get real channel metadata
            let (name, protocol) = factory
                .get_channel_metadata(channel_id)
                .await
                .unwrap_or_else(|| (format!("Channel {channel_id}"), "Unknown".to_string()));

            // Get real channel status
            let channel_guard = channel.read().await;
            let status = channel_guard.get_status().await;

            let channel_response = ChannelStatusResponse {
                id: channel_id,
                name,
                protocol,
                connected: status.is_connected,
                last_update: DateTime::<Utc>::from_timestamp(status.last_update as i64, 0)
                    .unwrap_or_else(Utc::now),
                error_count: status.error_count as u32,
                last_error: status.last_error,
            };
            channels.push(channel_response);
        }
    }

    Ok(Json(ApiResponse::success(channels)))
}

/// Get channel status
// OpenAPI path annotation removed
pub async fn get_channel_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ChannelStatus>>, StatusCode> {
    let id_u16 = id.parse::<u16>().map_err(|_| StatusCode::BAD_REQUEST)?;
    let factory = state.factory.read().await;

    if let Some(channel) = factory.get_channel(id_u16).await {
        // Get real channel metadata
        let (name, protocol) = factory
            .get_channel_metadata(id_u16)
            .await
            .unwrap_or_else(|| (format!("Channel {id_u16}"), "Unknown".to_string()));

        // Get real channel status
        let channel_guard = channel.read().await;
        let channel_status = channel_guard.get_status().await;
        let is_running = channel_guard.is_connected();
        let diagnostics = channel_guard
            .get_diagnostics()
            .await
            .unwrap_or_else(|_| serde_json::json!({}));

        let status = ChannelStatus {
            id: id_u16,
            name,
            protocol,
            connected: channel_status.is_connected,
            running: is_running,
            last_update: DateTime::<Utc>::from_timestamp(channel_status.last_update as i64, 0)
                .unwrap_or_else(Utc::now),
            error_count: channel_status.error_count as u32,
            last_error: channel_status.last_error,
            statistics: diagnostics
                .as_object()
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
        };
        Ok(Json(ApiResponse::success(status)))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Control channel operation
// OpenAPI path annotation removed
pub async fn control_channel(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(operation): Json<ChannelOperation>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let id_u16 = id.parse::<u16>().map_err(|_| StatusCode::BAD_REQUEST)?;
    let factory = state.factory.read().await;

    // Check if channel exists and get the channel
    let channel = match factory.get_channel(id_u16).await {
        Some(ch) => ch,
        None => return Err(StatusCode::NOT_FOUND),
    };

    // Execute operation based on type
    let result = match operation.operation.as_str() {
        "start" => {
            let mut channel_guard = channel.write().await;
            match channel_guard.connect().await {
                Ok(()) => format!("Channel {id_u16} connected successfully"),
                Err(e) => format!("Failed to connect channel {id_u16}: {e}"),
            }
        }
        "stop" => {
            let mut channel_guard = channel.write().await;
            match channel_guard.disconnect().await {
                Ok(()) => format!("Channel {id_u16} disconnected successfully"),
                Err(e) => format!("Failed to disconnect channel {id_u16}: {e}"),
            }
        }
        "restart" => {
            let mut channel_guard = channel.write().await;
            // First stop the channel
            let stop_result = channel_guard.disconnect().await;
            if let Err(e) = stop_result {
                return Ok(Json(ApiResponse::success(format!(
                    "Failed to stop channel {id_u16} during restart: {e}"
                ))));
            }

            // Wait a moment before starting
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Then start it again
            match channel_guard.connect().await {
                Ok(()) => format!("Channel {id_u16} restarted successfully"),
                Err(e) => format!("Failed to restart channel {id_u16}: {e}"),
            }
        }
        _ => {
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    Ok(Json(ApiResponse::success(result)))
}


/// Send control command (遥控)
pub async fn send_control(
    State(state): State<AppState>,
    Path(channel_id): Path<u16>,
    Json(cmd): Json<ControlCommand>,
) -> Result<Json<ApiResponse<bool>>, StatusCode> {
    let factory = state.factory.read().await;
    
    if let Some(channel) = factory.get_channel(channel_id).await {
        let mut channel_guard = channel.write().await;
        let redis_value = crate::core::combase::RedisValue::Integer(cmd.value as i64);
        
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

/// Send adjustment command (遥调)
pub async fn send_adjustment(
    State(state): State<AppState>,
    Path(channel_id): Path<u16>,
    Json(cmd): Json<AdjustmentCommand>,
) -> Result<Json<ApiResponse<bool>>, StatusCode> {
    let factory = state.factory.read().await;
    
    if let Some(channel) = factory.get_channel(channel_id).await {
        let mut channel_guard = channel.write().await;
        let redis_value = crate::core::combase::RedisValue::Float(cmd.value);
        
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







/// Create the API router with all routes
pub fn create_api_routes(factory: Arc<RwLock<ProtocolFactory>>) -> Router {
    let state = AppState::new(factory);

    Router::new()
        // 服务管理
        .route("/api/status", get(get_service_status))
        .route("/api/health", get(health_check))
        
        // 通道管理
        .route("/api/channels", get(get_all_channels))
        .route("/api/channels/{id}/status", get(get_channel_status))
        .route("/api/channels/{id}/control", post(control_channel))
        
        // 控制命令（简化版）
        .route("/api/control/{channel_id}", post(send_control))
        .route("/api/adjustment/{channel_id}", post(send_adjustment))
        
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_routes() {
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));
        let _app = create_api_routes(factory);
        // Basic test to ensure routes compile
        // Test passes if code compiles
    }

    #[test]
    fn test_openapi_spec_generation() {
        // OpenAPI spec generation test removed - function not available
        // TODO: Add this test when OpenAPI spec generation is implemented
    }
}
