//! Protocol Configuration Module
//! 
//! This module provides protocol-specific configuration structures and validation
//! for all supported communication protocols. It consolidates configuration
//! management for network, serial, and protocol-specific parameters.

use std::time::Duration;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::utils::error::{ComSrvError, Result};

/// Base communication configuration that all protocols extend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseCommConfig {
    /// Connection timeout duration
    pub timeout: Duration,
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Retry delay duration
    pub retry_delay: Duration,
    /// Keep-alive interval (0 = disabled)
    pub keep_alive_interval: Duration,
    /// Maximum connection age before reconnection
    pub max_connection_age: Option<Duration>,
    /// Enable/disable automatic reconnection
    pub auto_reconnect: bool,
    /// Connection pool settings
    pub pool_config: Option<ConnectionPoolConfig>,
}

impl BaseCommConfig {
    /// Create new base configuration with default values
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
            keep_alive_interval: Duration::from_secs(60),
            max_connection_age: Some(Duration::from_secs(3600)), // 1 hour
            auto_reconnect: true,
            pool_config: None,
        }
    }

    /// Set timeout duration
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set maximum retry attempts
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set retry delay
    pub fn with_retry_delay(mut self, retry_delay: Duration) -> Self {
        self.retry_delay = retry_delay;
        self
    }

    /// Set keep-alive interval
    pub fn with_keep_alive(mut self, keep_alive_interval: Duration) -> Self {
        self.keep_alive_interval = keep_alive_interval;
        self
    }

    /// Enable/disable automatic reconnection
    pub fn with_auto_reconnect(mut self, auto_reconnect: bool) -> Self {
        self.auto_reconnect = auto_reconnect;
        self
    }

    /// Set connection pool configuration
    pub fn with_pool_config(mut self, pool_config: ConnectionPoolConfig) -> Self {
        self.pool_config = Some(pool_config);
        self
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        if self.timeout.is_zero() {
            return Err(ComSrvError::ConfigError("Timeout must be greater than zero".to_string()));
        }
        
        if self.max_retries == 0 {
            return Err(ComSrvError::ConfigError("Max retries must be greater than zero".to_string()));
        }
        
        if self.retry_delay.is_zero() {
            return Err(ComSrvError::ConfigError("Retry delay must be greater than zero".to_string()));
        }
        
        if let Some(ref pool_config) = self.pool_config {
            pool_config.validate()?;
        }
        
        Ok(())
    }
}

impl Default for BaseCommConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Connection pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPoolConfig {
    /// Maximum number of connections in the pool
    pub max_connections: usize,
    /// Minimum number of connections to maintain
    pub min_connections: usize,
    /// Maximum idle time before closing connections
    pub max_idle_time: Duration,
    /// Connection validation interval
    pub validation_interval: Duration,
    /// Enable connection validation on borrow
    pub validate_on_borrow: bool,
}

impl ConnectionPoolConfig {
    /// Create new pool configuration with default values
    pub fn new() -> Self {
        Self {
            max_connections: 10,
            min_connections: 1,
            max_idle_time: Duration::from_secs(300), // 5 minutes
            validation_interval: Duration::from_secs(30),
            validate_on_borrow: true,
        }
    }

    /// Validate pool configuration values
    pub fn validate(&self) -> Result<()> {
        if self.max_connections == 0 {
            return Err(ComSrvError::ConfigError("Max connections must be greater than zero".to_string()));
        }
        
        if self.min_connections > self.max_connections {
            return Err(ComSrvError::ConfigError("Min connections cannot exceed max connections".to_string()));
        }
        
        if self.max_idle_time.is_zero() {
            return Err(ComSrvError::ConfigError("Max idle time must be greater than zero".to_string()));
        }
        
        if self.validation_interval.is_zero() {
            return Err(ComSrvError::ConfigError("Validation interval must be greater than zero".to_string()));
        }
        
        Ok(())
    }
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Network-specific configuration for TCP/IP protocols
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Host address
    pub host: String,
    /// Port number
    pub port: u16,
    /// Socket options
    pub socket_options: HashMap<String, String>,
    /// Enable TCP_NODELAY
    pub no_delay: bool,
    /// Socket receive buffer size
    pub recv_buffer_size: Option<usize>,
    /// Socket send buffer size
    pub send_buffer_size: Option<usize>,
}

