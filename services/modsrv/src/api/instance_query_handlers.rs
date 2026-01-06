//! Instance Query API Handlers
//!
//! Provides read-only endpoints for querying instance information and data.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use axum::{
    extract::{Path, Query, RawQuery, State},
    response::Json,
};
use bytes::Bytes;
use common::SuccessResponse;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;
use utoipa::ToSchema;
use voltage_rtdb::Rtdb;

use crate::app_state::AppState;
use crate::dto::{DataTypeQuery, InstancePointsResponse};
use crate::error::ModSrvError;

/// Pagination query parameters for listing instances
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    /// Optional product filter
    pub product_name: Option<String>,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

/// List instances with optional product filter and pagination
///
/// @route GET /api/instances?product_name={optional}&page={optional}&page_size={optional}
/// @input State(state): `Arc<AppState>` - Application state
/// @input Query(query): PaginationQuery - Pagination and filter parameters
/// @output Result<Json<SuccessResponse<serde_json::Value>>, AppError> - Paginated instances
/// @status 200 - Success with total, list, page, page_size
/// @status 500 - Database error
#[utoipa::path(
    get,
    path = "/api/instances",
    params(
        ("product_name" = Option<String>, Query, description = "Optional product filter"),
        ("page" = Option<u32>, Query, description = "Page number (default: 1)"),
        ("page_size" = Option<u32>, Query, description = "Items per page (default: 20, max: 100)")
    ),
    responses(
        (status = 200, description = "List instances with pagination", body = serde_json::Value,
            example = json!({
                "total": 10,
                "page": 1,
                "page_size": 20,
                "list": [
                    {
                        "instance_id": 1,
                        "instance_name": "pv_inverter_01",
                        "product_name": "pv_inverter",
                        "properties": {
                            "rated_power": 5000.0,
                            "manufacturer": "Huawei"
                        }
                    },
                    {
                        "instance_id": 2,
                        "instance_name": "battery_pack_01",
                        "product_name": "battery_pack",
                        "properties": {
                            "capacity_kwh": 10.0,
                            "voltage": 384.0
                        }
                    }
                ]
            })
        )
    ),
    tag = "modsrv"
)]
pub async fn list_instances(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    let product_name = query.product_name.as_deref();
    let page = query.page.max(1); // Ensure page is at least 1
    let page_size = query.page_size.clamp(1, 100); // Limit to reasonable range

    let result = state
        .instance_manager
        .list_instances_paginated(product_name, page, page_size)
        .await;

    match result {
        Ok((total, instances)) => Ok(Json(SuccessResponse::new(json!({
            "total": total,
            "page": page,
            "page_size": page_size,
            "list": instances
        })))),
        Err(e) => Err(ModSrvError::InternalError(format!(
            "Failed to list instances: {}",
            e
        ))),
    }
}

