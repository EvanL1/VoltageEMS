//! `InfluxDB` 2.x 官方客户端

use crate::error::{Error, Result};
use influxdb2::{models::Query, Client};

/// `InfluxDB` 2.x 客户端
#[derive(Debug)]
pub struct InfluxClient {
    client: Client,
    org: String,
    bucket: String,
}

impl InfluxClient {
    /// 创建新的客户端
    pub fn new(url: &str, org: &str, bucket: &str, token: &str) -> Result<Self> {
        tracing::debug!(
            "Creating InfluxDB client: url={}, org={}, bucket={}",
            url,
            org,
            bucket
        );
        let client = Client::new(url, org, token);

        Ok(Self {
            client,
            org: org.to_string(),
            bucket: bucket.to_string(),
        })
    }

    /// 写入线协议数据
    pub async fn write_line_protocol(&self, data: &str) -> Result<()> {
        let bucket = &self.bucket;
        let org = &self.org;
        let data_owned = data.to_string();

        tracing::debug!(
            "Writing to InfluxDB: org={}, bucket={}, data_len={}",
            org,
            bucket,
            data.len()
        );

        self.client
            .write_line_protocol(org, bucket, data_owned)
            .await
            .map_err(|e| Error::InfluxDB(format!("Write failed: {e}")))?;

        Ok(())
    }

    /// 执行查询 (Flux查询语言)
    pub async fn query(&self, query: &str) -> Result<String> {
        let bucket = &self.bucket;

        // 如果是简单的查询，转换为Flux格式
        let flux_query = if query.starts_with("from(") {
            query.to_string()
        } else {
            format!(
                r#"from(bucket: "{bucket}")
                |> range(start: -1h)
                |> filter(fn: (r) => r._measurement == "{query}")"#
            )
        };

        // 构建Query对象
        let query = Query::new(flux_query);

        let result = self
            .client
            .query_raw(Some(query))
            .await
            .map_err(|e| Error::InfluxDB(format!("Query failed: {e}")))?;

        Ok(format!("{result:?}"))
    }

    /// 健康检查
    pub async fn ping(&self) -> Result<()> {
        // 使用InfluxDB 2.x的health检查API
        let health_result = self
            .client
            .health()
            .await
            .map_err(|e| Error::InfluxDB(format!("Health check failed: {e}")))?;

        tracing::debug!("InfluxDB health check: {:?}", health_result);

        // 额外检查ready状态
        let ready = self
            .client
            .ready()
            .await
            .map_err(|e| Error::InfluxDB(format!("Ready check failed: {e}")))?;

        if !ready {
            return Err(Error::InfluxDB("InfluxDB is not ready".to_string()));
        }

        tracing::debug!("InfluxDB ready check: {}", ready);
        Ok(())
    }
}
