use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Alarm level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlarmLevel {
    /// Critical alarm
    Critical,
    /// Major alarm
    Major,
    /// Minor alarm
    Minor,
    /// Warning
    Warning,
    /// Information
    Info,
}

/// Alarm status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlarmStatus {
    /// New
    New,
    /// Acknowledged
    Acknowledged,
    /// Resolved
    Resolved,
}

/// Alarm event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alarm {
    /// Alarm ID
    pub id: Uuid,
    /// Alarm title
    pub title: String,
    /// Alarm description
    pub description: String,
    /// Alarm level
    pub level: AlarmLevel,
    /// Alarm status
    pub status: AlarmStatus,
    /// Creation time
    pub created_at: DateTime<Utc>,
    /// Update time
    pub updated_at: DateTime<Utc>,
    /// Acknowledgment time
    pub acknowledged_at: Option<DateTime<Utc>>,
    /// Acknowledged by user
    pub acknowledged_by: Option<String>,
    /// Resolution time
    pub resolved_at: Option<DateTime<Utc>>,
    /// Resolved by user
    pub resolved_by: Option<String>,
}

impl Alarm {
    /// Create new alarm
    pub fn new(title: String, description: String, level: AlarmLevel) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title,
            description,
            level,
            status: AlarmStatus::New,
            created_at: now,
            updated_at: now,
            acknowledged_at: None,
            acknowledged_by: None,
            resolved_at: None,
            resolved_by: None,
        }
    }

    /// Acknowledge alarm
    pub fn acknowledge(&mut self, user: String) {
        if self.status == AlarmStatus::New {
            self.status = AlarmStatus::Acknowledged;
            self.acknowledged_at = Some(Utc::now());
            self.acknowledged_by = Some(user);
            self.updated_at = Utc::now();
        }
    }

    /// Resolve alarm
    pub fn resolve(&mut self, user: String) {
        self.status = AlarmStatus::Resolved;
        self.resolved_at = Some(Utc::now());
        self.resolved_by = Some(user);
        self.updated_at = Utc::now();
    }

    /// Check if alarm is active
    pub fn is_active(&self) -> bool {
        matches!(self.status, AlarmStatus::New | AlarmStatus::Acknowledged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alarm_creation() {
        let alarm = Alarm::new(
            "Test Alarm".to_string(),
            "This is a test alarm".to_string(),
            AlarmLevel::Warning,
        );

        assert_eq!(alarm.title, "Test Alarm");
        assert_eq!(alarm.level, AlarmLevel::Warning);
        assert_eq!(alarm.status, AlarmStatus::New);
        assert!(alarm.is_active());
    }

    #[test]
    fn test_alarm_acknowledge() {
        let mut alarm = Alarm::new(
            "Test Alarm".to_string(),
            "This is a test alarm".to_string(),
            AlarmLevel::Warning,
        );

        alarm.acknowledge("Test User".to_string());
        
        assert_eq!(alarm.status, AlarmStatus::Acknowledged);
        assert!(alarm.acknowledged_by.is_some());
        assert!(alarm.acknowledged_at.is_some());
        assert!(alarm.is_active());
    }

    #[test]
    fn test_alarm_resolve() {
        let mut alarm = Alarm::new(
            "Test Alarm".to_string(),
            "This is a test alarm".to_string(),
            AlarmLevel::Warning,
        );

        alarm.resolve("Test User".to_string());
        
        assert_eq!(alarm.status, AlarmStatus::Resolved);
        assert!(alarm.resolved_by.is_some());
        assert!(alarm.resolved_at.is_some());
        assert!(!alarm.is_active());
    }
} 