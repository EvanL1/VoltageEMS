//! `InfluxDB` 2.x 官方client

use crate::error::{Error, Result};
use influxdb2::{models::Query, Client};

/// `InfluxDB` 2.x client
#[derive(Debug)]
pub struct InfluxClient {
    client: Client,
    org: String,
    bucket: String,
}

impl InfluxClient {
    /// Create新的client
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

    /// write线protocoldata
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

    /// Executequery (Fluxquery语言)
    pub async fn query(&self, query: &str) -> Result<String> {
        let bucket = &self.bucket;

        // 如果yessimple的query，converting为Flux格式
        let flux_query = if query.starts_with("from(") {
            query.to_string()
        } else {
            format!(
                r#"from(bucket: "{bucket}")
                |> range(start: -1h)
                |> filter(fn: (r) => r._measurement == "{query}")"#
            )
        };

        // buildingQuerypair象
        let query = Query::new(flux_query);

        let result = self
            .client
            .query_raw(Some(query))
            .await
            .map_err(|e| Error::InfluxDB(format!("Query failed: {e}")))?;

        Ok(format!("{result:?}"))
    }

    /// 健康checking
    pub async fn ping(&self) -> Result<()> {
        // usingInfluxDB 2.x的healthcheckingAPI
        let health_result = self
            .client
            .health()
            .await
            .map_err(|e| Error::InfluxDB(format!("Health check failed: {e}")))?;

        tracing::debug!("InfluxDB health check: {:?}", health_result);

        // extracheckingreadystate
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
