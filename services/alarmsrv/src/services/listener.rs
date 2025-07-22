//! Redis listener service for auto alarm generation

use anyhow::Result;
use futures::StreamExt;
use tracing::{error, info};
use voltage_libs::redis::RedisClient;

use crate::domain::{Alarm, AlarmLevel};
use crate::AppState;

/// Start Redis listener for auto alarm generation
pub async fn start_redis_listener(state: AppState) -> Result<()> {
    let redis_url = state.config.redis.get_connection_url();
    let mut client = RedisClient::new(&redis_url).await?;

    tokio::spawn(async move {
        loop {
            match client.subscribe(&["comsrv:*"]).await {
                Ok(pubsub) => {
                    info!("Redis connection successful, starting to listen for data...");

                    let mut stream = pubsub.into_on_message();
                    while let Some(msg) = stream.next().await {
                        if let Ok(payload) = msg.get_payload::<String>() {
                            // Process data message and generate alarms if needed
                            if let Err(e) = process_data_message(&state, &payload).await {
                                error!("Failed to process data message: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Redis connection failed: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    });

    Ok(())
}

/// Process data message and generate alarms
async fn process_data_message(state: &AppState, payload: &str) -> Result<()> {
    if let Ok(data) = serde_json::from_str::<serde_json::Value>(payload) {
        // Check various alarm conditions
        if let Some(value) = data.get("value").and_then(|v| v.as_f64()) {
            let mut alarms_to_create = Vec::new();

            // Temperature threshold
            if value > 80.0 {
                alarms_to_create.push(Alarm::new(
                    "High Temperature Alert".to_string(),
                    format!("High temperature detected: {:.1}°C", value),
                    AlarmLevel::Warning,
                ));
            }

            // Critical temperature
            if value > 90.0 {
                alarms_to_create.push(Alarm::new(
                    "Critical Temperature Alert".to_string(),
                    format!("Critical temperature detected: {:.1}°C", value),
                    AlarmLevel::Critical,
                ));
            }

            // Process each alarm
            for alarm in alarms_to_create {
                // Store in Redis
                if let Err(e) = state.alarm_store.store_alarm(&alarm).await {
                    error!("Failed to store auto-generated alarm: {}", e);
                    continue;
                }

                // Add to memory
                let mut alarms = state.alarms.write().await;
                alarms.push(alarm.clone());

                // Publish for cloud push
                if let Err(e) = state.alarm_store.publish_alarm_for_cloud(&alarm).await {
                    tracing::warn!("Failed to publish auto-generated alarm for cloud: {}", e);
                }

                info!(
                    "Auto-triggered alarm: {} (Level: {:?})",
                    alarm.title, alarm.level
                );
            }
        }
    }

    Ok(())
}
