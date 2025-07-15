//! Test configuration

use alarmsrv::config::{
    AlarmConfig, ApiConfig, MonitoringConfig, RedisConfig, RedisConnectionType, StorageConfig,
};
use alarmsrv::services::rules::{AlarmRule, AlarmRuleType};

/// Create a test configuration
pub fn test_config() -> AlarmConfig {
    AlarmConfig {
        redis: RedisConfig {
            connection_type: RedisConnectionType::Tcp,
            host: std::env::var("TEST_REDIS_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: std::env::var("TEST_REDIS_PORT")
                .unwrap_or_else(|_| "6379".to_string())
                .parse()
                .unwrap_or(6379),
            socket_path: None,
            password: std::env::var("TEST_REDIS_PASSWORD").ok(),
            database: 15, // Use database 15 for tests
        },
        api: ApiConfig {
            host: "127.0.0.1".to_string(),
            port: 0, // Use port 0 to get a random available port
        },
        storage: StorageConfig {
            retention_days: 7,
            auto_cleanup: false, // Disable auto cleanup in tests
            cleanup_interval_hours: 24,
        },
        monitoring: MonitoringConfig {
            enabled: false, // Disable monitoring in tests by default
            channels: vec![1001],
            point_types: vec!["m".to_string()],
            scan_interval: 1, // Fast scan for tests
        },
        alarm_rules: vec![
            AlarmRule {
                id: "test_rule_1".to_string(),
                name: "Test Temperature Rule".to_string(),
                description: "Test rule for temperature alarms".to_string(),
                channel_id: 1001,
                point_type: "m".to_string(),
                point_id: Some(10001),
                enabled: true,
                alarm_title: "Temperature Alarm".to_string(),
                alarm_description: "Temperature threshold exceeded".to_string(),
                rule_type: AlarmRuleType::Threshold {
                    high: Some(80.0),
                    low: Some(10.0),
                    high_high: Some(90.0),
                    low_low: Some(0.0),
                },
            },
            AlarmRule {
                id: "test_rule_2".to_string(),
                name: "Test Timeout Rule".to_string(),
                description: "Test rule for timeout alarms".to_string(),
                channel_id: 1001,
                point_type: "s".to_string(),
                point_id: None,
                enabled: true,
                alarm_title: "Timeout Alarm".to_string(),
                alarm_description: "Communication timeout detected".to_string(),
                rule_type: AlarmRuleType::Timeout {
                    warning_timeout: 5,
                    major_timeout: 10,
                    critical_timeout: 20,
                },
            },
        ],
    }
}

/// Create a minimal test configuration
pub fn minimal_test_config() -> AlarmConfig {
    let mut config = test_config();
    config.monitoring.enabled = false;
    config.alarm_rules.clear();
    config
}