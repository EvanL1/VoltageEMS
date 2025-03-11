//! Health Check API Handlers
//!
//! Provides health check endpoint for modsrv service monitoring.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use axum::{extract::State, response::Json};
use serde_json::json;
use std::sync::Arc;
use voltage_config::api::{AppError, SuccessResponse};

use crate::app_state::AppState;

/// Health check endpoint
///
/// Returns service health status including database connectivity,
/// loaded products, and active instances.
///
/// @route GET /health
/// @output Json<SuccessResponse<serde_json::Value>> - Service health metrics
/// @side-effects None (read-only operation)
pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, AppError> {
    let sqlite_status = if state.sqlite_client.is_some() {
        "connected"
    } else {
        "not configured"
    };

    // Get instance count from instance manager
    let instances = state
        .instance_manager
        .list_instances(None)
        .await
        .unwrap_or_default();
    let instance_count = instances.len();

    // Get product count
    let products = state
        .product_loader
        .get_all_products()
        .await
        .unwrap_or_default();
    let product_count = products.len();

    Ok(Json(SuccessResponse::new(json!({
        "status": "healthy",
        "service": "modsrv",
        "architecture": "product-instance",
        "sqlite": sqlite_status,
        "products_loaded": product_count,
        "instances_active": instance_count,
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))))
}
