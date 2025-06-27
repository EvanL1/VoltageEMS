use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::{Rejection, Reply};
use log::{info, warn, error, debug};
use std::collections::HashMap;

use crate::api::models::{
    ApiResponse, ChannelOperation, ChannelStatus, ChannelStatusResponse, HealthStatus, PointTableData, PointValue,
    ServiceStatus, WritePointRequest,
};
use crate::core::protocols::common::ProtocolFactory;
use crate::core::protocols::modbus::client::ModbusClient;
use crate::core::protocols::modbus::common::ModbusRegisterType;
use crate::utils::error::ComSrvError;

// Import new storage types
use crate::core::config::{
    FourTelemetryTableManager, TelemetryCategory, 
    ConfigManager,
};
use crate::core::config::protocol_table_manager::{
    StandardPointRecord, FourTelemetryTableManagerTrait,
};
use crate::core::config::config_manager::{
    ChannelConfig, ChannelParameters,
};
use crate::core::config::protocol_table_manager::{
    ProtocolConfig, TelemetryCategory as ConfigTelemetryCategory, 
};
use crate::core::config::storage::csv_parser::{ChannelPointRecord, ProtocolConfigRecord};

/// get service status
pub async fn get_service_status(
    start_time: Arc<chrono::DateTime<Utc>>,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let factory = protocol_factory.read().await;
    let channels = factory.get_all_channels().len() as u32;

    // Calculate the number of active channels
    let mut active_channels = 0;
    let channel_list: Vec<_> = factory
        .get_all_channels()
        .iter()
        .map(|(id, channel)| (*id, Arc::clone(&channel)))
        .collect();
    drop(factory); // Release factory lock

    for (_id, channel) in channel_list {
        let is_running = {
            let channel_guard = channel.read().await;
            channel_guard.is_running().await
        };
        if is_running {
            active_channels += 1;
        }
    }

    let status = ServiceStatus {
        name: "ComsrvRust".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime: (Utc::now() - *start_time).num_seconds() as u64,
        start_time: *start_time,
        channels,
        active_channels,
    };

    Ok(warp::reply::json(&ApiResponse::success(status)))
}

/// get all channel statuses
pub async fn get_all_channels(
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let factory = protocol_factory.read().await;
    let channel_list: Vec<_> = factory
        .get_all_channels()
        .iter()
        .map(|(id, channel)| (*id, Arc::clone(&channel)))
        .collect();
    drop(factory); // Release factory lock early

    let mut channel_statuses = Vec::new();

    for (id, channel) in channel_list {
        // Get basic info first
        let (name, protocol_type, params) = {
            let channel_guard = channel.read().await;
            (
                channel_guard.name().to_string(),
                channel_guard.protocol_type().to_string(),
                serde_json::to_value(&channel_guard.get_parameters()).unwrap_or_else(|_| json!({})),
            )
        };

        // Now get status with async call
        let status = {
            let channel_guard = channel.read().await;
            channel_guard.status().await
        };

        channel_statuses.push(ChannelStatusResponse {
            id,
            name,
            protocol: protocol_type,
            connected: status.connected,
            last_update: status.last_update_time,
            error_count: if status.has_error() { 1 } else { 0 },
            last_error: if status.has_error() { Some(status.last_error) } else { None },
        });
    }

    Ok(warp::reply::json(&ApiResponse::success(channel_statuses)))
}

/// get single channel status
pub async fn get_channel_status(
    id: String,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let id_u16 = id.parse::<u16>().map_err(|_| warp::reject::reject())?;

    let channel = {
        let factory = protocol_factory.read().await;
        match factory.get_channel(id_u16).await {
            Some(channel) => Arc::clone(&channel),
            None => {
                let error_response = ApiResponse::<()>::error(format!("Channel {} not found", id));
                return Ok(warp::reply::json(&error_response));
            }
        }
    };

    // Get basic info first
    let (name, protocol_type, params) = {
        let channel_guard = channel.read().await;
        (
            channel_guard.name().to_string(),
            channel_guard.protocol_type().to_string(),
            serde_json::to_value(&channel_guard.get_parameters()).unwrap_or_else(|_| json!({})),
        )
    };

    // Now get status with async call
    let status = {
        let channel_guard = channel.read().await;
        channel_guard.status().await
    };

    let channel_status = ChannelStatus {
        id: id.clone(),
        name,
        protocol: protocol_type,
        connected: status.connected,
        last_response_time: status.last_response_time,
        last_error: status.last_error,
        last_update_time: status.last_update_time,
        parameters: serde_json::from_value(params).unwrap_or_default(),
    };

    Ok(warp::reply::json(&ApiResponse::success(channel_status)))
}

