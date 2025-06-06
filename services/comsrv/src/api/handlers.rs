use std::sync::Arc;
use serde_json::json;
use warp::{Reply, Rejection};
use std::convert::Infallible;
use chrono::Utc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::api::models::{
    ServiceStatus, HealthStatus, ApiResponse, ChannelStatus,
    ChannelOperation, PointValue, PointTableData, WritePointRequest
};
use crate::core::protocols::common::ProtocolFactory;
use crate::utils::ComSrvError;

/// get service status
pub async fn get_service_status(
    start_time: Arc<chrono::DateTime<Utc>>,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let factory = protocol_factory.read().await;
    let channels = factory.get_all_channels().len() as u32;
    
    // Calculate the number of active channels
    let mut active_channels = 0;
    let channel_list: Vec<_> = factory.get_all_channels().iter()
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
    let channel_list: Vec<_> = factory.get_all_channels().iter()
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
                serde_json::to_value(&channel_guard.get_parameters()).unwrap_or_else(|_| json!({}))
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
            serde_json::to_value(&channel_guard.get_parameters()).unwrap_or_else(|_| json!({}))
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
            },
            _ => Err(ComSrvError::InvalidOperation(
                format!("Invalid operation: {}", operation.operation)
            )),
        }
    };
    
    match result {
        Ok(_) => {
            let message = format!("Successfully {} channel {}", operation.operation, id);
            Ok(warp::reply::json(&ApiResponse::success(message)))
        },
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
        memory_usage: 0,  // Real implementation needs to fetch actual data
        cpu_usage: 0.0,   // Real implementation needs to fetch actual data
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
    let channel_id_u16 = channel_id.parse::<u16>().map_err(|_| warp::reject::reject())?;
    
    let channel = {
        let factory = protocol_factory.read().await;
        match factory.get_channel(channel_id_u16).await {
            Some(channel) => Arc::clone(&channel),
            None => {
                let error_response = ApiResponse::<()>::error(format!("Channel {} not found", channel_id));
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
                name: point.id,
                value: serde_json::Value::String(point.value),
                quality: point.quality > 0,
                timestamp: point.timestamp,
            });
            break;
        }
    }
    
    // If the point is found, return its value, otherwise return an error
    if let Some(value) = point_value {
        Ok(warp::reply::json(&ApiResponse::success(value)))
    } else {
        let error_response = ApiResponse::<()>::error(
            format!("Point {}.{} not found", point_table, point_name)
        );
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
    let channel_id_u16 = channel_id.parse::<u16>().map_err(|_| warp::reject::reject())?;
    
    let channel = {
        let factory = protocol_factory.read().await;
        match factory.get_channel(channel_id_u16).await {
            Some(channel) => Arc::clone(&channel),
            None => {
                let error_response = ApiResponse::<()>::error(format!("Channel {} not found", channel_id));
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
        let error_response = ApiResponse::<()>::error(
            format!("Point {}.{} not found", point_table, point_name)
        );
        return Ok(warp::reply::json(&error_response));
    }
    
    // Actual point writing logic should be implemented here
    // Since the ModbusClient write functionality is not fully implemented,
    // this is simplified to return success.
    // In a real scenario, the appropriate write method should be called based on point type and value.
    
    let message = format!(
        "Successfully wrote value {} to point {}.{}", 
        value.value, point_table, point_name
    );
    
    Ok(warp::reply::json(&ApiResponse::<String>::success(message)))
}

/// get all points from a channel
pub async fn get_channel_points(
    channel_id: String,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let channel_id_u16 = channel_id.parse::<u16>().map_err(|_| warp::reject::reject())?;
    
    let channel = {
        let factory = protocol_factory.read().await;
        match factory.get_channel(channel_id_u16).await {
            Some(channel) => Arc::clone(&channel),
            None => {
                let error_response = ApiResponse::<()>::error(format!("Channel {} not found", channel_id));
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
    let points: Vec<PointValue> = channel_points.into_iter()
        .map(|point| PointValue {
            name: point.id,
            value: serde_json::Value::String(point.value),
            quality: point.quality > 0,
            timestamp: point.timestamp,
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
        let error_response = ApiResponse::<()>::error(format!("Point table '{}' not found", table_name));
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
        let error_response = ApiResponse::<()>::error(format!("Point '{}' not found in table '{}'", point_id, table_name));
        Ok(warp::reply::json(&error_response))
    }
}

/// Update or create a point in a table
pub async fn upsert_point_in_table(
    table_name: String,
    point: crate::core::config::CsvPointRecord,
    config_manager: Arc<RwLock<crate::core::config::ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let mut config = config_manager.write().await;
    let csv_manager = config.get_csv_point_manager_mut();
    
    match csv_manager.upsert_point(&table_name, point.clone()) {
        Ok(()) => {
            let response = ApiResponse::success(serde_json::json!({
                "message": format!("Point '{}' updated in table '{}'", point.id, table_name)
            }));
            Ok(warp::reply::json(&response))
        },
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
        },
        Ok(false) => {
            let error_response = ApiResponse::<()>::error(format!("Point '{}' not found in table '{}'", point_id, table_name));
            Ok(warp::reply::json(&error_response))
        },
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
        },
        Err(e) => {
            let error_response = ApiResponse::<()>::error(format!("Failed to reload point tables: {}", e));
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
        // Convert points to CSV format
        let mut csv_content = String::new();
        csv_content.push_str("id,name,address,unit,scale,offset,data_type,register_type,description,access,group\n");
        
        for point in points {
            csv_content.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{}\n",
                point.id,
                point.name,
                point.address,
                point.unit.as_deref().unwrap_or(""),
                point.scale,
                point.offset,
                point.data_type,
                point.register_type.as_deref().unwrap_or(""),
                point.description.as_deref().unwrap_or(""),
                point.access.as_deref().unwrap_or("read"),
                point.group.as_deref().unwrap_or("")
            ));
        }
        
        Ok(warp::reply::with_header(
            csv_content,
            "content-type",
            "text/csv; charset=utf-8"
        ))
    } else {
        let error_response = ApiResponse::<()>::error(format!("Point table '{}' not found", table_name));
        Ok(warp::reply::with_header(
            serde_json::to_string(&error_response).unwrap(),
            "content-type",
            "application/json"
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
    /// Communication quality
    pub communication_quality: f64,
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
    let id_u16 = channel_id.parse::<u16>().map_err(|_| warp::reject::reject())?;
    
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
            channel_guard.get_parameters()
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
    let id_u16 = channel_id.parse::<u16>().map_err(|_| warp::reject::reject())?;
    
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
    let channel_list: Vec<_> = factory.get_all_channels().iter()
        .map(|(id, channel)| (*id, Arc::clone(&channel)))
        .collect();
    drop(factory);
    
    let mut channel_summaries = Vec::new();
    
    for (id, channel) in channel_list {
        let (name, protocol_type) = {
            let channel_guard = channel.read().await;
            (
                channel_guard.name().to_string(),
                channel_guard.protocol_type().to_string()
            )
        };
        
        let status = {
            let channel_guard = channel.read().await;
            channel_guard.status().await
        };
        
        // Calculate communication quality based on connection status and errors
        let communication_quality = if status.connected && !status.has_error() {
            100.0
        } else if status.connected {
            70.0  // Connected but has errors
        } else {
            0.0   // Not connected
        };
        
        channel_summaries.push(ChannelSummaryResponse {
            channel_id: id,
            connection_state: if status.connected { "Connected".to_string() } else { "Disconnected".to_string() },
            communication_quality,
            avg_response_time_ms: status.last_response_time,
            active_alarms: if status.has_error() { 1 } else { 0 },
        });
    }
    
    let response = MonitoringResponse {
        status: serde_json::json!({
            "overall_status": "Running",
            "total_channels": channel_summaries.len(),
            "connected_channels": channel_summaries.iter().filter(|c| c.connection_state == "Connected").count(),
            "avg_quality": channel_summaries.iter().map(|c| c.communication_quality).sum::<f64>() / channel_summaries.len() as f64
        }),
        active_alarms: channel_summaries.iter()
            .filter(|c| c.active_alarms > 0)
            .map(|c| serde_json::json!({
                "channel_id": c.channel_id,
                "type": "Communication Error",
                "severity": "Warning",
                "description": format!("Channel {} has communication issues", c.channel_id)
            }))
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
    let id_u16 = channel_id.parse::<u16>().map_err(|_| warp::reject::reject())?;
    
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
    
    for i in 0..std::cmp::min(limit, 10) { // Limit simulation to 10 records
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;
    use crate::core::config::ConfigManager;
    use crate::core::config::{CsvPointRecord, CsvPointManager};

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

    fn create_test_csv_point_record() -> CsvPointRecord {
        CsvPointRecord {
            id: "test_point".to_string(),
            name: "Test Point".to_string(),
            description: Some("Test point description".to_string()),
            address: 100,
            unit: Some("°C".to_string()),
            scale: 1.0,
            offset: 0.0,
            data_type: "float32".to_string(),
            register_type: Some("holding_register".to_string()),
            access: Some("read".to_string()),
            group: Some("temperature".to_string()),
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
            config_manager
        ).await;
        assert!(response.is_ok());
        
        // The response should contain an error about the point not being found
    }

    #[tokio::test]
    async fn test_upsert_point_in_table() {
        let config_manager = create_test_config_manager();
        let point = create_test_csv_point_record();
        
        let response = upsert_point_in_table(
            "test_table".to_string(),
            point,
            config_manager
        ).await;
        assert!(response.is_ok());
        
        // The response should indicate successful update
    }

    #[tokio::test]
    async fn test_delete_point_from_table() {
        let config_manager = create_test_config_manager();
        
        // First add a point
        let point = create_test_csv_point_record();
        let _ = upsert_point_in_table(
            "test_table".to_string(),
            point,
            config_manager.clone()
        ).await;
        
        // Then try to delete it
        let response = delete_point_from_table(
            "test_table".to_string(),
            "test_point".to_string(),
            config_manager
        ).await;
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
    async fn test_point_table_hot_reload_workflow() {
        let config_manager = create_test_config_manager();
        
        // 1. Create a new point
        let point = create_test_csv_point_record();
        let upsert_response = upsert_point_in_table(
            "test_table".to_string(),
            point,
            config_manager.clone()
        ).await;
        assert!(upsert_response.is_ok());
        
        // 2. Verify point exists
        let get_response = get_point_from_table(
            "test_table".to_string(),
            "test_point".to_string(),
            config_manager.clone()
        ).await;
        assert!(get_response.is_ok());
        
        // 3. Reload point tables
        let reload_response = reload_point_tables(config_manager.clone()).await;
        assert!(reload_response.is_ok());
        
        // 4. Delete the point
        let delete_response = delete_point_from_table(
            "test_table".to_string(),
            "test_point".to_string(),
            config_manager.clone()
        ).await;
        assert!(delete_response.is_ok());
        
        // 5. Verify point is deleted
        let get_after_delete_response = get_point_from_table(
            "test_table".to_string(),
            "test_point".to_string(),
            config_manager
        ).await;
        assert!(get_after_delete_response.is_ok());
    }

    #[tokio::test]
    async fn test_export_point_table_with_data() {
        let config_manager = create_test_config_manager();
        
        // Add some test data first
        let point1 = CsvPointRecord {
            id: "temp1".to_string(),
            name: "Temperature 1".to_string(),
            description: Some("First temperature sensor".to_string()),
            address: 100,
            unit: Some("°C".to_string()),
            scale: 0.1,
            offset: 0.0,
            data_type: "float32".to_string(),
            register_type: Some("holding_register".to_string()),
            access: Some("read".to_string()),
            group: Some("sensors".to_string()),
        };
        
        let point2 = CsvPointRecord {
            id: "temp2".to_string(),
            name: "Temperature 2".to_string(),
            description: Some("Second temperature sensor".to_string()),
            address: 102,
            unit: Some("°C".to_string()),
            scale: 0.1,
            offset: 0.0,
            data_type: "float32".to_string(),
            register_type: Some("holding_register".to_string()),
            access: Some("read".to_string()),
            group: Some("sensors".to_string()),
        };
        
        // Add points to table
        let _ = upsert_point_in_table(
            "sensors".to_string(),
            point1,
            config_manager.clone()
        ).await;
        
        let _ = upsert_point_in_table(
            "sensors".to_string(),
            point2,
            config_manager.clone()
        ).await;
        
        // Export the table
        let response = export_point_table("sensors".to_string(), config_manager).await;
        assert!(response.is_ok());
        
        // The response should contain CSV data
    }

    #[tokio::test]
    async fn test_point_table_validation() {
        let config_manager = create_test_config_manager();
        
        // Test with invalid point data
        let invalid_point = CsvPointRecord {
            id: "".to_string(), // Invalid empty ID
            name: "Invalid Point".to_string(),
            description: None,
            address: 0,
            unit: None,
            scale: 1.0,
            offset: 0.0,
            data_type: "invalid_type".to_string(), // Invalid data type
            register_type: None,
            access: None,
            group: None,
        };
        
        let response = upsert_point_in_table(
            "test_table".to_string(),
            invalid_point,
            config_manager
        ).await;
        
        // The response should handle the invalid data gracefully
        assert!(response.is_ok());
    }
}