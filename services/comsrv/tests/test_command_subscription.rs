//! Test command subscription functionality

use comsrv::core::config::{ChannelConfig, ChannelLoggingConfig, ProtocolType};
use comsrv::core::framework::ProtocolFactory;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

fn create_test_channel_config(id: u16) -> ChannelConfig {
    let mut parameters = HashMap::new();
    parameters.insert(
        "host".to_string(),
        serde_yaml::Value::String("127.0.0.1".to_string()),
    );
    parameters.insert("port".to_string(), serde_yaml::Value::Number(502.into()));
    parameters.insert(
        "timeout".to_string(),
        serde_yaml::Value::Number(5000.into()),
    );

    ChannelConfig {
        id,
        name: format!("Test Channel {}", id),
        description: Some("Test channel for command subscription".to_string()),
        protocol: ProtocolType::Virtual.to_string(),
        parameters,
        logging: ChannelLoggingConfig::default(),
        table_config: None,
        points: Vec::new(),
        combined_points: Vec::new(),
    }
}

#[tokio::test]
async fn test_command_subscription_lifecycle() {
    // Ensure plugins are loaded (ignore error if already loaded)
    let _ = comsrv::plugins::plugin_registry::discovery::load_all_plugins();

    // Create protocol factory
    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // Create a test channel
    let config = create_test_channel_config(1);
    {
        let factory_guard = factory.write().await;
        factory_guard
            .create_channel(config)
            .await
            .expect("Failed to create channel");
    }

    // Start all channels (which should also start command subscriptions)
    {
        let factory_guard = factory.read().await;
        factory_guard
            .start_all_channels()
            .await
            .expect("Failed to start channels");
    }

    // Let it run for a bit
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Stop all channels (which should also stop command subscriptions)
    {
        let factory_guard = factory.read().await;
        factory_guard
            .stop_all_channels()
            .await
            .expect("Failed to stop channels");
    }

    println!("Command subscription lifecycle test completed successfully");
}

#[tokio::test]
async fn test_multiple_channels_command_subscription() {
    // Ensure plugins are loaded (ignore error if already loaded)
    let _ = comsrv::plugins::plugin_registry::discovery::load_all_plugins();

    // Create protocol factory
    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // Create multiple test channels
    for id in 1..=3 {
        let config = create_test_channel_config(id);
        let factory_guard = factory.write().await;
        factory_guard
            .create_channel(config)
            .await
            .expect(&format!("Failed to create channel {}", id));
    }

    // Start all channels
    {
        let factory_guard = factory.read().await;
        factory_guard
            .start_all_channels()
            .await
            .expect("Failed to start channels");
    }

    // Verify channel count
    {
        let factory_guard = factory.read().await;
        assert_eq!(factory_guard.channel_count(), 3);
    }

    // Let it run for a bit
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Stop all channels
    {
        let factory_guard = factory.read().await;
        factory_guard
            .stop_all_channels()
            .await
            .expect("Failed to stop channels");
    }

    println!("Multiple channels command subscription test completed successfully");
}
