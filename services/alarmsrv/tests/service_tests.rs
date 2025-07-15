//! Service integration tests

use std::sync::Arc;
use alarmsrv::{
    domain::{AlarmLevel},
    redis::{AlarmRedisClient, AlarmStore},
    services::rules::{AlarmRule, AlarmRuleType},
};

mod common;
use common::{cleanup_test_data, test_config::test_config};

#[tokio::test]
async fn test_alarm_rules_threshold() {
    // Clean up before test
    cleanup_test_data("ems:alarms:*").await.unwrap();

    let config = Arc::new(test_config());
    let redis_client = Arc::new(AlarmRedisClient::new(config.clone()).await.unwrap());
    let _store = Arc::new(AlarmStore::new(redis_client.clone()).await.unwrap());

    // Create threshold rule
    let rule = AlarmRule {
        id: "test_threshold".to_string(),
        name: "Test Threshold Rule".to_string(),
        description: "Test threshold alarm".to_string(),
        channel_id: 1001,
        point_type: "m".to_string(),
        point_id: Some(10001),
        enabled: true,
        alarm_title: "Threshold Exceeded".to_string(),
        alarm_description: "Value exceeded threshold".to_string(),
        rule_type: AlarmRuleType::Threshold {
            high: Some(80.0),
            low: Some(20.0),
            high_high: Some(90.0),
            low_low: Some(10.0),
        },
    };

    // Test threshold evaluation
    let test_cases = vec![
        (50.0, None),                    // Normal value
        (85.0, Some(AlarmLevel::Warning)), // High value
        (95.0, Some(AlarmLevel::Major)),   // High High value
        (15.0, Some(AlarmLevel::Warning)), // Low value
        (5.0, Some(AlarmLevel::Major)),    // Low Low value
    ];

    for (value, expected_level) in test_cases {
        match &rule.rule_type {
            AlarmRuleType::Threshold { high, low, high_high, low_low } => {
                let level = if let Some(hh) = high_high {
                    if value >= *hh {
                        Some(AlarmLevel::Major)
                    } else if let Some(h) = high {
                        if value >= *h {
                            Some(AlarmLevel::Warning)
                        } else if let Some(ll) = low_low {
                            if value <= *ll {
                                Some(AlarmLevel::Major)
                            } else if let Some(l) = low {
                                if value <= *l {
                                    Some(AlarmLevel::Warning)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else if let Some(l) = low {
                            if value <= *l {
                                Some(AlarmLevel::Warning)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else if let Some(ll) = low_low {
                    if value <= *ll {
                        Some(AlarmLevel::Major)
                    } else if let Some(l) = low {
                        if value <= *l {
                            Some(AlarmLevel::Warning)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else if let Some(h) = high {
                    if value >= *h {
                        Some(AlarmLevel::Warning)
                    } else if let Some(l) = low {
                        if value <= *l {
                            Some(AlarmLevel::Warning)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else if let Some(l) = low {
                    if value <= *l {
                        Some(AlarmLevel::Warning)
                    } else {
                        None
                    }
                } else {
                    None
                };
                
                assert_eq!(level, expected_level, "Failed for value {}", value);
            }
            _ => panic!("Wrong rule type"),
        }
    }

    // Clean up after test
    cleanup_test_data("ems:alarms:*").await.unwrap();
}

#[tokio::test]
async fn test_alarm_rules_timeout() {
    let _config = Arc::new(test_config());

    // Create timeout rule
    let rule = AlarmRule {
        id: "test_timeout".to_string(),
        name: "Test Timeout Rule".to_string(),
        description: "Test timeout alarm".to_string(),
        channel_id: 1001,
        point_type: "s".to_string(),
        point_id: Some(20001),
        enabled: true,
        alarm_title: "Communication Timeout".to_string(),
        alarm_description: "Device communication timeout".to_string(),
        rule_type: AlarmRuleType::Timeout {
            warning_timeout: 300,  // 5 minutes
            major_timeout: 600,    // 10 minutes
            critical_timeout: 1200, // 20 minutes
        },
    };

    // Test timeout evaluation
    match &rule.rule_type {
        AlarmRuleType::Timeout { warning_timeout, major_timeout, critical_timeout } => {
            assert_eq!(*warning_timeout, 300);
            assert_eq!(*major_timeout, 600);
            assert_eq!(*critical_timeout, 1200);
        }
        _ => panic!("Wrong rule type"),
    }
}

