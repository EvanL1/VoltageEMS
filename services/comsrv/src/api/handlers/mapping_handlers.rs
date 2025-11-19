//! Protocol mapping handlers
//!
//! This module contains handlers for:
//! - Getting all mapping configurations for a channel
//! - Batch updating mapping configurations
//! - Validating mapping configurations based on protocol

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use crate::api::routes::AppState;
use crate::dto::{AppError, MappingBatchUpdateResult, MappingUpdateMode, SuccessResponse};
use axum::{
    extract::{Path, State},
    response::Json,
};
use serde::Deserialize;
use serde_json::json;

// ============================================================================
// Validator Structures - Strong typing for runtime validation
// ============================================================================

/// Modbus mapping validator - Provides compile-time type safety through serde
///
/// This validator structure enables automatic type checking when deserializing
/// JSON mapping data. Instead of manual field-by-field validation, serde will:
/// - Reject non-numeric values for numeric fields
/// - Enforce range constraints (u8: 0-255, u16: 0-65535)
/// - Validate required fields existence
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields are read by serde during deserialization
struct ModbusMappingValidator {
    /// Modbus slave ID (1-247, 0 and 248-255 reserved)
    slave_id: u8,
    /// Modbus function code (1,2,3,4,5,6,15,16)
    function_code: u8,
    /// Register address (0-65535)
    register_address: u16,
    /// Data type (uint16, int16, uint32, int32, float32, float64)
    #[serde(default)]
    data_type: Option<String>,
    /// Byte order (ABCD, DCBA, BADC, CDAB, AB, BA)
    #[serde(default)]
    byte_order: Option<String>,
    /// Bit position for coil/discrete operations (optional)
    #[serde(default)]
    bit_position: Option<u8>,
}

/// CAN mapping validator - Type-safe CAN bus parameter validation
///
/// Validates CAN protocol-specific parameters including:
/// - CAN identifier ranges (standard/extended frame)
/// - Bit-level signal extraction parameters
/// - Data type and byte order specifications
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields are read by serde during deserialization
struct CanMappingValidator {
    /// CAN message identifier
    /// - Standard frame: 0x000-0x7FF (11-bit)
    /// - Extended frame: 0x00000000-0x1FFFFFFF (29-bit)
    can_id: u32,
    /// Signal start bit position (0-63 for 8-byte CAN frame)
    start_bit: u32,
    /// Signal bit length (1-64)
    bit_length: u32,
    /// Byte order (ABCD, DCBA, BADC, CDAB, AB, BA)
    byte_order: String,
    /// Data type (uint8, int8, uint16, int16, uint32, int32, float32, float64)
    data_type: String,
    /// Whether the signal is signed
    #[serde(default)]
    signed: bool,
    /// Scaling factor for physical value conversion
    #[serde(default = "default_scale")]
    scale: f64,
    /// Offset for physical value conversion
    #[serde(default)]
    offset: f64,
}

/// Virtual mapping validator - Expression-based simulation validation
#[derive(Debug, Deserialize)]
struct VirtualMappingValidator {
    /// Mathematical expression for value calculation
    /// Supports: +, -, *, /, %, pow(), sqrt(), abs()
    /// Point references: P{id} (e.g., "P1 + P2 * 0.5")
    expression: String,
}

// Helper functions for serde defaults
fn default_scale() -> f64 {
    1.0
}

