use std::sync::Arc;
use tokio::sync::RwLock;
use log::{debug, info};

use crate::redis_client::RedisClient;
use crate::websocket::hub::Hub;

/// Start Redis subscriber for real-time data
pub fn start_redis_subscriber(
    _hub: Arc<RwLock<Hub>>,
    _redis_client: Arc<RedisClient>,
) {
    tokio::spawn(async move {
        info!("Starting Redis subscriber for real-time data");
        
        // Subscribe to Redis patterns for real-time data
        let _patterns = vec![
            "*:m:*",  // Telemetry data
            "*:s:*",  // Signal data
            "alarm:*", // Alarm events
        ];
        
        // For now, we'll skip Redis subscription due to compatibility issues
        // TODO: Implement proper Redis pubsub integration
        info!("Redis subscriber started (stub implementation)");
        
        // TODO: Implement actual Redis subscription
        // For now, we'll just keep the task alive
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });
}
async fn handle_redis_message(
    hub: &Arc<RwLock<Hub>>,
    channel: &str,
    payload: &str,
) {
    debug!("Received Redis message on channel {}: {}", channel, payload);
    
    // Parse channel to extract channel_id and data_type
    let parts: Vec<&str> = channel.split(':').collect();
    if parts.len() >= 3 {
        if let Ok(channel_id) = parts[0].parse::<u32>() {
            let data_type = match parts[1] {
                "m" => "telemetry",
                "s" => "signal",
                "c" => "control",
                "a" => "adjustment",
                _ => return,
            };
            
            // Parse payload and create WebSocket message
            if let Ok(point_data) = serde_json::from_str::<serde_json::Value>(payload) {
                // Create WebSocket message with point data
                let ws_message = serde_json::json!({
                    "type": data_type,
                    "channel_id": channel_id,
                    "point_id": parts[2],
                    "data": point_data,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                });
                
                // Broadcast to subscribers
                let hub = hub.read().await;
                hub.broadcast_to_channel(channel_id, data_type, &ws_message.to_string());
            }
        }
    } else if channel.starts_with("alarm:") {
        // Handle alarm messages
        if let Ok(alarm_data) = serde_json::from_str::<serde_json::Value>(payload) {
            let ws_message = serde_json::json!({
                "type": "alarm",
                "data": alarm_data,
                "timestamp": chrono::Utc::now().to_rfc3339()
            });
            
            // Broadcast alarm to all subscribers of "alarm" type
            // Extract channel_id from alarm data if available
            let channel_id = alarm_data["channel_id"].as_u64().unwrap_or(0) as u32;
            
            let hub = hub.read().await;
            hub.broadcast_to_channel(channel_id, "alarm", &ws_message.to_string());
        }
    }
}

