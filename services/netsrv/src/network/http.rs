use crate::error::{NetSrvError, Result};
use crate::formatter::DataFormatter;
use crate::network::NetworkClient;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::any::Any;
use std::collections::HashMap;
use std::time::Duration;

/// HTTP client configuration
#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub url: String,
    pub method: String,
    pub headers: Option<HashMap<String, String>>,
    pub auth_type: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub token: Option<String>,
    pub timeout_ms: u64,
}

/// HTTP client for REST API communication
pub struct HttpClient {
    config: HttpConfig,
    client: Client,
    formatter: Box<dyn DataFormatter>,
    connected: bool,
}

impl HttpClient {
    /// Create a new HTTP client
    pub fn new(config: HttpConfig, formatter: Box<dyn DataFormatter>) -> Result<Self> {
        if config.url.is_empty() {
            return Err(NetSrvError::Config("HTTP URL is empty".to_string()));
        }

        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .map_err(|e| NetSrvError::Http(e.to_string()))?;

        Ok(HttpClient {
            config,
            client,
            formatter,
            connected: false,
        })
    }

    /// Format data using the client's formatter
    pub fn format_data(&self, data: &Value) -> Result<String> {
        self.formatter.format(data)
    }

    /// Build HTTP headers based on configuration
    fn build_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();

        // Add configured headers
        if let Some(config_headers) = &self.config.headers {
            headers.extend(config_headers.clone());
        }

        // Add authentication headers
        match self.config.auth_type.as_deref() {
            Some("bearer") => {
                if let Some(token) = &self.config.token {
                    headers.insert("Authorization".to_string(), format!("Bearer {}", token));
                }
            }
            Some("basic") => {
                if let (Some(username), Some(password)) = (&self.config.username, &self.config.password) {
                    use base64::{Engine as _, engine::general_purpose};
                    let credentials = general_purpose::STANDARD.encode(format!("{}:{}", username, password));
                    headers.insert("Authorization".to_string(), format!("Basic {}", credentials));
                }
            }
            Some("apikey") => {
                if let Some(token) = &self.config.token {
                    headers.insert("X-API-Key".to_string(), token.clone());
                }
            }
            _ => {}
        }

        headers
    }
}

#[async_trait]
impl NetworkClient for HttpClient {
    async fn connect(&mut self) -> Result<()> {
        // For HTTP, connection is tested by attempting a simple request
        // In this case, we'll just mark as connected since HTTP is connectionless
        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    async fn send(&self, data: &str) -> Result<()> {
        if !self.connected {
            return Err(NetSrvError::Connection("Not connected".to_string()));
        }

        let headers = self.build_headers();
        let mut request_builder = match self.config.method.to_uppercase().as_str() {
            "GET" => self.client.get(&self.config.url),
            "POST" => self.client.post(&self.config.url),
            "PUT" => self.client.put(&self.config.url),
            "PATCH" => self.client.patch(&self.config.url),
            _ => self.client.post(&self.config.url), // Default to POST
        };

        // Add headers
        for (key, value) in headers {
            request_builder = request_builder.header(&key, &value);
        }

        // Add body for non-GET requests
        if self.config.method.to_uppercase() != "GET" {
            request_builder = request_builder.body(data.to_string());
        }

        let response = request_builder.send().await
            .map_err(|e| NetSrvError::Http(format!("Failed to send HTTP request: {}", e)))?;

        if !response.status().is_success() {
            return Err(NetSrvError::Http(format!(
                "HTTP request failed with status: {}",
                response.status()
            )));
        }

        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
} 