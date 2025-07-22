use log::{debug, info};
use serde_json::Value;
use std::sync::Arc;

use crate::redis_client::RedisClient;
use crate::error::ApiResult;

/// 处理控制命令
pub async fn handle_control_command(
    redis: &Arc<RedisClient>,
    channel_id: u32,
    point_id: u32,
    value: Value,
    params: Option<Value>,
    session_id: &str,
    user_id: Option<&str>,
) -> ApiResult<bool> {
    debug!("Processing control command: channel={}, point={}, value={:?}", channel_id, point_id, value);
    
    // 构建控制命令
    let command = serde_json::json!({
        "session_id": session_id,
        "user_id": user_id,
        "channel_id": channel_id,
        "point_id": point_id,
        "value": value,
        "params": params,
        "timestamp": chrono::Utc::now().timestamp_millis(),
    });
    
    // 发布到Redis控制命令通道
    let redis_channel = format!("cmd:{}:control", channel_id);
    
    match redis.publish(&redis_channel, &command.to_string()).await {
        Ok(_) => {
            info!("Control command published successfully: {}", redis_channel);
            Ok(true)
        }
        Err(e) => {
            Err(crate::error::ApiError::InternalError(format!("Failed to publish control command: {}", e)))
        }
    }
}

/// 处理调节命令
pub async fn handle_adjustment_command(
    redis: &Arc<RedisClient>,
    channel_id: u32,
    point_id: u32,
    value: Value,
    params: Option<Value>,
    session_id: &str,
    user_id: Option<&str>,
) -> ApiResult<bool> {
    debug!("Processing adjustment command: channel={}, point={}, value={:?}", channel_id, point_id, value);
    
    // 构建调节命令
    let command = serde_json::json!({
        "session_id": session_id,
        "user_id": user_id,
        "channel_id": channel_id,
        "point_id": point_id,
        "value": value,
        "params": params,
        "timestamp": chrono::Utc::now().timestamp_millis(),
    });
    
    // 发布到Redis调节命令通道
    let redis_channel = format!("cmd:{}:adjustment", channel_id);
    
    match redis.publish(&redis_channel, &command.to_string()).await {
        Ok(_) => {
            info!("Adjustment command published successfully: {}", redis_channel);
            Ok(true)
        }
        Err(e) => {
            Err(crate::error::ApiError::InternalError(format!("Failed to publish adjustment command: {}", e)))
        }
    }
}