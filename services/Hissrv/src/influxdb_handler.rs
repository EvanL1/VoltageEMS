use crate::config::Config;
use crate::error::{HisSrvError, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use voltage_influxdb::{
    v1::InfluxDBv1Storage, DataPoint, FieldValue, TimeSeriesStorage, WriteConfig,
};

pub struct InfluxDBConnection {
    storage: Option<Arc<InfluxDBv1Storage>>,
    connected: bool,
    db_name: String,
}

impl InfluxDBConnection {
    pub fn new() -> Self {
        InfluxDBConnection {
            storage: None,
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
        let storage = if !influx_config.username.is_empty() && !influx_config.password.is_empty() {
            InfluxDBv1Storage::with_auth(
                &influx_config.url,
                &influx_config.database,
                &influx_config.username,
                &influx_config.password,
            )
        } else {
            InfluxDBv1Storage::new(&influx_config.url, &influx_config.database)
        };

        let storage = Arc::new(storage.with_config(WriteConfig {
            batch_size: 5000,
            batch_timeout: std::time::Duration::from_millis(100),
            ..Default::default()
        }));

        // Test connection with health check
        match storage.health_check().await {
            Ok(true) => {
                println!(
                    "Successfully connected to InfluxDB at {}",
                    influx_config.url
                );
                self.storage = Some(storage.clone());
                self.connected = true;
                self.db_name = influx_config.database.clone();

                // Set data retention policy
                let duration = format!("{}d", influx_config.retention_days);
                storage.set_retention_policy(&duration).await?;
                Ok(())
            }
            Ok(false) | Err(_) => {
                println!("Failed to connect to InfluxDB");
                Err(HisSrvError::ConnectionError {
                    message: "Failed to connect to InfluxDB".to_string(),
                    endpoint: influx_config.url.clone(),
                    retry_count: 0,
                })
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    // Retention policy is now handled in connect() method

    pub async fn write_hash_data(
        &self,
        key: &str,
        hash_data: HashMap<String, String>,
        _config: &Config,
    ) -> Result<usize> {
        if !self.connected || self.storage.is_none() {
            return Err(HisSrvError::ConnectionError {
                message: "Not connected to InfluxDB".to_string(),
                endpoint: "unknown".to_string(),
                retry_count: 0,
            });
        }

        let storage = self.storage.as_ref().unwrap();
        let mut points = Vec::new();

        for (field, value) in hash_data {
            let (is_numeric, numeric_value) = try_parse_numeric(&value);

            let mut point = DataPoint::new("rtdb_data");
            point
                .add_tag("key", key)
                .add_tag("field", &field)
                .set_timestamp(Utc::now());

            if is_numeric {
                point.add_field("value", numeric_value);
            } else {
                point.add_field("text_value", value.as_str());
            }

            points.push(point);
        }

        let points_count = points.len();
        match storage.write_points(points).await {
            Ok(_) => Ok(points_count),
            Err(e) => {
                tracing::error!("Failed to write batch for key {}: {}", key, e);
                Err(HisSrvError::WriteError(format!(
                    "Failed to write data: {}",
                    e
                )))
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
        if !self.connected || self.storage.is_none() {
            return Err(HisSrvError::ConnectionError {
                message: "Not connected to InfluxDB".to_string(),
                endpoint: "unknown".to_string(),
                retry_count: 0,
            });
        }

        let storage = self.storage.as_ref().unwrap();

        let mut point = DataPoint::new("rtdb_data");
        point
            .add_tag("key", key)
            .add_tag("type", data_type)
            .set_timestamp(Utc::now());

        if let Some(field) = field_name {
            point.add_tag("field", field);
        }

        if is_numeric {
            point.add_field("value", numeric_value);
        } else if let Some(text) = text_value {
            point.add_field("text_value", text);
        }

        if let Some(s) = score {
            point.add_field("score", s);
        }

        match storage.write_point(point).await {
            Ok(_) => Ok(()),
            Err(e) => Err(HisSrvError::WriteError(format!(
                "Failed to write point: {}",
                e
            ))),
        }
    }
}

pub fn try_parse_numeric(value: &str) -> (bool, f64) {
    match value.parse::<f64>() {
        Ok(num) => (true, num),
        Err(_) => (false, 0.0),
    }
}
