//! Health Check API Handlers
//!
//! Provides health check endpoint for modsrv service monitoring.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use axum::{extract::State, response::Json};
use common::system_metrics::SystemMetrics;
use common::{AppError, SuccessResponse};
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;

use crate::app_state::AppState;

/// Health check endpoint
///
/// Performs actual connectivity checks on dependencies.
/// Returns 503 if any critical dependency is unhealthy.
///
/// @route GET /health
/// @output Json<SuccessResponse<serde_json::Value>> - Service health metrics
/// @status 200 - Service is healthy (all dependencies reachable)
/// @status 503 - Service is unhealthy (one or more dependencies failed)
pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, AppError> {
    let mut checks = serde_json::Map::new();
    let mut overall_healthy = true;
    let mut errors = Vec::new();

    // Check SQLite connectivity using SqliteClient::ping()
    let sqlite_start = Instant::now();
    let sqlite_status = if let Some(client) = &state.sqlite_client {
        match client.ping().await {
            Ok(_) => {
                checks.insert(
                    "sqlite".to_string(),
                    json!({
                        "status": "healthy",
                        "message": "Connected",
                        "duration_ms": sqlite_start.elapsed().as_millis()
                    }),
                );
                "connected"
            },
            Err(e) => {
                overall_healthy = false;
                let err_msg = format!("Ping failed: {}", e);
                errors.push(format!("sqlite: {}", err_msg));
                checks.insert(
                    "sqlite".to_string(),
                    json!({
                        "status": "unhealthy",
                        "message": err_msg,
                        "duration_ms": sqlite_start.elapsed().as_millis()
                    }),
                );
                "error"
            },
        }
    } else {
        checks.insert(
            "sqlite".to_string(),
            json!({
                "status": "not_configured",
                "message": "SQLite client not initialized"
            }),
        );
        "not configured"
    };

    // Check instance manager
    let instance_start = Instant::now();
    match state.instance_manager.list_instances(None).await {
        Ok(instances) => {
            checks.insert(
                "instances".to_string(),
                json!({
                    "status": "healthy",
                    "count": instances.len(),
                    "duration_ms": instance_start.elapsed().as_millis()
                }),
            );
        },
        Err(e) => {
            overall_healthy = false;
            let err_msg = format!("Failed to list instances: {}", e);
            errors.push(format!("instances: {}", err_msg));
            checks.insert(
                "instances".to_string(),
                json!({
                    "status": "unhealthy",
                    "message": err_msg,
                    "duration_ms": instance_start.elapsed().as_millis()
                }),
            );
        },
    }

    // Check product loader (products are compile-time constants, always healthy)
    let product_start = Instant::now();
    let products = state.product_loader.get_all_products();
    checks.insert(
        "products".to_string(),
        json!({
            "status": "healthy",
            "count": products.len(),
            "duration_ms": product_start.elapsed().as_millis()
        }),
    );

    // Collect system metrics (CPU, memory)
    let metrics = SystemMetrics::collect();

    let status = if overall_healthy {
        "healthy"
    } else {
        "unhealthy"
    };

    let response = json!({
        "status": status,
        "service": "modsrv",
        "architecture": "product-instance",
        "sqlite": sqlite_status,
        "checks": checks,
        "system": {
            "cpu_count": metrics.cpu_count,
            "process_cpu_percent": metrics.process_cpu_percent,
            "process_memory_mb": metrics.process_memory_mb,
            "memory_total_mb": metrics.memory_total_mb
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    // Return 503 if unhealthy
    if !overall_healthy {
        return Err(AppError::service_unavailable(format!(
            "Service dependencies are unhealthy: {}",
            errors.join(", ")
        )));
    }

    Ok(Json(SuccessResponse::new(response)))
}
