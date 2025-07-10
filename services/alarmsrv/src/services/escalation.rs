//! Alarm escalation service

use anyhow::Result;
use tracing::{error, info};

use crate::AppState;

/// Process alarm escalation
pub async fn process_alarm_escalation(state: &AppState) -> Result<()> {
    let escalation_rules = state.classifier.get_escalation_rules();

    for rule in escalation_rules {
        match state.alarm_store.get_alarms_for_escalation(&rule).await {
            Ok(alarms) => {
                for alarm in alarms {
                    // Escalate alarm level
                    let mut escalated_alarm = alarm.clone();
                    escalated_alarm.escalate();

                    // Update in Redis
                    if let Err(e) = state.alarm_store.update_alarm(&escalated_alarm).await {
                        error!("Failed to update escalated alarm: {}", e);
                        continue;
                    }

                    // Publish escalation for cloud
                    if let Err(e) = state
                        .alarm_store
                        .publish_alarm_for_cloud(&escalated_alarm)
                        .await
                    {
                        tracing::warn!("Failed to publish escalated alarm for cloud: {}", e);
                    }

                    info!(
                        "Alarm escalated: {} -> {:?}",
                        escalated_alarm.title, escalated_alarm.level
                    );
                }
            }
            Err(e) => {
                error!("Failed to get alarms for escalation: {}", e);
            }
        }
    }

    Ok(())
}
