pub mod config;
pub mod error;
pub mod influxdb_handler;
pub mod redis_handler;

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;
    use chrono::Utc;
    use std::collections::HashMap;
    
    use crate::config::Config;
    use crate::redis_handler::{RedisData, RedisDataPoint};
    use crate::influxdb_handler::InfluxDBHandler;
    
    #[test]
    fn test_config_validation() {
        // Create a simple config for testing
        let config_str = r#"
        redis:
          hostname: "localhost"
          port: 6379
          database: 0
          connection_timeout: 5
          key_pattern: "*"
          polling_interval: 10
        
        influxdb:
          url: "http://localhost:8086"
          org: "voltage"
          token: "token123"
          bucket: "history"
          batch_size: 1000
          flush_interval: 30
        
        logging:
          level: "info"
        
        data_mapping:
          default_measurement: "ems_data"
          tag_mappings:
            - redis_source: "device"
              influx_tag: "device_id"
              extract_from_key: true
          field_mappings:
            - redis_source: "temperature"
              influx_field: "temperature"
              data_type: "float"
              scale_factor: 1.0
        "#;
        
        // Write to a temporary file
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test_config.yaml");
        std::fs::write(&file_path, config_str).unwrap();
        
        // Load and validate config
        let config = Config::from_file(&file_path).unwrap();
        assert!(config.validate().is_ok());
        
        // Verify config values
        assert_eq!(config.redis.hostname, "localhost");
        assert_eq!(config.redis.port, 6379);
        assert_eq!(config.influxdb.url, "http://localhost:8086");
        assert_eq!(config.influxdb.org, "voltage");
        assert_eq!(config.influxdb.token, "token123");
        assert_eq!(config.data_mapping.default_measurement, "ems_data");
    }
    
    #[test]
    fn test_redis_data_point_creation() {
        // Create test data points
        let string_data = RedisData::String("25.5".to_string());
        let int_data = RedisData::Integer(42);
        let float_data = RedisData::Float(98.6);
        let bool_data = RedisData::Boolean(true);
        
        let mut hash_map = HashMap::new();
        hash_map.insert("temp".to_string(), "22.5".to_string());
        hash_map.insert("humidity".to_string(), "65".to_string());
        let hash_data = RedisData::Hash(hash_map);
        
        // Create data points
        let now = Utc::now();
        let dp1 = RedisDataPoint {
            key: "sensor:1:temperature".to_string(),
            data: string_data,
            timestamp: now,
        };
        
        let dp2 = RedisDataPoint {
            key: "sensor:1:count".to_string(),
            data: int_data,
            timestamp: now,
        };
        
        // Verify data points
        match &dp1.data {
            RedisData::String(value) => assert_eq!(value, "25.5"),
            _ => panic!("Wrong data type"),
        }
        
        match &dp2.data {
            RedisData::Integer(value) => assert_eq!(*value, 42),
            _ => panic!("Wrong data type"),
        }
    }
    
    #[test]
    fn test_influxdb_point_creation() {
        // Create a simple config for InfluxDB handler
        let config_str = r#"
        redis:
          hostname: "localhost"
          port: 6379
          database: 0
          connection_timeout: 5
          key_pattern: "*"
          polling_interval: 10
        
        influxdb:
          url: "http://localhost:8086"
          org: "voltage"
          token: "token123"
          bucket: "history"
          batch_size: 1000
          flush_interval: 30
        
        logging:
          level: "info"
        
        data_mapping:
          default_measurement: "ems_data"
          tag_mappings:
            - redis_source: "sensor"
              influx_tag: "sensor_id"
              extract_from_key: true
          field_mappings:
            - redis_source: "temperature"
              influx_field: "temperature"
              data_type: "float"
              scale_factor: 1.0
              measurement: "environmental"
        "#;
        
        // Write to a temporary file
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test_config.yaml");
        std::fs::write(&file_path, config_str).unwrap();
        
        // Load config
        let config = Config::from_file(&file_path).unwrap();
        
        // Create InfluxDB handler
        let mut influxdb_handler = InfluxDBHandler::from_config(&config);
        
        // Create a test data point
        let now = Utc::now();
        let data_point = RedisDataPoint {
            key: "sensor:1:temperature".to_string(),
            data: RedisData::String("25.5".to_string()),
            timestamp: now,
        };
        
        // Process the data point - this should not fail
        // Even though we're not connected to InfluxDB, the point should be
        // added to the buffer without error
        let result = influxdb_handler.process_data_point(&data_point);
        assert!(result.is_ok());
    }
} 