use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

/// Simplified alarm metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmMetadata {
    /// Priority score based on level (0-100)
    pub priority: u32,
    /// Source information (channel, point, etc.)
    pub source: Option<String>,
    /// Additional tags for filtering
    pub tags: Vec<String>,
}

impl Default for AlarmMetadata {
    fn default() -> Self {
        Self {
            priority: 50,
            source: None,
            tags: Vec::new(),
        }
    }
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
    /// Alarm metadata
    pub metadata: AlarmMetadata,
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
        let priority = Self::level_to_priority(level);

        Self {
            id: Uuid::new_v4(),
            title,
            description,
            level,
            status: AlarmStatus::New,
            metadata: AlarmMetadata {
                priority,
                source: None,
                tags: Vec::new(),
            },
            created_at: now,
            updated_at: now,
            acknowledged_at: None,
            acknowledged_by: None,
            resolved_at: None,
            resolved_by: None,
        }
    }

    /// Create new alarm with source information
    pub fn new_with_source(
        title: String,
        description: String,
        level: AlarmLevel,
        source: String,
    ) -> Self {
        let mut alarm = Self::new(title, description, level);
        alarm.metadata.source = Some(source);
        alarm
    }

    /// Convert alarm level to priority score
    fn level_to_priority(level: AlarmLevel) -> u32 {
        match level {
            AlarmLevel::Critical => 90,
            AlarmLevel::Major => 70,
            AlarmLevel::Minor => 50,
            AlarmLevel::Warning => 30,
            AlarmLevel::Info => 10,
        }
    }

    /// Set alarm metadata
    pub fn set_metadata(&mut self, metadata: AlarmMetadata) {
        self.metadata = metadata;
        self.updated_at = Utc::now();
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

    /// Escalate alarm level
    pub fn escalate(&mut self) {
        let new_level = match self.level {
            AlarmLevel::Info => AlarmLevel::Warning,
            AlarmLevel::Warning => AlarmLevel::Minor,
            AlarmLevel::Minor => AlarmLevel::Major,
            AlarmLevel::Major => AlarmLevel::Critical,
            AlarmLevel::Critical => AlarmLevel::Critical, // Already at max
        };

        if new_level != self.level {
            self.level = new_level;
            self.updated_at = Utc::now();
        }
    }

    /// Check if alarm is active
    pub fn is_active(&self) -> bool {
        matches!(self.status, AlarmStatus::New | AlarmStatus::Acknowledged)
    }
}

/// Classification rule for automatic alarm categorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationRule {
    /// Rule name
    pub name: String,
    /// Target category
    pub category: String,
    /// Title patterns to match
    pub title_patterns: Vec<String>,
    /// Description patterns to match
    pub description_patterns: Vec<String>,
    /// Alarm level filter (None = all levels)
    pub level_filter: Option<Vec<AlarmLevel>>,
    /// Priority boost (0-10)
    pub priority_boost: u32,
    /// Tags to add
    pub tags: Vec<String>,
    /// Rule confidence (0.0-1.0)
    pub confidence: f64,
    /// Rule explanation
    pub reason: String,
}

/// Escalation rule for automatic alarm level escalation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationRule {
    /// Rule name
    pub name: String,
    /// Source alarm status
    pub from_status: AlarmStatus,
    /// Source alarm level
    pub from_level: AlarmLevel,
    /// Target alarm level
    pub to_level: AlarmLevel,
    /// Duration in minutes before escalation
    pub duration_minutes: u32,
    /// Escalation condition description
    pub condition: String,
}

/// Alarm category definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmCategory {
    /// Category name
    pub name: String,
    /// Category description
    pub description: String,
    /// Display color (hex)
    pub color: String,
    /// Display icon
    pub icon: String,
    /// Priority weight multiplier
    pub priority_weight: f32,
}

/// Cloud alarm format for netsrv integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudAlarm {
    /// Alarm ID
    pub id: String,
    /// Alarm title
    pub title: String,
    /// Alarm description
    pub description: String,
    /// Alarm level
    pub level: String,
    /// Alarm status
    pub status: String,
    /// Priority score
    pub priority: u32,
    /// Tags
    pub tags: Vec<String>,
    /// Timestamps
    pub created_at: String,
    pub updated_at: String,
    /// Device/source information
    pub source: String,
    /// Facility/location
    pub facility: String,
    /// Cloud metadata
    pub cloud_metadata: HashMap<String, String>,
}

impl CloudAlarm {
    /// Convert from internal alarm format
    pub fn from_alarm(alarm: &Alarm) -> Self {
        let mut cloud_metadata = HashMap::new();
        cloud_metadata.insert("service".to_string(), "alarmsrv".to_string());
        cloud_metadata.insert("version".to_string(), "1.0".to_string());

        if let Some(ref source) = alarm.metadata.source {
            cloud_metadata.insert("data_source".to_string(), source.clone());
        }

        Self {
            id: alarm.id.to_string(),
            title: alarm.title.clone(),
            description: alarm.description.clone(),
            level: format!("{:?}", alarm.level),
            status: format!("{:?}", alarm.status),
            priority: alarm.metadata.priority,
            tags: alarm.metadata.tags.clone(),
            created_at: alarm.created_at.to_rfc3339(),
            updated_at: alarm.updated_at.to_rfc3339(),
            source: alarm
                .metadata
                .source
                .clone()
                .unwrap_or_else(|| "ems-alarmsrv".to_string()),
            facility: "default".to_string(),
            cloud_metadata,
        }
    }
}

/// Alarm statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmStatistics {
    /// Total alarm count
    pub total: usize,
    /// Statistics by status
    pub by_status: AlarmStatusStats,
    /// Statistics by level
    pub by_level: AlarmLevelStats,
    /// Today's handled alarms count
    pub today_handled: usize,
    /// Active alarms count (new + acknowledged)
    pub active: usize,
}

/// Alarm status statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmStatusStats {
    /// New alarms
    pub new: usize,
    /// Acknowledged alarms
    pub acknowledged: usize,
    /// Resolved alarms
    pub resolved: usize,
}

/// Alarm level statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmLevelStats {
    /// Critical alarms
    pub critical: usize,
    /// Major alarms
    pub major: usize,
    /// Minor alarms
    pub minor: usize,
    /// Warning alarms
    pub warning: usize,
    /// Info alarms
    pub info: usize,
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
        assert_eq!(alarm.metadata.priority, 30);
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

    #[test]
    fn test_alarm_escalation() {
        let mut alarm = Alarm::new(
            "Test Alarm".to_string(),
            "This is a test alarm".to_string(),
            AlarmLevel::Warning,
        );

        alarm.escalate();
        assert_eq!(alarm.level, AlarmLevel::Minor);

        alarm.escalate();
        assert_eq!(alarm.level, AlarmLevel::Major);

        alarm.escalate();
        assert_eq!(alarm.level, AlarmLevel::Critical);

        // Should stay at Critical
        alarm.escalate();
        assert_eq!(alarm.level, AlarmLevel::Critical);
    }

    #[test]
    fn test_cloud_alarm_conversion() {
        let alarm = Alarm::new_with_source(
            "Test Alarm".to_string(),
            "This is a test alarm".to_string(),
            AlarmLevel::Warning,
            "test_source".to_string(),
        );

        let cloud_alarm = CloudAlarm::from_alarm(&alarm);

        assert_eq!(cloud_alarm.title, "Test Alarm");
        assert_eq!(cloud_alarm.source, "test_source");
        assert_eq!(cloud_alarm.priority, 30);
        assert!(cloud_alarm.cloud_metadata.contains_key("data_source"));
    }
}
