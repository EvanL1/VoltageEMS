use crate::config::Config;
use crate::error::{HisSrvError, Result};
use chrono::Utc;
use influxdb::{Client, InfluxDbWriteable, Timestamp, WriteQuery};
use std::collections::HashMap;

pub struct InfluxDBConnection {
    client: Option<Client>,
    connected: bool,
    db_name: String,
}

impl InfluxDBConnection {
    pub fn new() -> Self {
        InfluxDBConnection {
            client: None,
            connected: false,
            db_name: String::new(),
        }
    }

    pub async fn connect(&mut self, config: &Config) -> Result<()> {
        if !config.storage.backends.influxdb.enabled {
            println!("InfluxDB writing is disabled by configuration.");
            return Ok(());
        }

        let influx_config = &config.storage.backends.influxdb;
        let client = if !influx_config.username.is_empty() && !influx_config.password.is_empty() {
            Client::new(influx_config.url.clone(), influx_config.database.clone()).with_auth(
                influx_config.username.clone(),
                influx_config.password.clone(),
            )
        } else {
            Client::new(influx_config.url.clone(), influx_config.database.clone())
        };

        // Test connection with ping
        match client.ping().await {
            Ok(_) => {
                println!(
                    "Successfully connected to InfluxDB at {}",
                    influx_config.url
                );
                self.client = Some(client);
                self.connected = true;
                self.db_name = influx_config.database.clone();

                // Set data retention policy
                self.create_retention_policy(influx_config.retention_days).await?;
                Ok(())
            }
            Err(e) => {
                println!("Failed to connect to InfluxDB: {}", e);
                Err(HisSrvError::ConnectionError(format!(
                    "Failed to connect to InfluxDB: {}",
                    e
                )))
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub async fn create_retention_policy(&self, retention_days: u32) -> Result<()> {
        if !self.connected || self.client.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to InfluxDB".to_string(),
            ));
        }

        let client = self.client.as_ref().unwrap();
        let query = format!(
            "CREATE RETENTION POLICY \"{}_retention\" ON \"{}\" DURATION {}d REPLICATION 1 DEFAULT",
            self.db_name, self.db_name, retention_days
        );

        let read_query = influxdb::ReadQuery::new(query);
        match client.query(&read_query).await {
            Ok(_) => {
                println!("Created retention policy: {} days", retention_days);
                Ok(())
            }
            Err(e) => {
                // If policy already exists, try to update it
                let query = format!(
                    "ALTER RETENTION POLICY \"{}_retention\" ON \"{}\" DURATION {}d REPLICATION 1 DEFAULT",
                    self.db_name, self.db_name, retention_days
                );
                let read_query = influxdb::ReadQuery::new(query);
                match client.query(&read_query).await {
                    Ok(_) => {
                        println!("Updated retention policy: {} days", retention_days);
                        Ok(())
                    }
                    Err(e2) => {
                        println!("Error setting retention policy: {}", e2);
                        Err(HisSrvError::InfluxDBError(e2))
                    }
                }
            }
        }
    }

    pub async fn write_hash_data(
        &self,
        key: &str,
        hash_data: HashMap<String, String>,
        _config: &Config,
    ) -> Result<usize> {
        if !self.connected || self.client.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to InfluxDB".to_string(),
            ));
        }

        let client = self.client.as_ref().unwrap();
        let mut points_written = 0;
        
        for (field, value) in hash_data {
            let (is_numeric, numeric_value) = try_parse_numeric(&value);
            
            // Create WriteQuery using the builder pattern for InfluxDB 0.5.x
            let timestamp = Utc::now();
            let mut write_query = WriteQuery::new(timestamp.into(), "rtdb_data")
                .add_tag("key", key)
                .add_tag("field", &field);

            if is_numeric {
                write_query = write_query.add_field("value", numeric_value);
            } else {
                write_query = write_query.add_field("text_value", value.as_str());
            }

            match client.query(&write_query).await {
                Ok(_) => points_written += 1,
                Err(e) => {
                    // Log error but continue processing other fields
                    tracing::error!("Failed to write field {} for key {}: {}", field, key, e);
                }
            }
        }
        
        Ok(points_written)
    }

    pub async fn write_point(
        &self,
        key: &str,
        data_type: &str,
        field_name: Option<&str>,
        is_numeric: bool,
        numeric_value: f64,
        text_value: Option<&str>,
        score: Option<f64>,
    ) -> Result<()> {
        if !self.connected || self.client.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to InfluxDB".to_string(),
            ));
        }

        let client = self.client.as_ref().unwrap();

        // Create WriteQuery using the builder pattern for InfluxDB 0.5.x
        let timestamp = Utc::now();
        let mut write_query = WriteQuery::new(timestamp.into(), "rtdb_data")
            .add_tag("key", key)
            .add_tag("type", data_type);

        if let Some(field) = field_name {
            write_query = write_query.add_tag("field", field);
        }

        if is_numeric {
            write_query = write_query.add_field("value", numeric_value);
        } else if let Some(text) = text_value {
            write_query = write_query.add_field("text_value", text);
        }

        if let Some(s) = score {
            write_query = write_query.add_field("score", s);
        }

        match client.query(&write_query).await {
            Ok(_) => Ok(()),
            Err(e) => Err(HisSrvError::InfluxDBError(e)),
        }
    }
}

pub fn try_parse_numeric(value: &str) -> (bool, f64) {
    match value.parse::<f64>() {
        Ok(num) => (true, num),
        Err(_) => (false, 0.0),
    }
}