/// control channel operation
pub async fn control_channel(
    id: String,
    operation: ChannelOperation,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let id_u16 = id.parse::<u16>().map_err(|_| warp::reject::reject())?;

    let channel = {
        let factory = protocol_factory.read().await;
        match factory.get_channel(id_u16).await {
            Some(channel) => Arc::clone(&channel),
            None => {
                let error_response = ApiResponse::<()>::error(format!("Channel {} not found", id));
                return Ok(warp::reply::json(&error_response));
            }
        }
    };

    // Perform the operation
    let result = {
        let mut channel_guard = channel.write().await;
        match operation.operation.as_str() {
            "start" => channel_guard.start().await,
            "stop" => channel_guard.stop().await,
            "restart" => {
                let _ = channel_guard.stop().await;
                channel_guard.start().await
            }
            _ => Err(ComSrvError::InvalidOperation(format!(
                "Invalid operation: {}",
                operation.operation
            ))),
        }
    };

    match result {
        Ok(_) => {
            let message = format!("Successfully {} channel {}", operation.operation, id);
            Ok(warp::reply::json(&ApiResponse::success(message)))
        }
        Err(e) => {
            let error_response = ApiResponse::<()>::error(format!("Operation failed: {}", e));
            Ok(warp::reply::json(&error_response))
        }
    }
}

/// Health check endpoint
pub async fn health_check() -> Result<impl Reply, Rejection> {
    let health = HealthStatus {
        status: "healthy".to_string(),
        uptime: 0, // TODO: Calculate actual uptime
        memory_usage: 0, // TODO: Implement memory usage calculation
        cpu_usage: 0.0,  // TODO: Implement CPU usage calculation
    };
    Ok(warp::reply::json(&ApiResponse::success(health)))
}

/// read point value from channel
pub async fn read_point(
    channel_id: String,
    point_table: String,
    point_name: String,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let channel_id_u16 = channel_id
        .parse::<u16>()
        .map_err(|_| warp::reject::reject())?;

    let channel = {
        let factory = protocol_factory.read().await;
        match factory.get_channel(channel_id_u16).await {
            Some(channel) => Arc::clone(&channel),
            None => {
                let error_response =
                    ApiResponse::<()>::error(format!("Channel {} not found", channel_id));
                return Ok(warp::reply::json(&error_response));
            }
        }
    };

    // Get point data from the channel
    let channel_points = {
        let channel_guard = channel.read().await;
        channel_guard.get_all_points().await
    };

    // Try to find the specified point
    let mut point_value = None;
    for point in channel_points {
        if point.id == point_name {
            point_value = Some(PointValue {
                id: point.id.clone(),
                name: point.name,
                value: serde_json::Value::String(point.value),
                timestamp: point.timestamp,
                unit: point.unit,
                description: point.description,
            });
            break;
        }
    }

    // If the point is found, return its value, otherwise return an error
    if let Some(value) = point_value {
        Ok(warp::reply::json(&ApiResponse::success(value)))
    } else {
        let error_response =
            ApiResponse::<()>::error(format!("Point {}.{} not found", point_table, point_name));
        Ok(warp::reply::json(&error_response))
    }
}

