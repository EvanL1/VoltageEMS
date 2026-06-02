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

use crate::models::{DataPoint, PatternEntry, ServiceConfig};

#[allow(dead_code)]
pub async fn collect<R: Rtdb>(rtdb: &R, cfg: &ServiceConfig) -> Vec<DataPoint> {
    collect_patterns(rtdb, cfg, &cfg.subscribe_patterns).await
}

/// Collect only the supplied subset of patterns (used by the per-pattern
/// scheduler to avoid collecting patterns that are not yet due).
pub async fn collect_patterns<R: Rtdb>(
    rtdb: &R,
    cfg: &ServiceConfig,
    patterns: &[PatternEntry],
) -> Vec<DataPoint> {
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

    for entry in patterns {
        let pattern = &entry.pattern;
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
                    // Reject NaN/±Inf: storing them in TimescaleDB would poison
                    // aggregate queries (AVG/SUM propagate NaN to whole rollup).
                    // A gap in the time series is the correct representation of
                    // "no real data" — never write a sentinel that an analyst
                    // can't distinguish from a real reading.
                    if !v.is_finite() {
                        debug!(
                            "Skipping non-finite value {} from key='{}' field='{}'",
                            v, key, field
                        );
                        continue;
                    }
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
        if let Some(raw) = fields.get(*key)
            && let Ok(s) = std::str::from_utf8(raw)
        {
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
    None
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use voltage_rtdb::Rtdb;
    use voltage_rtdb::helpers::create_test_rtdb;

    use crate::models::ServiceConfig;

    #[tokio::test]
    async fn collect_drops_non_finite_values() {
        let rtdb = create_test_rtdb();

        // Seed: real value, NaN, +Inf, -Inf, real zero, non-numeric string.
        // Only "42.5" and "0" should make it through to TSDB.
        rtdb.hash_set("inst:1:M", "100", Bytes::from("42.5"))
            .await
            .unwrap();
        rtdb.hash_set("inst:1:M", "101", Bytes::from("NaN"))
            .await
            .unwrap();
        rtdb.hash_set("inst:1:M", "102", Bytes::from("inf"))
            .await
            .unwrap();
        rtdb.hash_set("inst:1:M", "103", Bytes::from("-inf"))
            .await
            .unwrap();
        rtdb.hash_set("inst:1:M", "104", Bytes::from("0"))
            .await
            .unwrap();
        rtdb.hash_set("inst:1:M", "105", Bytes::from("ERR"))
            .await
            .unwrap();

        let cfg = ServiceConfig {
            subscribe_patterns: vec![crate::models::PatternEntry::new("inst:*:M")],
            exclude_patterns: vec![],
            ..ServiceConfig::default()
        };

        let points = collect(rtdb.as_ref(), &cfg).await;

        // Numeric finite values: 100 and 104. Non-numeric "ERR" survives as
        // string_value (analyst can audit). NaN/±Inf must be dropped entirely
        // — they cannot land in the FLOAT8 column.
        let numeric: Vec<_> = points.iter().filter(|p| p.value.is_some()).collect();
        assert_eq!(
            numeric.len(),
            2,
            "only 42.5 and 0 should keep numeric value"
        );

        for p in &points {
            if let Some(v) = p.value {
                assert!(v.is_finite(), "non-finite value {} leaked through", v);
            }
        }

        // String fallback for "ERR" still allowed (post-mortem auditing).
        assert!(
            points
                .iter()
                .any(|p| p.point_id == "105" && p.string_value.as_deref() == Some("ERR"))
        );
    }
}