/// Get all mapping configurations for a channel
///
/// Returns all protocol-specific mapping configurations for the channel.
///
/// @route GET /api/channels/{id}/mappings
/// @input Path(channel_id): u16 - Channel ID
/// @input State(state): AppState - Application state
/// @output Json<ApiResponse<GroupedMappings>> - Grouped point mappings by type
/// @status 200 - Mappings retrieved successfully
/// @status 404 - Channel not found
/// @status 500 - Database error
#[utoipa::path(
    get,
    path = "/api/channels/{id}/mappings",
    params(
        ("id" = u16, Path, description = "Channel identifier")
    ),
    responses((status = 200, description = "Mappings retrieved", body = crate::dto::GroupedMappings)),
    tag = "comsrv"
)]
pub async fn get_channel_mappings_handler(
    Path(channel_id): Path<u16>,
    State(state): State<AppState>,
) -> Result<Json<SuccessResponse<crate::dto::GroupedMappings>>, AppError> {
    // 1. Verify channel exists
    let channel_exists: Option<(i64,)> =
        sqlx::query_as("SELECT channel_id FROM channels WHERE channel_id = ?")
            .bind(channel_id as i64)
            .fetch_optional(&state.sqlite_pool)
            .await
            .map_err(|e| {
                tracing::error!("Database error checking channel: {}", e);
                AppError::internal_error("Database operation failed")
            })?;

    if channel_exists.is_none() {
        return Err(AppError::not_found(format!(
            "Channel {} not found",
            channel_id
        )));
    }

    // 2. Query all four point tables and collect mappings by type
    let mut telemetry_mappings = Vec::new();
    let mut signal_mappings = Vec::new();
    let mut control_mappings = Vec::new();
    let mut adjustment_mappings = Vec::new();

    let tables = [
        "telemetry_points",
        "signal_points",
        "control_points",
        "adjustment_points",
    ];

    for (table_idx, table) in tables.iter().enumerate() {
        let query = format!(
            "SELECT point_id, signal_name, protocol_mappings FROM {} WHERE channel_id = ? ORDER BY point_id",
            table
        );
        let rows: Vec<(i64, String, Option<String>)> = sqlx::query_as(&query)
            .bind(channel_id as i64)
            .fetch_all(&state.sqlite_pool)
            .await
            .map_err(|e| {
                tracing::error!("Error querying {}: {}", table, e);
                AppError::internal_error("Database operation failed")
            })?;

        // Select target vector based on table type
        let target_vec = match table_idx {
            0 => &mut telemetry_mappings,
            1 => &mut signal_mappings,
            2 => &mut control_mappings,
            3 => &mut adjustment_mappings,
            _ => unreachable!(),
        };

        for (point_id, signal_name, protocol_mappings_json) in rows {
            // Parse protocol_mappings JSON if present
            let protocol_data = if let Some(json_str) = protocol_mappings_json {
                match serde_json::from_str::<serde_json::Value>(&json_str) {
                    Ok(value) => {
                        // Convert null to empty object for consistent API response
                        if value.is_null() {
                            serde_json::Value::Object(serde_json::Map::new())
                        } else {
                            value
                        }
                    },
                    Err(e) => {
                        tracing::error!(
                            "Failed to parse protocol_mappings JSON for point {} in {}: {}",
                            point_id,
                            table,
                            e
                        );
                        serde_json::Value::Object(serde_json::Map::new())
                    },
                }
            } else {
                serde_json::Value::Object(serde_json::Map::new())
            };

            target_vec.push(crate::dto::PointMappingDetail {
                point_id: point_id as u32,
                signal_name,
                protocol_data,
            });
        }
    }

    Ok(Json(SuccessResponse::new(crate::dto::GroupedMappings {
        telemetry: telemetry_mappings,
        signal: signal_mappings,
        control: control_mappings,
        adjustment: adjustment_mappings,
    })))
}