/// Search instances by name with fuzzy matching (no pagination)
///
/// Returns all instances matching the search keyword. Use this for autocomplete
/// or quick lookup scenarios where you need all matches without pagination.
///
/// URL format: `/api/instances/search?{keyword}`
/// - The keyword is passed directly as the raw query string (no parameter name needed)
/// - Empty keyword returns all instances
///
/// @route GET /api/instances/search?{keyword}
/// @input State(state): `Arc<AppState>` - Application state
/// @input RawQuery(raw_query): `Option<String>` - Raw query string as keyword
/// @output Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> - Matching instances
/// @status 200 - Success with list of matching instances
/// @status 500 - Database error
#[utoipa::path(
    get,
    path = "/api/instances/search",
    params(
        ("keyword" = Option<String>, Query, description = "Optional fuzzy keyword (legacy raw query also supported)"),
        ("ids" = Option<String>, Query, description = "Optional instance id filter, comma-separated (e.g., ids=1,2,3)")
    ),
    responses(
        (status = 200, description = "Matching instances", body = serde_json::Value,
            example = json!({
                "list": [
                    {
                        "instance_id": 1,
                        "instance_name": "pcs_01",
                        "product_name": "pcs",
                        "properties": {}
                    },
                    {
                        "instance_id": 2,
                        "instance_name": "pcs_02",
                        "product_name": "pcs",
                        "properties": {}
                    }
                ]
            })
        )
    ),
    tag = "modsrv"
)]
pub async fn search_instances(
    State(state): State<Arc<AppState>>,
    RawQuery(raw_query): RawQuery,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // raw_query is Option<String>:
    // /search?pcs                   => Some("pcs")                 (legacy keyword-only)
    // /search?ids=1,2,3             => Some("ids=1,2,3")           (filter by ids)
    // /search?keyword=pcs&ids=1,2   => Some("keyword=pcs&ids=1,2") (named params)
    // /search?pcs&ids=1,2           => Some("pcs&ids=1,2")         (mixed legacy + ids)
    // /search?                      => Some("")
    // /search                       => None

    fn parse_ids_param(value: &str) -> Vec<u32> {
        value
            .split(',')
            .filter_map(|s| s.trim().parse::<u32>().ok())
            .collect()
    }

    let raw = raw_query.unwrap_or_default();
    let mut keyword = String::new();
    let mut ids: Vec<u32> = Vec::new();

    if raw.contains('=') || raw.contains('&') {
        for part in raw.split('&') {
            if let Some((k, v)) = part.split_once('=') {
                match k {
                    "ids" | "id" => ids.extend(parse_ids_param(v)),
                    "keyword" | "q" => {
                        if keyword.is_empty() {
                            keyword = v.to_string();
                        }
                    },
                    _ => {},
                }
            } else if keyword.is_empty() && !part.trim().is_empty() {
                keyword = part.to_string();
            }
        }
    } else {
        keyword = raw;
    }

    // Load base instances (by keyword and optional ids filter)
    let instances: Vec<crate::product_loader::Instance> = if ids.is_empty() {
        // Search all instances without pagination (use large page_size)
        // Empty keyword returns all instances
        match state
            .instance_manager
            .search_instances(&keyword, None, 1, 1000)
            .await
        {
            Ok((_total, instances)) => instances,
            Err(e) => {
                return Err(ModSrvError::InternalError(format!(
                    "Failed to search instances: {}",
                    e
                )))
            },
        }
    } else {
        let mut selected = Vec::new();
        for id in &ids {
            match state.instance_manager.get_instance(*id).await {
                Ok(inst) => {
                    if !keyword.is_empty() && !inst.instance_name().contains(&keyword) {
                        continue;
                    }
                    selected.push(inst);
                },
                Err(e) if e.to_string().contains("not found") => {
                    // Search semantics: missing ids are ignored
                    continue;
                },
                Err(e) => {
                    return Err(ModSrvError::InternalError(format!(
                        "Failed to load instance {}: {}",
                        id, e
                    )))
                },
            }
        }
        selected.sort_by_key(|i| i.instance_id());
        selected
    };

    // Cache product templates by product_name to avoid repeated queries
    // Use Arc<Product> to avoid deep cloning Product structs
    let mut product_cache: HashMap<String, Arc<crate::product_loader::Product>> = HashMap::new();

    let mut list: Vec<serde_json::Value> = Vec::with_capacity(instances.len());
    for inst in instances {
        let product_name = inst.product_name().to_string();

        // Load product template (cached) - includes properties, measurements, actions
        let product = if let Some(cached) = product_cache.get(&product_name) {
            Arc::clone(cached) // O(1) ref count increment
        } else {
            let p = Arc::new(
                state
                    .product_loader
                    .get_product(&product_name)
                    .await
                    .map_err(|e| {
                        ModSrvError::InternalError(format!(
                            "Failed to load product {}: {}",
                            product_name, e
                        ))
                    })?,
            );
            product_cache.insert(product_name.clone(), Arc::clone(&p));
            p
        };

        list.push(json!({
            "instance_id": inst.core.instance_id,
            "instance_name": inst.core.instance_name,
            "product_name": inst.core.product_name,
            "properties": inst.core.properties,
            "points": {
                "properties": product.properties,
                "measurements": product.measurements,
                "actions": product.actions
            }
        }));
    }

    Ok(Json(SuccessResponse::new(json!({ "list": list }))))
}