impl NetworkConfig {
    /// Create new network configuration
    pub fn new(host: String, port: u16) -> Self {
        Self {
            host,
            port,
            socket_options: HashMap::new(),
            no_delay: true,
            recv_buffer_size: None,
            send_buffer_size: None,
        }
    }

    /// Add socket option
    pub fn with_socket_option<K, V>(mut self, key: K, value: V) -> Self 
    where 
        K: Into<String>,
        V: Into<String>,
    {
        self.socket_options.insert(key.into(), value.into());
        self
    }

    /// Set TCP_NODELAY option
    pub fn with_no_delay(mut self, no_delay: bool) -> Self {
        self.no_delay = no_delay;
        self
    }

    /// Set receive buffer size
    pub fn with_recv_buffer_size(mut self, size: usize) -> Self {
        self.recv_buffer_size = Some(size);
        self
    }

    /// Set send buffer size
    pub fn with_send_buffer_size(mut self, size: usize) -> Self {
        self.send_buffer_size = Some(size);
        self
    }

    /// Validate network configuration
    pub fn validate(&self) -> Result<()> {
        if self.host.is_empty() {
            return Err(ComSrvError::ConfigError("Host cannot be empty".to_string()));
        }
        
        if self.port == 0 {
            return Err(ComSrvError::ConfigError("Port must be greater than zero".to_string()));
        }
        
        Ok(())
    }
}

/// Serial communication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialConfig {
    /// Serial port path
    pub port_path: String,
    /// Baud rate
    pub baud_rate: u32,
    /// Data bits
    pub data_bits: DataBits,
    /// Stop bits
    pub stop_bits: StopBits,
    /// Parity
    pub parity: Parity,
    /// Flow control
    pub flow_control: FlowControl,
}

impl SerialConfig {
    /// Create new serial configuration with common defaults
    pub fn new(port_path: String) -> Self {
        Self {
            port_path,
            baud_rate: 9600,
            data_bits: DataBits::Eight,
            stop_bits: StopBits::One,
            parity: Parity::None,
            flow_control: FlowControl::None,
        }
    }

    /// Set baud rate
    pub fn with_baud_rate(mut self, baud_rate: u32) -> Self {
        self.baud_rate = baud_rate;
        self
    }

    /// Set data bits
    pub fn with_data_bits(mut self, data_bits: DataBits) -> Self {
        self.data_bits = data_bits;
        self
    }

    /// Set stop bits
    pub fn with_stop_bits(mut self, stop_bits: StopBits) -> Self {
        self.stop_bits = stop_bits;
        self
    }

    /// Set parity
    pub fn with_parity(mut self, parity: Parity) -> Self {
        self.parity = parity;
        self
    }

    /// Set flow control
    pub fn with_flow_control(mut self, flow_control: FlowControl) -> Self {
        self.flow_control = flow_control;
        self
    }

    /// Validate serial configuration
    pub fn validate(&self) -> Result<()> {
        if self.port_path.is_empty() {
            return Err(ComSrvError::ConfigError("Port path cannot be empty".to_string()));
        }
        
        // Common baud rates validation
        match self.baud_rate {
            300 | 600 | 1200 | 2400 | 4800 | 9600 | 19200 | 38400 | 57600 | 115200 | 230400 | 460800 | 921600 => {},
            _ => return Err(ComSrvError::ConfigError(format!("Unsupported baud rate: {}", self.baud_rate))),
        }
        
        Ok(())
    }
}

/// Serial data bits configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataBits {
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
}

/// Serial stop bits configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopBits {
    One = 1,
    Two = 2,
}

