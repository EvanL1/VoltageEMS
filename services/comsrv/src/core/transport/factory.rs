//! Transport Factory
//!
//! This module provides a factory for creating and managing different transport types.
//! It allows protocols to request specific transport implementations without knowing
//! the details of how they are created.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tracing::{debug, info};

#[cfg(all(target_os = "linux", feature = "can"))]
use super::can::{CanTransport, CanTransportBuilder, CanTransportConfig};
use super::gpio::{GpioTransport, GpioTransportBuilder, GpioTransportConfig};
#[cfg(any(test, feature = "test-utils"))]
use super::mock::{MockTransport, MockTransportBuilder, MockTransportConfig};
use super::serial::{SerialTransport, SerialTransportBuilder, SerialTransportConfig};
use super::tcp::{TcpTransport, TcpTransportBuilder, TcpTransportConfig};
use super::traits::{Transport, TransportConfig, TransportError};

/// Supported transport types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportType {
    /// TCP network transport
    Tcp,
    /// Serial port transport
    Serial,
    /// GPIO transport for DI/DO
    Gpio,
    /// CAN bus transport
    #[cfg(all(target_os = "linux", feature = "can"))]
    Can,
    /// Mock transport for testing
    Mock,
}

impl fmt::Display for TransportType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportType::Tcp => write!(f, "tcp"),
            TransportType::Serial => write!(f, "serial"),
            TransportType::Gpio => write!(f, "gpio"),
            #[cfg(all(target_os = "linux", feature = "can"))]
            TransportType::Can => write!(f, "can"),
            TransportType::Mock => write!(f, "mock"),
        }
    }
}

impl std::str::FromStr for TransportType {
    type Err = TransportError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tcp" => Ok(TransportType::Tcp),
            "serial" | "rtu" => Ok(TransportType::Serial),
            "gpio" | "di" | "do" | "digital" => Ok(TransportType::Gpio),
            #[cfg(all(target_os = "linux", feature = "can"))]
            "can" | "canbus" => Ok(TransportType::Can),
            "mock" => Ok(TransportType::Mock),
            _ => Err(TransportError::ConfigError(format!(
                "Unknown transport type: {s}"
            ))),
        }
    }
}

/// Generic transport configuration that can hold any transport config
#[derive(Debug, Clone)]
pub enum AnyTransportConfig {
    /// TCP transport configuration
    Tcp(TcpTransportConfig),
    /// Serial transport configuration
    Serial(SerialTransportConfig),
    /// GPIO transport configuration
    Gpio(GpioTransportConfig),
    /// CAN transport configuration
    #[cfg(all(target_os = "linux", feature = "can"))]
    Can(CanTransportConfig),
    /// Mock transport configuration
    #[cfg(any(test, feature = "test-utils"))]
    Mock(MockTransportConfig),
}

