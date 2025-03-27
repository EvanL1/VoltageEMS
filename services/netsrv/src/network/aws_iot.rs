use crate::config::network_config::AwsIotConfig;
use crate::error::{NetSrvError, Result};
use crate::formatter::DataFormatter;
use crate::network::NetworkClient;
use async_trait::async_trait;
use log::{debug, error, info, warn};
use paho_mqtt as paho;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct AwsIotClient {
    config: AwsIotConfig,
    client: Option<paho::AsyncClient>,
    formatter: Box<dyn DataFormatter>,
    connected: Arc<Mutex<bool>>,
}

impl AwsIotClient {
    pub fn new(config: AwsIotConfig, formatter: Box<dyn DataFormatter>) -> Self {
        AwsIotClient {
            config,
            client: None,
            formatter,
            connected: Arc::new(Mutex::new(false)),
        }
    }

    fn get_qos(&self) -> i32 {
        match self.config.qos {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 0,
        }
    }
}

#[async_trait]
impl NetworkClient for AwsIotClient {
    async fn connect(&mut self) -> Result<()> {
        // Create MQTT client options
        let create_opts = paho::CreateOptionsBuilder::new()
            .server_uri(format!("ssl://{}:8883", self.config.endpoint))
            .client_id(&self.config.client_id)
            .finalize();

        // Create client
        let client = paho::AsyncClient::new(create_opts)
            .map_err(|e| NetSrvError::AwsIotError(format!("Error creating AWS IoT client: {}", e)))?;

        // Set connection options
        let ssl_opts = paho::SslOptionsBuilder::new()
            .trust_store(&self.config.ca_path)
            .map_err(|e| NetSrvError::AwsIotError(format!("Error setting CA certificate: {}", e)))?
            .key_store(&self.config.cert_path)
            .map_err(|e| NetSrvError::AwsIotError(format!("Error setting client certificate: {}", e)))?
            .private_key(&self.config.key_path)
            .map_err(|e| NetSrvError::AwsIotError(format!("Error setting client key: {}", e)))?
            .finalize();

        let conn_opts = paho::ConnectOptionsBuilder::new()
            .ssl_options(ssl_opts)
            .keep_alive_interval(Duration::from_secs(30))
            .clean_session(true)
            .finalize();

        // Connect to AWS IoT Core
        client.connect(conn_opts).await
            .map_err(|e| NetSrvError::AwsIotError(format!("Error connecting to AWS IoT Core: {}", e)))?;

        // Set connection status callback
        let connected = self.connected.clone();
        client.set_connected_callback(move |_| {
            *connected.lock().unwrap() = true;
            info!("Connected to AWS IoT Core");
        });

        client.set_connection_lost_callback(move |_| {
            *connected.lock().unwrap() = false;
            error!("Connection to AWS IoT Core lost");
        });

        self.client = Some(client);
        *self.connected.lock().unwrap() = true;

        info!("Connected to AWS IoT Core: {}", self.config.endpoint);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(client) = &self.client {
            client.disconnect(None).await
                .map_err(|e| NetSrvError::AwsIotError(format!("Error disconnecting from AWS IoT Core: {}", e)))?;
        }
        
        *self.connected.lock().unwrap() = false;
        self.client = None;
        
        Ok(())
    }

    async fn send(&self, data: &str) -> Result<()> {
        if !self.is_connected() {
            return Err(NetSrvError::ConnectionError(
                "Not connected to AWS IoT Core".to_string(),
            ));
        }

        if let Some(client) = &self.client {
            let message = paho::Message::new(&self.config.topic, data, self.get_qos());
            client.publish(message).await
                .map_err(|e| NetSrvError::AwsIotError(format!("Error publishing to AWS IoT Core: {}", e)))?;
            
            debug!("Published message to AWS IoT Core topic: {}", self.config.topic);
            Ok(())
        } else {
            Err(NetSrvError::ConnectionError(
                "AWS IoT client not initialized".to_string(),
            ))
        }
    }

    fn is_connected(&self) -> bool {
        *self.connected.lock().unwrap()
    }
} 