/// Serial parity configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Parity {
    None,
    Odd,
    Even,
    Mark,
    Space,
}

/// Serial flow control configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowControl {
    None,
    Software,
    Hardware,
}

/// Modbus-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusConfig {
    /// Base communication configuration
    pub base: BaseCommConfig,
    /// Slave/Unit ID
    pub slave_id: u8,
    /// Polling rate in milliseconds
    pub poll_rate: u64,
    /// Point table configuration
    pub point_tables: HashMap<String, String>,
}

impl ModbusConfig {
    /// Create new Modbus configuration
    pub fn new(slave_id: u8) -> Self {
        Self {
            base: BaseCommConfig::new(),
            slave_id,
            poll_rate: 1000, // 1 second default
            point_tables: HashMap::new(),
        }
    }

    /// Set base configuration
    pub fn with_base_config(mut self, base: BaseCommConfig) -> Self {
        self.base = base;
        self
    }

    /// Set polling rate
    pub fn with_poll_rate(mut self, poll_rate: u64) -> Self {
        self.poll_rate = poll_rate;
        self
    }

    /// Add point table
    pub fn with_point_table<K, V>(mut self, key: K, value: V) -> Self 
    where 
        K: Into<String>,
        V: Into<String>,
    {
        self.point_tables.insert(key.into(), value.into());
        self
    }

    /// Validate Modbus configuration
    pub fn validate(&self) -> Result<()> {
        self.base.validate()?;
        
        if self.slave_id == 0 || self.slave_id > 247 {
            return Err(ComSrvError::ConfigError("Slave ID must be between 1 and 247".to_string()));
        }
        
        if self.poll_rate == 0 {
            return Err(ComSrvError::ConfigError("Poll rate must be greater than zero".to_string()));
        }
        
        Ok(())
    }
}

/// Modbus TCP specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusTcpConfig {
    /// Modbus configuration
    pub modbus: ModbusConfig,
    /// Network configuration
    pub network: NetworkConfig,
}

impl ModbusTcpConfig {
    /// Create new Modbus TCP configuration
    pub fn new(host: String, port: u16, slave_id: u8) -> Self {
        Self {
            modbus: ModbusConfig::new(slave_id),
            network: NetworkConfig::new(host, port),
        }
    }

    /// Validate Modbus TCP configuration
    pub fn validate(&self) -> Result<()> {
        self.modbus.validate()?;
        self.network.validate()?;
        Ok(())
    }
}

/// Modbus RTU specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusRtuConfig {
    /// Modbus configuration
    pub modbus: ModbusConfig,
    /// Serial configuration
    pub serial: SerialConfig,
}

impl ModbusRtuConfig {
    /// Create new Modbus RTU configuration
    pub fn new(port_path: String, slave_id: u8) -> Self {
        Self {
            modbus: ModbusConfig::new(slave_id),
            serial: SerialConfig::new(port_path),
        }
    }

    /// Validate Modbus RTU configuration
    pub fn validate(&self) -> Result<()> {
        self.modbus.validate()?;
        self.serial.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_comm_config_creation() {
        let config = BaseCommConfig::new();
        assert_eq!(config.timeout, Duration::from_secs(5));
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay, Duration::from_millis(1000));
        assert!(config.auto_reconnect);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_modbus_tcp_config() {
        let config = ModbusTcpConfig::new("192.168.1.100".to_string(), 502, 1);
        assert_eq!(config.network.host, "192.168.1.100");
        assert_eq!(config.network.port, 502);
        assert_eq!(config.modbus.slave_id, 1);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_modbus_rtu_config() {
        let config = ModbusRtuConfig::new("/dev/ttyUSB0".to_string(), 1);
        assert_eq!(config.serial.port_path, "/dev/ttyUSB0");
        assert_eq!(config.modbus.slave_id, 1);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = ModbusConfig::new(0); // Invalid slave ID
        assert!(config.validate().is_err());
        
        config.slave_id = 1;
        assert!(config.validate().is_ok());
    }
} 