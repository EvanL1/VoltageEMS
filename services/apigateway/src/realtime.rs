use actix_web::{web, HttpResponse};
use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use chrono::{DateTime, Utc};

use crate::error::{ApiGatewayError, ApiResult};
use crate::redis_client::{RedisClient, RedisClientExt};

/// Channel status
#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelStatus {
    pub channel_id: u32,
    pub name: String,
    pub status: String, // online, offline, error
    pub last_update: Option<DateTime<Utc>>,
    pub point_count: u32,
    pub error_count: u32,
}

/// Point data
#[derive(Debug, Serialize, Deserialize)]
pub struct PointData {
    pub point_id: u32,
    pub point_type: String, // YC, YX, YK, YT
    pub value: serde_json::Value,
    pub quality: u8,
    pub timestamp: DateTime<Utc>,
    pub description: Option<String>,
}

/// Query parameters for real-time data
#[derive(Debug, Deserialize)]
pub struct RealtimeQuery {
    pub channel_ids: Option<String>, // comma-separated
    pub point_types: Option<String>, // comma-separated
    pub limit: Option<usize>,
}

/// Get all channel statuses
pub async fn get_channels(
    redis_client: web::Data<Arc<RedisClient>>,
) -> ApiResult<HttpResponse> {
    debug!("Getting all channel statuses");

    // Get channel list from Redis
    let pattern = "channel:*:status";
    let keys = redis_client.keys_api(pattern).await
        .map_err(|e| ApiGatewayError::ServiceError(format!("Failed to scan Redis keys: {}", e)))?;

    let mut channels = Vec::new();
    for key in keys {
        // Parse channel ID from key
        if let Some(channel_id_str) = key.split(':').nth(1) {
            if let Ok(channel_id) = channel_id_str.parse::<u32>() {
                // Get channel status from Redis
                let status_data = redis_client.get_api(&key).await
                    .map_err(|e| ApiGatewayError::ServiceError(format!("Failed to get channel status: {}", e)))?;

                if let Some(status_json) = status_data {
                    match serde_json::from_str::<serde_json::Value>(&status_json) {
                        Ok(status_value) => {
                            let channel_status = ChannelStatus {
                                channel_id,
                                name: status_value.get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Unknown")
                                    .to_string(),
                                status: status_value.get("status")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("offline")
                                    .to_string(),
                                last_update: status_value.get("last_update")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                                    .map(|dt| dt.with_timezone(&Utc)),
                                point_count: status_value.get("point_count")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0) as u32,
                                error_count: status_value.get("error_count")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0) as u32,
                            };
                            channels.push(channel_status);
                        }
                        Err(e) => {
                            error!("Failed to parse channel status JSON: {}", e);
                        }
                    }
                }
            }
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "channels": channels,
        "total": channels.len()
    })))
}

/// Get real-time point data
pub async fn get_points(
    path: web::Path<u32>,
    query: web::Query<RealtimeQuery>,
    redis_client: web::Data<Arc<RedisClient>>,
) -> ApiResult<HttpResponse> {
    let channel_id = path.into_inner();
    debug!("Getting points for channel {}", channel_id);

    let mut points = Vec::new();
    
    // Determine point types to query
    let point_types = if let Some(types) = &query.point_types {
        types.split(',').collect::<Vec<_>>()
    } else {
        vec!["m", "s", "c", "a"] // YC, YX, YK, YT
    };

    // Query each point type
    for point_type in point_types {
        let pattern = format!("{}:{}:*", channel_id, point_type);
        let keys = redis_client.keys_api(&pattern).await
            .map_err(|e| ApiGatewayError::ServiceError(format!("Failed to scan Redis keys: {}", e)))?;

        let limit = query.limit.unwrap_or(1000);
        for (idx, key) in keys.iter().enumerate() {
            if idx >= limit {
                break;
            }

            // Parse point ID from key
            if let Some(point_id_str) = key.split(':').nth(2) {
                if let Ok(point_id) = point_id_str.parse::<u32>() {
                    // Get point data from Redis
                    let point_data = redis_client.get_api(key).await
                        .map_err(|e| ApiGatewayError::ServiceError(format!("Failed to get point data: {}", e)))?;

                    if let Some(data_str) = point_data {
                        // Parse value:quality:timestamp format
                        let parts: Vec<&str> = data_str.split(':').collect();
                        if parts.len() >= 3 {
                            let value = serde_json::Value::String(parts[0].to_string());
                            let quality = parts[1].parse::<u8>().unwrap_or(0);
                            let timestamp = parts[2].parse::<i64>()
                                .ok()
                                .and_then(|ts| DateTime::from_timestamp(ts, 0))
                                .unwrap_or_else(Utc::now);

                            let point = PointData {
                                point_id,
                                point_type: match point_type {
                                    "m" => "YC".to_string(),
                                    "s" => "YX".to_string(),
                                    "c" => "YK".to_string(),
                                    "a" => "YT".to_string(),
                                    _ => point_type.to_string(),
                                },
                                value,
                                quality,
                                timestamp,
                                description: None,
                            };
                            points.push(point);
                        }
                    }
                }
            }
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "channel_id": channel_id,
        "points": points,
        "total": points.len()
    })))
}

/// Get aggregated statistics
pub async fn get_statistics(
    redis_client: web::Data<Arc<RedisClient>>,
) -> ApiResult<HttpResponse> {
    debug!("Getting real-time statistics");

    // Get all channel statuses
    let pattern = "channel:*:status";
    let keys = redis_client.keys_api(pattern).await
        .map_err(|e| ApiGatewayError::ServiceError(format!("Failed to scan Redis keys: {}", e)))?;

    let total_channels = keys.len();
    let mut online_channels = 0;
    let mut total_points = 0;
    let mut total_errors = 0;

    for key in keys {
        let status_data = redis_client.get(&key).await
            .map_err(|e| ApiError::ServiceError(format!("Failed to get channel status: {}", e)))?;

        if let Some(status_json) = status_data {
            if let Ok(status_value) = serde_json::from_str::<serde_json::Value>(&status_json) {
                if status_value.get("status").and_then(|v| v.as_str()) == Some("online") {
                    online_channels += 1;
                }
                total_points += status_value.get("point_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                total_errors += status_value.get("error_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
            }
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "total_channels": total_channels,
        "online_channels": online_channels,
        "offline_channels": total_channels - online_channels,
        "total_points": total_points,
        "total_errors": total_errors,
        "timestamp": Utc::now()
    })))
}