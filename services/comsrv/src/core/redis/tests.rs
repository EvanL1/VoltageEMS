//! Tests for Redis storage with pub/sub functionality

#[cfg(test)]
mod tests {
    use super::super::publisher::{PointMessage, PublisherConfig};
    use super::super::storage::RedisStorage;
    use super::super::types::*;
    use crate::core::config::types::redis::PubSubConfig;
    use std::time::Duration;
    use tokio::time::sleep;

    // Helper function to create test pubsub config
    fn test_pubsub_config() -> PubSubConfig {
        PubSubConfig {
            enabled: true,
            batch_size: 10,
            batch_timeout_ms: 50,
            publish_on_set: true,
            message_version: "1.0".to_string(),
        }
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_storage_with_publisher() {
        let redis_url = "redis://127.0.0.1:6379";
        let config = test_pubsub_config();

        let mut storage = RedisStorage::new(redis_url)
            .await
            .expect("Failed to create storage with publisher");

        // Test single point set
        storage
            .set_point(1001, TYPE_MEASUREMENT, 10001, 25.6)
            .await
            .expect("Failed to set point");

        // Verify point was stored
        let result = storage
            .get_point(1001, TYPE_MEASUREMENT, 10001)
            .await
            .expect("Failed to get point");

        assert!(result.is_some());
        let (value, _timestamp) = result.unwrap();
        assert_eq!(value, 25.6);

        // Test batch set
        let updates = vec![
            PointUpdate {
                channel_id: 1001,
                point_type: TYPE_MEASUREMENT,
                point_id: 10002,
                value: 30.5,
            },
            PointUpdate {
                channel_id: 1001,
                point_type: TYPE_SIGNAL,
                point_id: 20001,
                value: 1.0,
            },
        ];

        storage
            .set_points(&updates)
            .await
            .expect("Failed to batch set points");

        // Give publisher time to process
        sleep(Duration::from_millis(100)).await;

        // Close storage and wait for publisher
        storage.close().await;
    }

    #[test]
    fn test_point_message_serialization() {
        let msg = PointMessage::new(
            1001,
            "m".to_string(),
            10001,
            25.6,
            1234567890,
            "1.0".to_string(),
        );

        let json = msg.to_json().expect("Failed to serialize message");
        assert!(json.contains(r#""channel_id":1001"#));
        assert!(json.contains(r#""point_type":"m""#));
        assert!(json.contains(r#""point_id":10001"#));
        assert!(json.contains(r#""value":25.6"#));
        assert!(json.contains(r#""timestamp":1234567890"#));
        assert!(json.contains(r#""version":"1.0""#));
    }

    #[test]
    fn test_message_channel_name() {
        let msg = PointMessage::new(
            1001,
            "m".to_string(),
            10001,
            25.6,
            1234567890,
            "1.0".to_string(),
        );

        assert_eq!(msg.channel_name(), "1001:m:10001");
    }

    #[tokio::test]
    async fn test_publisher_config() {
        let config = PublisherConfig {
            enabled: true,
            batch_size: 50,
            batch_timeout: Duration::from_millis(100),
            message_version: "1.0".to_string(),
        };

        assert!(config.enabled);
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.batch_timeout.as_millis(), 100);
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_storage_without_publisher() {
        let redis_url = "redis://127.0.0.1:6379";

        let mut storage = RedisStorage::new(redis_url)
            .await
            .expect("Failed to create storage");

        // Should work normally without publisher
        storage
            .set_point(1001, TYPE_MEASUREMENT, 10001, 25.6)
            .await
            .expect("Failed to set point");

        let result = storage
            .get_point(1001, TYPE_MEASUREMENT, 10001)
            .await
            .expect("Failed to get point");

        assert!(result.is_some());
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_batch_publishing_performance() {
        let redis_url = "redis://127.0.0.1:6379";
        let mut config = test_pubsub_config();
        config.batch_size = 100;

        let mut storage = RedisStorage::new(redis_url)
            .await
            .expect("Failed to create storage");

        // Create large batch
        let mut updates = Vec::new();
        for i in 0..1000 {
            updates.push(PointUpdate {
                channel_id: 1001,
                point_type: TYPE_MEASUREMENT,
                point_id: 10000 + i,
                value: i as f64,
            });
        }

        let start = std::time::Instant::now();
        storage
            .set_points(&updates)
            .await
            .expect("Failed to batch set points");

        let elapsed = start.elapsed();
        println!("Batch set {} points in {:?}", updates.len(), elapsed);

        // Wait for publishing to complete
        sleep(Duration::from_millis(200)).await;
        storage.close().await;
    }
}
