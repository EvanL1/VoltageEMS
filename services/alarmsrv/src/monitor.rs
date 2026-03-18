//! Alarm monitoring engine
//!
//! Polls enabled rules every `data_fetch_interval` seconds, reads current
//! values from Redis and creates/resolves alerts accordingly.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use voltage_rtdb::Rtdb;

use crate::db;
use crate::state::AppState;

pub async fn run_monitor(state: Arc<AppState>, shutdown: CancellationToken) {
    let interval = Duration::from_secs(state.config.data_fetch_interval);
    info!(
        "Alarm monitor started (interval={}s)",
        state.config.data_fetch_interval
    );

    // Mark as running
    {
        let mut ms = state.monitor_status.write().await;
        ms.running = true;
    }

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                info!("Alarm monitor shutting down");
                break;
            }
            _ = tokio::time::sleep(interval) => {
                check_all_rules(&state).await;
            }
        }
    }

    let mut ms = state.monitor_status.write().await;
    ms.running = false;
}

pub async fn run_alarm_count_broadcaster(state: Arc<AppState>, shutdown: CancellationToken) {
    info!("Alarm count broadcast task started (interval=30s)");
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                send_alarm_count_broadcast(&state).await;
            }
        }
    }
}

async fn check_all_rules(state: &Arc<AppState>) {
    let rules = match db::get_all_enabled_rules(&state.db).await {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to load enabled rules: {}", e);
            return;
        },
    };

    if rules.is_empty() {
        debug!("No enabled rules to check");
        state.monitor_status.write().await.last_check_time = Some(Utc::now().timestamp());
        return;
    }

    debug!("Checking {} enabled rules", rules.len());

    // Process all rules concurrently
    let tasks: Vec<_> = rules
        .into_iter()
        .map(|rule| {
            let state = Arc::clone(state);
            tokio::spawn(async move {
                check_single_rule(state, rule).await;
            })
        })
        .collect();

    futures::future::join_all(tasks).await;

    state.monitor_status.write().await.last_check_time = Some(Utc::now().timestamp());
}

async fn check_single_rule(state: Arc<AppState>, rule: crate::models::AlertRule) {
    // Read value from Redis
    let redis_key = rule.redis_key();
    let redis_field = rule.redis_field();

    let current_value = match state.rtdb.hash_get(&redis_key, &redis_field).await {
        Ok(Some(v)) => {
            let s = String::from_utf8_lossy(&v);
            match s.parse::<f64>() {
                Ok(val) => val,
                Err(_) => {
                    warn!(
                        "Invalid value format: {}:{} = {}",
                        redis_key, redis_field, s
                    );
                    return;
                },
            }
        },
        Ok(None) => {
            debug!(
                "No data for rule '{}' at {}:{}",
                rule.rule_name, redis_key, redis_field
            );
            return;
        },
        Err(e) => {
            warn!("Redis error for rule '{}': {}", rule.rule_name, e);
            return;
        },
    };

    let is_triggered = rule.evaluate(current_value);

    let existing_alert = match db::get_alert_by_rule_id(&state.db, rule.id).await {
        Ok(a) => a,
        Err(e) => {
            error!(
                "DB error checking alert for rule '{}': {}",
                rule.rule_name, e
            );
            return;
        },
    };

    if is_triggered {
        if let Some(alert) = existing_alert {
            // Already active – just update current value
            if let Err(e) = db::update_alert_value(&state.db, alert.id, current_value).await {
                error!("Failed to update alert value: {}", e);
            }
            debug!(
                "Updated alert '{}': value={}",
                rule.rule_name, current_value
            );
        } else {
            // New alarm triggered
            match db::insert_alert(&state.db, &rule, current_value).await {
                Ok(alert_id) => {
                    warn!(
                        "ALARM TRIGGERED: rule='{}' value={} {} {}",
                        rule.rule_name, current_value, rule.operator, rule.value
                    );
                    state
                        .broadcaster
                        .send_alarm_triggered(alert_id, &rule, current_value)
                        .await;
                    send_alarm_count_broadcast(&state).await;
                },
                Err(e) => {
                    error!(
                        "Failed to insert alert for rule '{}': {}",
                        rule.rule_name, e
                    );
                },
            }
        }
    } else if let Some(alert) = existing_alert {
        // Alarm recovered
        match db::resolve_alert(&state.db, &alert, current_value).await {
            Ok(_event_id) => {
                info!(
                    "ALARM RECOVERED: rule='{}' value={}",
                    rule.rule_name, current_value
                );
                state
                    .broadcaster
                    .send_alarm_recovery(alert.id, &rule, Some(current_value), "条件恢复")
                    .await;
                send_alarm_count_broadcast(&state).await;
            },
            Err(e) => {
                error!(
                    "Failed to resolve alert for rule '{}': {}",
                    rule.rule_name, e
                );
            },
        }
    }
}

