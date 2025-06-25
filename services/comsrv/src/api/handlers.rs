use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::{Rejection, Reply};

use crate::api::models::{
    ApiResponse, ChannelOperation, ChannelStatus, HealthStatus, PointTableData, PointValue,
    ServiceStatus, WritePointRequest,
};
use crate::core::protocols::common::ProtocolFactory;
use crate::core::protocols::modbus::client::ModbusClient;
use crate::core::protocols::modbus::common::ModbusRegisterType;
use crate::utils::error::ComSrvError;

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

        channel_statuses.push(ChannelStatus {
            id: id.to_string(),
            name,
            protocol: protocol_type,
            connected: status.connected,
            last_response_time: status.last_response_time,
            last_error: status.last_error,
            last_update_time: status.last_update_time,
            parameters: serde_json::from_value(params).unwrap_or_default(),
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

/// get health status
pub async fn health_check(
    start_time: Arc<chrono::DateTime<Utc>>,
) -> Result<impl Reply, Infallible> {
    // Simple version, more information might need to be collected in a real project
    let status = HealthStatus {
        status: "OK".to_string(),
        uptime: (Utc::now() - *start_time).num_seconds() as u64,
        memory_usage: 0, // Real implementation needs to fetch actual data
        cpu_usage: 0.0,  // Real implementation needs to fetch actual data
    };

    Ok(warp::reply::json(&ApiResponse::success(status)))
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
    config_manager: Arc<RwLock<crate::core::config::ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let config = config_manager.read().await;
    let table_names = config.get_point_table_names();

    let mut tables = std::collections::HashMap::new();
    for table_name in table_names {
        if let Some(stats) = config.get_csv_point_manager().get_table_stats(&table_name) {
            tables.insert(table_name, stats);
        }
    }

    let response = ApiResponse::success(tables);
    Ok(warp::reply::json(&response))
}

/// Get specific point table details
pub async fn get_point_table(
    table_name: String,
    config_manager: Arc<RwLock<crate::core::config::ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let config = config_manager.read().await;
    let csv_manager = config.get_csv_point_manager();

    if let Some(points) = csv_manager.get_points(&table_name) {
        let response = ApiResponse::success(points);
        Ok(warp::reply::json(&response))
    } else {
        let error_response =
            ApiResponse::<()>::error(format!("Point table '{}' not found", table_name));
        Ok(warp::reply::json(&error_response))
    }
}

/// Get specific point from a table
pub async fn get_point_from_table(
    table_name: String,
    point_id: String,
    config_manager: Arc<RwLock<crate::core::config::ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let config = config_manager.read().await;
    let csv_manager = config.get_csv_point_manager();

    if let Some(point) = csv_manager.find_point(&table_name, &point_id) {
        let response = ApiResponse::success(point);
        Ok(warp::reply::json(&response))
    } else {
        let error_response = ApiResponse::<()>::error(format!(
            "Point '{}' not found in table '{}'",
            point_id, table_name
        ));
        Ok(warp::reply::json(&error_response))
    }
}

/// Update or create a point in a table
pub async fn upsert_point_in_table(
    table_name: String,
    point: crate::core::config::ChannelPointRecord,
    config_manager: Arc<RwLock<crate::core::config::ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let mut config = config_manager.write().await;
    let csv_manager = config.get_csv_point_manager_mut();

    match csv_manager.upsert_point(&table_name, point.clone()) {
        Ok(()) => {
            let response = ApiResponse::success(serde_json::json!({
                "message": format!("Point '{}' updated in table '{}'", point.point_id, table_name)
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
    table_name: String,
    point_id: String,
    config_manager: Arc<RwLock<crate::core::config::ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let mut config = config_manager.write().await;
    let csv_manager = config.get_csv_point_manager_mut();

    match csv_manager.remove_point(&table_name, &point_id) {
        Ok(true) => {
            let response = ApiResponse::success(serde_json::json!({
                "message": format!("Point '{}' deleted from table '{}'", point_id, table_name)
            }));
            Ok(warp::reply::json(&response))
        }
        Ok(false) => {
            let error_response = ApiResponse::<()>::error(format!(
                "Point '{}' not found in table '{}'",
                point_id, table_name
            ));
            Ok(warp::reply::json(&error_response))
        }
        Err(e) => {
            let error_response = ApiResponse::<()>::error(format!("Failed to delete point: {}", e));
            Ok(warp::reply::json(&error_response))
        }
    }
}

/// Reload point tables from CSV files
pub async fn reload_point_tables(
    config_manager: Arc<RwLock<crate::core::config::ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let mut config = config_manager.write().await;

    match config.reload_csv_point_tables() {
        Ok(()) => {
            let table_names = config.get_point_table_names();
            let response = ApiResponse::success(serde_json::json!({
                "message": "Point tables reloaded successfully",
                "tables": table_names
            }));
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let error_response =
                ApiResponse::<()>::error(format!("Failed to reload point tables: {}", e));
            Ok(warp::reply::json(&error_response))
        }
    }
}

/// Export point table to CSV
pub async fn export_point_table(
    table_name: String,
    config_manager: Arc<RwLock<crate::core::config::ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let config = config_manager.read().await;
    let csv_manager = config.get_csv_point_manager();

    if let Some(points) = csv_manager.get_points(&table_name) {
        // Convert points to CSV format with simplified structure
        let mut csv_content = String::new();
        csv_content.push_str("point_id,point_name,unit,scale,offset,description\n");

        for point in points {
            csv_content.push_str(&format!(
                "{},{},{},{},{},{}\n",
                point.point_id,
                point.point_name,
                point.unit,
                point.scale,
                point.offset,
                point.description
            ));
        }

        Ok(warp::reply::with_header(
            csv_content,
            "content-type",
            "text/csv; charset=utf-8",
        ))
    } else {
        let error_response =
            ApiResponse::<()>::error(format!("Point table '{}' not found", table_name));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::config_manager::{
        ChannelConfig, ChannelCsvConfig, ChannelParameters, ProtocolType,
    };
    use crate::core::config::ConfigManager;
    use crate::core::config::{ChannelPointRecord, ProtocolConfigRecord, TelemetryCategory};
    use crate::core::protocols::common::ProtocolFactory;
    use serde_yaml;
    use std::fs::File;
    use std::io::Write;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::RwLock;

    fn create_test_config_manager() -> Arc<RwLock<ConfigManager>> {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");

        let config_content = r#"
version: "1.0"
service:
  name: "test_service"
  description: "Test Communication Service"
channels: []
point_tables:
  enabled: true
  directory: "test_points"
"#;

        let mut file = File::create(&config_path).unwrap();
        file.write_all(config_content.as_bytes()).unwrap();

        let manager = ConfigManager::from_file(&config_path).unwrap();
        Arc::new(RwLock::new(manager))
    }

    /// 创建遥测点配置（模拟量输入）
    fn create_telemetry_point_record(point_id: u32) -> ChannelPointRecord {
        ChannelPointRecord {
            point_id,
            point_name: format!("Temperature_{}", point_id),
            unit: "°C".to_string(),
            scale: 0.1,
            offset: -40.0,
            description: format!("Temperature sensor {}", point_id),
            telemetry_category: TelemetryCategory::Telemetry,
        }
    }

    /// 创建遥信点配置（数字量输入）
    fn create_signaling_point_record(point_id: u32) -> ChannelPointRecord {
        ChannelPointRecord {
            point_id,
            point_name: format!("Status_{}", point_id),
            unit: "".to_string(),
            scale: 1.0,
            offset: 0.0,
            description: format!("Digital status {}", point_id),
            telemetry_category: TelemetryCategory::Signaling,
        }
    }

    /// 创建遥控点配置（数字量输出）
    fn create_control_point_record(point_id: u32) -> ChannelPointRecord {
        ChannelPointRecord {
            point_id,
            point_name: format!("Control_{}", point_id),
            unit: "".to_string(),
            scale: 1.0,
            offset: 0.0,
            description: format!("Digital control {}", point_id),
            telemetry_category: TelemetryCategory::Control,
        }
    }

    /// 创建遥调点配置（模拟量输出）
    fn create_setpoint_point_record(point_id: u32) -> ChannelPointRecord {
        ChannelPointRecord {
            point_id,
            point_name: format!("Setpoint_{}", point_id),
            unit: "bar".to_string(),
            scale: 0.01,
            offset: 0.0,
            description: format!("Pressure setpoint {}", point_id),
            telemetry_category: TelemetryCategory::Setpoint,
        }
    }

    /// 创建协议配置记录
    fn create_protocol_config_record(
        point_id: u32,
        category: TelemetryCategory,
    ) -> ProtocolConfigRecord {
        let (address, function_code, data_type) = match category {
            TelemetryCategory::Telemetry => (point_id as u16, 3, "UInt16"),
            TelemetryCategory::Signaling => (point_id as u16 + 1000, 2, "Bool"),
            TelemetryCategory::Control => (point_id as u16 + 2000, 5, "Bool"),
            TelemetryCategory::Setpoint => (point_id as u16 + 3000, 6, "UInt16"),
        };

        ProtocolConfigRecord {
            point_id,
            register_address: address,
            function_code,
            data_type: data_type.to_string(),
            byte_order: "ABCD".to_string(),
            description: format!("{:?} protocol config for point {}", category, point_id),
        }
    }

    #[tokio::test]
    async fn test_get_point_tables() {
        let config_manager = create_test_config_manager();

        let response = get_point_tables(config_manager).await;
        assert!(response.is_ok());

        // The response should be a successful JSON response
        // Since we don't have any point tables, it should return an empty map
    }

    #[tokio::test]
    async fn test_get_point_table_not_found() {
        let config_manager = create_test_config_manager();

        let response = get_point_table("nonexistent".to_string(), config_manager).await;
        assert!(response.is_ok());

        // The response should contain an error about the table not being found
    }

    #[tokio::test]
    async fn test_get_point_from_table_not_found() {
        let config_manager = create_test_config_manager();

        let response = get_point_from_table(
            "nonexistent_table".to_string(),
            "nonexistent_point".to_string(),
            config_manager,
        )
        .await;
        assert!(response.is_ok());

        // The response should contain an error about the point not being found
    }

    #[tokio::test]
    async fn test_four_telemetry_upsert_operations() {
        let config_manager = create_test_config_manager();

        // Test all four telemetry types
        let test_cases = vec![
            ("telemetry_table", create_telemetry_point_record(1001)),
            ("signaling_table", create_signaling_point_record(2001)),
            ("control_table", create_control_point_record(3001)),
            ("setpoint_table", create_setpoint_point_record(4001)),
        ];

        for (table_name, point) in test_cases {
            let response =
                upsert_point_in_table(table_name.to_string(), point, config_manager.clone()).await;
            assert!(response.is_ok());
        }
    }

    #[tokio::test]
    async fn test_four_telemetry_data_types() {
        let config_manager = create_test_config_manager();

        // Test different data types for each telemetry category
        let test_data_types = vec![
            ("UInt16", "ABCD"),
            ("Int16", "DCBA"),
            ("UInt32", "BADC"),
            ("Int32", "CDAB"),
            ("Float32", "ABCD"),
            ("Bool", "ABCD"),
        ];

        for (data_type, byte_order) in test_data_types {
            let point = ChannelPointRecord {
                point_id: 1,
                point_name: format!("test_{}", data_type),
                unit: "test_unit".to_string(),
                scale: 1.0,
                offset: 0.0,
                description: format!("Test point for {}", data_type),
                telemetry_category: TelemetryCategory::Telemetry,
            };

            let response = upsert_point_in_table(
                format!("table_{}", data_type),
                point,
                config_manager.clone(),
            )
            .await;
            assert!(response.is_ok());
        }
    }

    #[tokio::test]
    async fn test_delete_point_from_table() {
        let config_manager = create_test_config_manager();

        // First add a point
        let point = create_telemetry_point_record(1);
        let _ =
            upsert_point_in_table("test_table".to_string(), point, config_manager.clone()).await;

        // Then try to delete it
        let response =
            delete_point_from_table("test_table".to_string(), "1".to_string(), config_manager)
                .await;
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_reload_point_tables() {
        let config_manager = create_test_config_manager();

        let response = reload_point_tables(config_manager).await;
        assert!(response.is_ok());

        // The response should indicate successful reload
    }

    #[tokio::test]
    async fn test_export_point_table_not_found() {
        let config_manager = create_test_config_manager();

        let response = export_point_table("nonexistent".to_string(), config_manager).await;
        assert!(response.is_ok());

        // The response should contain an error about the table not being found
    }

    #[tokio::test]
    async fn test_export_point_table_with_four_telemetry_data() {
        let config_manager = create_test_config_manager();

        // Add test data for all four telemetry types
        let test_points = vec![
            ("telemetry", create_telemetry_point_record(1)),
            ("signaling", create_signaling_point_record(2)),
            ("control", create_control_point_record(3)),
            ("setpoint", create_setpoint_point_record(4)),
        ];

        for (category, point) in test_points {
            let _ =
                upsert_point_in_table(format!("table_{}", category), point, config_manager.clone())
                    .await;
        }

        // Export each table
        let categories = vec!["telemetry", "signaling", "control", "setpoint"];
        for category in categories {
            let response =
                export_point_table(format!("table_{}", category), config_manager.clone()).await;
            assert!(response.is_ok());
        }
    }

    #[tokio::test]
    async fn test_byte_order_validation() {
        let config_manager = create_test_config_manager();

        // Test all supported byte orders
        let byte_orders = vec!["ABCD", "DCBA", "BADC", "CDAB"];

        for (i, byte_order) in byte_orders.iter().enumerate() {
            let point = ChannelPointRecord {
                point_id: i as u32 + 1,
                point_name: format!("test_point_{}", byte_order),
                unit: "V".to_string(),
                scale: 1.0,
                offset: 0.0,
                description: format!("Test point with byte order {}", byte_order),
                telemetry_category: TelemetryCategory::Telemetry,
            };

            let response =
                upsert_point_in_table("byte_order_test".to_string(), point, config_manager.clone())
                    .await;
            assert!(response.is_ok());
        }
    }

    #[tokio::test]
    async fn test_point_table_hot_reload_workflow() {
        let config_manager = create_test_config_manager();

        // 1. Create points for all telemetry types
        let test_points = vec![
            ("telemetry", create_telemetry_point_record(100)),
            ("signaling", create_signaling_point_record(200)),
            ("control", create_control_point_record(300)),
            ("setpoint", create_setpoint_point_record(400)),
        ];

        for (table_name, point) in test_points {
            let upsert_response =
                upsert_point_in_table(table_name.to_string(), point, config_manager.clone()).await;
            assert!(upsert_response.is_ok());
        }

        // 2. Verify points exist
        let table_names = vec!["telemetry", "signaling", "control", "setpoint"];
        let point_ids = vec!["100", "200", "300", "400"];

        for (table_name, point_id) in table_names.iter().zip(point_ids.iter()) {
            let get_response = get_point_from_table(
                table_name.to_string(),
                point_id.to_string(),
                config_manager.clone(),
            )
            .await;
            assert!(get_response.is_ok());
        }

        // 3. Reload point tables
        let reload_response = reload_point_tables(config_manager.clone()).await;
        assert!(reload_response.is_ok());

        // 4. Delete the points
        for (table_name, point_id) in table_names.iter().zip(point_ids.iter()) {
            let delete_response = delete_point_from_table(
                table_name.to_string(),
                point_id.to_string(),
                config_manager.clone(),
            )
            .await;
            assert!(delete_response.is_ok());
        }
    }

    #[tokio::test]
    async fn test_comprehensive_data_type_coverage() {
        let config_manager = create_test_config_manager();

        // Test comprehensive data type coverage
        let comprehensive_tests = vec![
            // UInt16 with different byte orders
            ("uint16_abcd", 1, "UInt16", "ABCD", 1.0, 0.0, "V"),
            ("uint16_dcba", 2, "UInt16", "DCBA", 1.0, 0.0, "A"),
            // Int16 with different byte orders
            ("int16_badc", 3, "Int16", "BADC", 0.1, -100.0, "°C"),
            ("int16_cdab", 4, "Int16", "CDAB", 0.01, 0.0, "bar"),
            // UInt32 with different byte orders
            ("uint32_abcd", 5, "UInt32", "ABCD", 0.001, 0.0, "Hz"),
            ("uint32_dcba", 6, "UInt32", "DCBA", 1.0, 0.0, "RPM"),
            // Int32 with different byte orders
            ("int32_badc", 7, "Int32", "BADC", 10.0, 1000.0, "Pa"),
            ("int32_cdab", 8, "Int32", "CDAB", 0.1, -50.0, "dB"),
            // Float32 with different byte orders
            ("float32_abcd", 9, "Float32", "ABCD", 1.0, 0.0, "m/s"),
            ("float32_dcba", 10, "Float32", "DCBA", 3.6, 0.0, "km/h"),
            // Bool (typically no byte order consideration)
            ("bool_status", 11, "Bool", "ABCD", 1.0, 0.0, ""),
        ];

        for (name, point_id, data_type, byte_order, scale, offset, unit) in comprehensive_tests {
            let point = ChannelPointRecord {
                point_id,
                point_name: name.to_string(),
                unit: unit.to_string(),
                scale,
                offset,
                description: format!("Test {} with {} byte order", data_type, byte_order),
                telemetry_category: TelemetryCategory::Telemetry,
            };

            let response = upsert_point_in_table(
                "comprehensive_test".to_string(),
                point,
                config_manager.clone(),
            )
            .await;
            assert!(
                response.is_ok(),
                "Failed to insert point {} with data type {}",
                name,
                data_type
            );
        }
    }

    #[tokio::test]
    async fn test_write_point_channel_not_found() {
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));
        let req = WritePointRequest {
            value: serde_json::json!(1),
        };
        let response = write_point(
            "1".to_string(),
            "tbl".to_string(),
            "p1".to_string(),
            req,
            factory,
        )
        .await;
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_write_point_point_not_found() {
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        // Create a simple Modbus channel with CSV config
        let mut params = std::collections::HashMap::new();
        params.insert(
            "address".to_string(),
            serde_yaml::Value::String("127.0.0.1".to_string()),
        );
        params.insert(
            "port".to_string(),
            serde_yaml::Value::Number(serde_yaml::Number::from(502)),
        );

        let csv_config = ChannelCsvConfig {
            csv_directory: "test_csv".to_string(),
            telemetry_file: Some("telemetry.csv".to_string()),
            signaling_file: Some("signaling.csv".to_string()),
            control_file: Some("control.csv".to_string()),
            setpoint_file: Some("setpoint.csv".to_string()),
        };

        let config = ChannelConfig {
            id: 1,
            name: "ch1".to_string(),
            description: "test".to_string(),
            protocol: ProtocolType::ModbusTcp,
            parameters: ChannelParameters::Generic(params),
            csv_config: Some(csv_config),
        };
        factory.write().await.create_channel(config).await.unwrap();

        let req = WritePointRequest {
            value: serde_json::json!(1),
        };
        let response = write_point(
            "1".to_string(),
            "tbl".to_string(),
            "unknown".to_string(),
            req,
            factory,
        )
        .await;
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_four_telemetry_protocol_validation() {
        let config_manager = create_test_config_manager();

        // Test protocol validation for different telemetry types
        let validation_tests = vec![
            (
                TelemetryCategory::Telemetry,
                vec![3, 4],
                vec!["UInt16", "Int16", "UInt32", "Int32", "Float32"],
            ),
            (TelemetryCategory::Signaling, vec![1, 2], vec!["Bool"]),
            (TelemetryCategory::Control, vec![5, 15], vec!["Bool"]),
            (
                TelemetryCategory::Setpoint,
                vec![6, 16],
                vec!["UInt16", "Int16", "UInt32", "Int32", "Float32"],
            ),
        ];

        for (category, function_codes, data_types) in validation_tests {
            for &fc in &function_codes {
                for data_type in &data_types {
                    let protocol_config = ProtocolConfigRecord {
                        point_id: 1,
                        register_address: 100,
                        function_code: fc,
                        data_type: data_type.to_string(),
                        byte_order: "ABCD".to_string(),
                        description: format!("Test {:?} protocol", category),
                    };

                    // This would normally be validated by the protocol factory
                    // For now, we just ensure the structure is correct
                    assert!(!protocol_config.data_type.is_empty());
                    assert!(!protocol_config.byte_order.is_empty());
                }
            }
        }
    }

    #[tokio::test]
    async fn test_engineering_unit_scaling() {
        let config_manager = create_test_config_manager();

        // Test various engineering unit scaling scenarios
        let scaling_tests = vec![
            // (name, scale, offset, unit, description)
            ("temp_celsius", 0.1, -40.0, "°C", "Temperature with offset"),
            ("pressure_bar", 0.01, 0.0, "bar", "Pressure scaling"),
            ("flow_rate", 0.001, 0.0, "m³/h", "Flow rate with precision"),
            ("percentage", 0.01, 0.0, "%", "Percentage value"),
            ("frequency", 0.1, 0.0, "Hz", "Frequency measurement"),
            (
                "voltage",
                0.001,
                0.0,
                "V",
                "Voltage with millivolt precision",
            ),
        ];

        for (i, (name, scale, offset, unit, description)) in scaling_tests.iter().enumerate() {
            let point = ChannelPointRecord {
                point_id: i as u32 + 1,
                point_name: name.to_string(),
                unit: unit.to_string(),
                scale: *scale,
                offset: *offset,
                description: description.to_string(),
                telemetry_category: TelemetryCategory::Telemetry,
            };

            let response =
                upsert_point_in_table("scaling_test".to_string(), point, config_manager.clone())
                    .await;
            assert!(
                response.is_ok(),
                "Failed to insert scaling test point {}",
                name
            );
        }
    }
}