/// List all instances (lightweight: id + name only)
///
/// @route GET /api/instances/list
#[utoipa::path(
    get,
    path = "/api/instances/list",
    responses(
        (status = 200, description = "Instance list", body = serde_json::Value,
            example = json!({
                "list": [
                    {"id": 1, "name": "battery_01"},
                    {"id": 2, "name": "pcs_01"}
                ]
            })
        )
    ),
    tag = "modsrv"
)]
pub async fn list_instances_slim(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    let instances: Vec<(u32, String)> =
        sqlx::query_as("SELECT instance_id, instance_name FROM instances ORDER BY instance_id")
            .fetch_all(&state.instance_manager.pool)
            .await
            .map_err(|e| ModSrvError::InternalError(format!("Failed to list instances: {}", e)))?;

    let list: Vec<serde_json::Value> = instances
        .into_iter()
        .map(|(id, name)| json!({"id": id, "name": name}))
        .collect();

    Ok(Json(SuccessResponse::new(json!({ "list": list }))))
}

/// Get a specific instance by ID
///
/// @route GET /api/instances/{id}
/// @input Path(id): u16 - Instance ID
/// @output Result<Json<SuccessResponse<serde_json::Value>>, AppError> - Instance details
/// @status 200 - Success with instance data
/// @status 404 - Instance not found
#[utoipa::path(
    get,
    path = "/api/instances/{id}",
    params(
        ("id" = u16, Path, description = "Instance ID")
    ),
    responses(
        (status = 200, description = "Instance details", body = serde_json::Value,
            example = json!({
                "instance": {
                    "instance_id": 1,
                    "instance_name": "pv_inverter_01",
                    "product_name": "pv_inverter",
                    "properties": {
                        "rated_power": 5000.0,
                        "manufacturer": "Huawei",
                        "model": "SUN2000-5KTL-L1",
                        "grid_type": "three_phase"
                    },
                    "created_at": "2025-10-15T10:30:00Z",
                    "updated_at": "2025-10-15T14:25:00Z"
                }
            })
        )
    ),
    tag = "modsrv"
)]
pub async fn get_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match state.instance_manager.get_instance(id).await {
        Ok(instance) => Ok(Json(SuccessResponse::new(json!({
            "instance": instance
        })))),
        Err(e) => {
            if e.to_string().contains("not found") {
                Err(ModSrvError::InstanceNotFound(id.to_string()))
            } else {
                Err(ModSrvError::InternalError(format!(
                    "Failed to get instance: {}",
                    e
                )))
            }
        },
    }
}

/// Get real-time data for an instance
///
/// Returns current measurement, action, and property values from Redis.
///
/// @route GET /api/instances/{id}/data?data_type={optional}
/// @input Path(id): u16 - Instance ID
/// @input Query(query): DataTypeQuery - Optional data type filter (M/A/P)
/// @output Result<Json<SuccessResponse<serde_json::Value>>, AppError> - Instance data points
/// @status 200 - Success with data points
/// @status 404 - Instance not found
/// @status 500 - Database error
#[utoipa::path(
    get,
    path = "/api/instances/{id}/data",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("type" = Option<String>, Query, description = "Optional data type filter (measurement/action)")
    ),
    responses(
        (status = 200, description = "Instance data", body = serde_json::Value,
            example = json!({
                "measurements": {
                    "101": "650.5",
                    "102": "12.3",
                    "103": "4500.0"
                },
                "actions": {
                    "201": "4500.0"
                },
                "properties": {
                    "rated_power": 5000.0,
                    "manufacturer": "Huawei"
                }
            })
        )
    ),
    tag = "modsrv"
)]
pub async fn get_instance_data(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
    Query(query): Query<DataTypeQuery>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match state
        .instance_manager
        .get_instance_data(id, query.data_type.as_deref())
        .await
    {
        Ok(data) => Ok(Json(SuccessResponse::new(data))),
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("not found") {
                Err(ModSrvError::InstanceNotFound(id.to_string()))
            } else {
                Err(ModSrvError::InternalError(format!(
                    "Failed to get instance data: {}",
                    e
                )))
            }
        },
    }
}

