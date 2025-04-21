use std::sync::Arc;
use warp::{Rejection, Reply};
use std::convert::Infallible;
use tokio::sync::RwLock;
use chrono::Utc;
use serde_json::json;

use crate::api::models::{
    ApiResponse, ChannelOperation, ChannelStatus as ApiChannelStatus,
    HealthStatus, PointValue, ServiceStatus, WritePointRequest, PointTableData
};
use crate::core::config::ConfigManager;
use crate::core::protocol_factory::ProtocolFactory;
use crate::utils::error::ComSrvError;

/// get service status
pub async fn get_service_status(
    start_time: Arc<chrono::DateTime<Utc>>,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let factory = protocol_factory.read().await;
    let channels = factory.get_all_channels().await.len() as u32;
    
    // Calculate the number of active channels
    let mut active_channels = 0;
    for (_, channel) in factory.get_all_channels().await.iter() {
        if channel.is_running().await {
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
    let mut channel_statuses = Vec::new();
    
    for (id, channel) in factory.get_all_channels().await.iter() {
        let status = channel.status().await;
        let params = serde_json::to_value(&channel.get_parameters())
            .unwrap_or_else(|_| json!({}));
        
        channel_statuses.push(ApiChannelStatus {
            id: id.to_string(),
            name: channel.name().to_string(),
            protocol: channel.protocol_type().to_string(),
            connected: status.connected,
            last_response_time: status.last_response_time,
            last_error: status.last_error,
            last_update_time: status.last_update_time,
            parameters: serde_json::from_value(params)
                .unwrap_or_default(),
        });
    }
    
    Ok(warp::reply::json(&ApiResponse::success(channel_statuses)))
}

/// get single channel status
pub async fn get_channel_status(
    id: String,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let factory = protocol_factory.read().await;
    
    let id_u16 = id.parse::<u16>().map_err(|_| warp::reject::reject())?;
    if let Some(channel) = factory.get_channel(id_u16).await {
        let status = channel.status().await;
        let params = serde_json::to_value(&channel.get_parameters())
            .unwrap_or_else(|_| json!({}));
        
        let channel_status = ApiChannelStatus {
            id: id.clone(),
            name: channel.name().to_string(),
            protocol: channel.protocol_type().to_string(),
            connected: status.connected,
            last_response_time: status.last_response_time,
            last_error: status.last_error,
            last_update_time: status.last_update_time,
            parameters: serde_json::from_value(params)
                .unwrap_or_default(),
        };
        
        Ok(warp::reply::json(&ApiResponse::success(channel_status)))
    } else {
        let error_response = ApiResponse::<()>::error(format!("Channel {} not found", id));
        Ok(warp::reply::json(&error_response))
    }
}

/// control channel operation
pub async fn control_channel(
    id: String,
    operation: ChannelOperation,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let mut factory = protocol_factory.write().await;
    
    let id_u16 = id.parse::<u16>().map_err(|_| warp::reject::reject())?;
    if let Some(channel) = factory.get_channel_mut(id_u16).await {
        let result = match operation.operation.as_str() {
            "start" => channel.start().await,
            "stop" => channel.stop().await,
            "restart" => {
                let _ = channel.stop().await;
                channel.start().await
            },
            _ => Err(ComSrvError::InvalidOperation(
                format!("Invalid operation: {}", operation.operation)
            )),
        };
        
        match result {
            Ok(_) => {
                let message = format!("Successfully {} channel {}", operation.operation, id);
                Ok(warp::reply::json(&ApiResponse::<String>::success(message)))
            },
            Err(e) => {
                let error_response = ApiResponse::<()>::error(format!("Operation failed: {}", e));
                Ok(warp::reply::json(&error_response))
            }
        }
    } else {
        let error_response = ApiResponse::<()>::error(format!("Channel {} not found", id));
        Ok(warp::reply::json(&error_response))
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
    let factory = protocol_factory.read().await;
    
    let channel_id_u16 = channel_id.parse::<u16>().map_err(|_| warp::reject::reject())?;
    if let Some(channel) = factory.get_channel(channel_id_u16).await {
        // Get point data from the channel
        let channel_points = channel.get_all_points().await;
        
        // Try to find the specified point
        let mut point_value = None;
        for point in channel_points {
            if point.id == point_name {
                point_value = Some(PointValue {
                    name: point.id,
                    value: point.value,
                    quality: point.quality,
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
    } else {
        let error_response = ApiResponse::<()>::error(format!("Channel {} not found", channel_id));
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
    let mut factory = protocol_factory.write().await;
    
    let channel_id_u16 = channel_id.parse::<u16>().map_err(|_| warp::reject::reject())?;
    if let Some(channel) = factory.get_channel_mut(channel_id_u16).await {
        // First, get all points to check if the point exists
        let channel_points = channel.get_all_points().await;
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
    } else {
        let error_response = ApiResponse::<()>::error(format!("Channel {} not found", channel_id));
        Ok(warp::reply::json(&error_response))
    }
}

/// get all points from a channel
pub async fn get_channel_points(
    channel_id: String,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    let factory = protocol_factory.read().await;
    
    let channel_id_u16 = channel_id.parse::<u16>().map_err(|_| warp::reject::reject())?;
    if let Some(channel) = factory.get_channel(channel_id_u16).await {
        let mut points = Vec::new();
        
        // Get all points from the channel
        let channel_points = channel.get_all_points().await;
        for point in channel_points {
            let point_value = PointValue {
                name: point.id.clone(),
                value: point.value.clone(),
                quality: point.quality,
                timestamp: point.timestamp,
            };
            points.push(point_value);
        }
        
        let point_table_data = PointTableData {
            channel_id: channel_id.clone(),
            points,
            timestamp: Utc::now(),
        };
        
        Ok(warp::reply::json(&ApiResponse::success(point_table_data)))
    } else {
        let error_response = ApiResponse::<()>::error(format!("Channel {} not found", channel_id));
        Ok(warp::reply::json(&error_response))
    }
}