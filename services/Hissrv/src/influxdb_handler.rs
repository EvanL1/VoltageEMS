use crate::config::Config;
use crate::error::{HisSrvError, Result};
use influxdb::{Client, InfluxDbWriteable, WriteQuery, Timestamp};
use chrono::Utc;

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
        if !config.enable_influxdb {
            println!("InfluxDB writing is disabled by configuration.");
            return Ok(());
        }

        let client = if !config.influxdb_user.is_empty() && !config.influxdb_password.is_empty() {
            Client::new(config.influxdb_url.clone(), config.influxdb_db.clone())
                .with_auth(config.influxdb_user.clone(), config.influxdb_password.clone())
        } else {
            Client::new(config.influxdb_url.clone(), config.influxdb_db.clone())
        };

        // Test connection with ping
        match client.ping().await {
            Ok(_) => {
                println!("Successfully connected to InfluxDB at {}", config.influxdb_url);
                self.client = Some(client);
                self.connected = true;
                self.db_name = config.influxdb_db.clone();

                // Set data retention policy
                self.create_retention_policy(config.retention_days).await?;
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