use crate::config::InfluxDBConfig;
use crate::error::{HisSrvError, Result};
use crate::storage::{DataPoint, DataValue, QueryFilter, QueryResult, Storage, StorageStats};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Utc};
use influxdb::{Client, ReadQuery, Timestamp, WriteQuery};

pub struct InfluxDBStorage {
    client: Option<Client>,
    config: InfluxDBConfig,
    connected: bool,
    last_write_time: Option<DateTime<Utc>>,
    last_read_time: Option<DateTime<Utc>>,
}

impl InfluxDBStorage {
    pub fn new(config: InfluxDBConfig) -> Self {
        Self {
            client: None,
            config,
            connected: false,
            last_write_time: None,
            last_read_time: None,
        }
    }

    async fn create_retention_policy(&self) -> Result<()> {
        if let Some(client) = &self.client {
            let query = format!(
                "CREATE RETENTION POLICY \"{}_retention\" ON \"{}\" DURATION {}d REPLICATION 1 DEFAULT",
                self.config.database, self.config.database, self.config.retention_days
            );

            let read_query = ReadQuery::new(query);
            match client.query(&read_query).await {
                Ok(_) => {
                    tracing::info!(
                        "Created InfluxDB retention policy: {} days",
                        self.config.retention_days
                    );
                    Ok(())
                }
                Err(_) => {
                    // Try to alter existing policy
                    let query = format!(
                        "ALTER RETENTION POLICY \"{}_retention\" ON \"{}\" DURATION {}d REPLICATION 1 DEFAULT",
                        self.config.database, self.config.database, self.config.retention_days
                    );
                    let read_query = ReadQuery::new(query);
                    match client.query(&read_query).await {
                        Ok(_) => {
                            tracing::info!(
                                "Updated InfluxDB retention policy: {} days",
                                self.config.retention_days
                            );
                            Ok(())
                        }
                        Err(e2) => {
                            tracing::warn!("Could not set retention policy: {}", e2);
                            Ok(()) // Not a critical error
                        }
                    }
                }
            }
        } else {
            Err(HisSrvError::ConnectionError(
                "InfluxDB client not initialized".to_string(),
            ))
        }
    }

    fn data_point_to_write_query(&self, data_point: &DataPoint) -> WriteQuery {
        let timestamp =
            Timestamp::Nanoseconds(data_point.timestamp.timestamp_nanos_opt().unwrap_or(0) as u128);
        let mut write_query =
            WriteQuery::new(timestamp, "hissrv_data").add_tag("key", data_point.key.as_str());

        // Add tags
        for (tag_key, tag_value) in &data_point.tags {
            write_query = write_query.add_tag(tag_key.as_str(), tag_value.as_str());
        }

        // Add metadata as tags
        for (meta_key, meta_value) in &data_point.metadata {
            write_query = write_query.add_tag(&format!("meta_{}", meta_key), meta_value.as_str());
        }

        // Add value based on type
        match &data_point.value {
            DataValue::String(s) => {
                write_query = write_query.add_field("text_value", s.as_str());
            }
            DataValue::Integer(i) => {
                write_query = write_query.add_field("int_value", *i);
            }
            DataValue::Float(f) => {
                write_query = write_query.add_field("float_value", *f);
            }
            DataValue::Boolean(b) => {
                write_query = write_query.add_field("bool_value", *b);
            }
            DataValue::Json(j) => {
                write_query = write_query.add_field("json_value", j.to_string());
            }
            DataValue::Binary(b) => {
                write_query = write_query.add_field("binary_size", b.len() as i64);
                // Store base64 encoded binary data
                write_query =
                    write_query.add_field("binary_value", general_purpose::STANDARD.encode(b));
            }
        }

        write_query
    }
}

#[async_trait]
impl Storage for InfluxDBStorage {
    async fn connect(&mut self) -> Result<()> {
        if !self.config.enabled {
            tracing::info!("InfluxDB storage is disabled in configuration");
            return Ok(());
        }

        let client = if !self.config.username.is_empty() && !self.config.password.is_empty() {
            Client::new(self.config.url.clone(), self.config.database.clone())
                .with_auth(self.config.username.clone(), self.config.password.clone())
        } else {
            Client::new(self.config.url.clone(), self.config.database.clone())
        };

        match client.ping().await {
            Ok(_) => {
                tracing::info!("Successfully connected to InfluxDB at {}", self.config.url);
                self.client = Some(client);
                self.connected = true;

                // Create retention policy
                self.create_retention_policy().await?;
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to connect to InfluxDB: {}", e);
                Err(HisSrvError::ConnectionError(format!(
                    "Failed to connect to InfluxDB: {}",
                    e
                )))
            }
        }
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.client = None;
        self.connected = false;
        tracing::info!("Disconnected from InfluxDB");
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        self.connected && self.client.is_some()
    }