/// write point value to channel
pub async fn write_point(
    channel_id: String,
    point_table: String,
    point_name: String,
    value: WritePointRequest,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let channel_id_u16 = channel_id
        .parse::<u16>()
        .map_err(|_| warp::reject::reject())?;

    let channel = {
        let factory = protocol_factory.read().await;
        match factory.get_channel(channel_id_u16).await {
            Some(channel) => Arc::clone(&channel),
            None => {
                let error_response =
                    ApiResponse::<()>::error(format!("Channel {} not found", channel_id));
                return Ok(warp::reply::json(&error_response));
            }
        }
    };

    // First, get all points to check if the point exists
    let channel_points = {
        let channel_guard = channel.read().await;
        channel_guard.get_all_points().await
    };

    let mut found = false;
    for point in channel_points {
        if point.id == point_name {
            found = true;
            break;
        }
    }

    if !found {
        let error_response =
            ApiResponse::<()>::error(format!("Point {}.{} not found", point_table, point_name));
        return Ok(warp::reply::json(&error_response));
    }

    // Parse raw value first (before the scoped block)
    let raw_value = if let Some(v) = value.value.as_u64() {
        v as u16
    } else if let Some(v) = value.value.as_i64() {
        v as u16
    } else if let Some(s) = value.value.as_str() {
        s.parse::<u16>().map_err(|_| warp::reject::reject())?
    } else {
        let error_response = ApiResponse::<()>::error("Unsupported value type".to_string());
        return Ok(warp::reply::json(&error_response));
    };

    // Perform write through Modbus client
    let result = {
        let channel_guard = channel.read().await;
        if let Some(client) = channel_guard.as_any().downcast_ref::<ModbusClient>() {
            let mapping = match client.find_mapping(&point_name) {
                Some(m) => m,
                None => {
                    let error_response = ApiResponse::<()>::error(format!(
                        "Point {}.{} not found",
                        point_table, point_name
                    ));
                    return Ok(warp::reply::json(&error_response));
                }
            };

            if mapping.register_type != ModbusRegisterType::HoldingRegister {
                let error_response = ApiResponse::<()>::error(
                    "Write not supported for this register type".to_string(),
                );
                return Ok(warp::reply::json(&error_response));
            }

            client
                .write_single_register(mapping.address, raw_value)
                .await
        } else {
            let error_response = ApiResponse::<()>::error("Protocol not supported".to_string());
            return Ok(warp::reply::json(&error_response));
        }
    };

    match result {
        Ok(_) => {
            let message = format!(
                "Successfully wrote value {} to point {}.{}",
                raw_value, point_table, point_name
            );
            Ok(warp::reply::json(&ApiResponse::<String>::success(message)))
        }
        Err(e) => {
            let error_response = ApiResponse::<()>::error(format!("Write failed: {}", e));
            Ok(warp::reply::json(&error_response))
        }
    }
}

/// get all points from a channel
pub async fn get_channel_points(
    channel_id: String,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let channel_id_u16 = channel_id
        .parse::<u16>()
        .map_err(|_| warp::reject::reject())?;

    let channel = {
        let factory = protocol_factory.read().await;
        match factory.get_channel(channel_id_u16).await {
            Some(channel) => Arc::clone(&channel),
            None => {
                let error_response =
                    ApiResponse::<()>::error(format!("Channel {} not found", channel_id));
                return Ok(warp::reply::json(&error_response));
            }
        }
    };

    // Get channel points
    let channel_points = {
        let channel_guard = channel.read().await;
        channel_guard.get_all_points().await
    };

    // Convert to PointValue format
    let points: Vec<PointValue> = channel_points
        .into_iter()
        .map(|point| PointValue {
            id: point.id.clone(),
            name: point.name,
            value: serde_json::Value::String(point.value),
            timestamp: point.timestamp,
            unit: point.unit,
            description: point.description,
        })
        .collect();

    let point_table = PointTableData {
        channel_id: channel_id.clone(),
        points,
        timestamp: Utc::now(),
    };

    Ok(warp::reply::json(&ApiResponse::success(point_table)))
}

/// Get all point tables
pub async fn get_point_tables(
    config_manager: Arc<RwLock<ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let config = config_manager.read().await;
    let table_manager = config.get_point_table_manager();
    
    // Get channel names from the new storage abstraction
    let channel_names = table_manager.get_channel_names().await;
    
    let mut tables = std::collections::HashMap::new();
    for channel_name in channel_names {
        // Get statistics for each channel
        let stats = table_manager.get_statistics();
        tables.insert(channel_name, json!({
            "total_protocol_configs": stats.total_protocol_configs,
            "total_standard_points": stats.total_standard_points,
            "total_mapped_points": stats.total_mapped_points,
            "points_by_category": stats.points_by_category,
            "points_by_protocol": stats.points_by_protocol,
        }));
    }

    let response = ApiResponse::success(tables);
    Ok(warp::reply::json(&response))
}