/// Batch update mapping configurations for a channel
///
/// Updates all protocol-specific mapping configurations for the channel in a single transaction.
/// Supports validate-only mode for pre-checking without writing.
/// Can optionally trigger automatic channel reload.
///
/// @route PUT /api/channels/{id}/mappings
/// @input Path(channel_id): u16 - Channel ID
/// @input State(state): AppState - Application state
/// @input Json(req): MappingBatchUpdateRequest - Batch mapping update request
/// @output Json<ApiResponse<MappingBatchUpdateResult>> - Update result
/// @status 200 - Mappings updated successfully
/// @status 400 - Validation error
/// @status 404 - Channel not found
/// @status 500 - Database error
#[utoipa::path(
    put,
    path = "/api/channels/{id}/mappings",
    params(
        ("id" = u16, Path, description = "Channel identifier")
    ),
    request_body(
        content = crate::dto::MappingBatchUpdateRequest,
        description = "Batch update protocol-specific mappings with validation support",
        examples(
            ("Modbus TCP - Telemetry Points" = (
                summary = "Modbus TCP telemetry mapping (FC 3 - Read Holding Registers)",
                description = "Map telemetry points using function code 3 for reading measurements",
                value = json!({
                    "mappings": [
                        {
                            "point_id": 101,
                            "four_remote": "T",
                            "protocol_data": {
                                "slave_id": 1,
                                "function_code": 3,
                                "register_address": 100,
                                "data_type": "float32",
                                "byte_order": "ABCD"
                            }
                        },
                        {
                            "point_id": 102,
                            "four_remote": "T",
                            "protocol_data": {
                                "slave_id": 1,
                                "function_code": 3,
                                "register_address": 102,
                                "data_type": "uint16",
                                "byte_order": "AB"
                            }
                        }
                    ],
                    "validate_only": false,
                    "reload_channel": false,
                    "mode": "replace"
                })
            )),
            ("Modbus TCP - Control Points" = (
                summary = "Modbus TCP control mapping (FC 5 - Write Single Coil)",
                description = "Map control points using function code 5 for on/off control",
                value = json!({
                    "mappings": [
                        {
                            "point_id": 201,
                            "four_remote": "C",
                            "protocol_data": {
                                "slave_id": 1,
                                "function_code": 5,
                                "register_address": 0,
                                "data_type": "uint16",
                                "byte_order": "AB"
                            }
                        },
                        {
                            "point_id": 202,
                            "four_remote": "C",
                            "protocol_data": {
                                "slave_id": 1,
                                "function_code": 5,
                                "register_address": 1,
                                "data_type": "uint16",
                                "byte_order": "AB"
                            }
                        }
                    ],
                    "validate_only": false,
                    "reload_channel": true,
                    "mode": "replace"
                })
            )),
            ("Modbus TCP - Adjustment Points" = (
                summary = "Modbus TCP adjustment mapping (FC 16 - Write Multiple Registers)",
                description = "Map adjustment points using function code 16 for setpoint control",
                value = json!({
                    "mappings": [
                        {
                            "point_id": 301,
                            "four_remote": "A",
                            "protocol_data": {
                                "slave_id": 1,
                                "function_code": 16,
                                "register_address": 200,
                                "data_type": "float32",
                                "byte_order": "ABCD"
                            }
                        },
                        {
                            "point_id": 302,
                            "four_remote": "A",
                            "protocol_data": {
                                "slave_id": 1,
                                "function_code": 6,
                                "register_address": 202,
                                "data_type": "int16",
                                "byte_order": "AB"
                            }
                        }
                    ],
                    "validate_only": false,
                    "reload_channel": false,
                    "mode": "replace"
                })
            )),
            ("Modbus RTU - Mixed Types" = (
                summary = "Modbus RTU mixed point types (T/S/C/A)",
                description = "Complete example with all four remote types on RTU channel",
                value = json!({
                    "mappings": [
                        {
                            "point_id": 101,
                            "four_remote": "T",
                            "protocol_data": {
                                "slave_id": 1,
                                "function_code": 3,
                                "register_address": 0,
                                "data_type": "float32",
                                "byte_order": "ABCD"
                            }
                        },
                        {
                            "point_id": 151,
                            "four_remote": "S",
                            "protocol_data": {
                                "slave_id": 1,
                                "function_code": 2,
                                "register_address": 100,
                                "data_type": "uint16",
                                "byte_order": "AB"
                            }
                        },
                        {
                            "point_id": 201,
                            "four_remote": "C",
                            "protocol_data": {
                                "slave_id": 1,
                                "function_code": 5,
                                "register_address": 0,
                                "data_type": "uint16",
                                "byte_order": "AB"
                            }
                        },
                        {
                            "point_id": 301,
                            "four_remote": "A",
                            "protocol_data": {
                                "slave_id": 1,
                                "function_code": 16,
                                "register_address": 200,
                                "data_type": "float32",
                                "byte_order": "ABCD"
                            }
                        }
                    ],
                    "validate_only": false,
                    "reload_channel": false,
                    "mode": "replace"
                })
            )),
            ("CAN Bus - Telemetry Signals" = (
                summary = "CAN bus signal mapping with bit extraction",
                description = "Map CAN signals using bit-level extraction parameters",
                value = json!({
                    "mappings": [
                        {
                            "point_id": 101,
                            "four_remote": "T",
                            "protocol_data": {
                                "can_id": 0x18FF50E5,
                                "start_bit": 0,
                                "bit_length": 16,
                                "byte_order": "AB",
                                "data_type": "uint16",
                                "signed": false,
                                "scale": 0.1,
                                "offset": 0.0
                            }
                        },
                        {
                            "point_id": 102,
                            "four_remote": "T",
                            "protocol_data": {
                                "can_id": 0x18FF50E5,
                                "start_bit": 16,
                                "bit_length": 16,
                                "byte_order": "AB",
                                "data_type": "int16",
                                "signed": true,
                                "scale": 0.01,
                                "offset": -100.0
                            }
                        }
                    ],
                    "validate_only": false,
                    "reload_channel": false,
                    "mode": "replace"
                })
            )),
            ("Virtual - Expression Mapping" = (
                summary = "Virtual protocol with expression-based calculations",
                description = "Map virtual points using mathematical expressions",
                value = json!({
                    "mappings": [
                        {
                            "point_id": 101,
                            "four_remote": "T",
                            "protocol_data": {
                                "expression": "P1 + P2"
                            }
                        },
                        {
                            "point_id": 102,
                            "four_remote": "T",
                            "protocol_data": {
                                "expression": "P1 * 0.5 + P3"
                            }
                        },
                        {
                            "point_id": 103,
                            "four_remote": "T",
                            "protocol_data": {
                                "expression": "pow(P1, 2) + sqrt(P2)"
                            }
                        }
                    ],
                    "validate_only": false,
                    "reload_channel": false,
                    "mode": "replace"
                })
            )),
            ("Validation Only - Dry Run" = (
                summary = "Validate mappings without writing to database",
                description = "Use validate_only mode to check configuration before applying",
                value = json!({
                    "mappings": [
                        {
                            "point_id": 101,
                            "four_remote": "T",
                            "protocol_data": {
                                "slave_id": 1,
                                "function_code": 3,
                                "register_address": 100,
                                "data_type": "float32",
                                "byte_order": "ABCD"
                            }
                        }
                    ],
                    "validate_only": true,
                    "reload_channel": false,
                    "mode": "replace"
                })
            )),
            ("Merge Mode - Partial Update" = (
                summary = "Merge mode (default) - partial field update",
                description = "**Merge mode (default)**: Updates only specified fields while preserving all others. Example: if point 101 has {slave_id:1, function_code:3, register_address:100, data_type:\"float32\", byte_order:\"ABCD\"}, this request only updates register_address to 150, all other fields remain unchanged. The merged result is validated before saving.",
                value = json!({
                    "mappings": [
                        {
                            "point_id": 101,
                            "four_remote": "T",
                            "protocol_data": {
                                "register_address": 150,  // Only update this field
                                "data_type": "uint16"      // Only update this field
                                // Other fields (slave_id, function_code, byte_order) remain unchanged
                            }
                        },
                        {
                            "point_id": 102,
                            "four_remote": "T",
                            "protocol_data": {
                                "byte_order": "DCBA"  // Only change byte order, keep everything else
                            }
                        }
                    ],
                    "validate_only": false,
                    "reload_channel": false,
                    "mode": "merge"  // Default mode - can be omitted
                })
            ))
        )
    ),
    responses(
        (status = 200, description = "Mappings updated successfully", body = crate::dto::MappingBatchUpdateResult),
        (status = 400, description = "Validation error (invalid parameters or protocol mismatch)"),
        (status = 404, description = "Channel not found"),
        (status = 500, description = "Internal server error (database operation failed)")
    ),
    tag = "comsrv"
)]
pub async fn update_channel_mappings_handler(
    Path(channel_id): Path<u16>,
    State(state): State<AppState>,
    Json(mut req): Json<crate::dto::MappingBatchUpdateRequest>,
) -> Result<Json<SuccessResponse<crate::dto::MappingBatchUpdateResult>>, AppError> {
    // 1. Verify channel exists and get protocol
    let channel_info: Option<(String, bool)> =
        sqlx::query_as("SELECT protocol, enabled FROM channels WHERE channel_id = ?")
            .bind(channel_id as i64)
            .fetch_optional(&state.sqlite_pool)
            .await
            .map_err(|e| {
                tracing::error!("Database error checking channel: {}", e);
                AppError::internal_error("Database operation failed")
            })?;

    let Some((protocol, _is_enabled)) = channel_info else {
        return Err(AppError::internal_error(format!(
            "Channel {} not found",
            channel_id
        )));
    };

    // 1.5. Normalize protocol_data types BEFORE validation
    // This ensures validation works with properly typed numeric fields
    for item in req.mappings.iter_mut() {
        item.protocol_data = normalize_protocol_data(&protocol, &item.protocol_data);
    }

    // 2. Validate input when in Replace mode. In Merge mode, we will validate after merging with existing.
    if matches!(req.mode, crate::dto::MappingUpdateMode::Replace) {
        let validation_errors = validate_mappings(&protocol, &req.mappings);
        if !validation_errors.is_empty() {
            return Err(AppError::bad_request(format!(
                "Validation errors: {}",
                validation_errors.join("; ")
            )));
        }
    }

    // Structural validation: table & point existence
    let mut structure_errors = Vec::new();
    for (idx, item) in req.mappings.iter().enumerate() {
        let table = match item.four_remote.as_str() {
            "T" => "telemetry_points",
            "S" => "signal_points",
            "C" => "control_points",
            "A" => "adjustment_points",
            _ => {
                structure_errors.push(format!(
                    "Item {}: invalid four_remote {}",
                    idx, item.four_remote
                ));
                continue;
            },
        };
        let exists: Option<(i64,)> = sqlx::query_as(&format!(
            "SELECT point_id FROM {} WHERE channel_id = ? AND point_id = ?",
            table
        ))
        .bind(channel_id as i64)
        .bind(item.point_id as i64)
        .fetch_optional(&state.sqlite_pool)
        .await
        .map_err(|e| AppError::internal_error(format!("DB error: {}", e)))?;
        if exists.is_none() {
            structure_errors.push(format!(
                "Item {}: point_id {} not found in {} for channel {}",
                idx, item.point_id, table, channel_id
            ));
        }
    }
    if !structure_errors.is_empty() {
        return Err(AppError::bad_request(structure_errors.join("; ")));
    }

    if req.validate_only {
        return Ok(Json(SuccessResponse::new(MappingBatchUpdateResult {
            updated_count: req.mappings.len(),
            channel_reloaded: false,
            validation_errors: vec![],
            message: format!("Validation OK for {} mappings", req.mappings.len()),
        })));
    }

    let mut tx = state
        .sqlite_pool
        .begin()
        .await
        .map_err(|e| AppError::internal_error(format!("Failed to start transaction: {}", e)))?;

    let mut updated = 0usize;
    for item in &req.mappings {
        let table = match item.four_remote.as_str() {
            "T" => "telemetry_points",
            "S" => "signal_points",
            "C" => "control_points",
            "A" => "adjustment_points",
            _ => unreachable!(),
        };

        // Merge/Replace
        let mut new_json = match req.mode {
            MappingUpdateMode::Replace => Some(item.protocol_data.clone()),
            MappingUpdateMode::Merge => {
                let existing: Option<(Option<String>,)> = sqlx::query_as(&format!(
                    "SELECT protocol_mappings FROM {} WHERE channel_id = ? AND point_id = ?",
                    table
                ))
                .bind(channel_id as i64)
                .bind(item.point_id as i64)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| AppError::internal_error(format!("DB read error: {}", e)))?;

                let mut base = existing
                    .and_then(|row| row.0)
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                    .unwrap_or(json!({}));
                if let serde_json::Value::Object(ref mut base_map) = base {
                    if let serde_json::Value::Object(new_map) = &item.protocol_data {
                        for (k, v) in new_map {
                            base_map.insert(k.clone(), v.clone());
                        }
                    }
                }
                Some(base)
            },
        };

        // Normalize merged data before validation (may contain old un-normalized data from database)
        new_json = new_json.map(|v| normalize_protocol_data(&protocol, &v));

        // For Merge mode, validate the merged JSON before writing
        if matches!(req.mode, crate::dto::MappingUpdateMode::Merge) {
            if let Some(ref merged) = new_json {
                let merged_item = crate::dto::PointMappingItem {
                    point_id: item.point_id,
                    four_remote: item.four_remote.clone(),
                    protocol_data: merged.clone(),
                };
                let errors = validate_mappings(&protocol, &[merged_item]);
                if !errors.is_empty() {
                    return Err(AppError::bad_request(format!(
                        "Validation errors: {}",
                        errors.join("; ")
                    )));
                }
            }
        }

        // Serialize the normalized JSON for database storage
        let serialized = match new_json {
            Some(serde_json::Value::Object(ref m)) if m.is_empty() => None,
            Some(v) => Some(serde_json::to_string(&v).unwrap_or("{}".to_string())),
            None => None,
        };

        sqlx::query(&format!(
            "UPDATE {} SET protocol_mappings = ? WHERE channel_id = ? AND point_id = ?",
            table
        ))
        .bind(serialized)
        .bind(channel_id as i64)
        .bind(item.point_id as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::internal_error(format!("DB update error: {}", e)))?;
        updated += 1;
    }

    tx.commit()
        .await
        .map_err(|e| AppError::internal_error(format!("Commit failed: {}", e)))?;

    Ok(Json(SuccessResponse::new(MappingBatchUpdateResult {
        updated_count: updated,
        channel_reloaded: false,
        validation_errors: vec![],
        message: format!(
            "Updated {} mapping(s) in {} mode",
            updated,
            match req.mode {
                MappingUpdateMode::Replace => "replace",
                MappingUpdateMode::Merge => "merge",
            }
        ),
    })))

    // Original implementation commented out - requires redesign for JSON-based mappings
    /*
    let mut tx = state
        .sqlite_pool
        .begin()
        .await
        .map_err(|e| AppError::internal_error(format!("Database operation failed: {}", e)))?;

    // ... original code ...

    tx.commit()
        .await
        .map_err(|e| AppError::internal_error(format!("Database operation failed: {}", e)))?;
    */

    // Note: Channel reload and result return code below is also disabled
    // since the batch update functionality is temporarily unavailable

    /*
    // 7. Optionally reload channel if it's running
    let channel_reloaded = if req.reload_channel && is_enabled {
        let is_running = {
            let factory = state.factory.read().await;
            factory.get_channel(channel_id).is_some()
        };

        if is_running {
            // Trigger channel reload by calling reload handler internally
            tracing::info!("Auto-reloading channel {} after mapping update", channel_id);

            // Simple reload: disconnect and reconnect
            let factory = state.factory.read().await;
            if let Some(channel_arc) = factory.get_channel(channel_id) {
                let mut channel = channel_arc.write().await;
                if let Err(e) = channel.disconnect().await {
                    tracing::warn!(
                        "Error disconnecting channel {} for reload: {}",
                        channel_id,
                        e
                    );
                }
                if let Err(e) = channel.connect().await {
                    tracing::error!(
                        "Error reconnecting channel {} after mapping update: {}",
                        channel_id,
                        e
                    );
                    return Err(AppError::internal_error(format!(
                        "Mappings updated but channel reload failed: {}",
                        e
                    )));
                }
            }
            true
        } else {
            false
        }
    } else {
        false
    };

    let message = if channel_reloaded {
        format!("Updated {} mappings and reloaded channel", updated_count)
    } else {
        format!("Updated {} mappings", updated_count)
    };

    Ok(Json(SuccessResponse::new(
        crate::dto::MappingBatchUpdateResult {
            updated_count,
            channel_reloaded,
            validation_errors: vec![],
            message,
        },
    )))
    */
}

