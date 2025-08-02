//! Simple API for AlarmSrv
//!
//! This module provides a streamlined HTTP API using Axum, combining handlers and routes.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::alarm_service::{AlarmLevel, AlarmQuery, AlarmService, AlarmStatus};
use crate::error::{AlarmError, Result};

/// Application state
#[derive(Clone)]
pub struct AppState {
    pub alarm_service: Arc<AlarmService>,
}

/// Request to create a new alarm
#[derive(Debug, Deserialize)]
pub struct CreateAlarmRequest {
    pub title: String,
    pub description: String,
    pub level: AlarmLevel,
    pub source: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// Request to acknowledge/resolve an alarm
#[derive(Debug, Deserialize)]
pub struct AlarmActionRequest {
    pub user: String,
}

/// Query parameters for alarm listing
#[derive(Debug, Deserialize)]
pub struct AlarmQueryParams {
    pub status: Option<String>,
    pub level: Option<String>,
    pub source: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl From<AlarmQueryParams> for AlarmQuery {
    fn from(params: AlarmQueryParams) -> Self {
        Self {
            status: params.status.and_then(|s| match s.as_str() {
                "New" => Some(AlarmStatus::New),
                "Acknowledged" => Some(AlarmStatus::Acknowledged),
                "Resolved" => Some(AlarmStatus::Resolved),
                _ => None,
            }),
            level: params.level.and_then(|l| match l.as_str() {
                "Critical" => Some(AlarmLevel::Critical),
                "Major" => Some(AlarmLevel::Major),
                "Minor" => Some(AlarmLevel::Minor),
                "Warning" => Some(AlarmLevel::Warning),
                "Info" => Some(AlarmLevel::Info),
                _ => None,
            }),
            source: params.source,
            start_time: None, // TODO: Add time range query support
            end_time: None,
            limit: params.limit,
            offset: params.offset,
        }
    }
}

/// Health check endpoint
pub async fn health_check(State(state): State<AppState>) -> Result<Json<Value>> {
    debug!("Health check requested");

    let is_healthy = state.alarm_service.health_check().await.unwrap_or(false);

    let response = json!({
        "status": if is_healthy { "healthy" } else { "unhealthy" },
        "service": "alarmsrv",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    if is_healthy {
        Ok(Json(response))
    } else {
        Err(AlarmError::Internal("Service unhealthy".to_string()))
    }
}

/// Get service status
pub async fn get_status(State(state): State<AppState>) -> Result<Json<Value>> {
    debug!("Status requested");

    let stats = state.alarm_service.get_statistics().await?;

    let response = json!({
        "service": "alarmsrv",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "running",
        "statistics": {
            "total_alarms": stats.total,
            "active_alarms": stats.active,
            "new": stats.new,
            "acknowledged": stats.acknowledged,
            "resolved": stats.resolved
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    Ok(Json(response))
}

/// List alarms with optional filtering
pub async fn list_alarms(
    State(state): State<AppState>,
    Query(params): Query<AlarmQueryParams>,
) -> Result<Json<Value>> {
    debug!("Listing alarms with params: {:?}", params);

    let query = AlarmQuery::from(params);
    let result = state.alarm_service.query_alarms(&query).await?;

    let response = json!({
        "total": result.total,
        "offset": result.offset,
        "limit": result.limit,
        "data": result.data
    });

    Ok(Json(response))
}

/// Create a new alarm
pub async fn create_alarm(
    State(state): State<AppState>,
    Json(request): Json<CreateAlarmRequest>,
) -> Result<Json<Value>> {
    info!("Creating alarm: {}", request.title);

    let mut alarm = if let Some(source) = request.source {
        crate::alarm_service::Alarm::with_source(
            request.title,
            request.description,
            request.level,
            source,
        )
    } else {
        crate::alarm_service::Alarm::new(request.title, request.description, request.level)
    };

    // Add tags if provided
    if let Some(tags) = request.tags {
        alarm.tags = tags;
    }

    state.alarm_service.store_alarm(&alarm).await?;

    let response = json!({
        "id": alarm.id,
        "message": "Alarm created successfully",
        "alarm": alarm
    });

    Ok(Json(response))
}

/// Get a specific alarm
pub async fn get_alarm(
    State(state): State<AppState>,
    Path(alarm_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("Getting alarm: {}", alarm_id);

    let id = Uuid::parse_str(&alarm_id)
        .map_err(|_| AlarmError::InvalidInput("Invalid alarm ID format".to_string()))?;

    match state.alarm_service.get_alarm(&id).await? {
        Some(alarm) => Ok(Json(json!(alarm))),
        None => Err(AlarmError::AlarmNotFound(alarm_id)),
    }
}

/// Acknowledge an alarm
pub async fn acknowledge_alarm(
    State(state): State<AppState>,
    Path(alarm_id): Path<String>,
    Json(request): Json<AlarmActionRequest>,
) -> Result<Json<Value>> {
    info!(
        "Acknowledging alarm: {} by user: {}",
        alarm_id, request.user
    );

    let id = Uuid::parse_str(&alarm_id)
        .map_err(|_| AlarmError::InvalidInput("Invalid alarm ID format".to_string()))?;

    let updated_alarm = state
        .alarm_service
        .acknowledge_alarm(&id, &request.user)
        .await?;

    let response = json!({
        "message": "Alarm acknowledged successfully",
        "alarm": updated_alarm
    });

    Ok(Json(response))
}

/// Resolve an alarm
pub async fn resolve_alarm(
    State(state): State<AppState>,
    Path(alarm_id): Path<String>,
    Json(request): Json<AlarmActionRequest>,
) -> Result<Json<Value>> {
    info!("Resolving alarm: {} by user: {}", alarm_id, request.user);

    let id = Uuid::parse_str(&alarm_id)
        .map_err(|_| AlarmError::InvalidInput("Invalid alarm ID format".to_string()))?;

    let updated_alarm = state
        .alarm_service
        .resolve_alarm(&id, &request.user)
        .await?;

    let response = json!({
        "message": "Alarm resolved successfully",
        "alarm": updated_alarm
    });

    Ok(Json(response))
}

/// Get alarm statistics
pub async fn get_statistics(State(state): State<AppState>) -> Result<Json<Value>> {
    debug!("Getting alarm statistics");

    let stats = state.alarm_service.get_statistics().await?;

    Ok(Json(json!(stats)))
}

/// Create API router
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Health check
        .route("/health", get(health_check))
        // Service status
        .route("/api/v1/status", get(get_status))
        // Alarms endpoints
        .route("/api/v1/alarms", get(list_alarms).post(create_alarm))
        .route("/api/v1/alarms/{id}", get(get_alarm))
        .route("/api/v1/alarms/{id}/ack", post(acknowledge_alarm))
        .route("/api/v1/alarms/{id}/resolve", post(resolve_alarm))
        // Statistics
        .route("/api/v1/stats", get(get_statistics))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alarm_query_params_conversion() {
        let params = AlarmQueryParams {
            status: Some("New".to_string()),
            level: Some("Critical".to_string()),
            source: Some("test_source".to_string()),
            limit: Some(50),
            offset: Some(10),
        };

        let query = AlarmQuery::from(params);
        assert_eq!(query.status, Some(AlarmStatus::New));
        assert_eq!(query.level, Some(AlarmLevel::Critical));
        assert_eq!(query.source, Some("test_source".to_string()));
        assert_eq!(query.limit, Some(50));
        assert_eq!(query.offset, Some(10));
    }

    #[test]
    fn test_invalid_status_conversion() {
        let params = AlarmQueryParams {
            status: Some("Invalid".to_string()),
            level: None,
            source: None,
            limit: None,
            offset: None,
        };

        let query = AlarmQuery::from(params);
        assert_eq!(query.status, None);
    }
}