impl AnyTransportConfig {
    /// Get the transport type for this configuration
    pub fn transport_type(&self) -> TransportType {
        match self {
            AnyTransportConfig::Tcp(_) => TransportType::Tcp,
            AnyTransportConfig::Serial(_) => TransportType::Serial,
            AnyTransportConfig::Gpio(_) => TransportType::Gpio,
            #[cfg(all(target_os = "linux", feature = "can"))]
            AnyTransportConfig::Can(_) => TransportType::Can,
            #[cfg(any(test, feature = "test-utils"))]
            AnyTransportConfig::Mock(_) => TransportType::Mock,
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), TransportError> {
        match self {
            AnyTransportConfig::Tcp(config) => config.validate(),
            AnyTransportConfig::Serial(config) => config.validate(),
            AnyTransportConfig::Gpio(config) => config.validate(),
            #[cfg(all(target_os = "linux", feature = "can"))]
            AnyTransportConfig::Can(config) => config.validate(),
            #[cfg(any(test, feature = "test-utils"))]
            AnyTransportConfig::Mock(config) => config.validate(),
        }
    }
}

/// Transport builder registry trait
trait TransportBuilderRegistry: Send + Sync + std::fmt::Debug {
    /// Build a transport instance
    fn build_transport(
        &self,
        config: AnyTransportConfig,
    ) -> Result<Box<dyn Transport>, TransportError>;
}

/// Concrete transport builder implementations
#[derive(Debug)]
#[allow(dead_code)]
struct TcpBuilderImpl {
    builder: TcpTransportBuilder,
}

impl TcpBuilderImpl {
    fn new() -> Self {
        Self {
            builder: TcpTransportBuilder::new(),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct SerialBuilderImpl {
    builder: SerialTransportBuilder,
}

impl SerialBuilderImpl {
    fn new() -> Self {
        Self {
            builder: SerialTransportBuilder::new(),
        }
    }
}

#[cfg(any(test, feature = "test-utils"))]
#[derive(Debug)]
struct MockBuilderImpl {
    builder: mock::MockTransportBuilder,
}

#[cfg(any(test, feature = "test-utils"))]
impl MockBuilderImpl {
    fn new() -> Self {
        Self {
            builder: mock::MockTransportBuilder::new(),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct GpioBuilderImpl {
    builder: GpioTransportBuilder,
}

impl GpioBuilderImpl {
    fn new() -> Self {
        Self {
            builder: GpioTransportBuilder::new(),
        }
    }
}

#[derive(Debug)]
#[cfg(all(target_os = "linux", feature = "can"))]
#[allow(dead_code)]
struct CanBuilderImpl {
    builder: CanTransportBuilder,
}

#[cfg(all(target_os = "linux", feature = "can"))]
impl CanBuilderImpl {
    fn new() -> Self {
        Self {
            builder: CanTransportBuilder::new(),
        }
    }
}

/// Transport factory for creating transport instances
#[derive(Debug)]
pub struct TransportFactory {
    /// Registry of transport builders
    builders: HashMap<TransportType, Box<dyn TransportBuilderRegistry>>,
    /// Statistics about created transports
    stats: Arc<DashMap<TransportType, u64>>,
}

impl Default for TransportFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl TransportFactory {
    /// Create a new transport factory with default builders
    pub fn new() -> Self {
        let mut factory = Self {
            builders: HashMap::new(),
            stats: Arc::new(DashMap::new()),
        };

        // Register default transport builders
        factory.register_tcp_builder();
        factory.register_serial_builder();
        #[cfg(any(test, feature = "test-utils"))]
        factory.register_mock_builder();
        factory.register_gpio_builder();
        #[cfg(all(target_os = "linux", feature = "can"))]
        factory.register_can_builder();

        factory
    }

    /// Register TCP transport builder
    fn register_tcp_builder(&mut self) {
        let builder = TcpBuilderImpl::new();
        self.builders.insert(TransportType::Tcp, Box::new(builder));
        info!("Registered TCP transport builder");
    }

    /// Register serial transport builder
    fn register_serial_builder(&mut self) {
        let builder = SerialBuilderImpl::new();
        self.builders
            .insert(TransportType::Serial, Box::new(builder));
        info!("Registered Serial transport builder");
    }

    /// Register mock transport builder
    #[cfg(any(test, feature = "test-utils"))]
    fn register_mock_builder(&mut self) {
        let builder = MockBuilderImpl::new();
        self.builders.insert(TransportType::Mock, Box::new(builder));
        info!("Registered Mock transport builder");
    }

    /// Register GPIO transport builder
    fn register_gpio_builder(&mut self) {
        let builder = GpioBuilderImpl::new();
        self.builders.insert(TransportType::Gpio, Box::new(builder));
        info!("Registered GPIO transport builder");
    }

    /// Register CAN transport builder
    #[cfg(all(target_os = "linux", feature = "can"))]
    fn register_can_builder(&mut self) {
        let builder = CanBuilderImpl::new();
        self.builders.insert(TransportType::Can, Box::new(builder));
        info!("Registered CAN transport builder");
    }

    /// Create a transport instance from configuration
    pub async fn create_transport(
        &self,
        config: AnyTransportConfig,
    ) -> Result<Box<dyn Transport>, TransportError> {
        let transport_type = config.transport_type();

        debug!("Creating transport of type: {transport_type}");

        // Validate configuration first
        config.validate()?;

        // Get the appropriate builder
        let builder = self.builders.get(&transport_type).ok_or_else(|| {
            TransportError::ConfigError(format!(
                "No builder registered for transport type: {transport_type}"
            ))
        })?;

        // Build the transport
        let _transport = builder.build_transport(config)?;

        // Update statistics
        self.stats
            .entry(transport_type)
            .and_modify(|count| *count += 1)
            .or_insert(1);

        info!("Successfully created {} transport", transport_type);
        Ok(_transport)
    }

    /// Create a TCP transport with given configuration
    pub async fn create_tcp_transport(
        &self,
        config: TcpTransportConfig,
    ) -> Result<Box<dyn Transport>, TransportError> {
        self.create_transport(AnyTransportConfig::Tcp(config)).await
    }

    /// Create a serial transport with given configuration
    pub async fn create_serial_transport(
        &self,
        config: SerialTransportConfig,
    ) -> Result<Box<dyn Transport>, TransportError> {
        self.create_transport(AnyTransportConfig::Serial(config))
            .await
    }

    /// Create a mock transport with given configuration
    #[cfg(any(test, feature = "test-utils"))]
    pub async fn create_mock_transport(
        &self,
        config: MockTransportConfig,
    ) -> Result<Box<dyn Transport>, TransportError> {
        self.create_transport(AnyTransportConfig::Mock(config))
            .await
    }

    /// Get list of supported transport types
    pub fn supported_transport_types(&self) -> Vec<TransportType> {
        self.builders.keys().copied().collect()
    }

    /// Check if a transport type is supported
    pub fn is_transport_supported(&self, transport_type: TransportType) -> bool {
        self.builders.contains_key(&transport_type)
    }

    /// Get creation statistics
    pub fn get_creation_stats(&self) -> HashMap<TransportType, u64> {
        self.stats
            .iter()
            .map(|entry| (*entry.key(), *entry.value()))
            .collect()
    }

    /// Reset creation statistics
    pub fn reset_stats(&self) {
        self.stats.clear();
    }

    /// Get default configuration for a transport type
    pub fn get_default_config(&self, transport_type: TransportType) -> AnyTransportConfig {
        match transport_type {
            TransportType::Tcp => AnyTransportConfig::Tcp(TcpTransportConfig::default()),
            TransportType::Serial => AnyTransportConfig::Serial(SerialTransportConfig::default()),
            TransportType::Gpio => AnyTransportConfig::Gpio(GpioTransportConfig::default()),
            #[cfg(all(target_os = "linux", feature = "can"))]
            TransportType::Can => AnyTransportConfig::Can(CanTransportConfig::default()),
            #[cfg(any(test, feature = "test-utils"))]
            TransportType::Mock => AnyTransportConfig::Mock(mock::MockTransportConfig::default()),
            #[cfg(not(any(test, feature = "test-utils")))]
            TransportType::Mock => {
                panic!("Mock transport is only available in test builds");
            }
        }
    }
}

// Implementations for concrete builder types
impl TransportBuilderRegistry for TcpBuilderImpl {
    fn build_transport(
        &self,
        config: AnyTransportConfig,
    ) -> Result<Box<dyn Transport>, TransportError> {
        match config {
            AnyTransportConfig::Tcp(tcp_config) => {
                let _transport = TcpTransport::new(tcp_config)?;
                Ok(Box::new(_transport))
            }
            _ => Err(TransportError::ConfigError(
                "Invalid config type for TCP transport".to_string(),
            )),
        }
    }
}

impl TransportBuilderRegistry for SerialBuilderImpl {
    fn build_transport(
        &self,
        config: AnyTransportConfig,
    ) -> Result<Box<dyn Transport>, TransportError> {
        match config {
            AnyTransportConfig::Serial(serial_config) => {
                let _transport = SerialTransport::new(serial_config)?;
                Ok(Box::new(_transport))
            }
            _ => Err(TransportError::ConfigError(
                "Invalid config type for Serial transport".to_string(),
            )),
        }
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl TransportBuilderRegistry for MockBuilderImpl {
    fn build_transport(
        &self,
        config: AnyTransportConfig,
    ) -> Result<Box<dyn Transport>, TransportError> {
        match config {
            AnyTransportConfig::Mock(mock_config) => {
                let _transport = mock::MockTransport::new(mock_config)?;
                Ok(Box::new(_transport))
            }
            _ => Err(TransportError::ConfigError(
                "Invalid config type for Mock transport".to_string(),
            )),
        }
    }
}

impl TransportBuilderRegistry for GpioBuilderImpl {
    fn build_transport(
        &self,
        config: AnyTransportConfig,
    ) -> Result<Box<dyn Transport>, TransportError> {
        match config {
            AnyTransportConfig::Gpio(gpio_config) => {
                let _transport = GpioTransport::new(gpio_config)?;
                Ok(Box::new(_transport))
            }
            _ => Err(TransportError::ConfigError(
                "GPIO builder requires GPIO configuration".to_string(),
            )),
        }
    }
}

#[cfg(all(target_os = "linux", feature = "can"))]
impl TransportBuilderRegistry for CanBuilderImpl {
    fn build_transport(
        &self,
        config: AnyTransportConfig,
    ) -> Result<Box<dyn Transport>, TransportError> {
        match config {
            #[cfg(all(target_os = "linux", feature = "can"))]
            AnyTransportConfig::Can(can_config) => {
                let _transport = CanTransport::new(can_config)?;
                Ok(Box::new(_transport))
            }
            _ => Err(TransportError::ConfigError(
                "CAN builder requires CAN configuration".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_type_display() {
        assert_eq!(TransportType::Tcp.to_string(), "tcp");
        assert_eq!(TransportType::Serial.to_string(), "serial");
        assert_eq!(TransportType::Gpio.to_string(), "gpio");
        #[cfg(all(target_os = "linux", feature = "can"))]
        assert_eq!(TransportType::Can.to_string(), "can");
        assert_eq!(TransportType::Mock.to_string(), "mock");
    }

    #[test]
    fn test_transport_type_from_str() {
        assert_eq!("tcp".parse::<TransportType>().unwrap(), TransportType::Tcp);
        assert_eq!("TCP".parse::<TransportType>().unwrap(), TransportType::Tcp);
        assert_eq!(
            "serial".parse::<TransportType>().unwrap(),
            TransportType::Serial
        );
        assert_eq!(
            "rtu".parse::<TransportType>().unwrap(),
            TransportType::Serial
        );
        assert_eq!(
            "gpio".parse::<TransportType>().unwrap(),
            TransportType::Gpio
        );
        assert_eq!("di".parse::<TransportType>().unwrap(), TransportType::Gpio);
        assert_eq!("do".parse::<TransportType>().unwrap(), TransportType::Gpio);
        assert_eq!(
            "digital".parse::<TransportType>().unwrap(),
            TransportType::Gpio
        );
        #[cfg(all(target_os = "linux", feature = "can"))]
        {
            assert_eq!("can".parse::<TransportType>().unwrap(), TransportType::Can);
            assert_eq!(
                "canbus".parse::<TransportType>().unwrap(),
                TransportType::Can
            );
        }
        assert_eq!(
            "mock".parse::<TransportType>().unwrap(),
            TransportType::Mock
        );

        assert!("invalid".parse::<TransportType>().is_err());
    }

    #[test]
    fn test_transport_factory_creation() {
        let factory = TransportFactory::new();

        assert!(factory.is_transport_supported(TransportType::Tcp));
        assert!(factory.is_transport_supported(TransportType::Serial));
        assert!(factory.is_transport_supported(TransportType::Gpio));
        #[cfg(all(target_os = "linux", feature = "can"))]
        assert!(factory.is_transport_supported(TransportType::Can));
        assert!(factory.is_transport_supported(TransportType::Mock));

        let supported_types = factory.supported_transport_types();
        #[cfg(all(target_os = "linux", feature = "can"))]
        assert_eq!(supported_types.len(), 5);
        #[cfg(not(all(target_os = "linux", feature = "can")))]
        assert_eq!(supported_types.len(), 4);
    }

    #[tokio::test]
    async fn test_create_tcp_transport() {
        let factory = TransportFactory::new();
        let config = TcpTransportConfig::default();

        let _transport = factory.create_tcp_transport(config).await;
        assert!(_transport.is_ok());

        let _transport = _transport.unwrap();
        assert_eq!(_transport.transport_type(), "tcp");
    }

    #[tokio::test]
    async fn test_create_serial_transport() {
        let factory = TransportFactory::new();
        let config = SerialTransportConfig::default();

        let _transport = factory.create_serial_transport(config).await;
        assert!(_transport.is_ok());

        let _transport = _transport.unwrap();
        assert_eq!(_transport.transport_type(), "serial");
    }

    #[tokio::test]
    async fn test_create_mock_transport() {
        let factory = TransportFactory::new();
        let config = MockTransportConfig::default();

        let _transport = factory.create_mock_transport(config).await;
        assert!(_transport.is_ok());

        let _transport = _transport.unwrap();
        assert_eq!(_transport.transport_type(), "mock");
    }

    #[tokio::test]
    async fn test_creation_stats() {
        let factory = TransportFactory::new();

        // Create some transports
        let _ = factory
            .create_tcp_transport(TcpTransportConfig::default())
            .await;
        let _ = factory
            .create_mock_transport(MockTransportConfig::default())
            .await;
        let _ = factory
            .create_mock_transport(MockTransportConfig::default())
            .await;

        let stats = factory.get_creation_stats();
        assert_eq!(stats.get(&TransportType::Tcp), Some(&1));
        assert_eq!(stats.get(&TransportType::Mock), Some(&2));
        assert_eq!(stats.get(&TransportType::Serial), None);
        assert_eq!(stats.get(&TransportType::Gpio), None);
        #[cfg(all(target_os = "linux", feature = "can"))]
        assert_eq!(stats.get(&TransportType::Can), None);

        factory.reset_stats();
        let stats = factory.get_creation_stats();
        assert!(stats.is_empty());
    }

    #[test]
    fn test_any_transport_config() {
        let tcp_config = AnyTransportConfig::Tcp(TcpTransportConfig::default());
        assert_eq!(tcp_config.transport_type(), TransportType::Tcp);
        assert!(tcp_config.validate().is_ok());

        let serial_config = AnyTransportConfig::Serial(SerialTransportConfig::default());
        assert_eq!(serial_config.transport_type(), TransportType::Serial);
        assert!(serial_config.validate().is_ok());

        let mut gpio_test_config = GpioTransportConfig::default();
        gpio_test_config
            .pins
            .push(crate::core::transport::gpio::GpioPinConfig {
                pin: 18,
                mode: crate::core::transport::gpio::GpioPinMode::DigitalOutput,
                initial_value: Some(false),
                debounce_ms: None,
                label: Some("Test LED".to_string()),
            });
        let gpio_config = AnyTransportConfig::Gpio(gpio_test_config);
        assert_eq!(gpio_config.transport_type(), TransportType::Gpio);
        assert!(gpio_config.validate().is_ok());

        #[cfg(all(target_os = "linux", feature = "can"))]
        {
            let can_config = AnyTransportConfig::Can(CanTransportConfig::default());
            assert_eq!(can_config.transport_type(), TransportType::Can);
            assert!(can_config.validate().is_ok());
        }

        let mock_config = AnyTransportConfig::Mock(MockTransportConfig::default());
        assert_eq!(mock_config.transport_type(), TransportType::Mock);
        assert!(mock_config.validate().is_ok());
    }

    #[test]
    fn test_default_configurations() {
        let factory = TransportFactory::new();

        let tcp_default = factory.get_default_config(TransportType::Tcp);
        assert!(matches!(tcp_default, AnyTransportConfig::Tcp(_)));

        let serial_default = factory.get_default_config(TransportType::Serial);
        assert!(matches!(serial_default, AnyTransportConfig::Serial(_)));

        let gpio_default = factory.get_default_config(TransportType::Gpio);
        assert!(matches!(gpio_default, AnyTransportConfig::Gpio(_)));

        #[cfg(all(target_os = "linux", feature = "can"))]
        {
            let can_default = factory.get_default_config(TransportType::Can);
            assert!(matches!(can_default, AnyTransportConfig::Can(_)));
        }

        let mock_default = factory.get_default_config(TransportType::Mock);
        assert!(matches!(mock_default, AnyTransportConfig::Mock(_)));
    }
}