/// Validate mapping configurations based on protocol
///
/// Uses strong-typed Validator structures to automatically validate types and ranges.
/// Serde deserialization provides automatic type checking (u8, u16, u32, etc.).
/// Additional business rules are enforced after type validation.
fn validate_mappings(protocol: &str, mappings: &[crate::dto::PointMappingItem]) -> Vec<String> {
    let mut errors = Vec::new();

    for mapping in mappings {
        match protocol.to_lowercase().as_str() {
            "modbus_tcp" | "modbus_rtu" | "modbus" => {
                // Attempt strong-typed deserialization - automatic type/range validation
                match serde_json::from_value::<ModbusMappingValidator>(
                    mapping.protocol_data.clone(),
                ) {
                    Ok(validated) => {
                        // ✅ Type validation passed, now check business rules

                        // 1. Slave ID range (1-247, 0 and 248-255 reserved by Modbus spec)
                        if validated.slave_id == 0 || validated.slave_id >= 248 {
                            errors.push(format!(
                                "Point {}: slave_id {} invalid (must be 1-247, 0 and 248-255 are reserved)",
                                mapping.point_id, validated.slave_id
                            ));
                        }

                        // 2. Function code validity
                        let valid_fcs = [1u8, 2, 3, 4, 5, 6, 15, 16];
                        if !valid_fcs.contains(&validated.function_code) {
                            errors.push(format!(
                                "Point {}: function_code {} invalid (valid: 1,2,3,4,5,6,15,16)",
                                mapping.point_id, validated.function_code
                            ));
                        }

                        // 3. Optional data type enumeration
                        if let Some(ref dt) = validated.data_type {
                            let valid_types = [
                                "bool", "boolean", "uint16", "int16", "uint32", "int32", "float32",
                                "float64",
                            ];
                            if !valid_types.contains(&dt.as_str()) {
                                errors.push(format!(
                                    "Point {}: data_type '{}' invalid (valid: {})",
                                    mapping.point_id,
                                    dt,
                                    valid_types.join(", ")
                                ));
                            }
                        }

                        // 4. Optional byte order enumeration
                        if let Some(ref bo) = validated.byte_order {
                            let valid_orders = ["ABCD", "DCBA", "BADC", "CDAB", "AB", "BA"];
                            if !valid_orders.contains(&bo.as_str()) {
                                errors.push(format!(
                                    "Point {}: byte_order '{}' invalid (valid: {})",
                                    mapping.point_id,
                                    bo,
                                    valid_orders.join(", ")
                                ));
                            }
                        }

                        // 5. Business rule: Function code must match point type
                        let fc_error = validate_modbus_function_code_match(
                            validated.function_code,
                            mapping.four_remote.as_str(),
                            mapping.point_id,
                        );
                        if let Some(err) = fc_error {
                            errors.push(err);
                        }
                    },
                    Err(e) => {
                        // ❌ Type validation failed (wrong type, missing field, out of range)
                        errors.push(format!(
                            "Point {}: Modbus mapping validation failed - {}",
                            mapping.point_id, e
                        ));
                    },
                }
            },
            "can" => {
                // CAN protocol validation - full support (no longer "Unsupported")
                match serde_json::from_value::<CanMappingValidator>(mapping.protocol_data.clone()) {
                    Ok(validated) => {
                        // ✅ Type validation passed, now check business rules

                        // 1. CAN ID range validation
                        //    Standard frame: 0x000-0x7FF (2047)
                        //    Extended frame: 0x00000000-0x1FFFFFFF (536870911)
                        if validated.can_id > 0x1FFFFFFF {
                            errors.push(format!(
                                "Point {}: can_id 0x{:X} exceeds maximum (0x1FFFFFFF for extended frame)",
                                mapping.point_id, validated.can_id
                            ));
                        }

                        // 2. Start bit range (CAN frame is max 8 bytes = 64 bits)
                        if validated.start_bit >= 64 {
                            errors.push(format!(
                                "Point {}: start_bit {} exceeds max (0-63 for 8-byte CAN frame)",
                                mapping.point_id, validated.start_bit
                            ));
                        }

                        // 3. Bit length validation (1-64 bits)
                        if validated.bit_length == 0 || validated.bit_length > 64 {
                            errors.push(format!(
                                "Point {}: bit_length {} invalid (must be 1-64)",
                                mapping.point_id, validated.bit_length
                            ));
                        }

                        // 4. Validate start_bit + bit_length doesn't exceed frame size
                        if validated.start_bit + validated.bit_length > 64 {
                            errors.push(format!(
                                "Point {}: signal exceeds frame boundary (start_bit {} + bit_length {} > 64)",
                                mapping.point_id, validated.start_bit, validated.bit_length
                            ));
                        }

                        // 5. Data type enumeration
                        let valid_types = [
                            "uint8", "int8", "uint16", "int16", "uint32", "int32", "float32",
                            "float64",
                        ];
                        if !valid_types.contains(&validated.data_type.as_str()) {
                            errors.push(format!(
                                "Point {}: data_type '{}' invalid (valid: {})",
                                mapping.point_id,
                                validated.data_type,
                                valid_types.join(", ")
                            ));
                        }

                        // 6. Byte order enumeration
                        let valid_orders = ["ABCD", "DCBA", "BADC", "CDAB", "AB", "BA"];
                        if !valid_orders.contains(&validated.byte_order.as_str()) {
                            errors.push(format!(
                                "Point {}: byte_order '{}' invalid (valid: {})",
                                mapping.point_id,
                                validated.byte_order,
                                valid_orders.join(", ")
                            ));
                        }
                    },
                    Err(e) => {
                        errors.push(format!(
                            "Point {}: CAN mapping validation failed - {}",
                            mapping.point_id, e
                        ));
                    },
                }
            },
            "virtual" => {
                // Virtual protocol validation
                match serde_json::from_value::<VirtualMappingValidator>(
                    mapping.protocol_data.clone(),
                ) {
                    Ok(validated) => {
                        // Check expression is not empty
                        if validated.expression.trim().is_empty() {
                            errors.push(format!(
                                "Point {}: expression cannot be empty",
                                mapping.point_id
                            ));
                        }
                    },
                    Err(e) => {
                        errors.push(format!(
                            "Point {}: Virtual mapping validation failed - {}",
                            mapping.point_id, e
                        ));
                    },
                }
            },
            other => {
                errors.push(format!("Unsupported protocol: {}", other));
                break; // Protocol error affects all mappings
            },
        }
    }

    errors
}

