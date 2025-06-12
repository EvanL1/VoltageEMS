use crate::config::network_config::HttpConfig;
use crate::error::{NetSrvError, Result};
use crate::formatter::DataFormatter;
use crate::network::NetworkClient;
use async_trait::async_trait;
use log::{debug, info};
use reqwest::{Client, Method};
use serde_json::Value;
use std::any::Any;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

pub struct HttpClient {
    config: HttpConfig,
    client: Client,
    formatter: Box<dyn DataFormatter>,
    connected: bool,
}

impl HttpClient {
    pub fn new(config: HttpConfig, formatter: Box<dyn DataFormatter>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            config,
            client,
            formatter,
            connected: false,
        }
    }

    fn get_method(&self) -> Method {
        Method::from_str(&self.config.method.to_uppercase()).unwrap_or(Method::POST)
    }

    fn get_headers(&self) -> HashMap<String, String> {
        let mut headers = self.config.headers.clone().unwrap_or_default();
        
        // Add authentication headers if configured
        match self.config.auth_type.as_deref() {
            Some("bearer") => {
                if let Some(token) = &self.config.token {
                    headers.insert("Authorization".to_string(), format!("Bearer {}", token));
                }
            }
            Some("basic") => {
                if let (Some(username), Some(password)) = (&self.config.username, &self.config.password) {
                    let credentials = base64::encode(format!("{}:{}", username, password));
                    headers.insert("Authorization".to_string(), format!("Basic {}", credentials));
                }
            }
            _ => {}
        }

        // Set default content type if not specified
        if !headers.contains_key("Content-Type") && !headers.contains_key("content-type") {
            headers.insert("Content-Type".to_string(), "application/json".to_string());
        }

        headers
    }

    /// Format data using the configured formatter
    pub fn format_data(&self, data: &Value) -> Result<String> {
        self.formatter.format(data)
    }
}

#[async_trait]
impl NetworkClient for HttpClient {
    async fn connect(&mut self) -> Result<()> {
        info!("Connecting to HTTP endpoint: {}", self.config.url);
        
        // For HTTP, we just validate the configuration and mark as connected
        // Actual connection will be established when sending data
        if self.config.url.is_empty() {
            return Err(NetSrvError::ConfigError("HTTP URL is empty".to_string()));
        }

        self.connected = true;
        info!("HTTP client initialized successfully");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting HTTP client");
        self.connected = false;
        info!("HTTP client disconnected");
        Ok(())
    }

    async fn send(&self, data: &str) -> Result<()> {
        if !self.is_connected() {
            return Err(NetSrvError::ConnectionError("Not connected".to_string()));
        }

        let method = self.get_method();
        let mut request = self.client.request(method.clone(), &self.config.url);

        // Add headers
        let headers = self.get_headers();
        for (key, value) in headers {
            request = request.header(key, value);
        }

        // Add body for POST, PUT, PATCH methods
        if matches!(method, Method::POST | Method::PUT | Method::PATCH) {
            request = request.body(data.to_string());
        }

        // Send request
        let response = request.send().await
            .map_err(|e| NetSrvError::HttpError(format!("Failed to send HTTP request: {}", e)))?;

        if response.status().is_success() {
            debug!("HTTP request sent successfully to: {}", self.config.url);
            Ok(())
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(NetSrvError::HttpError(format!(
                "HTTP request failed with status {}: {}", 
                status, 
                error_text
            )))
        }
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
} 