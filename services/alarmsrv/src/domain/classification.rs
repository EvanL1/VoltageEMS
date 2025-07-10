use crate::config::AlarmConfig;
use crate::domain::*;
use chrono::Timelike;
use std::sync::Arc;

/// Alarm classifier with rule-based classification and escalation
pub struct AlarmClassifier {
    config: Arc<AlarmConfig>,
    classification_rules: Vec<ClassificationRule>,
    escalation_rules: Vec<EscalationRule>,
    categories: Vec<AlarmCategory>,
}

impl AlarmClassifier {
    /// Create new alarm classifier
    pub fn new(config: Arc<AlarmConfig>) -> Self {
        let mut classifier = Self {
            config,
            classification_rules: Vec::new(),
            escalation_rules: Vec::new(),
            categories: Vec::new(),
        };

        classifier.initialize_default_rules();
        classifier
    }

    /// Classify an alarm based on its content and rules
    pub async fn classify(&self, alarm: &Alarm) -> AlarmClassification {
        // Apply classification rules
        for rule in &self.classification_rules {
            if self.matches_rule(alarm, rule) {
                return AlarmClassification {
                    category: rule.category.clone(),
                    priority: self.calculate_priority(alarm, rule),
                    tags: self.extract_tags(alarm, rule),
                    confidence: rule.confidence,
                    reason: rule.reason.clone(),
                };
            }
        }

        // Default classification for unmatched alarms
        AlarmClassification {
            category: "unclassified".to_string(),
            priority: self.default_priority_for_level(alarm.level),
            tags: vec!["auto-generated".to_string()],
            confidence: 0.5,
            reason: "No matching classification rule found".to_string(),
        }
    }

    /// Get all alarm categories
    pub fn get_categories(&self) -> Vec<AlarmCategory> {
        self.categories.clone()
    }

    /// Get number of classification rules
    pub fn get_rule_count(&self) -> usize {
        self.classification_rules.len()
    }

    /// Get escalation rules
    pub fn get_escalation_rules(&self) -> Vec<EscalationRule> {
        self.escalation_rules.clone()
    }

    /// Initialize default classification rules
    fn initialize_default_rules(&mut self) {
        // Temperature-related alarms
        self.classification_rules.push(ClassificationRule {
            name: "high_temperature".to_string(),
            category: "environmental".to_string(),
            title_patterns: vec!["temperature".to_string(), "热".to_string()],
            description_patterns: vec!["high temperature".to_string(), "overheating".to_string()],
            level_filter: None,
            priority_boost: 1,
            tags: vec!["temperature".to_string(), "environmental".to_string()],
            confidence: 0.9,
            reason: "Temperature-related alarm detected".to_string(),
        });

        // Power-related alarms
        self.classification_rules.push(ClassificationRule {
            name: "power_failure".to_string(),
            category: "power".to_string(),
            title_patterns: vec![
                "power".to_string(),
                "voltage".to_string(),
                "current".to_string(),
            ],
            description_patterns: vec!["power failure".to_string(), "voltage drop".to_string()],
            level_filter: Some(vec![AlarmLevel::Critical, AlarmLevel::Major]),
            priority_boost: 2,
            tags: vec!["power".to_string(), "critical".to_string()],
            confidence: 0.95,
            reason: "Power system alarm detected".to_string(),
        });

        // Communication alarms
        self.classification_rules.push(ClassificationRule {
            name: "communication_failure".to_string(),
            category: "communication".to_string(),
            title_patterns: vec![
                "connection".to_string(),
                "communication".to_string(),
                "network".to_string(),
            ],
            description_patterns: vec!["connection lost".to_string(), "timeout".to_string()],
            level_filter: None,
            priority_boost: 0,
            tags: vec!["communication".to_string(), "network".to_string()],
            confidence: 0.8,
            reason: "Communication alarm detected".to_string(),
        });

        // System alarms
        self.classification_rules.push(ClassificationRule {
            name: "system_error".to_string(),
            category: "system".to_string(),
            title_patterns: vec![
                "system".to_string(),
                "error".to_string(),
                "fault".to_string(),
            ],
            description_patterns: vec!["system error".to_string(), "malfunction".to_string()],
            level_filter: None,
            priority_boost: 1,
            tags: vec!["system".to_string(), "error".to_string()],
            confidence: 0.85,
            reason: "System error alarm detected".to_string(),
        });

        // Initialize escalation rules
        self.escalation_rules.push(EscalationRule {
            name: "new_to_major".to_string(),
            from_status: AlarmStatus::New,
            from_level: AlarmLevel::Warning,
            to_level: AlarmLevel::Major,
            duration_minutes: 30,
            condition: "Unacknowledged warning alarms after 30 minutes".to_string(),
        });

        self.escalation_rules.push(EscalationRule {
            name: "major_to_critical".to_string(),
            from_status: AlarmStatus::New,
            from_level: AlarmLevel::Major,
            to_level: AlarmLevel::Critical,
            duration_minutes: 60,
            condition: "Unacknowledged major alarms after 1 hour".to_string(),
        });

        // Initialize categories
        self.categories = vec![
            AlarmCategory {
                name: "environmental".to_string(),
                description: "Environmental conditions (temperature, humidity, etc.)".to_string(),
                color: "#FF6B6B".to_string(),
                icon: "thermometer".to_string(),
                priority_weight: 1.2,
            },
            AlarmCategory {
                name: "power".to_string(),
                description: "Power system related alarms".to_string(),
                color: "#4ECDC4".to_string(),
                icon: "zap".to_string(),
                priority_weight: 1.5,
            },
            AlarmCategory {
                name: "communication".to_string(),
                description: "Communication and network issues".to_string(),
                color: "#45B7D1".to_string(),
                icon: "wifi".to_string(),
                priority_weight: 1.0,
            },
            AlarmCategory {
                name: "system".to_string(),
                description: "System errors and malfunctions".to_string(),
                color: "#FFA07A".to_string(),
                icon: "alert-triangle".to_string(),
                priority_weight: 1.3,
            },
            AlarmCategory {
                name: "security".to_string(),
                description: "Security related alarms".to_string(),
                color: "#FF1744".to_string(),
                icon: "shield".to_string(),
                priority_weight: 1.8,
            },
            AlarmCategory {
                name: "unclassified".to_string(),
                description: "Alarms that don't match any classification rule".to_string(),
                color: "#9E9E9E".to_string(),
                icon: "help-circle".to_string(),
                priority_weight: 0.8,
            },
        ];
    }