/// Normalize protocol_data field types to ensure consistent JSON storage
///
/// Ensures numeric fields are stored as JSON numbers (not strings) for consistency.
/// This prevents type mismatches between GET and PUT operations.
///
/// ## Type Rules
/// ### Modbus Protocol
/// - `slave_id`: number
/// - `function_code`: number
/// - `register_address`: number
/// - `bit_position`: number (if present)
/// - `byte_order`: string (unchanged)
/// - `data_type`: string (unchanged)
///
/// ### CAN Protocol
/// - `can_id`: number
/// - `start_bit`: number
/// - `bit_length`: number
/// - `scale`: number
/// - `offset`: number
/// - `byte_order`: string (unchanged)
/// - `data_type`: string (unchanged)
/// - `signed`: boolean (unchanged)
///
/// ### Virtual Protocol
/// - No numeric normalization needed (expression-based)
///
/// @param protocol: Protocol name (modbus_tcp/modbus_rtu/can/virt)
/// @param value: protocol_data JSON value to normalize
/// @return Normalized JSON value with corrected types
fn normalize_protocol_data(protocol: &str, value: &serde_json::Value) -> serde_json::Value {
    use serde_json::{Number, Value};

    let Some(obj) = value.as_object() else {
        // Not an object, return as-is
        return value.clone();
    };

    let mut normalized = obj.clone();

    // Helper: convert string to number if possible
    let to_number = |v: &Value| -> Option<Value> {
        match v {
            Value::Number(n) => Some(Value::Number(n.clone())),
            Value::String(s) => {
                if let Ok(n) = s.parse::<i64>() {
                    Some(Value::Number(Number::from(n)))
                } else if let Ok(f) = s.parse::<f64>() {
                    Number::from_f64(f).map(Value::Number)
                } else {
                    None
                }
            },
            _ => None,
        }
    };

    match protocol {
        "modbus_tcp" | "modbus_rtu" => {
            // Normalize Modbus numeric fields
            let numeric_fields = [
                "slave_id",
                "function_code",
                "register_address",
                "bit_position",
            ];
            for field in numeric_fields {
                if let Some(v) = obj.get(field) {
                    if let Some(normalized_v) = to_number(v) {
                        normalized.insert(field.to_string(), normalized_v);
                    }
                }
            }
        },
        "can" => {
            // Normalize CAN numeric fields
            let numeric_fields = ["can_id", "start_bit", "bit_length", "scale", "offset"];
            for field in numeric_fields {
                if let Some(v) = obj.get(field) {
                    if let Some(normalized_v) = to_number(v) {
                        normalized.insert(field.to_string(), normalized_v);
                    }
                }
            }
        },
        "virt" => {
            // Virtual protocol: no numeric normalization needed
        },
        _ => {
            // Unknown protocol: return as-is
        },
    }

    Value::Object(normalized)
}

