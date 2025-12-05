//! Admin API handlers for service management
//!
//! Provides endpoints for:
//! - Dynamic log level adjustment
//! - Service runtime configuration

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Request to set log level
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetLogLevelRequest {
    /// Log level string (e.g., "debug", "info", "warn", "error", "trace")
    /// or full filter spec (e.g., "info,modsrv=debug")
    pub level: String,
}

/// Response for log level operations
#[derive(Debug, Serialize, ToSchema)]
pub struct LogLevelResponse {
    /// Current log level
    pub level: String,
    /// Operation status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Error message if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Set log level dynamically
///
/// POST /api/admin/logs/level
/// Body: {"level": "debug"}
#[utoipa::path(
    post,
    path = "/api/admin/logs/level",
    request_body = SetLogLevelRequest,
    responses(
        (status = 200, description = "Log level updated successfully", body = LogLevelResponse),
        (status = 400, description = "Invalid log level", body = LogLevelResponse)
    ),
    tag = "admin"
)]
pub async fn set_log_level(Json(req): Json<SetLogLevelRequest>) -> impl IntoResponse {
    match common::logging::set_log_level(&req.level) {
        Ok(_) => (
            StatusCode::OK,
            Json(LogLevelResponse {
                level: req.level,
                status: Some("ok".to_string()),
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(LogLevelResponse {
                level: common::logging::get_log_level(),
                status: None,
                error: Some(e),
            }),
        ),
    }
}

/// Get current log level
///
/// GET /api/admin/logs/level
#[utoipa::path(
    get,
    path = "/api/admin/logs/level",
    responses(
        (status = 200, description = "Current log level", body = LogLevelResponse)
    ),
    tag = "admin"
)]
pub async fn get_log_level() -> impl IntoResponse {
    Json(LogLevelResponse {
        level: common::logging::get_log_level(),
        status: None,
        error: None,
    })
}