    /// Check if alarm matches classification rule
    fn matches_rule(&self, alarm: &Alarm, rule: &ClassificationRule) -> bool {
        // Check level filter
        if let Some(levels) = &rule.level_filter {
            if !levels.contains(&alarm.level) {
                return false;
            }
        }

        // Check title patterns
        let title_lower = alarm.title.to_lowercase();
        let title_matches = rule
            .title_patterns
            .iter()
            .any(|pattern| title_lower.contains(&pattern.to_lowercase()));

        // Check description patterns
        let desc_lower = alarm.description.to_lowercase();
        let desc_matches = rule
            .description_patterns
            .iter()
            .any(|pattern| desc_lower.contains(&pattern.to_lowercase()));

        // Rule matches if either title or description matches
        title_matches || desc_matches
    }

    /// Calculate priority based on alarm level and rule
    fn calculate_priority(&self, alarm: &Alarm, rule: &ClassificationRule) -> u32 {
        let base_priority = match alarm.level {
            AlarmLevel::Critical => 100,
            AlarmLevel::Major => 80,
            AlarmLevel::Minor => 60,
            AlarmLevel::Warning => 40,
            AlarmLevel::Info => 20,
        };

        // Apply category weight
        let category_weight = self
            .categories
            .iter()
            .find(|c| c.name == rule.category)
            .map(|c| c.priority_weight)
            .unwrap_or(1.0);

        ((base_priority as f32 * category_weight) as u32) + (rule.priority_boost * 10)
    }

    /// Extract tags based on alarm content and rule
    fn extract_tags(&self, alarm: &Alarm, rule: &ClassificationRule) -> Vec<String> {
        let mut tags = rule.tags.clone();

        // Add level-based tag
        tags.push(format!("{:?}", alarm.level).to_lowercase());

        // Add time-based tag
        let hour = alarm.created_at.hour();
        if hour >= 18 || hour < 6 {
            tags.push("off-hours".to_string());
        }

        // Add urgency tag for critical alarms
        if alarm.level == AlarmLevel::Critical {
            tags.push("urgent".to_string());
        }

        tags.sort();
        tags.dedup();
        tags
    }

    /// Get default priority for alarm level
    fn default_priority_for_level(&self, level: AlarmLevel) -> u32 {
        match level {
            AlarmLevel::Critical => 90,
            AlarmLevel::Major => 70,
            AlarmLevel::Minor => 50,
            AlarmLevel::Warning => 30,
            AlarmLevel::Info => 10,
        }
    }
}
