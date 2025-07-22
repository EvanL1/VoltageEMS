//! Common test utilities and helpers

use alarmsrv::{
    api::routes,
    domain::{Alarm, AlarmClassifier, AlarmLevel},
    redis::{AlarmQueryService, AlarmRedisClient, AlarmStatisticsManager, AlarmStore},
    AppState,
};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod test_config;

/// Create a test app state with mock services
pub async fn create_test_state() -> Result<AppState> {
    let config = Arc::new(test_config::test_config());
    let redis_client = Arc::new(AlarmRedisClient::new(config.clone()).await?);
    let alarm_store = Arc::new(AlarmStore::new(redis_client.clone()).await?);
    let query_service = Arc::new(AlarmQueryService::new(redis_client.clone()));
    let stats_manager = Arc::new(AlarmStatisticsManager::new(redis_client.clone()));
    let classifier = Arc::new(AlarmClassifier::new(config.clone()));

    Ok(AppState {
        alarms: Arc::new(RwLock::new(Vec::new())),
        config,
        redis_client,
        alarm_store,
        query_service,
        stats_manager,
        classifier,
    })
}

/// Create a test router for API testing
pub async fn create_test_router() -> Result<axum::Router> {
    let state = create_test_state().await?;
    Ok(routes::create_router(state))
}

/// Create a sample alarm for testing
pub fn create_test_alarm(title: &str, level: AlarmLevel) -> Alarm {
    Alarm::new(title.to_string(), format!("Test alarm: {}", title), level)
}

/// Clean up test data from Redis
pub async fn cleanup_test_data(key_pattern: &str) -> Result<()> {
    let config = Arc::new(test_config::test_config());
    let redis_client = Arc::new(AlarmRedisClient::new(config).await?);

    let mut client_guard = redis_client.get_client().await?;
    if let Some(conn) = client_guard.as_mut() {
        let keys: Vec<String> = conn.keys(key_pattern).await?;
        if !keys.is_empty() {
            for key in keys {
                conn.del(&[key.as_str()]).await?;
            }
        }
    }

    Ok(())
}

/// Wait for a condition to be true with timeout
pub async fn wait_for<F>(mut condition: F, timeout_secs: u64) -> bool
where
    F: FnMut() -> bool,
{
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_secs);

    while start.elapsed() < timeout {
        if condition() {
            return true;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_test_state() {
        let result = create_test_state().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_test_alarm() {
        let alarm = create_test_alarm("Test", AlarmLevel::Warning);
        assert_eq!(alarm.title, "Test");
        assert_eq!(alarm.level, AlarmLevel::Warning);
    }
}
