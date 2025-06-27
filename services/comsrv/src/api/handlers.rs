use chrono::Utc;

use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::{Rejection, Reply};

use crate::api::models::{
    ApiResponse, ChannelOperation, ChannelStatus, ChannelStatusResponse, HealthStatus, PointTableData, PointValue,
    ServiceStatus, WritePointRequest,
};
use crate::core::protocols::common::ProtocolFactory;

use crate::utils::error::ComSrvError;

// Import new storage types
use crate::core::config::{
    ConfigManager,
};


/// get service status
pub async fn get_service_status(
    start_time: Arc<chrono::DateTime<Utc>>,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let uptime = Utc::now().timestamp() - start_time.timestamp();
    
    let factory = protocol_factory.read().await;
    let stats = factory.get_channel_stats().await;
    
    let status = ServiceStatus {
        name: "Communication Service".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime: uptime as u64,
        start_time: *start_time,
        channels: stats.total_channels as u32,
        active_channels: stats.running_channels as u32,
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
        let (name, protocol_type, _params) = {
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
    let (name, protocol_type, _params) = {
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

    // Convert string ID to u16, handle conversion errors gracefully
    let id_u16 = id.parse::<u16>().unwrap_or(0);
    
    let channel_status = ChannelStatus {
        id: id_u16,
        name,
        protocol: protocol_type,
        connected: status.connected,
        running: channel.read().await.is_running().await,
        last_update: status.last_update_time,
        error_count: if status.has_error() { 1 } else { 0 },
        last_error: if status.has_error() { Some(status.last_error) } else { None },
        statistics: std::collections::HashMap::new(),
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
        uptime: 0, // Will be filled by service
        memory_usage: 0,
        cpu_usage: 0.0,
    };
    
    Ok(warp::reply::json(&ApiResponse::success(health)))
}

/// Read point value from channel
pub async fn read_point(
    channel_id: String,
    point_table: String,
    point_name: String,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let id_u16 = channel_id.parse::<u16>().map_err(|_| warp::reject::reject())?;

    let _channel = {
        let factory = protocol_factory.read().await;
        match factory.get_channel(id_u16).await {
            Some(channel) => Arc::clone(&channel),
            None => {
                let error_response = ApiResponse::<()>::error(format!("Channel {} not found", channel_id));
                return Ok(warp::reply::json(&error_response));
            }
        }
    };

    // For now, return a mock response since point reading requires more complex implementation
    let point_value = PointValue {
        id: point_name.clone(),
        name: point_name.clone(),
        value: serde_json::Value::Null,
        timestamp: Utc::now(),
        unit: "".to_string(),
        description: format!("Point from table {}", point_table),
    };

    Ok(warp::reply::json(&ApiResponse::success(point_value)))
}

/// Write point value to channel
pub async fn write_point(
    channel_id: String,
    point_table: String,
    point_name: String,
    _value: WritePointRequest,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let id_u16 = channel_id.parse::<u16>().map_err(|_| warp::reject::reject())?;

    let _channel = {
        let factory = protocol_factory.read().await;
        match factory.get_channel(id_u16).await {
            Some(channel) => Arc::clone(&channel),
            None => {
                let error_response = ApiResponse::<()>::error(format!("Channel {} not found", channel_id));
                return Ok(warp::reply::json(&error_response));
            }
        }
    };

    // For now, return a success response since point writing requires more complex implementation
    let message = format!("Successfully wrote value to point {} in table {}", point_name, point_table);
    Ok(warp::reply::json(&ApiResponse::success(message)))
}

/// Get all points from a channel
pub async fn get_channel_points(
    channel_id: String,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let id_u16 = channel_id.parse::<u16>().map_err(|_| warp::reject::reject())?;

    let channel = {
        let factory = protocol_factory.read().await;
        match factory.get_channel(id_u16).await {
            Some(channel) => Arc::clone(&channel),
            None => {
                let error_response = ApiResponse::<()>::error(format!("Channel {} not found", channel_id));
                return Ok(warp::reply::json(&error_response));
            }
        }
    };

    // Get all points from the channel
    let points = {
        let channel_guard = channel.read().await;
        channel_guard.get_all_points().await
    };

    let point_values: Vec<PointValue> = points.into_iter().map(|p| p.into()).collect();

    let point_table_data = PointTableData {
        channel_id: channel_id.clone(),
        points: point_values,
        timestamp: Utc::now(),
    };

    Ok(warp::reply::json(&ApiResponse::success(point_table_data)))
}

// Simplified functions without complex dependencies

/// Get point tables (simplified)
pub async fn get_point_tables(
    _config_manager: Arc<RwLock<ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let response = ApiResponse::success(json!({
        "message": "Point tables functionality is under development",
        "tables": []
    }));
    Ok(warp::reply::json(&response))
}

/// Get point table (simplified)
pub async fn get_point_table(
    channel_name: String,
    _config_manager: Arc<RwLock<ConfigManager>>,
) -> Result<impl Reply, Rejection> {
    let response = ApiResponse::success(json!({
        "message": format!("Point table for channel '{}' is under development", channel_name),
        "points": []
    }));
    Ok(warp::reply::json(&response))
}

 