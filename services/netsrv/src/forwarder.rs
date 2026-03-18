/// Periodic data forwarding: Redis → MQTT property topic.
///
/// Two background tasks:
/// - `run_data_forwarder` – scans Redis every `report_interval_secs` and
///   publishes to the property topic.
/// - `run_system_monitor` – collects system metrics every
///   `system_monitor_interval_secs` and publishes them.
///
/// `upload_once` is called directly by the call-data handler for on-demand
/// uploads.
use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use regex::Regex;
use rumqttc::QoS;
use tokio::time::{self, Duration};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, warn};
use voltage_rtdb::Rtdb;

use crate::models::{PropertyEntry, PropertyPayload};
use crate::state::AppState;
use crate::system_monitor;

// ── Public entry points ───────────────────────────────────────────────────────

pub async fn run_data_forwarder(state: Arc<AppState>, shutdown: CancellationToken) {
    loop {
        let interval = state.config.read().await.report_interval_secs;

        tokio::select! {
            _ = time::sleep(Duration::from_secs(interval)) => {}
            _ = shutdown.cancelled() => return,
        }

        upload_once(Arc::clone(&state)).await;
    }
}

pub async fn run_system_monitor(state: Arc<AppState>, shutdown: CancellationToken) {
    loop {
        let (enabled, interval) = {
            let cfg = state.config.read().await;
            (cfg.system_monitor_enabled, cfg.system_monitor_interval_secs)
        };

        tokio::select! {
            _ = time::sleep(Duration::from_secs(interval)) => {}
            _ = shutdown.cancelled() => return,
        }

        if !enabled {
            continue;
        }

        let metrics = system_monitor::collect();
        let device_sn = state.device.device_sn.clone();

        let entry = PropertyEntry {
            source: "gateway".to_string(),
            device: device_sn,
            data_type: "T".to_string(),
            value: serde_json::from_value(serde_json::to_value(&metrics).unwrap_or_default())
                .unwrap_or_default(),
        };

        let payload = PropertyPayload {
            timestamp: Utc::now().timestamp(),
            property: vec![entry],
        };

        if let Err(e) = publish_payload(&state, payload).await {
            warn!("System monitor upload failed: {}", e);
        }
    }
}

/// Triggered by `call-data` MQTT command or on timer tick.
pub async fn upload_once(state: Arc<AppState>) {
    let (patterns, excludes, batch_size) = {
        let cfg = state.config.read().await;
        (
            cfg.subscribe_patterns.clone(),
            cfg.exclude_patterns.clone(),
            cfg.report_batch_size,
        )
    };

    let exclude_res: Vec<Regex> = excludes
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect();

    let entries = collect_redis_data(state.rtdb.as_ref(), &patterns, &exclude_res).await;
    if entries.is_empty() {
        debug!("No data to upload");
        return;
    }

    // Publish in batches
    for chunk in entries.chunks(batch_size) {
        let payload = PropertyPayload {
            timestamp: Utc::now().timestamp(),
            property: chunk.to_vec(),
        };
        if let Err(e) = publish_payload(&state, payload).await {
            error!("Data upload failed: {}", e);
        }
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

async fn collect_redis_data<R: Rtdb>(
    rtdb: &R,
    patterns: &[String],
    excludes: &[Regex],
) -> Vec<PropertyEntry> {
    let mut entries = Vec::new();

    for pattern in patterns {
        let keys = match rtdb.scan_match(pattern).await {
            Ok(k) => k,
            Err(e) => {
                warn!("SCAN '{}' failed: {}", pattern, e);
                continue;
            }
        };

        for key in keys {
            if excludes.iter().any(|re| re.is_match(&key)) {
                continue;
            }

            // Parse key into (source, device, data_type)
            let parts: Vec<&str> = key.splitn(3, ':').collect();
            if parts.len() < 3 {
                continue;
            }
            let (source, device, data_type) = (parts[0], parts[1], parts[2]);

            let fields = match rtdb.hash_get_all(&key).await {
                Ok(f) if !f.is_empty() => f,
                Ok(_) => continue,
                Err(e) => {
                    warn!("HGETALL '{}' failed: {}", key, e);
                    continue;
                }
            };

            let value: HashMap<String, serde_json::Value> = fields
                .into_iter()
                .filter(|(k, _)| !k.starts_with('_'))
                .map(|(k, v)| {
                    let parsed = std::str::from_utf8(&v)
                        .ok()
                        .and_then(|s| s.parse::<f64>().ok().map(|n| serde_json::json!(n)))
                        .unwrap_or(serde_json::Value::Null);
                    (k, parsed)
                })
                .collect();

            if value.is_empty() {
                continue;
            }

            entries.push(PropertyEntry {
                source: source.to_string(),
                // Spaces are preserved (cloud side handles them)
                device: device.to_string(),
                data_type: data_type.to_string(),
                value,
            });
        }
    }

    entries
}

async fn publish_payload(state: &AppState, payload: PropertyPayload) -> anyhow::Result<()> {
    let guard = state.mqtt_client.lock().await;
    let client = guard
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("MQTT not connected"))?;

    let json = serde_json::to_string(&payload)?;
    client
        .publish(&state.topics.property, QoS::AtLeastOnce, false, json)
        .await?;
    Ok(())
}
