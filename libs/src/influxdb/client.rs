//! InfluxDB HTTP 客户端

use crate::config::InfluxConfig;
use crate::error::{Error, Result};
use reqwest::{Client, StatusCode};
use std::time::Duration;

/// InfluxDB 客户端
pub struct InfluxClient {
    client: Client,
    config: InfluxConfig,
}

impl InfluxClient {
    /// 从配置创建客户端
    pub fn from_config(config: InfluxConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|e| Error::Http(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { client, config })
    }

    /// 写入线协议数据
    pub async fn write_line_protocol(&self, data: &str) -> Result<()> {
        let url = format!("{}/write?db={}", self.config.url, self.config.database);

        let mut request = self.client.post(&url);

        if let (Some(ref username), Some(ref password)) =
            (&self.config.username, &self.config.password)
        {
            request = request.basic_auth(username, Some(password));
        }

        let response = request
            .body(data.to_string())
            .send()
            .await
            .map_err(|e| Error::Http(format!("Write request failed: {}", e)))?;

        match response.status() {
            StatusCode::NO_CONTENT | StatusCode::OK => Ok(()),
            status => {
                let error_text = response.text().await.unwrap_or_else(|_| status.to_string());
                Err(Error::InfluxDB(format!(
                    "Write failed: {} - {}",
                    status, error_text
                )))
            }
        }
    }

    /// 执行查询
    pub async fn query(&self, query: &str) -> Result<String> {
        let url = format!("{}/query?db={}", self.config.url, self.config.database);

        let mut request = self.client.get(&url).query(&[("q", query)]);

        if let (Some(ref username), Some(ref password)) =
            (&self.config.username, &self.config.password)
        {
            request = request.basic_auth(username, Some(password));
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::Http(format!("Query request failed: {}", e)))?;

        match response.status() {
            StatusCode::OK => response
                .text()
                .await
                .map_err(|e| Error::Http(format!("Failed to read response: {}", e))),
            status => {
                let error_text = response.text().await.unwrap_or_else(|_| status.to_string());
                Err(Error::InfluxDB(format!(
                    "Query failed: {} - {}",
                    status, error_text
                )))
            }
        }
    }

    /// 健康检查
    pub async fn ping(&self) -> Result<()> {
        let url = format!("{}/ping", self.config.url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Http(format!("Ping request failed: {}", e)))?;

        match response.status() {
            StatusCode::NO_CONTENT | StatusCode::OK => Ok(()),
            status => Err(Error::InfluxDB(format!("Ping failed: {}", status))),
        }
    }
}