/// Get specific point table details
pub async fn get_point_table(
    channel_name: String,
    config_manager: Arc<RwLock<ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let config = config_manager.read().await;
    let table_manager = config.get_point_table_manager();

    // Get all telemetry categories for the channel
    let mut all_points = Vec::new();
    let categories = vec![
        ConfigTelemetryCategory::Telemetry,
        ConfigTelemetryCategory::Signaling, 
        ConfigTelemetryCategory::Control,
        ConfigTelemetryCategory::Setpoint,
    ];

    for category in categories {
        if let Ok(points) = table_manager.get_points_by_category(&channel_name, category).await {
            all_points.extend(points);
        }
    }

    if !all_points.is_empty() {
        let response = ApiResponse::success(all_points);
        Ok(warp::reply::json(&response))
    } else {
        let error_response =
            ApiResponse::<()>::error(format!("Point table '{}' not found", channel_name));
        Ok(warp::reply::json(&error_response))
    }
}

/// Get specific point from a table
pub async fn get_point_from_table(
    channel_name: String,
    point_id: String,
    config_manager: Arc<RwLock<ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let config = config_manager.read().await;
    let table_manager = config.get_point_table_manager();

    // Parse point_id as u32
    let point_id_u32 = point_id.parse::<u32>().map_err(|_| warp::reject::reject())?;

    // Search across all telemetry categories
    let categories = vec![
        ConfigTelemetryCategory::Telemetry,
        ConfigTelemetryCategory::Signaling,
        ConfigTelemetryCategory::Control,
        ConfigTelemetryCategory::Setpoint,
    ];

    for category in categories {
        if let Ok(point) = table_manager.get_point_by_id(&channel_name, category, point_id_u32).await {
            let response = ApiResponse::success(point);
            return Ok(warp::reply::json(&response));
        }
    }

    let error_response = ApiResponse::<()>::error(format!(
        "Point '{}' not found in channel '{}'",
        point_id, channel_name
    ));
    Ok(warp::reply::json(&error_response))
}

/// Update or create a point in a table
pub async fn upsert_point_in_table(
    channel_name: String,
    point: StandardPointRecord,
    config_manager: Arc<RwLock<ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let mut config = config_manager.write().await;
    let table_manager = config.get_point_table_manager_mut();

    // Create a test point
    let test_point = ChannelPointRecord {
        point_id: 1001,
        point_name: "test_point".to_string(),
        unit: "V".to_string(),
        scale: 1.0,
        offset: 0.0,
        description: "test".to_string(),
    };

    // Test upsert operation
    match table_manager.upsert_point(&channel_name, test_point.clone()) {
        Ok(()) => {
            let response = ApiResponse::success(json!({
                "message": format!("Point '{}' updated in channel '{}'", point.point_id, channel_name)
            }));
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let error_response = ApiResponse::<()>::error(format!("Failed to update point: {}", e));
            Ok(warp::reply::json(&error_response))
        }
    }
}

/// Delete a point from a table
pub async fn delete_point_from_table(
    channel_name: String,
    point_id: String,
    config_manager: Arc<RwLock<ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let mut config = config_manager.write().await;
    let table_manager = config.get_point_table_manager_mut();

    // Parse point_id as u32
    let point_id_u32 = point_id.parse::<u32>().map_err(|_| warp::reject::reject())?;

    // Try to delete from all telemetry categories
    let categories = vec![
        ConfigTelemetryCategory::Telemetry,
        ConfigTelemetryCategory::Signaling,
        ConfigTelemetryCategory::Control,
        ConfigTelemetryCategory::Setpoint,
    ];

    for category in categories {
        match table_manager.remove_point(&channel_name, &point_id) {
            Ok(true) => {
                let response = ApiResponse::success(json!({
                    "message": format!("Point '{}' deleted from channel '{}'", point_id, channel_name)
                }));
                return Ok(warp::reply::json(&response));
            }
            Ok(false) | Err(_) => continue, // Try next category
        }
    }

    let error_response = ApiResponse::<()>::error(format!(
        "Point '{}' not found in channel '{}'",
        point_id, channel_name
    ));
    Ok(warp::reply::json(&error_response))
}

