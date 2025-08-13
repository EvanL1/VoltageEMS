mod config;
mod error;
mod formatter;

use crate::config::{load_config, FormatType, NetworkConfig};
use crate::error::Result;
use crate::formatter::{AsciiFormatter, DataFormatter, JsonFormatter};
use clap::Parser;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use voltage_libs::redis::RedisClient;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the configuration file
    #[clap(short, long, value_parser, default_value = "config/netsrv.yml")]
    config: PathBuf,
}

/// Network service for forwarding Redis data to external networks
pub struct NetSrv {
    config: crate::config::Config,
    redis_client: Arc<RedisClient>,
    network_clients: HashMap<String, Box<dyn NetworkClient>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

/// Simplified network client trait
#[async_trait::async_trait]
pub trait NetworkClient: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    fn is_connected(&self) -> bool;
    async fn send(&self, data: &str) -> Result<()>;
    fn name(&self) -> &str;
}

/// Simple HTTP client
pub struct HttpClient {
    name: String,
    config: crate::config::HttpConfig,
    formatter: Box<dyn DataFormatter>,
    client: reqwest::Client,
}

impl HttpClient {
    pub fn new(
        config: crate::config::HttpConfig,
        formatter: Box<dyn DataFormatter>,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| {
                crate::error::NetSrvError::Network(format!("Failed to create HTTP client: {}", e))
            })?;

        Ok(Self {
            name: config.name.clone(),
            config,
            formatter,
            client,
        })
    }
}

#[async_trait::async_trait]
impl NetworkClient for HttpClient {
    async fn connect(&mut self) -> Result<()> {
        info!("HTTP client '{}' ready for: {}", self.name, self.config.url);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("HTTP client '{}' disconnected", self.name);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        true
    }

    async fn send(&self, data: &str) -> Result<()> {
        let formatted_data = self.formatter.format(data)?;

        let mut request = match self.config.method.to_uppercase().as_str() {
            "GET" => self.client.get(&self.config.url),
            "POST" => self.client.post(&self.config.url),
            "PUT" => self.client.put(&self.config.url),
            "DELETE" => self.client.delete(&self.config.url),
            "PATCH" => self.client.patch(&self.config.url),
            _ => {
                return Err(crate::error::NetSrvError::Config(format!(
                    "Unsupported HTTP method: {}",
                    self.config.method
                )))
            },
        };

        // Add headers
        for (key, value) in &self.config.headers {
            request = request.header(key, value);
        }

        // Send request
        let response = if matches!(
            self.config.method.to_uppercase().as_str(),
            "POST" | "PUT" | "PATCH"
        ) {
            request.body(formatted_data).send().await
        } else {
            request.send().await
        };

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    debug!(
                        "HTTP request to '{}' successful: {}",
                        self.name,
                        resp.status()
                    );
                } else {
                    warn!(
                        "HTTP request to '{}' failed with status: {}",
                        self.name,
                        resp.status()
                    );
                }
            },
            Err(e) => {
                error!("HTTP request to '{}' failed: {}", self.name, e);
                return Err(crate::error::NetSrvError::Network(format!(
                    "HTTP request failed: {}",
                    e
                )));
            },
        }

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Simple MQTT client
pub struct MqttClient {
    name: String,
    config: crate::config::MqttConfig,
    formatter: Box<dyn DataFormatter>,
    client: Option<rumqttc::AsyncClient>,
    connected: bool,
}

impl MqttClient {
    pub fn new(
        config: crate::config::MqttConfig,
        formatter: Box<dyn DataFormatter>,
    ) -> Result<Self> {
        Ok(Self {
            name: config.name.clone(),
            config,
            formatter,
            client: None,
            connected: false,
        })
    }
}

