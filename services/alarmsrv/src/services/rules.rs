//! Alarm rules engine for configurable alarm triggers

#[allow(unused_imports)]
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::domain::{Alarm, AlarmLevel};

/// Alarm rule type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AlarmRuleType {
    /// Threshold alarm rule
    Threshold {
        /// High limit
        high: Option<f64>,
        /// Low limit
        low: Option<f64>,
        /// High-high limit (critical)
        high_high: Option<f64>,
        /// Low-low limit (critical)
        low_low: Option<f64>,
    },
    /// Communication timeout alarm rule
    Timeout {
        /// Warning timeout in seconds
        warning_timeout: u64,
        /// Major timeout in seconds
        major_timeout: u64,
        /// Critical timeout in seconds
        critical_timeout: u64,
    },
}

/// Alarm rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmRule {
    /// Rule ID
    pub id: String,
    /// Rule name
    pub name: String,
    /// Rule description
    pub description: String,
    /// Channel ID to monitor
    pub channel_id: u16,
    /// Point type (m/s/c/a)
    pub point_type: String,
    /// Point ID to monitor (None = all points of this type)
    pub point_id: Option<u32>,
    /// Rule type and parameters
    pub rule_type: AlarmRuleType,
    /// Whether the rule is enabled
    pub enabled: bool,
    /// Alarm title template (can use {value}, {point_id}, {channel_id})
    pub alarm_title: String,
    /// Alarm description template
    pub alarm_description: String,
}

/// Point data for rule evaluation
#[derive(Debug, Clone)]
pub struct PointData {
    pub channel_id: u16,
    pub point_type: String,
    pub point_id: u32,
    pub value: f64,
    pub timestamp: DateTime<Utc>,
}

/// Alarm rules manager
pub struct AlarmRulesEngine {
    /// All configured rules
    rules: Vec<AlarmRule>,
    /// Last known data for timeout detection
    last_data: HashMap<String, DateTime<Utc>>,
}