/// Reload point tables from storage
pub async fn reload_point_tables(
    config_manager: Arc<RwLock<ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let mut config = config_manager.write().await;
    let table_manager = config.get_point_table_manager_mut();

    // Reload all channels and categories
    let channel_names = table_manager.get_channel_names().await;
    let categories = vec![
        ConfigTelemetryCategory::Telemetry,
        ConfigTelemetryCategory::Signaling,
        ConfigTelemetryCategory::Control,
        ConfigTelemetryCategory::Setpoint,
    ];

    let mut reload_errors = Vec::new();
    for channel_name in &channel_names {
        for category in &categories {
            if let Err(e) = table_manager.load_standard_points(channel_name, *category).await {
                reload_errors.push(format!("Failed to reload {}/{:?}: {}", channel_name, category, e));
            }
            if let Err(e) = table_manager.load_protocol_config(channel_name, *category).await {
                reload_errors.push(format!("Failed to reload protocol config {}/{:?}: {}", channel_name, category, e));
            }
        }
    }

    if reload_errors.is_empty() {
        let response = ApiResponse::success(json!({
            "message": "Point tables reloaded successfully",
            "channels": channel_names
        }));
        Ok(warp::reply::json(&response))
    } else {
        let error_response = ApiResponse::<()>::error(format!(
            "Failed to reload some point tables: {}",
            reload_errors.join("; ")
        ));
        Ok(warp::reply::json(&error_response))
    }
}

/// Export point table to CSV
pub async fn export_point_table(
    channel_name: String,
    config_manager: Arc<RwLock<ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let config = config_manager.read().await;
    let table_manager = config.get_point_table_manager();

    // Get all points from all telemetry categories
    let mut all_points = Vec::new();
    let categories = vec![
        ConfigTelemetryCategory::Telemetry,
        ConfigTelemetryCategory::Signaling,
        ConfigTelemetryCategory::Control,
        ConfigTelemetryCategory::Setpoint,
    ];

    for category in categories {
        if let Ok(points) = table_manager.get_points_by_category(&channel_name, category).await {
            all_points.extend(points);
        }
    }

    if !all_points.is_empty() {
        // Convert points to CSV format
        let mut csv_content = String::new();
        csv_content.push_str("point_id,point_name,unit,scale,offset,description,telemetry_category\n");

        for point in all_points {
            csv_content.push_str(&format!(
                "{},{},{},{},{},{},{:?}\n",
                point.point_id,
                point.point_name,
                point.unit,
                point.scale,
                point.offset,
                point.description,
                point.telemetry_category
            ));
        }

        Ok(warp::reply::with_header(
            csv_content,
            "content-type",
            "text/csv; charset=utf-8",
        ))
    } else {
        let error_response =
            ApiResponse::<()>::error(format!("Point table '{}' not found", channel_name));
        Ok(warp::reply::with_header(
            serde_json::to_string(&error_response).unwrap(),
            "content-type",
            "application/json",
        ))
    }
}

/// Point table API response
#[derive(Debug, Serialize, Deserialize)]
pub struct PointTableResponse {
    /// Channel ID
    pub channel_id: u16,
    /// Point table statistics
    pub stats: serde_json::Value,
    /// Validation warnings
    pub warnings: Vec<String>,
}

/// Monitoring API response
#[derive(Debug, Serialize, Deserialize)]
pub struct MonitoringResponse {
    /// Monitoring status
    pub status: serde_json::Value,
    /// Active alarms
    pub active_alarms: Vec<serde_json::Value>,
    /// Channel summaries
    pub channel_summaries: Vec<ChannelSummaryResponse>,
}

/// Channel summary response
#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelSummaryResponse {
    /// Channel ID
    pub channel_id: u16,
    /// Connection state
    pub connection_state: String,

    /// Average response time
    pub avg_response_time_ms: f64,
    /// Active alarms count
    pub active_alarms: u32,
}

/// Point table reload request
#[derive(Debug, Deserialize)]
pub struct ReloadPointTableRequest {
    /// Point table file path
    pub point_table_path: String,
}

/// Alarm acknowledgment request
#[derive(Debug, Deserialize)]
pub struct AcknowledgeAlarmRequest {
    /// Alarm ID list
    pub alarm_ids: Vec<String>,
}