    async fn store_data_point(&mut self, data_point: &DataPoint) -> Result<()> {
        if !self.connected || self.client.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to InfluxDB".to_string(),
            ));
        }

        let client = self.client.as_ref().unwrap();
        let write_query = self.data_point_to_write_query(data_point);

        match client.query(&write_query).await {
            Ok(_) => {
                self.last_write_time = Some(Utc::now());
                Ok(())
            }
            Err(e) => Err(HisSrvError::InfluxDBError(e)),
        }
    }

    async fn store_data_points(&mut self, data_points: &[DataPoint]) -> Result<()> {
        if !self.connected || self.client.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to InfluxDB".to_string(),
            ));
        }

        let client = self.client.as_ref().unwrap();
        let mut write_queries: Vec<WriteQuery> = Vec::new();

        for data_point in data_points {
            write_queries.push(self.data_point_to_write_query(data_point));
        }

        // Write in batches
        let batch_size = self.config.batch_size as usize;
        for chunk in write_queries.chunks(batch_size) {
            for query in chunk {
                if let Err(e) = client.query(query).await {
                    tracing::error!("Failed to write batch to InfluxDB: {}", e);
                    return Err(HisSrvError::InfluxDBError(e));
                }
            }
        }

        self.last_write_time = Some(Utc::now());
        Ok(())
    }

    async fn query_data_points(&self, filter: &QueryFilter) -> Result<QueryResult> {
        if !self.connected || self.client.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to InfluxDB".to_string(),
            ));
        }

        let client = self.client.as_ref().unwrap();
        let mut query = format!("SELECT * FROM hissrv_data");
        let mut where_clauses = Vec::new();

        // Add key pattern filter
        if let Some(pattern) = &filter.key_pattern {
            where_clauses.push(format!("key =~ /{}/", pattern.replace("*", ".*")));
        }

        // Add time range filters
        if let Some(start_time) = filter.start_time {
            where_clauses.push(format!("time >= '{}'", start_time.to_rfc3339()));
        }
        if let Some(end_time) = filter.end_time {
            where_clauses.push(format!("time <= '{}'", end_time.to_rfc3339()));
        }

        // Add tag filters
        for (tag_key, tag_value) in &filter.tags {
            where_clauses.push(format!("{} = '{}'", tag_key, tag_value));
        }

        if !where_clauses.is_empty() {
            query.push_str(&format!(" WHERE {}", where_clauses.join(" AND ")));
        }

        // Add ordering
        query.push_str(" ORDER BY time DESC");

        // Add limit and offset
        if let Some(limit) = filter.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }

        let read_query = ReadQuery::new(query);
        match client.query(&read_query).await {
            Ok(_result) => {
                // Parse InfluxDB result to DataPoint objects
                // This is a simplified implementation
                let data_points: Vec<DataPoint> = Vec::new(); // TODO: Parse actual results

                Ok(QueryResult {
                    data_points,
                    total_count: None,
                    has_more: false,
                })
            }
            Err(e) => Err(HisSrvError::InfluxDBError(e)),
        }
    }

    async fn delete_data_points(&mut self, _filter: &QueryFilter) -> Result<u64> {
        // InfluxDB deletion is limited, typically done via retention policies
        // For now, return 0 as deletion count
        Ok(0)
    }

    async fn get_keys(&self, pattern: Option<&str>) -> Result<Vec<String>> {
        if !self.connected || self.client.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to InfluxDB".to_string(),
            ));
        }

        let query = if let Some(p) = pattern {
            format!(
                "SHOW TAG VALUES FROM hissrv_data WITH KEY = \"key\" WHERE key =~ /{}/",
                p.replace("*", ".*")
            )
        } else {
            "SHOW TAG VALUES FROM hissrv_data WITH KEY = \"key\"".to_string()
        };

        let read_query = ReadQuery::new(query);
        match self.client.as_ref().unwrap().query(&read_query).await {
            Ok(_result) => {
                // Parse result to extract keys
                // This is a simplified implementation
                Ok(Vec::new()) // TODO: Parse actual keys
            }
            Err(e) => Err(HisSrvError::InfluxDBError(e)),
        }
    }

    async fn get_statistics(&self) -> Result<StorageStats> {
        Ok(StorageStats {
            total_data_points: 0,  // TODO: Implement actual counting
            storage_size_bytes: 0, // TODO: Get database size
            last_write_time: self.last_write_time,
            last_read_time: self.last_read_time,
            connection_status: if self.connected {
                "connected".to_string()
            } else {
                "disconnected".to_string()
            },
        })
    }

    fn get_name(&self) -> &str {
        "influxdb"
    }

    fn get_config(&self) -> serde_json::Value {
        serde_json::to_value(&self.config).unwrap_or_default()
    }
}

// Add base64 dependency to Cargo.toml if not already present