impl AlarmRulesEngine {
    /// Create new rules engine
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            last_data: HashMap::new(),
        }
    }

    /// Load rules from configuration
    pub fn load_rules(&mut self, rules: Vec<AlarmRule>) {
        info!("Loading {} alarm rules", rules.len());
        self.rules = rules.into_iter().filter(|r| r.enabled).collect();
        info!("Loaded {} enabled alarm rules", self.rules.len());
    }

    /// Evaluate rules against point data
    pub fn evaluate(&mut self, data: &PointData) -> Vec<Alarm> {
        let mut alarms = Vec::new();

        // Update last data timestamp
        let key = format!("{}:{}:{}", data.channel_id, data.point_type, data.point_id);
        self.last_data.insert(key.clone(), data.timestamp);

        // Find matching rules
        let matching_rules: Vec<_> = self
            .rules
            .iter()
            .filter(|rule| {
                rule.channel_id == data.channel_id
                    && rule.point_type == data.point_type
                    && (rule.point_id.is_none() || rule.point_id == Some(data.point_id))
            })
            .collect();

        debug!("Found {} matching rules for {}", matching_rules.len(), key);

        // Evaluate each matching rule
        for rule in matching_rules {
            if let Some(alarm) = self.evaluate_rule(rule, data) {
                alarms.push(alarm);
            }
        }

        alarms
    }

    /// Check for timeout alarms
    pub fn check_timeouts(&self, current_time: DateTime<Utc>) -> Vec<Alarm> {
        let mut alarms = Vec::new();

        // Group rules by monitored points
        let timeout_rules: Vec<_> = self
            .rules
            .iter()
            .filter(|r| matches!(r.rule_type, AlarmRuleType::Timeout { .. }))
            .collect();

        for rule in timeout_rules {
            // Check specific point or all points
            if let Some(point_id) = rule.point_id {
                let key = format!("{}:{}:{}", rule.channel_id, rule.point_type, point_id);
                if let Some(alarm) = self.check_point_timeout(rule, &key, current_time) {
                    alarms.push(alarm);
                }
            } else {
                // Check all points of this type
                let prefix = format!("{}:{}:", rule.channel_id, rule.point_type);
                let timeout_points: Vec<_> = self
                    .last_data
                    .iter()
                    .filter(|(k, _)| k.starts_with(&prefix))
                    .collect();

                for (key, _) in timeout_points {
                    if let Some(alarm) = self.check_point_timeout(rule, key, current_time) {
                        alarms.push(alarm);
                    }
                }
            }
        }

        alarms
    }

    /// Evaluate a single rule against data
    fn evaluate_rule(&self, rule: &AlarmRule, data: &PointData) -> Option<Alarm> {
        match &rule.rule_type {
            AlarmRuleType::Threshold {
                high,
                low,
                high_high,
                low_low,
            } => {
                let mut level = None;
                let mut threshold_type = "";
                let mut threshold_value = 0.0;

                // Check thresholds in order of severity
                if let Some(hh) = high_high {
                    if data.value > *hh {
                        level = Some(AlarmLevel::Critical);
                        threshold_type = "High-High";
                        threshold_value = *hh;
                    }
                }
                if level.is_none() {
                    if let Some(ll) = low_low {
                        if data.value < *ll {
                            level = Some(AlarmLevel::Critical);
                            threshold_type = "Low-Low";
                            threshold_value = *ll;
                        }
                    }
                }
                if level.is_none() {
                    if let Some(h) = high {
                        if data.value > *h {
                            level = Some(AlarmLevel::Major);
                            threshold_type = "High";
                            threshold_value = *h;
                        }
                    }
                }
                if level.is_none() {
                    if let Some(l) = low {
                        if data.value < *l {
                            level = Some(AlarmLevel::Major);
                            threshold_type = "Low";
                            threshold_value = *l;
                        }
                    }
                }

                if let Some(alarm_level) = level {
                    let title = rule
                        .alarm_title
                        .replace("{value}", &format!("{:.2}", data.value))
                        .replace("{point_id}", &data.point_id.to_string())
                        .replace("{channel_id}", &data.channel_id.to_string())
                        .replace("{threshold_type}", threshold_type)
                        .replace("{threshold_value}", &format!("{:.2}", threshold_value));

                    let description = rule
                        .alarm_description
                        .replace("{value}", &format!("{:.2}", data.value))
                        .replace("{point_id}", &data.point_id.to_string())
                        .replace("{channel_id}", &data.channel_id.to_string())
                        .replace("{threshold_type}", threshold_type)
                        .replace("{threshold_value}", &format!("{:.2}", threshold_value));

                    Some(Alarm::new(title, description, alarm_level))
                } else {
                    None
                }
            }
            AlarmRuleType::Timeout { .. } => {
                // Timeout alarms are handled separately
                None
            }
        }
    }

    /// Check timeout for a specific point
    fn check_point_timeout(
        &self,
        rule: &AlarmRule,
        key: &str,
        current_time: DateTime<Utc>,
    ) -> Option<Alarm> {
        let last_update = self.last_data.get(key)?;
        let elapsed = (current_time - *last_update).num_seconds() as u64;

        if let AlarmRuleType::Timeout {
            warning_timeout,
            major_timeout,
            critical_timeout,
        } = &rule.rule_type
        {
            let (level, timeout_desc) = if elapsed > *critical_timeout {
                (
                    AlarmLevel::Critical,
                    format!("{}s (Critical)", critical_timeout),
                )
            } else if elapsed > *major_timeout {
                (AlarmLevel::Major, format!("{}s (Major)", major_timeout))
            } else if elapsed > *warning_timeout {
                (
                    AlarmLevel::Warning,
                    format!("{}s (Warning)", warning_timeout),
                )
            } else {
                return None;
            };

            // Extract point info from key
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() != 3 {
                warn!("Invalid key format: {}", key);
                return None;
            }

            let title = rule
                .alarm_title
                .replace("{point_id}", parts[2])
                .replace("{channel_id}", parts[0])
                .replace("{timeout}", &timeout_desc)
                .replace("{elapsed}", &elapsed.to_string());

            let description = rule
                .alarm_description
                .replace("{point_id}", parts[2])
                .replace("{channel_id}", parts[0])
                .replace("{timeout}", &timeout_desc)
                .replace("{elapsed}", &elapsed.to_string())
                .replace("{last_update}", &last_update.to_rfc3339());

            Some(Alarm::new(title, description, level))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_threshold_rule_evaluation() {
        let mut engine = AlarmRulesEngine::new();

        let rule = AlarmRule {
            id: "test_threshold".to_string(),
            name: "Temperature Threshold".to_string(),
            description: "Monitor temperature limits".to_string(),
            channel_id: 1001,
            point_type: "m".to_string(),
            point_id: Some(10001),
            rule_type: AlarmRuleType::Threshold {
                high: Some(80.0),
                low: Some(20.0),
                high_high: Some(95.0),
                low_low: Some(10.0),
            },
            enabled: true,
            alarm_title: "Temperature {threshold_type} Limit Exceeded".to_string(),
            alarm_description:
                "Temperature {value}°C exceeds {threshold_type} limit of {threshold_value}°C"
                    .to_string(),
        };

        engine.load_rules(vec![rule]);

        // Test high-high threshold
        let data = PointData {
            channel_id: 1001,
            point_type: "m".to_string(),
            point_id: 10001,
            value: 96.5,
            timestamp: Utc::now(),
        };

        let alarms = engine.evaluate(&data);
        assert_eq!(alarms.len(), 1);
        assert_eq!(alarms[0].level, AlarmLevel::Critical);
        assert!(alarms[0].title.contains("High-High"));
    }

    #[test]
    fn test_timeout_rule() {
        let mut engine = AlarmRulesEngine::new();

        let rule = AlarmRule {
            id: "test_timeout".to_string(),
            name: "Communication Timeout".to_string(),
            description: "Monitor data update timeout".to_string(),
            channel_id: 1001,
            point_type: "m".to_string(),
            point_id: Some(10001),
            rule_type: AlarmRuleType::Timeout {
                warning_timeout: 60,
                major_timeout: 300,
                critical_timeout: 600,
            },
            enabled: true,
            alarm_title: "Point {point_id} Communication Timeout ({timeout})".to_string(),
            alarm_description: "No data received for {elapsed} seconds".to_string(),
        };

        engine.load_rules(vec![rule]);

        // Add initial data
        let data = PointData {
            channel_id: 1001,
            point_type: "m".to_string(),
            point_id: 10001,
            value: 50.0,
            timestamp: Utc::now() - Duration::seconds(400),
        };
        engine.evaluate(&data);

        // Check timeouts
        let alarms = engine.check_timeouts(Utc::now());
        assert_eq!(alarms.len(), 1);
        assert_eq!(alarms[0].level, AlarmLevel::Major);
    }
}