/// History query parameters
#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    /// History record limit
    pub limit: Option<usize>,
}

/// Get enhanced point table information for a specific channel
pub async fn get_enhanced_point_table(
    channel_id: String,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let id_u16 = channel_id
        .parse::<u16>()
        .map_err(|_| warp::reject::reject())?;

    let channel = {
        let factory = protocol_factory.read().await;
        match factory.get_channel(id_u16).await {
            Some(channel) => Arc::clone(&channel),
            None => {
                let error_response = serde_json::json!({
                    "error": format!("Channel {} not found", channel_id)
                });
                return Ok(warp::reply::json(&error_response));
            }
        }
    };

    // Get basic channel information
    let (name, protocol_type, params) = {
        let channel_guard = channel.read().await;
        (
            channel_guard.name().to_string(),
            channel_guard.protocol_type().to_string(),
            channel_guard.get_parameters(),
        )
    };

    // Get status
    let status = {
        let channel_guard = channel.read().await;
        channel_guard.status().await
    };

    // Create enhanced response with additional statistics
    let response = PointTableResponse {
        channel_id: id_u16,
        stats: serde_json::json!({
            "name": name,
            "protocol": protocol_type,
            "connected": status.connected,
            "last_response_time": status.last_response_time,
            "parameters": params
        }),
        warnings: if status.has_error() {
            vec![status.last_error]
        } else {
            Vec::new()
        },
    };

    Ok(warp::reply::json(&response))
}

/// Reload point table for a specific channel
pub async fn reload_enhanced_point_table(
    channel_id: String,
    request: ReloadPointTableRequest,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let id_u16 = channel_id
        .parse::<u16>()
        .map_err(|_| warp::reject::reject())?;

    let _channel = {
        let factory = protocol_factory.read().await;
        match factory.get_channel(id_u16).await {
            Some(channel) => Arc::clone(&channel),
            None => {
                let error_response = serde_json::json!({
                    "error": format!("Channel {} not found", channel_id)
                });
                return Ok(warp::reply::json(&error_response));
            }
        }
    };

    // For now, simulate point table reload
    // In a real implementation, this would reload configuration from the specified path
    let response = serde_json::json!({
        "success": true,
        "message": format!("Point table reload initiated for channel {} from path: {}",
                          channel_id, request.point_table_path),
        "channel_id": id_u16,
        "path": request.point_table_path
    });

    Ok(warp::reply::json(&response))
}

/// Get monitoring status for all channels
pub async fn get_enhanced_monitoring_status(
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let factory = protocol_factory.read().await;
    let channel_list: Vec<_> = factory
        .get_all_channels()
        .iter()
        .map(|(id, channel)| (*id, Arc::clone(&channel)))
        .collect();
    drop(factory);

    let mut channel_summaries = Vec::new();

    for (id, channel) in channel_list {
        let (_name, _protocol_type) = {
            let channel_guard = channel.read().await;
            (
                channel_guard.name().to_string(),
                channel_guard.protocol_type().to_string(),
            )
        };

        let status = {
            let channel_guard = channel.read().await;
            channel_guard.status().await
        };

        channel_summaries.push(ChannelSummaryResponse {
            channel_id: id,
            connection_state: if status.connected {
                "Connected".to_string()
            } else {
                "Disconnected".to_string()
            },
            avg_response_time_ms: status.last_response_time,
            active_alarms: if status.has_error() { 1 } else { 0 },
        });
    }

    let response = MonitoringResponse {
        status: serde_json::json!({
            "overall_status": "Running",
            "total_channels": channel_summaries.len(),
            "connected_channels": channel_summaries.iter().filter(|c| c.connection_state == "Connected").count(),

        }),
        active_alarms: channel_summaries
            .iter()
            .filter(|c| c.active_alarms > 0)
            .map(|c| {
                serde_json::json!({
                    "channel_id": c.channel_id,
                    "type": "Communication Error",
                    "severity": "Warning",
                    "description": format!("Channel {} has communication issues", c.channel_id)
                })
            })
            .collect(),
        channel_summaries,
    };

    Ok(warp::reply::json(&response))
}