async fn send_alarm_count_broadcast(state: &Arc<AppState>) {
    match db::get_active_alarm_counts(&state.db).await {
        Ok(counts) => {
            state.broadcaster.send_alarm_count(&counts).await;
        },
        Err(e) => {
            error!("Failed to get alarm counts: {}", e);
        },
    }
}

/// Called when a rule is updated – resolves its alerts if it's now disabled.
pub async fn on_rule_updated(state: &Arc<AppState>, rule_id: i64) {
    let rule = match db::get_rule_by_id(&state.db, rule_id).await {
        Ok(Some(r)) => r,
        _ => return,
    };

    if !rule.enabled {
        let resolved = db::resolve_alerts_by_rule_id(&state.db, rule_id)
            .await
            .unwrap_or_default();
        for alert in &resolved {
            state
                .broadcaster
                .send_alarm_recovery(alert.id, &rule, None, "规则被禁用")
                .await;
        }
        // Broadcast updated count only when at least one alert was resolved.
        // (Must check `resolved` before the DB call, since the alerts are
        // already marked recovered at this point.)
        if !resolved.is_empty() {
            send_alarm_count_broadcast(state).await;
        }
    }
}

/// Called when a rule is deleted – resolves its active alerts first.
pub async fn on_rule_deleted(state: &Arc<AppState>, rule: &crate::models::AlertRule) {
    let resolved = db::resolve_alerts_by_rule_id(&state.db, rule.id)
        .await
        .unwrap_or_default();
    for alert in &resolved {
        state
            .broadcaster
            .send_alarm_recovery(alert.id, rule, None, "规则被删除")
            .await;
    }
    if !resolved.is_empty() {
        send_alarm_count_broadcast(state).await;
    }
}

/// Manual rule check (for the `/monitor/check-rule/{id}` endpoint).
pub async fn manual_check_rule(
    state: &Arc<AppState>,
    rule_id: i64,
) -> anyhow::Result<serde_json::Value> {
    let rule = db::get_rule_by_id(&state.db, rule_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Rule not found"))?;

    if !rule.enabled {
        return Ok(serde_json::json!({
            "success": false,
            "message": "规则已禁用",
            "data": {},
        }));
    }

    let value_str = state
        .rtdb
        .hash_get(&rule.redis_key(), &rule.redis_field())
        .await?;

    let Some(raw) = value_str else {
        return Ok(serde_json::json!({
            "success": false,
            "message": "无法从Redis获取数据",
            "data": {
                "redis_key": rule.redis_key(),
                "point_id": rule.point_id,
            },
        }));
    };

    let raw_str = String::from_utf8_lossy(&raw);
    let current_value: f64 = raw_str
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid value: {}", raw_str))?;
    let is_triggered = rule.evaluate(current_value);
    let has_active = db::get_alert_by_rule_id(&state.db, rule.id)
        .await?
        .is_some();

    Ok(serde_json::json!({
        "success": true,
        "message": "手动检查完成",
        "data": {
            "rule_name": rule.rule_name,
            "current_value": current_value,
            "threshold_value": rule.value,
            "operator": rule.operator,
            "is_triggered": is_triggered,
            "has_active_alert": has_active,
            "redis_key": rule.redis_key(),
            "check_time": chrono::Utc::now().to_rfc3339(),
        },
    }))
}
