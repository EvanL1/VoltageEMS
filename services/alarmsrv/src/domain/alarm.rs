//! Alarm entity and domain logic
//!
//! This module extends the basic Alarm type with additional
//! domain-specific functionality.

use crate::domain::types::Alarm;

impl Alarm {
    /// Check if alarm needs escalation based on its current state
    pub fn needs_escalation(&self, duration_minutes: u32) -> bool {
        use crate::domain::AlarmStatus;
        use chrono::{Duration, Utc};

        if self.status != AlarmStatus::New && self.status != AlarmStatus::Acknowledged {
            return false;
        }

        let age = Utc::now() - self.created_at;
        age > Duration::minutes(duration_minutes as i64)
    }

    /// Check if alarm should be auto-resolved based on conditions
    pub fn should_auto_resolve(&self, condition: &str) -> bool {
        // This could be extended with more sophisticated logic
        // For now, just a placeholder
        match condition {
            "no_activity_24h" => {
                use chrono::{Duration, Utc};
                let inactive_duration = Utc::now() - self.updated_at;
                inactive_duration > Duration::hours(24)
            }
            _ => false,
        }
    }

    /// Get alarm urgency score based on multiple factors
    pub fn urgency_score(&self) -> u32 {
        use chrono::{Duration, Utc};

        let mut score = self.metadata.priority;

        // Add time-based urgency
        let age = Utc::now() - self.created_at;
        if age > Duration::hours(24) {
            score += 20;
        } else if age > Duration::hours(12) {
            score += 10;
        } else if age > Duration::hours(6) {
            score += 5;
        }

        // Cap at 100
        score.min(100)
    }
}

/// Alarm-related domain services
pub mod services {
    use crate::domain::{Alarm, AlarmLevel};

    /// Calculate alarm trend based on recent alarms
    pub fn calculate_alarm_trend(recent_alarms: &[Alarm]) -> AlarmTrend {
        if recent_alarms.is_empty() {
            return AlarmTrend::Stable;
        }

        // Count critical and major alarms in last hour vs previous hour
        use chrono::{Duration, Utc};
        let one_hour_ago = Utc::now() - Duration::hours(1);
        let two_hours_ago = Utc::now() - Duration::hours(2);

        let last_hour_critical = recent_alarms
            .iter()
            .filter(|a| a.created_at > one_hour_ago)
            .filter(|a| matches!(a.level, AlarmLevel::Critical | AlarmLevel::Major))
            .count();

        let previous_hour_critical = recent_alarms
            .iter()
            .filter(|a| a.created_at > two_hours_ago && a.created_at <= one_hour_ago)
            .filter(|a| matches!(a.level, AlarmLevel::Critical | AlarmLevel::Major))
            .count();

        if last_hour_critical > previous_hour_critical * 2 {
            AlarmTrend::Increasing
        } else if last_hour_critical < previous_hour_critical / 2 {
            AlarmTrend::Decreasing
        } else {
            AlarmTrend::Stable
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum AlarmTrend {
        Increasing,
        Stable,
        Decreasing,
    }
}
