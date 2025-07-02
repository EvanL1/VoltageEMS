use crate::config::RedisConfig;
use crate::error::{HisSrvError, Result};
use crate::storage::{DataPoint, DataValue, StorageManager};
use std::sync::Arc;
use tokio::sync::RwLock;
use redis::{Client, Commands};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use chrono::Utc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubSubMessage {
    pub id: String,
    pub channel: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source: String,
    pub data: MessageData,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageData {
    DataUpdate {
        key: String,
        value: DataValue,
        tags: HashMap<String, String>,
    },
    EventNotification {
        event_type: String,
        event_data: serde_json::Value,
    },
    SystemStatus {
        service: String,
        status: String,
        details: HashMap<String, String>,
    },
}

pub struct RedisSubscriber {
    client: Option<Client>,
    config: RedisConfig,
    message_sender: mpsc::UnboundedSender<PubSubMessage>,
    connected: bool,
}

impl RedisSubscriber {
    pub fn new(
        config: RedisConfig,
        message_sender: mpsc::UnboundedSender<PubSubMessage>,
    ) -> Self {
        Self {
            client: None,
            config,
            message_sender,
            connected: false,
        }
    }

    pub async fn connect(&mut self) -> Result<()> {
        let redis_url = self.get_redis_url();
        let client = Client::open(redis_url)?;

        // Test connection
        let mut conn = client.get_connection()?;
        let ping_result: String = conn.ping()?;
        
        if ping_result != "PONG" {
            return Err(HisSrvError::ConnectionError("Redis subscriber connection test failed".to_string()));
        }

        tracing::info!("Redis subscriber connected successfully");
        self.client = Some(client);
        self.connected = true;

        Ok(())
    }

    pub async fn start_listening(&mut self) -> Result<()> {
        if !self.connected || self.client.is_none() {
            return Err(HisSrvError::ConnectionError("Redis subscriber not connected".to_string()));
        }

        let client = self.client.as_ref().unwrap();
        let mut pubsub = client.get_connection()?.as_pubsub();

        // Subscribe to configured channels
        for channel in &self.config.subscription.channels {
            tracing::info!("Subscribing to Redis channel: {}", channel);
            if channel.contains('*') {
                pubsub.psubscribe(channel)?;
            } else {
                pubsub.subscribe(channel)?;
            }
        }

        tracing::info!("Starting Redis subscription listener");

        // Listen for messages in a loop
        loop {
            match pubsub.get_message() {
                Ok(msg) => {
                    let channel_name = msg.get_channel_name();
                    let payload: String = msg.get_payload()?;

                    tracing::debug!("Received message on channel {}: {}", channel_name, payload);

                    match self.parse_message(channel_name, &payload) {
                        Ok(pubsub_message) => {
                            if let Err(e) = self.message_sender.send(pubsub_message) {
                                tracing::error!("Failed to send message to processor: {}", e);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse message from channel {}: {}", channel_name, e);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Error receiving message from Redis: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        }
    }

    fn get_redis_url(&self) -> String {
        let conn_config = &self.config.connection;
        if !conn_config.socket.is_empty() {
            format!("unix://{}", conn_config.socket)
        } else {
            if conn_config.password.is_empty() {
                format!("redis://{}:{}/{}", conn_config.host, conn_config.port, conn_config.database)
            } else {
                format!(
                    "redis://:{}@{}:{}/{}",
                    conn_config.password, conn_config.host, conn_config.port, conn_config.database
                )
            }
        }
    }

    fn parse_message(&self, channel: &str, payload: &str) -> Result<PubSubMessage> {
        // Try to parse as structured message first
        if let Ok(structured_msg) = serde_json::from_str::<PubSubMessage>(payload) {
            return Ok(structured_msg);
        }

        // Otherwise, create a basic message
        let message_data = if channel.starts_with("data:") {
            // Assume it's a data update
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(payload) {
                MessageData::DataUpdate {
                    key: channel.strip_prefix("data:").unwrap_or(channel).to_string(),
                    value: self.json_to_data_value(&json_value),
                    tags: HashMap::new(),
                }
            } else {
                MessageData::DataUpdate {
                    key: channel.strip_prefix("data:").unwrap_or(channel).to_string(),
                    value: DataValue::String(payload.to_string()),
                    tags: HashMap::new(),
                }
            }
        } else if channel.starts_with("events:") {
            MessageData::EventNotification {
                event_type: channel.strip_prefix("events:").unwrap_or(channel).to_string(),
                event_data: serde_json::from_str(payload).unwrap_or(serde_json::Value::String(payload.to_string())),
            }
        } else {
            MessageData::SystemStatus {
                service: "unknown".to_string(),
                status: payload.to_string(),
                details: HashMap::new(),
            }
        };

        Ok(PubSubMessage {
            id: Uuid::new_v4().to_string(),
            channel: channel.to_string(),
            timestamp: Utc::now(),
            source: "redis_subscriber".to_string(),
            data: message_data,
            metadata: HashMap::new(),
        })
    }

    fn json_to_data_value(&self, json: &serde_json::Value) -> DataValue {
        match json {
            serde_json::Value::String(s) => DataValue::String(s.clone()),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    DataValue::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    DataValue::Float(f)
                } else {
                    DataValue::String(n.to_string())
                }
            }
            serde_json::Value::Bool(b) => DataValue::Boolean(*b),
            _ => DataValue::Json(json.clone()),
        }
    }
}

pub struct MessageProcessor {
    storage_manager: Arc<RwLock<StorageManager>>,
    message_receiver: mpsc::UnboundedReceiver<PubSubMessage>,
}

impl MessageProcessor {
    pub fn new(
        storage_manager: Arc<RwLock<StorageManager>>,
        message_receiver: mpsc::UnboundedReceiver<PubSubMessage>,
    ) -> Self {
        Self {
            storage_manager,
            message_receiver,
        }
    }

    pub async fn start_processing(&mut self) -> Result<()> {
        tracing::info!("Starting message processor");

        while let Some(message) = self.message_receiver.recv().await {
            if let Err(e) = self.process_message(message).await {
                tracing::error!("Failed to process message: {}", e);
            }
        }

        Ok(())
    }

    async fn process_message(&mut self, message: PubSubMessage) -> Result<()> {
        tracing::debug!("Processing message: {} from channel {}", message.id, message.channel);

        match &message.data {
            MessageData::DataUpdate { key, value, tags } => {
                self.handle_data_update(&message, key, value, tags).await?;
            }
            MessageData::EventNotification { event_type, event_data } => {
                self.handle_event_notification(&message, event_type, event_data).await?;
            }
            MessageData::SystemStatus { service, status, details } => {
                self.handle_system_status(&message, service, status, details).await?;
            }
        }

        Ok(())
    }

    async fn handle_data_update(
        &mut self,
        message: &PubSubMessage,
        key: &str,
        value: &DataValue,
        tags: &HashMap<String, String>,
    ) -> Result<()> {
        // Create data point
        let mut data_point = DataPoint {
            key: key.to_string(),
            timestamp: message.timestamp,
            value: value.clone(),
            tags: tags.clone(),
            metadata: message.metadata.clone(),
        };

        // Add message source and channel as metadata
        data_point.metadata.insert("source".to_string(), message.source.clone());
        data_point.metadata.insert("channel".to_string(), message.channel.clone());
        data_point.metadata.insert("message_id".to_string(), message.id.clone());

        // Determine storage backend
        let backend_name = self.determine_storage_backend(key);
        
        let mut storage_manager = self.storage_manager.write().await;
        if let Some(backend) = storage_manager.get_backend(Some(&backend_name)) {
            backend.store_data_point(&data_point).await?;
            tracing::debug!("Stored data point for key {} in backend {}", key, backend_name);
        } else {
            tracing::warn!("No storage backend found for key {}", key);
        }

        Ok(())
    }

    async fn handle_event_notification(
        &mut self,
        message: &PubSubMessage,
        event_type: &str,
        event_data: &serde_json::Value,
    ) -> Result<()> {
        // Store event as data point
        let data_point = DataPoint {
            key: format!("events:{}", event_type),
            timestamp: message.timestamp,
            value: DataValue::Json(event_data.clone()),
            tags: HashMap::from([
                ("type".to_string(), "event".to_string()),
                ("event_type".to_string(), event_type.to_string()),
            ]),
            metadata: message.metadata.clone(),
        };

        // Use default storage backend for events
        let mut storage_manager = self.storage_manager.write().await;
        if let Some(backend) = storage_manager.get_backend(None) {
            backend.store_data_point(&data_point).await?;
            tracing::debug!("Stored event {} in default backend", event_type);
        }

        Ok(())
    }

    async fn handle_system_status(
        &mut self,
        message: &PubSubMessage,
        service: &str,
        status: &str,
        details: &HashMap<String, String>,
    ) -> Result<()> {
        // Store system status as data point
        let mut tags = HashMap::from([
            ("type".to_string(), "system_status".to_string()),
            ("service".to_string(), service.to_string()),
            ("status".to_string(), status.to_string()),
        ]);

        // Add details as tags
        for (key, value) in details {
            tags.insert(format!("detail_{}", key), value.clone());
        }

        let data_point = DataPoint {
            key: format!("system:{}:status", service),
            timestamp: message.timestamp,
            value: DataValue::String(status.to_string()),
            tags,
            metadata: message.metadata.clone(),
        };

        // Use default storage backend for system status
        let mut storage_manager = self.storage_manager.write().await;
        if let Some(backend) = storage_manager.get_backend(None) {
            backend.store_data_point(&data_point).await?;
            tracing::debug!("Stored system status for service {}", service);
        }

        Ok(())
    }

    fn determine_storage_backend(&self, key: &str) -> String {
        // Simple logic - can be made configurable
        if key.starts_with("temp:") || key.starts_with("sensor:") {
            "influxdb".to_string()
        } else if key.starts_with("logs:") || key.starts_with("events:") {
            "redis".to_string()
        } else {
            "influxdb".to_string() // Default
        }
    }
}