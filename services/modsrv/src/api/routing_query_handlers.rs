//! Instance Routing Query API Handlers
//!
//! This module provides API handlers for querying routing configurations.
//! It includes functions to retrieve routing information for instances, channels,
//! and the overall routing table.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use voltage_config::api::SuccessResponse;

use crate::app_state::AppState;
use crate::error::ModSrvError;
use crate::redis_state::{self, RoutingDirection};

#[derive(Debug, Deserialize)]
pub struct RoutingQuery {
    pub direction: Option<String>,
    pub pattern: Option<String>,
}

fn parse_direction(direction: Option<String>) -> Result<RoutingDirection, String> {
    match direction
        .as_deref()
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("m2c") => Ok(RoutingDirection::ModelToChannel),
        Some("c2m") | None => Ok(RoutingDirection::ChannelToModel),
        Some(other) => Err(format!(
            "Unsupported direction '{}'. Use 'c2m' or 'm2c'",
            other
        )),
    }
}

/// Get all routing entries for an instance
///
/// Returns measurement and action routing configuration categorized by type.
///
/// @route GET /api/instances/{id}/routing
/// @input Path(id): u16 - Instance ID
/// @output Json<SuccessResponse<serde_json::Value>> - Categorized routing entries
/// @status 200 - Success with categorized routing lists
/// @status 500 - Database error
#[utoipa::path(
    get,
    path = "/api/instances/{id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID")
    ),
    responses(
        (status = 200, description = "Instance routing categorized by type", body = serde_json::Value,
            example = json!({
                "instance_id": 1,
                "measurement": [
                    {"channel": {"id": 1, "four_remote": "T", "point_id": 101}, "point_id": 101, "enabled": true},
                    {"channel": {"id": 1, "four_remote": "T", "point_id": 102}, "point_id": 102, "enabled": true}
                ],
                "action": [
                    {"channel": {"id": 1, "four_remote": "C", "point_id": 201}, "point_id": 201, "enabled": true}
                ]
            })
        ),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn get_instance_routing_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u16>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Get both measurement and action routing
    let measurement_result = state.instance_manager.get_measurement_routing(id).await;

    let action_result = state.instance_manager.get_action_routing(id).await;

    // Check for database errors - fail fast instead of returning empty list
    let measurements = match measurement_result {
        Ok(data) => data,
        Err(e) => {
            return Err(ModSrvError::InternalError(format!(
                "Database error querying measurement routing: {}",
                e
            )));
        },
    };

    let actions = match action_result {
        Ok(data) => data,
        Err(e) => {
            return Err(ModSrvError::InternalError(format!(
                "Database error querying action routing: {}",
                e
            )));
        },
    };

    // Build categorized routing entries
    let mut measurement_entries = Vec::new();
    let mut action_entries = Vec::new();

    for m in measurements {
        measurement_entries.push(json!({
            "channel": {
                "id": m.channel_id,
                "four_remote": m.channel_type,
                "point_id": m.channel_point_id
            },
            "point_id": m.measurement_id,
            "enabled": m.enabled
        }));
    }

    for a in actions {
        action_entries.push(json!({
            "channel": {
                "id": a.channel_id,
                "four_remote": a.channel_type,
                "point_id": a.channel_point_id
            },
            "point_id": a.action_id,
            "enabled": a.enabled
        }));
    }

    Ok(Json(SuccessResponse::new(json!({
        "instance_id": id,
        "measurement": measurement_entries,
        "action": action_entries
    }))))
}

#[utoipa::path(
    get,
    path = "/api/routing",
    params(
        ("direction" = Option<String>, Query, description = "Routing direction: c2m or m2c"),
        ("pattern" = Option<String>, Query, description = "Optional key prefix filter")
    ),
    responses(
        (status = 200, description = "Routing table entries", body = serde_json::Value,
            example = json!({
                "direction": "c2m",
                "count": 5,
                "routes": {
                    "1:T:101": "pv_inverter_01:M:101",
                    "1:T:102": "pv_inverter_01:M:102",
                    "2:T:101": "battery_pack_01:M:101"
                }
            })
        ),
        (status = 400, description = "Invalid direction parameter"),
        (status = 500, description = "Redis error")
    ),
    tag = "modsrv"
)]
pub async fn get_routing_table_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RoutingQuery>,
) -> Result<Json<SuccessResponse<Value>>, ModSrvError> {
    let direction = match parse_direction(query.direction) {
        Ok(d) => d,
        Err(e) => return Err(ModSrvError::InvalidData(e.to_string())),
    };

    match redis_state::get_routing(
        state.instance_manager.rtdb.as_ref(),
        direction,
        query.pattern.as_deref(),
    )
    .await
    {
        Ok(map) => Ok(Json(SuccessResponse::new(json!({
            "direction": direction.to_string(),
            "count": map.len(),
            "routes": map,
        })))),
        Err(e) => Err(ModSrvError::InternalError(format!(
            "Failed to read routing: {}",
            e
        ))),
    }
}

// NOTE: get_channel_routing_handler has been removed as it is now replaced by:
// - get_routing_by_channel_handler in global_routing_handlers.rs