#[async_trait::async_trait]
impl NetworkClient for MqttClient {
    async fn connect(&mut self) -> Result<()> {
        let broker_url = url::Url::parse(&self.config.broker)
            .map_err(|e| crate::error::NetSrvError::Config(format!("Invalid broker URL: {}", e)))?;

        let host = broker_url.host_str().unwrap_or("localhost");
        let port = broker_url.port().unwrap_or(1883);

        let mut mqttoptions = rumqttc::MqttOptions::new(&self.config.client_id, host, port);

        if let (Some(ref username), Some(ref password)) =
            (&self.config.username, &self.config.password)
        {
            mqttoptions.set_credentials(username, password);
        }

        let (client, mut eventloop) = rumqttc::AsyncClient::new(mqttoptions, 10);

        // Start event loop in background
        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(_) => {},
                    Err(e) => {
                        error!("MQTT event loop error: {}", e);
                        break;
                    },
                }
            }
        });

        self.client = Some(client);
        self.connected = true;
        info!("MQTT client '{}' connected to {}:{}", self.name, host, port);

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(ref client) = self.client {
            let _ = client.disconnect().await;
        }
        self.client = None;
        self.connected = false;
        info!("MQTT client '{}' disconnected", self.name);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    async fn send(&self, data: &str) -> Result<()> {
        if let Some(ref client) = self.client {
            let formatted_data = self.formatter.format(data)?;

            let qos = match self.config.qos {
                0 => rumqttc::QoS::AtMostOnce,
                1 => rumqttc::QoS::AtLeastOnce,
                2 => rumqttc::QoS::ExactlyOnce,
                _ => rumqttc::QoS::AtMostOnce,
            };

            let topic = self.config.topic_prefix.to_string();

            client
                .publish(topic, qos, false, formatted_data)
                .await
                .map_err(|e| {
                    crate::error::NetSrvError::Network(format!("MQTT publish failed: {}", e))
                })?;

            debug!("MQTT client '{}' published data", self.name);
        }
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl NetSrv {
    pub async fn new(config: crate::config::Config) -> Result<Self> {
        // Initialize Redis client
        let redis_client = Arc::new(RedisClient::new(&config.redis.url).await.map_err(|e| {
            crate::error::NetSrvError::Redis(format!("Failed to connect to Redis: {}", e))
        })?);

        let mut network_clients: HashMap<String, Box<dyn NetworkClient>> = HashMap::new();

        // Initialize network clients
        for network_config in &config.networks {
            let formatter: Box<dyn DataFormatter> = match &network_config {
                NetworkConfig::Mqtt(mqtt_config) => match mqtt_config.format_type {
                    FormatType::Json => Box::new(JsonFormatter::new()),
                    FormatType::Ascii => Box::new(AsciiFormatter::new()),
                    FormatType::Binary => Box::new(JsonFormatter::new()), // Fallback to JSON for binary
                },
                NetworkConfig::Http(_) => Box::new(JsonFormatter::new()), // Default to JSON for HTTP
            };

            let client: Box<dyn NetworkClient> = match network_config {
                NetworkConfig::Mqtt(mqtt_config) => {
                    Box::new(MqttClient::new(mqtt_config.clone(), formatter)?)
                },
                NetworkConfig::Http(http_config) => {
                    Box::new(HttpClient::new(http_config.clone(), formatter)?)
                },
            };

            network_clients.insert(client.name().to_string(), client);
        }

        Ok(Self {
            config,
            redis_client,
            network_clients,
            shutdown_tx: None,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        info!(
            "Starting network service with {} clients",
            self.network_clients.len()
        );

        // Connect all network clients
        for (name, client) in &mut self.network_clients {
            match client.connect().await {
                Ok(_) => info!("Connected to network '{}'", name),
                Err(e) => warn!("Failed to connect to network '{}': {}", name, e),
            }
        }

        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        // Start data forwarding loop
        let _redis_client = Arc::clone(&self.redis_client);
        let data_key_pattern = self.config.data.redis_data_key.clone();
        let polling_interval =
            std::time::Duration::from_secs(self.config.data.redis_polling_interval_secs);

        // Create a simplified data forwarding loop
        let forwarding_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(polling_interval);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // In a real implementation, we would:
                        // 1. Subscribe to Redis pattern updates
                        // 2. Fetch latest data from matching keys
                        // 3. Forward data to all connected network clients
                        debug!("Data forwarding tick - pattern: {}", data_key_pattern);

                        // For now, just log that we're running
                        // Note: Redis health check would require Arc<Mutex<RedisClient>>
                        debug!("Data forwarding service is running");
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Data forwarding loop shutting down");
                        break;
                    }
                }
            }
        });

        info!("Network service started successfully");

        // Wait for shutdown signal
        forwarding_task.await.map_err(|e| {
            crate::error::NetSrvError::Runtime(format!("Forwarding task failed: {}", e))
        })?;

        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down network service");

        // Send shutdown signal
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()).await;
        }

        // Disconnect all network clients
        for (name, client) in &mut self.network_clients {
            match client.disconnect().await {
                Ok(_) => info!("Disconnected from network '{}'", name),
                Err(e) => warn!("Error disconnecting from network '{}': {}", name, e),
            }
        }

        info!("Network service shutdown complete");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Parse command line arguments
    let _args = Args::parse();

    // Load configuration
    let config = match load_config() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            std::process::exit(1);
        },
    };

    info!("Starting Network Service: {}", config.service.name);
    info!("Redis URL: {}", config.redis.url);
    info!("Networks configured: {}", config.networks.len());

    // Initialize and start the network service
    let netsrv = NetSrv::new(config).await?;

    // Set up signal handling for graceful shutdown
    let mut netsrv_for_signal = netsrv;

    tokio::select! {
        result = netsrv_for_signal.start() => {
            if let Err(e) = result {
                error!("Network service error: {}", e);
                std::process::exit(1);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
            if let Err(e) = netsrv_for_signal.shutdown().await {
                error!("Error during shutdown: {}", e);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