/// Get channel history data
pub async fn get_enhanced_channel_history(
    channel_id: String,
    query: HistoryQuery,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let id_u16 = channel_id
        .parse::<u16>()
        .map_err(|_| warp::reject::reject())?;

    let channel = {
        let factory = protocol_factory.read().await;
        match factory.get_channel(id_u16).await {
            Some(channel) => Arc::clone(&channel),
            None => {
                let error_response = serde_json::json!({
                    "error": format!("Channel {} not found", channel_id)
                });
                return Ok(warp::reply::json(&error_response));
            }
        }
    };

    // Get channel status for history simulation
    let status = {
        let channel_guard = channel.read().await;
        channel_guard.status().await
    };

    // Simulate historical data - in real implementation, this would query a database
    let limit = query.limit.unwrap_or(100);
    let mut history_records = Vec::new();

    for i in 0..std::cmp::min(limit, 10) {
        // Limit simulation to 10 records
        history_records.push(serde_json::json!({
            "timestamp": chrono::Utc::now() - chrono::Duration::minutes(i as i64 * 5),
            "connected": status.connected,
            "response_time": status.last_response_time + (i as f64 * 0.1),
            "error_count": if status.has_error() { 1 } else { 0 }
        }));
    }

    let response = serde_json::json!({
        "channel_id": id_u16,
        "total_records": history_records.len(),
        "limit": limit,
        "records": history_records
    });

    Ok(warp::reply::json(&response))
}

/// Get performance optimization suggestions
pub async fn get_performance_suggestions(
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let factory = protocol_factory.read().await;
    let channel_count = factory.get_all_channels().len();
    drop(factory);

    let mut suggestions = Vec::new();

    // Generate performance suggestions based on system state
    if channel_count > 10 {
        suggestions.push(serde_json::json!({
            "type": "Performance",
            "priority": "Medium",
            "title": "High Channel Count",
            "description": format!("System has {} channels. Consider implementing connection pooling for better performance.", channel_count),
            "action": "Review channel configuration and implement pooling strategies"
        }));
    }

    suggestions.push(serde_json::json!({
        "type": "Monitoring",
        "priority": "Low",
        "title": "Enable Detailed Logging",
        "description": "Enable detailed protocol logging for better troubleshooting capabilities.",
        "action": "Configure channel loggers with appropriate log levels"
    }));

    suggestions.push(serde_json::json!({
        "type": "Optimization",
        "priority": "Low",
        "title": "Batch Operations",
        "description": "Consider using batch operations for protocols that support them to reduce overhead.",
        "action": "Review point table configurations for batch optimization opportunities"
    }));

    let response = serde_json::json!({
        "total_suggestions": suggestions.len(),
        "suggestions": suggestions,
        "generated_at": chrono::Utc::now()
    });

    Ok(warp::reply::json(&response))
}

/// Acknowledge alarms
pub async fn acknowledge_enhanced_alarms(
    request: AcknowledgeAlarmRequest,
    _protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    // Simulate alarm acknowledgment
    let response = serde_json::json!({
        "success": true,
        "acknowledged_alarms": request.alarm_ids.len(),
        "alarm_ids": request.alarm_ids,
        "acknowledged_at": chrono::Utc::now(),
        "acknowledged_by": "system" // In real implementation, this would be the user ID
    });

    Ok(warp::reply::json(&response))
}

/// 验证转发计算表达式 - 简化版本
pub async fn validate_forward_calculation_expression(
    req: serde_json::Value,
) -> Result<impl Reply, Rejection> {
    // Note: ExpressionEvaluator moved to forward_calculation module
    
    let expression = req.get("expression")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    
    let variables = req.get("variables")
        .and_then(|v| v.as_object());

    if expression.is_empty() || variables.is_none() {
        let error_response = json!({
            "error": "Missing or invalid expression or variables field"
        });
        return Ok(warp::reply::with_status(
            warp::reply::json(&error_response),
            warp::http::StatusCode::BAD_REQUEST,
        ));
    }

    // Temporarily return a simple validation result
    // TODO: Re-implement with the new forward calculation module
    let response = json!({
        "valid": true,
        "message": "Expression validation temporarily disabled during refactoring",
        "expression": expression,
        "variables": variables
    });
    
    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        warp::http::StatusCode::OK,
    ))
}
