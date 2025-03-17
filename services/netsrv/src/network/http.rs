use crate::config::network_config::HttpConfig;
use crate::error::{NetSrvError, Result};
use crate::formatter::DataFormatter;
use crate::network::NetworkClient;
use async_trait::async_trait;
use log::{debug, error, info, warn};
use reqwest::{Client, ClientBuilder, Method, RequestBuilder};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct HttpClient {
    config: HttpConfig,
    client: Option<Client>,
    formatter: Box<dyn DataFormatter>,
    connected: Arc<Mutex<bool>>,
}

impl HttpClient {
    pub fn new(config: HttpConfig, formatter: Box<dyn DataFormatter>) -> Self {
        HttpClient {
            config,
            client: None,
            formatter,
            connected: Arc::new(Mutex::new(false)),
        }
    }

    fn get_method(&self) -> Method {
        match self.config.method.to_uppercase().as_str() {
            "GET" => Method::GET,
            "POST" => Method::POST,
            "PUT" => Method::PUT,
            "DELETE" => Method::DELETE,
            "PATCH" => Method::PATCH,
            _ => Method::POST,
        }
    }

    fn build_request(&self, data: &str) -> Result<RequestBuilder> {
        if let Some(client) = &self.client {
            let method = self.get_method();
            let mut request = client.request(method, &self.config.url);

            // 添加请求头
            if let Some(headers) = &self.config.headers {
                for (key, value) in headers {
                    request = request.header(key, value);
                }
            }

            // 添加认证
            if let Some(auth_type) = &self.config.auth_type {
                match auth_type.to_lowercase().as_str() {
                    "basic" => {
                        if let (Some(username), Some(password)) = (&self.config.username, &self.config.password) {
                            request = request.basic_auth(username, Some(password));
                        } else {
                            return Err(NetSrvError::HttpError(
                                "Basic auth requires username and password".to_string(),
                            ));
                        }
                    }
                    "bearer" => {
                        if let Some(token) = &self.config.token {
                            request = request.bearer_auth(token);
                        } else {
                            return Err(NetSrvError::HttpError(
                                "Bearer auth requires token".to_string(),
                            ));
                        }
                    }
                    _ => {
                        warn!("Unknown auth type: {}", auth_type);
                    }
                }
            }

            // 添加请求体
            if method != Method::GET {
                request = request.body(data.to_string());
                request = request.header("Content-Type", "application/json");
            }

            Ok(request)
        } else {
            Err(NetSrvError::ConnectionError(
                "HTTP client not initialized".to_string(),
            ))
        }
    }
}

#[async_trait]
impl NetworkClient for HttpClient {
    async fn connect(&mut self) -> Result<()> {
        let mut client_builder = ClientBuilder::new();

        // 设置超时
        let timeout = Duration::from_millis(self.config.timeout_ms);
        client_builder = client_builder.timeout(timeout);

        // 创建客户端
        let client = client_builder.build()?;
        self.client = Some(client);
        *self.connected.lock().unwrap() = true;

        info!("HTTP client initialized for URL: {}", self.config.url);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.client = None;
        *self.connected.lock().unwrap() = false;
        Ok(())
    }

    async fn send(&self, data: &str) -> Result<()> {
        if !self.is_connected() {
            return Err(NetSrvError::ConnectionError(
                "HTTP client not connected".to_string(),
            ));
        }

        let request = self.build_request(data)?;
        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(NetSrvError::HttpError(format!(
                "HTTP request failed with status {}: {}",
                status, body
            )));
        }

        debug!("HTTP request sent successfully to: {}", self.config.url);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        *self.connected.lock().unwrap()
    }
} 