//! Redis integration tests

use alarmsrv::{
    domain::{AlarmLevel, AlarmStatus},
    redis::{AlarmQueryService, AlarmRedisClient, AlarmStatisticsManager, AlarmStore},
};
use std::sync::Arc;

mod common;
use common::{cleanup_test_data, create_test_alarm, test_config::test_config};

#[tokio::test]
async fn test_alarm_store_basic_operations() {
    // Clean up before test
    cleanup_test_data("ems:alarms:*").await.unwrap();

    let config = Arc::new(test_config());
    let redis_client = Arc::new(AlarmRedisClient::new(config).await.unwrap());
    let store = AlarmStore::new(redis_client).await.unwrap();

    // Create and store an alarm
    let alarm = create_test_alarm("Redis Test Alarm", AlarmLevel::Warning);
    let result = store.store_alarm(&alarm).await;
    assert!(result.is_ok());

    // Get the alarm
    let retrieved = store.get_alarm(&alarm.id.to_string()).await.unwrap();
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.id, alarm.id);
    assert_eq!(retrieved.title, "Redis Test Alarm");
    assert_eq!(retrieved.level, AlarmLevel::Warning);

    // Acknowledge the alarm
    let ack_result = store
        .acknowledge_alarm(&alarm.id.to_string(), "test_user".to_string())
        .await;
    assert!(ack_result.is_ok());
    let acked_alarm = ack_result.unwrap();
    assert_eq!(acked_alarm.status, AlarmStatus::Acknowledged);

    // Resolve the alarm
    let resolve_result = store
        .resolve_alarm(&alarm.id.to_string(), "test_user".to_string())
        .await;
    assert!(resolve_result.is_ok());
    let resolved_alarm = resolve_result.unwrap();
    assert_eq!(resolved_alarm.status, AlarmStatus::Resolved);

    // Clean up after test
    cleanup_test_data("ems:alarms:*").await.unwrap();
}

#[tokio::test]
async fn test_alarm_query_service() {
    // Clean up before test
    cleanup_test_data("ems:alarms:*").await.unwrap();

    let config = Arc::new(test_config());
    let redis_client = Arc::new(AlarmRedisClient::new(config).await.unwrap());
    let store = AlarmStore::new(redis_client.clone()).await.unwrap();
    let query_service = AlarmQueryService::new(redis_client);

    // Create test alarms
    let levels = vec![AlarmLevel::Critical, AlarmLevel::Major, AlarmLevel::Minor];
    for (i, level) in levels.iter().enumerate() {
        let alarm = create_test_alarm(&format!("Test Alarm {}", i), *level);
        store.store_alarm(&alarm).await.unwrap();
    }

    // Query all alarms
    let alarms = query_service
        .get_alarms(None, None, None, None)
        .await
        .unwrap();
    assert_eq!(alarms.len(), 3);

    // Query by level
    let critical_alarms = query_service
        .get_alarms(None, Some(AlarmLevel::Critical), None, None)
        .await
        .unwrap();
    assert_eq!(critical_alarms.len(), 1);
    assert_eq!(critical_alarms[0].level, AlarmLevel::Critical);

    // Query with limit
    let limited_alarms = query_service
        .get_alarms(None, None, None, Some(2))
        .await
        .unwrap();
    assert_eq!(limited_alarms.len(), 2);

    // Clean up after test
    cleanup_test_data("ems:alarms:*").await.unwrap();
}

#[tokio::test]
async fn test_alarm_statistics() {
    // Clean up before test
    cleanup_test_data("ems:alarms:*").await.unwrap();

    let config = Arc::new(test_config());
    let redis_client = Arc::new(AlarmRedisClient::new(config).await.unwrap());
    let store = AlarmStore::new(redis_client.clone()).await.unwrap();
    let stats_manager = AlarmStatisticsManager::new(redis_client);

    // Create alarms with different levels
    let test_data = vec![
        ("Critical 1", AlarmLevel::Critical),
        ("Critical 2", AlarmLevel::Critical),
        ("Major 1", AlarmLevel::Major),
        ("Minor 1", AlarmLevel::Minor),
        ("Warning 1", AlarmLevel::Warning),
        ("Info 1", AlarmLevel::Info),
    ];

    for (title, level) in test_data {
        let alarm = create_test_alarm(title, level);
        store.store_alarm(&alarm).await.unwrap();
    }

    // Get statistics
    let stats = stats_manager.get_alarm_statistics().await.unwrap();
    assert_eq!(stats.total, 6);
    assert_eq!(stats.by_level.critical, 2);
    assert_eq!(stats.by_level.major, 1);
    assert_eq!(stats.by_level.minor, 1);
    assert_eq!(stats.by_level.warning, 1);
    assert_eq!(stats.by_level.info, 1);

    // Clean up after test
    cleanup_test_data("ems:alarms:*").await.unwrap();
}

#[tokio::test]
async fn test_alarm_cleanup() {
    // Clean up before test
    cleanup_test_data("ems:alarms:*").await.unwrap();

    let config = Arc::new(test_config());
    let redis_client = Arc::new(AlarmRedisClient::new(config).await.unwrap());
    let store = AlarmStore::new(redis_client.clone()).await.unwrap();

    // Create and resolve some alarms
    for i in 0..3 {
        let alarm = create_test_alarm(&format!("Cleanup Test {}", i), AlarmLevel::Info);
        store.store_alarm(&alarm).await.unwrap();

        if i == 0 {
            // Resolve the first alarm
            store
                .resolve_alarm(&alarm.id.to_string(), "test".to_string())
                .await
                .unwrap();
        }
    }

    // Clean up old resolved alarms (0 days retention = delete all resolved)
    let deleted = store.cleanup_old_alarms(0).await.unwrap();
    assert_eq!(deleted, 1);

    // Verify only unresolved alarms remain
    let query_service = AlarmQueryService::new(redis_client.clone());
    let remaining = query_service
        .get_alarms(None, None, None, None)
        .await
        .unwrap();
    assert_eq!(remaining.len(), 2);

    // Clean up after test
    cleanup_test_data("ems:alarms:*").await.unwrap();
}