/// Get point definitions with routing for an instance
///
/// Returns measurement and action points with their routing configurations.
/// Each point includes both the product template definition and the instance-specific
/// routing configuration (if configured).
///
/// @route GET /api/instances/{id}/points
/// @input Path(id): u16 - Instance ID
/// @output `Result<Json<SuccessResponse<InstancePointsResponse>>, AppError>` - Points with routing
/// @status 200 - Success with point definitions
/// @status 404 - Instance not found
/// @status 500 - Database error
#[utoipa::path(
    get,
    path = "/api/instances/{id}/points",
    params(
        ("id" = u16, Path, description = "Instance ID")
    ),
    responses(
        (status = 200, description = "Instance points with routing configurations",
            body = InstancePointsResponse,
            example = json!({
                "instance_name": "pv_inverter_01",
                "measurements": [
                    {
                        "measurement_id": 1,
                        "name": "DC Voltage",
                        "unit": "V",
                        "description": "DC input voltage",
                        "routing": {
                            "channel_id": 1001,
                            "channel_type": "T",
                            "channel_point_id": 101,
                            "enabled": true
                        }
                    },
                    {
                        "measurement_id": 2,
                        "name": "DC Current",
                        "unit": "A",
                        "description": "DC input current"
                    }
                ],
                "actions": [
                    {
                        "action_id": 1,
                        "name": "Power Setpoint",
                        "unit": "kW",
                        "description": "Active power setpoint",
                        "routing": {
                            "channel_id": 1001,
                            "channel_type": "A",
                            "channel_point_id": 201,
                            "enabled": true
                        }
                    }
                ]
            })
        ),
        (status = 404, description = "Instance not found"),
        (status = 500, description = "Internal error")
    ),
    tag = "modsrv"
)]
pub async fn get_instance_points(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
) -> Result<Json<SuccessResponse<InstancePointsResponse>>, ModSrvError> {
    // Query instance_name for response (InstancePointsResponse still needs it for now)
    let instance = state.instance_manager.get_instance(id).await.map_err(|e| {
        if e.to_string().contains("not found") {
            ModSrvError::InstanceNotFound(id.to_string())
        } else {
            ModSrvError::InternalError(format!("Failed to get instance: {}", e))
        }
    })?;

    match state.instance_manager.load_instance_points(id).await {
        Ok((measurements, actions)) => {
            let response = InstancePointsResponse {
                instance_name: instance.instance_name().to_string(),
                measurements,
                actions,
            };
            Ok(Json(SuccessResponse::new(response)))
        },
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("not found") {
                Err(ModSrvError::InstanceNotFound(id.to_string()))
            } else {
                Err(ModSrvError::InternalError(format!(
                    "Failed to get instance points: {}",
                    e
                )))
            }
        },
    }
}

// ============================================================================
// Test Helper: Set Measurement Value
// ============================================================================

/// Request DTO for setting measurement value (testing purpose)
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct SetMeasurementRequest {
    /// Point ID (numeric or semantic name)
    #[serde(alias = "id", alias = "measurement_id")]
    #[schema(example = "101")]
    pub point_id: String,

    /// Value to set
    #[schema(example = 650.5)]
    pub value: f64,
}

/// Set instance measurement value (for testing)
///
/// Directly writes a measurement value to Redis, bypassing the normal
/// data flow (channel → routing → instance). Useful for testing rules
/// and calculations without actual device data.
///
/// @route POST /api/instances/{id}/measurement
/// @input Path(id): u16 - Instance ID
/// @input Json(req): SetMeasurementRequest - Point ID and value
/// @output Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError>
/// @status 200 - Measurement set successfully
/// @status 500 - Redis error
#[utoipa::path(
    post,
    path = "/api/instances/{id}/measurement",
    params(("id" = u16, Path, description = "Instance ID")),
    request_body(content = SetMeasurementRequest, description = "Measurement point and value to set"),
    responses(
        (status = 200, description = "Measurement set successfully", body = serde_json::Value,
            example = json!({
                "instance_id": 1,
                "point_id": "101",
                "value": 650.5,
                "status": "set"
            })
        ),
        (status = 500, description = "Redis error")
    ),
    tag = "modsrv"
)]
pub async fn set_instance_measurement(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
    Json(req): Json<SetMeasurementRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Get RTDB reference from instance manager
    let rtdb = &state.instance_manager.rtdb;

    // Build M value Hash key: inst:{id}:M
    let key = crate::config::InstanceRedisKeys::measurement_hash(id);

    // Write to Redis Hash
    rtdb.hash_set(&key, &req.point_id, Bytes::from(req.value.to_string()))
        .await
        .map_err(|e| ModSrvError::RedisError(e.to_string()))?;

    info!(
        "Set measurement inst:{}:M[{}] = {}",
        id, req.point_id, req.value
    );

    Ok(Json(SuccessResponse::new(json!({
        "instance_id": id,
        "point_id": req.point_id,
        "value": req.value,
        "status": "set"
    }))))
}
