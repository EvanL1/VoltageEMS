/// Redis data collection.
///
/// For each subscribe_pattern, scans all matching Redis keys and reads the
/// hash contents via `HGETALL`.  Non-numeric field values are stored as
/// `string_value`.  Special fields beginning with `_` are excluded from
/// measurement data (except `_timestamp` / `__updated` which supply the time).
use chrono::{DateTime, Utc};
use regex::Regex;
use tracing::{debug, warn};
use voltage_rtdb::Rtdb;

use crate::models::{DataPoint, ServiceConfig};

pub async fn collect<R: Rtdb>(rtdb: &R, cfg: &ServiceConfig) -> Vec<DataPoint> {
    let exclude_regexes: Vec<Regex> = cfg
        .exclude_patterns
        .iter()
        .filter_map(|p| {
            Regex::new(p)
                .map_err(|e| warn!("Invalid exclude pattern '{}': {}", p, e))
                .ok()
        })
        .collect();

    let mut all_points = Vec::new();

    for pattern in &cfg.subscribe_patterns {
        let keys = match rtdb.scan_match(pattern).await {
            Ok(k) => k,
            Err(e) => {
                warn!("SCAN failed for pattern '{}': {}", pattern, e);
                continue;
            },
        };

        debug!("Pattern '{}' matched {} keys", pattern, keys.len());

        for key in keys {
            // Apply exclude filters
            if exclude_regexes.iter().any(|re| re.is_match(&key)) {
                continue;
            }

            let fields = match rtdb.hash_get_all(&key).await {
                Ok(f) if !f.is_empty() => f,
                Ok(_) => continue,
                Err(e) => {
                    warn!("HGETALL failed for '{}': {}", key, e);
                    continue;
                },
            };

            // Extract timestamp from the hash if present.
            let time = extract_timestamp(&fields).unwrap_or_else(Utc::now);

            for (field, raw_bytes) in &fields {
                // Skip internal/meta fields
                if field.starts_with('_') {
                    continue;
                }

                let raw = match std::str::from_utf8(raw_bytes) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                let (value, string_value) = if let Ok(v) = raw.parse::<f64>() {
                    (Some(v), None)
                } else {
                    (None, Some(raw.to_string()))
                };

                all_points.push(DataPoint {
                    time,
                    redis_key: key.clone(),
                    point_id: field.clone(),
                    value,
                    string_value,
                });
            }
        }
    }

    all_points
}

/// Try to read `_timestamp` or `__updated` from a hash's fields.
fn extract_timestamp(
    fields: &std::collections::HashMap<String, bytes::Bytes>,
) -> Option<DateTime<Utc>> {
    for key in &["_timestamp", "__updated"] {
        if let Some(raw) = fields.get(*key) {
            if let Ok(s) = std::str::from_utf8(raw) {
                // Try Unix seconds
                if let Ok(secs) = s.trim().parse::<i64>() {
                    if let Some(dt) = DateTime::from_timestamp(secs, 0) {
                        return Some(dt);
                    }
                    // Try milliseconds (13-digit)
                    if let Some(dt) = DateTime::from_timestamp_millis(secs) {
                        return Some(dt);
                    }
                }
                // Try ISO format
                if let Ok(dt) = DateTime::parse_from_rfc3339(s.trim()) {
                    return Some(dt.with_timezone(&Utc));
                }
            }
        }
    }
    None
}