/// Validate Modbus function code matches point type (business rule)
///
/// Enforces the Modbus specification requirement that read/write function codes
/// must match the point's data direction:
/// - T/S points (read-only): FC 1/2/3/4
/// - C points (write coils): FC 5/6
/// - A points (write registers): FC 6/16
///
/// Returns Some(error_message) if validation fails, None if valid.
fn validate_modbus_function_code_match(
    function_code: u8,
    four_remote: &str,
    point_id: u32,
) -> Option<String> {
    match four_remote {
        "T" | "S" => {
            // Telemetry/Signal points must use read function codes
            if ![1, 2, 3, 4].contains(&function_code) {
                return Some(format!(
                    "Point {}: {} point requires read FC (1/2/3/4), got FC {} (write)",
                    point_id, four_remote, function_code
                ));
            }
        },
        "C" => {
            // Control points can use coil write (5/15) or register write (6/16)
            if ![5, 6, 15, 16].contains(&function_code) {
                return Some(format!(
                    "Point {}: C point requires write FC (5/6/15/16), got FC {}",
                    point_id, function_code
                ));
            }
        },
        "A" => {
            // Adjustment points must use register write function codes
            if ![6, 16].contains(&function_code) {
                return Some(format!(
                    "Point {}: A point requires register write FC (6/16), got FC {}",
                    point_id, function_code
                ));
            }
        },
        _ => {
            // Invalid four_remote type (should be caught by structural validation)
            return Some(format!(
                "Point {}: invalid four_remote type '{}'",
                point_id, four_remote
            ));
        },
    }

    None // Validation passed
}

/// Helper function to get signal name for a point
#[allow(dead_code)]
async fn get_point_signal_name(pool: &sqlx::SqlitePool, point_id: u32) -> Result<String, AppError> {
    // Try all point tables
    let tables = [
        "telemetry_points",
        "signal_points",
        "control_points",
        "adjustment_points",
    ];

    for table in &tables {
        let query = format!("SELECT signal_name FROM {} WHERE point_id = ?", table);
        let result: Option<(String,)> = sqlx::query_as(&query)
            .bind(point_id as i64)
            .fetch_optional(pool)
            .await
            .map_err(|e| {
                tracing::error!("Error querying {}: {}", table, e);
                AppError::internal_error("Database operation failed")
            })?;

        if let Some((signal_name,)) = result {
            return Ok(signal_name);
        }
    }

    // Point not found in any table, return point_id as fallback
    Ok(format!("point_{}", point_id))
}
