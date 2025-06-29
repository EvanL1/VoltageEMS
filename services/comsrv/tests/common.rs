//! Test Common Utilities
//! 
//! Common utilities and helper functions for comsrv integration tests

use std::collections::HashMap;
use std::fs;

use std::time::Duration;
use tokio::time::sleep;
use tempfile::TempDir;

use comsrv::core::config::ConfigManager;
use comsrv::core::storage::redis_storage::{RedisStore, RealtimeValue};
use comsrv::utils::error::Result;

/// Test configuration builder for creating test scenarios
pub struct TestConfigBuilder {
    channels: Vec<String>,
    redis_enabled: bool,
    redis_url: String,
    temp_dir: TempDir,
}

impl TestConfigBuilder {
    /// Create new test configuration builder
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        Self {
            channels: Vec::new(),
            redis_enabled: false,
            redis_url: "redis://127.0.0.1:6379".to_string(),
            temp_dir,
        }
    }
    
    /// Add Modbus TCP channel for testing
    pub fn add_modbus_tcp_channel(mut self, id: u16, host: &str, port: u16, slave_id: u8) -> Self {
        let channel = format!(r#"
  - id: {}
    name: "ModbusTCP_Test_{}"
    description: "Test Modbus TCP channel"
    protocol: "ModbusTcp"
    parameters:
      host: "{}"
      port: {}
      slave_id: {}
      timeout_ms: 5000
      poll_interval_ms: 1000
    logging:
      enabled: true
      level: "info"
"#, id, id, host, port, slave_id);
        
        self.channels.push(channel);
        self
    }
    
    /// Add Modbus RTU channel for testing
    pub fn add_modbus_rtu_channel(mut self, id: u16, port: &str, baud_rate: u32, slave_id: u8) -> Self {
        let channel = format!(r#"
  - id: {}
    name: "ModbusRTU_Test_{}"
    description: "Test Modbus RTU channel"
    protocol: "ModbusRtu"
    parameters:
      port: "{}"
      baud_rate: {}
      slave_id: {}
      timeout_ms: 5000
      poll_interval_ms: 2000
      data_bits: 8
      stop_bits: 1
      parity: "none"
    logging:
      enabled: true
      level: "info"
"#, id, id, port, baud_rate, slave_id);
        
        self.channels.push(channel);
        self
    }
    
    /// Add IEC60870-5-104 channel for testing
    pub fn add_iec104_channel(mut self, id: u16, host: &str, port: u16) -> Self {
        let channel = format!(r#"
  - id: {}
    name: "IEC104_Test_{}"
    description: "Test IEC104 channel"
    protocol: "Iec104"
    parameters:
      host: "{}"
      port: {}
      timeout_ms: 30000
      k: 12
      w: 8
      t0: 30
      t1: 15
      t2: 10
      t3: 20
    logging:
      enabled: true
      level: "info"
"#, id, id, host, port);
        
        self.channels.push(channel);
        self
    }
    
    /// Add CAN channel for testing
    pub fn add_can_channel(mut self, id: u16, interface: &str) -> Self {
        let channel = format!(r#"
  - id: {}
    name: "CAN_Test_{}"
    description: "Test CAN channel"
    protocol: "Can"
    parameters:
      interface: "{}"
      bitrate: 500000
      sample_point: 0.875
      restart_ms: 100
    logging:
      enabled: true
      level: "info"
"#, id, id, interface);
        
        self.channels.push(channel);
        self
    }
    
    /// Enable Redis for testing
    pub fn with_redis(mut self, url: &str) -> Self {
        self.redis_enabled = true;
        self.redis_url = url.to_string();
        self
    }
    
    /// Build the final configuration
    pub fn build(self) -> ConfigManager {
        let redis_config = if self.redis_enabled {
            format!(r#"
  redis:
    enabled: true
    url: "{}"
    database: 0
    timeout_ms: 5000
    max_retries: 3"#, self.redis_url)
        } else {
            r#"
  redis:
    enabled: false
    url: "redis://127.0.0.1:6379""#.to_string()
        };
        
        let channels_config = if self.channels.is_empty() {
            "channels: []".to_string()
        } else {
            format!("channels:{}", self.channels.join(""))
        };
        
        let config_content = format!(r#"
service:
  name: "ComsrvTest"
  description: "Test service configuration"
  api:
    enabled: true
    bind_address: "127.0.0.1:3000"
    version: "v1"
{redis_config}
  logging:
    level: "info"
    console: true

{channels_config}
"#);
        
        // Write config to temp file
        let config_path = self.temp_dir.path().join("test_config.yaml");
        fs::write(&config_path, config_content).expect("Failed to write test config");
        
        ConfigManager::from_file(&config_path).expect("Failed to load test configuration")
    }
}

/// Test data helper for creating realistic test data
pub struct TestDataHelper;

impl TestDataHelper {
    /// Create test realtime values
    pub fn create_realtime_values() -> Vec<(&'static str, RealtimeValue)> {
        vec![
            ("test:voltage_l1", RealtimeValue {
                raw: 2300.0,
                processed: 230.0,
                timestamp: chrono::Utc::now().to_rfc3339(),
            }),
            ("test:current_l1", RealtimeValue {
                raw: 125.5,
                processed: 125.5,
                timestamp: chrono::Utc::now().to_rfc3339(),
            }),
            ("test:relay_status", RealtimeValue {
                raw: 1.0,
                processed: 1.0,
                timestamp: chrono::Utc::now().to_rfc3339(),
            }),
        ]
    }
    
    /// Generate random test data for stress testing
    pub fn generate_random_values(count: usize) -> Vec<RealtimeValue> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        (0..count).map(|_| RealtimeValue {
            raw: rng.gen_range(0.0..1000.0),
            processed: rng.gen_range(0.0..100.0),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }).collect()
    }
    
    /// Generate Modbus register data for testing
    pub fn generate_modbus_registers(count: usize) -> Vec<(u16, u16)> {
        (0..count).enumerate().map(|(i, _)| {
            let address = 40001 + i as u16;
            let value = (i * 123 + 456) as u16;
            (address, value)
        }).collect()
    }
    
    /// Generate Modbus coil data for testing
    pub fn generate_modbus_coils(count: usize) -> Vec<(u16, bool)> {
        (0..count).enumerate().map(|(i, _)| {
            let address = 1 + i as u16;
            let value = i % 2 == 0;
            (address, value)
        }).collect()
    }
    
    /// Generate Modbus point mappings for testing
    pub fn generate_modbus_point_mappings(channel_id: u16) -> Vec<ModbusPointMapping> {
        vec![
            ModbusPointMapping {
                point_id: format!("modbus_{}:voltage_l1", channel_id),
                address: "40001".to_string(),
                data_type: "uint16".to_string(),
                scale: 0.1,
                offset: 0.0,
                unit: Some("V".to_string()),
            },
            ModbusPointMapping {
                point_id: format!("modbus_{}:current_l1", channel_id),
                address: "40002".to_string(),
                data_type: "uint16".to_string(),
                scale: 0.01,
                offset: 0.0,
                unit: Some("A".to_string()),
            },
            ModbusPointMapping {
                point_id: format!("modbus_{}:frequency", channel_id),
                address: "40003".to_string(),
                data_type: "uint16".to_string(),
                scale: 0.01,
                offset: 0.0,
                unit: Some("Hz".to_string()),
            },
            ModbusPointMapping {
                point_id: format!("modbus_{}:power_active", channel_id),
                address: "40004".to_string(),
                data_type: "uint32".to_string(),
                scale: 1.0,
                offset: 0.0,
                unit: Some("W".to_string()),
            },
            ModbusPointMapping {
                point_id: format!("modbus_{}:relay_1", channel_id),
                address: "1".to_string(),
                data_type: "bool".to_string(),
                scale: 1.0,
                offset: 0.0,
                unit: None,
            },
        ]
    }
}

/// Modbus point mapping structure for testing
#[derive(Debug, Clone)]
pub struct ModbusPointMapping {
    pub point_id: String,
    pub address: String,
    pub data_type: String,
    pub scale: f64,
    pub offset: f64,
    pub unit: Option<String>,
}

/// Test server mock for protocol testing
pub struct MockServer {
    pub protocol: String,
    pub bind_address: String,
    pub port: u16,
}

impl MockServer {
    /// Create new mock server
    pub fn new(protocol: &str, bind_address: &str, port: u16) -> Self {
        Self {
            protocol: protocol.to_string(),
            bind_address: bind_address.to_string(),
            port,
        }
    }
    
    /// Start mock Modbus TCP server
    pub async fn start_modbus_tcp_mock(&self) -> Result<tokio::task::JoinHandle<()>> {
        let bind_addr = format!("{}:{}", self.bind_address, self.port);
        
        let handle = tokio::spawn(async move {
            // Simple mock server that accepts connections
            let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();
            tracing::info!("Mock Modbus TCP server listening on {}", bind_addr);
            
            loop {
                match listener.accept().await {
                    Ok((socket, addr)) => {
                        tracing::debug!("Mock server accepted connection from {}", addr);
                        
                        tokio::spawn(async move {
                            let mut buf = [0u8; 1024];
                            loop {
                                match socket.try_read(&mut buf) {
                                    Ok(0) => break, // Connection closed
                                    Ok(n) => {
                                        tracing::debug!("Mock server received {} bytes", n);
                                        // Echo back mock response
                                        let response = [0x00, 0x01, 0x00, 0x00, 0x00, 0x05, 0x01, 0x03, 0x02, 0x00, 0x64];
                                        let _ = socket.try_write(&response);
                                    }
                                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                        sleep(Duration::from_millis(1)).await;
                                    }
                                    Err(_) => break,
                                }
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("Mock server accept error: {}", e);
                        break;
                    }
                }
            }
        });
        
        // Give the server time to start
        sleep(Duration::from_millis(100)).await;
        Ok(handle)
    }
}

/// Test assertions and verification helpers
pub struct TestAssertions;

impl TestAssertions {
    /// Verify data was stored correctly in Redis
    pub async fn assert_redis_data_stored(store: &RedisStore, key: &str, expected_value: f64) -> Result<()> {
        let value = store.get_realtime_value(key).await?;
        assert!(value.is_some(), "Expected value for key '{}' not found", key);
        
        let realtime_value = value.unwrap();
        assert!((realtime_value.processed - expected_value).abs() < 0.001, 
               "Value mismatch for key '{}': expected {}, got {}", 
               key, expected_value, realtime_value.processed);
        
        Ok(())
    }
    
    /// Verify protocol statistics
    pub fn assert_protocol_stats(stats: &HashMap<String, serde_json::Value>, 
                                 expected_requests: u64) {
        if let Some(total_requests) = stats.get("total_requests") {
            let requests = total_requests.as_u64().unwrap_or(0);
            assert!(requests >= expected_requests, 
                   "Expected at least {} requests, got {}", expected_requests, requests);
        }
    }
    

}

/// Mock service for testing Redis operations
pub struct MockRedisService {
    store: Option<RedisStore>,
}

impl MockRedisService {
    /// Create a new mock service
    pub async fn new(redis_url: Option<&str>) -> Result<Self> {
        let store = if let Some(url) = redis_url {
            let config = comsrv::core::config::config_manager::RedisConfig {
                enabled: true,
                url: url.to_string(),
                database: 1, // Use database 1 for testing
                timeout_ms: 5000,
                max_connections: Some(5),
                max_retries: 3,
            };
            RedisStore::from_config(&config).await?
        } else {
            None
        };
        
        Ok(Self { store })
    }
    
    /// Get the Redis store if available
    pub fn store(&self) -> Option<&RedisStore> {
        self.store.as_ref()
    }
    
    /// Test basic Redis operations
    pub async fn test_basic_operations(&self) -> Result<()> {
        if let Some(store) = &self.store {
            // Test storing and retrieving a value
            let test_value = RealtimeValue {
                raw: 100.0,
                processed: 10.0,
                timestamp: chrono::Utc::now().to_rfc3339(),
            };
            
            store.set_realtime_value("test:basic", &test_value).await?;
            let retrieved = store.get_realtime_value("test:basic").await?;
            
            assert!(retrieved.is_some(), "Failed to retrieve stored value");
            let retrieved_value = retrieved.unwrap();
            assert_eq!(retrieved_value.processed, test_value.processed);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_builder_modbus_tcp() {
        let config = TestConfigBuilder::new()
            .add_modbus_tcp_channel(1, "127.0.0.1", 502, 1)
            .build();
        
        let channels = config.get_channels();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].protocol, "ModbusTcp");
    }
    
    #[test]
    fn test_config_builder_multiple_protocols() {
        let config = TestConfigBuilder::new()
            .add_modbus_tcp_channel(1, "127.0.0.1", 502, 1)
            .add_modbus_rtu_channel(2, "/dev/ttyUSB0", 9600, 2)
            .add_iec104_channel(3, "192.168.1.100", 2404)
            .add_can_channel(4, "can0")
            .build();
        
        let channels = config.get_channels();
        assert_eq!(channels.len(), 4);
    }
    
    #[test]
    fn test_data_helper_creation() {
        let values = TestDataHelper::create_realtime_values();
        assert!(!values.is_empty());
        
        let random_values = TestDataHelper::generate_random_values(10);
        assert_eq!(random_values.len(), 10);
    }
    
    #[tokio::test]
    async fn test_mock_redis_service() {
        // Test without Redis
        let mock_service = MockRedisService::new(None).await.unwrap();
        assert!(mock_service.store().is_none());
        
        // Test with Redis (will fail if Redis is not running, but that's expected)
        match MockRedisService::new(Some("redis://127.0.0.1:6379/1")).await {
            Ok(service) => {
                if service.store().is_some() {
                    let _ = service.test_basic_operations().await;
                }
            }
            Err(_) => {
                // Redis not available, which is fine for unit tests
            }
        }
    }
} 