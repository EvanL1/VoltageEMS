//! Alarm processing service

use anyhow::Result;
use tracing::{error, info};

use crate::services::escalation::process_alarm_escalation;
use crate::AppState;

/// Start alarm processing worker
pub async fn start_alarm_processor(state: AppState) -> Result<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

        loop {
            interval.tick().await;

            // Process alarm escalation
            if let Err(e) = process_alarm_escalation(&state).await {
                error!("Failed to process alarm escalation: {}", e);
            }

            // Clean up old resolved alarms
            if let Err(e) = cleanup_old_alarms(&state).await {
                error!("Failed to cleanup old alarms: {}", e);
            }
        }
    });

    Ok(())
}

/// Clean up old resolved alarms
async fn cleanup_old_alarms(state: &AppState) -> Result<()> {
    let retention_days = state.config.storage.retention_days;

    match state.alarm_store.cleanup_old_alarms(retention_days).await {
        Ok(count) => {
            if count > 0 {
                info!("Cleaned up {} old alarms", count);
            }
        }
        Err(e) => {
            error!("Failed to cleanup old alarms: {}", e);
        }
    }

    Ok(())
}
