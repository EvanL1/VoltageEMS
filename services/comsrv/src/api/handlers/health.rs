//! Health Check and Service Status Handlers
//!
//! Provides endpoints for monitoring service health and operational status.

use axum::{extract::State, response::Json};
use chrono::Utc;
use common::system_metrics::SystemMetrics;

use crate::api::routes::{get_service_start_time, AppState};
use crate::dto::{AppError, HealthStatus, ServiceStatus, SuccessResponse};

/// Get service status endpoint
///
/// @route GET /api/status
/// @input State(state): AppState - Application state with factory
/// @output `Json<SuccessResponse<ServiceStatus>>` - Service status including channels
/// @status 200 - Success with {total_channels, active_channels, uptime, version}
/// @status 500 - Internal server error
#[utoipa::path(
    get,
    path = "/api/status",
    responses(
        (status = 200, description = "Service status retrieved", body = crate::dto::ServiceStatus)
    ),
    tag = "comsrv"
)]
pub async fn get_service_status(
    State(state): State<AppState>,
) -> Result<Json<SuccessResponse<ServiceStatus>>, AppError> {
    let manager = state.channel_manager.read().await;
    let total_channels = manager.channel_count();
    let active_channels = manager.running_channel_count().await;

    // Get actual service start time and calculate uptime
    let start_time = get_service_start_time();
    let uptime_duration = Utc::now() - start_time;
    let uptime_seconds = uptime_duration.num_seconds().max(0).try_into().unwrap_or(0);

    let status = ServiceStatus {
        name: "Communication Service".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime: uptime_seconds,
        start_time,
        channels: u32::try_from(total_channels).unwrap_or(u32::MAX),
        active_channels: u32::try_from(active_channels).unwrap_or(u32::MAX),
    };

    Ok(Json(SuccessResponse::new(status)))
}

/// Health check endpoint
///
/// @route GET /health
/// @input None
/// @output `Json<SuccessResponse<HealthStatus>>` - Health status metrics
/// @status 200 - Service is healthy
/// @status 503 - Service is unhealthy
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy"),
        (status = 503, description = "Service is unhealthy")
    ),
    tag = "comsrv"
)]
pub async fn health_check() -> Result<Json<SuccessResponse<HealthStatus>>, AppError> {
    // Get actual uptime from service start time
    let start_time = get_service_start_time();
    let uptime_duration = Utc::now() - start_time;
    let uptime_seconds: u64 = uptime_duration.num_seconds().max(0).try_into().unwrap_or(0);

    // Collect system metrics (CPU, memory)
    let metrics = SystemMetrics::collect();

    let health = HealthStatus {
        status: common::ServiceStatus::Healthy,
        service: "comsrv".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds,
        timestamp: Utc::now(),
        checks: std::collections::HashMap::new(),
        system: Some(serde_json::to_value(&metrics).unwrap_or_default()),
    };

    Ok(Json(SuccessResponse::new(health)))
}
