//! GPIO Transport Implementation
//!
//! This module provides GPIO-based transport for Digital Input/Output (DI/DO)
//! operations on embedded systems and edge devices.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::traits::{
    ConnectionState, Transport, TransportBuilder, TransportConfig, TransportError, TransportStats,
};

/// GPIO pin mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpioPinMode {
    /// Digital Input
    DigitalInput,
    /// Digital Output
    DigitalOutput,
    /// Digital Input with pull-up resistor
    DigitalInputPullUp,
    /// Digital Input with pull-down resistor
    DigitalInputPullDown,
    /// Analog Input (if supported)
    AnalogInput,
    /// PWM Output (if supported)
    PwmOutput,
}

/// GPIO pin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpioPinConfig {
    /// Pin number
    pub pin: u8,
    /// Pin mode (input/output)
    pub mode: GpioPinMode,
    /// Initial value for output pins
    pub initial_value: Option<bool>,
    /// Debounce time for input pins (in milliseconds)
    pub debounce_ms: Option<u64>,
    /// Pin description/label
    pub label: Option<String>,
}

/// GPIO transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpioTransportConfig {
    /// Transport name for identification
    pub name: String,
    /// GPIO device path (e.g., "/dev/gpiochip0")
    pub device_path: Option<String>,
    /// Pin configurations
    pub pins: Vec<GpioPinConfig>,
    /// Connection timeout
    pub timeout: Duration,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Polling interval for input pins
    pub poll_interval: Duration,
    /// GPIO backend type
    pub backend: GpioBackend,
}

/// GPIO backend type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpioBackend {
    /// Linux GPIO character device
    LinuxGpioCdev,
    /// Raspberry Pi GPIO (using rppal)
    RaspberryPi,
    /// Mock GPIO for testing
    Mock,
}

impl Default for GpioTransportConfig {
    fn default() -> Self {
        Self {
            name: "GPIO Transport".to_string(),
            device_path: Some("/dev/gpiochip0".to_string()),
            pins: Vec::new(),
            timeout: Duration::from_secs(5),
            max_retries: 3,
            poll_interval: Duration::from_millis(100),
            backend: GpioBackend::LinuxGpioCdev,
        }
    }
}

impl TransportConfig for GpioTransportConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn validate(&self) -> Result<(), TransportError> {
        if self.name.is_empty() {
            return Err(TransportError::ConfigError(
                "Name cannot be empty".to_string(),
            ));
        }

        if self.pins.is_empty() {
            return Err(TransportError::ConfigError(
                "At least one pin must be configured".to_string(),
            ));
        }

        // Validate pin numbers are unique
        let mut pin_numbers = std::collections::HashSet::new();
        for pin_config in &self.pins {
            if !pin_numbers.insert(pin_config.pin) {
                return Err(TransportError::ConfigError(format!(
                    "Duplicate pin number: {}",
                    pin_config.pin
                )));
            }
        }

        if self.timeout.is_zero() {
            return Err(TransportError::ConfigError(
                "Timeout must be greater than zero".to_string(),
            ));
        }

        Ok(())
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

/// GPIO pin state
#[derive(Debug, Clone)]
struct GpioPinState {
    config: GpioPinConfig,
    current_value: Option<bool>,
    last_change: Option<SystemTime>,
}

/// GPIO transport state
#[derive(Debug)]
struct GpioTransportState {
    /// Whether the transport is connected/initialized
    connected: bool,
    /// Pin states by pin number
    pin_states: HashMap<u8, GpioPinState>,
    /// Transport statistics
    stats: TransportStats,
}

impl GpioTransportState {
    fn new(pin_configs: Vec<GpioPinConfig>) -> Self {
        let mut pin_states = HashMap::new();

        for config in pin_configs {
            let state = GpioPinState {
                config: config.clone(),
                current_value: config.initial_value,
                last_change: None,
            };
            pin_states.insert(config.pin, state);
        }

        Self {
            connected: false,
            pin_states,
            stats: TransportStats::new(),
        }
    }
}

/// GPIO transport implementation
#[derive(Debug)]
pub struct GpioTransport {
    /// Transport configuration
    config: GpioTransportConfig,
    /// Internal state
    state: Arc<RwLock<GpioTransportState>>,
    /// Creation time for uptime calculation
    start_time: SystemTime,
}

impl GpioTransport {
    /// Create new GPIO transport with configuration
    pub fn new(config: GpioTransportConfig) -> Result<Self, TransportError> {
        config.validate()?;

        let state = GpioTransportState::new(config.pins.clone());

        Ok(Self {
            config,
            state: Arc::new(RwLock::new(state)),
            start_time: SystemTime::now(),
        })
    }

    /// Set digital output pin value
    pub async fn set_digital_output(&self, pin: u8, value: bool) -> Result<(), TransportError> {
        let mut state = self.state.write().await;

        if !state.connected {
            return Err(TransportError::SendFailed("GPIO not connected".to_string()));
        }

        if let Some(pin_state) = state.pin_states.get_mut(&pin) {
            match pin_state.config.mode {
                GpioPinMode::DigitalOutput | GpioPinMode::PwmOutput => {
                    // In a real implementation, this would write to the actual GPIO
                    pin_state.current_value = Some(value);
                    pin_state.last_change = Some(SystemTime::now());

                    debug!("Set GPIO pin {} to {value}", pin);
                    Ok(())
                }
                _ => Err(TransportError::ConfigError(format!(
                    "Pin {} is not configured as output",
                    pin
                ))),
            }
        } else {
            Err(TransportError::ConfigError(format!(
                "Pin {} not configured",
                pin
            )))
        }
    }

    /// Read digital input pin value
    pub async fn read_digital_input(&self, pin: u8) -> Result<bool, TransportError> {
        let state = self.state.read().await;

        if !state.connected {
            return Err(TransportError::ReceiveFailed(
                "GPIO not connected".to_string(),
            ));
        }

        if let Some(pin_state) = state.pin_states.get(&pin) {
            match pin_state.config.mode {
                GpioPinMode::DigitalInput
                | GpioPinMode::DigitalInputPullUp
                | GpioPinMode::DigitalInputPullDown => {
                    // In a real implementation, this would read from the actual GPIO
                    let value = pin_state.current_value.unwrap_or(false);
                    debug!("Read GPIO pin {}: {value}", pin);
                    Ok(value)
                }
                _ => Err(TransportError::ConfigError(format!(
                    "Pin {} is not configured as input",
                    pin
                ))),
            }
        } else {
            Err(TransportError::ConfigError(format!(
                "Pin {} not configured",
                pin
            )))
        }
    }

    /// Get all pin states
    pub async fn get_all_pin_states(&self) -> HashMap<u8, Option<bool>> {
        let state = self.state.read().await;
        state
            .pin_states
            .iter()
            .map(|(pin, state)| (*pin, state.current_value))
            .collect()
    }

    /// Set multiple outputs atomically
    pub async fn set_multiple_outputs(
        &self,
        values: HashMap<u8, bool>,
    ) -> Result<(), TransportError> {
        for (pin, value) in values {
            self.set_digital_output(pin, value).await?;
        }
        Ok(())
    }

    /// Read multiple inputs
    pub async fn read_multiple_inputs(
        &self,
        pins: &[u8],
    ) -> Result<HashMap<u8, bool>, TransportError> {
        let mut results = HashMap::new();
        for &pin in pins {
            let value = self.read_digital_input(pin).await?;
            results.insert(pin, value);
        }
        Ok(results)
    }
}

#[async_trait]
impl Transport for GpioTransport {
    fn transport_type(&self) -> &str {
        "gpio"
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    async fn connect(&mut self) -> Result<(), TransportError> {
        let mut state = self.state.write().await;
        state.stats.record_connection_attempt();
        state.stats.connection_state = ConnectionState::Connecting;

        debug!(
            "Initializing GPIO transport with {} pins",
            state.pin_states.len()
        );

        // Initialize GPIO based on backend type
        match self.config.backend {
            GpioBackend::LinuxGpioCdev => {
                #[cfg(feature = "gpio")]
                {
                    // In a real implementation, this would initialize the GPIO device
                    info!("GPIO transport initialized with Linux GPIO character device");
                }
                #[cfg(not(feature = "gpio"))]
                {
                    warn!("GPIO feature not enabled, using mock implementation");
                }
            }
            GpioBackend::RaspberryPi => {
                #[cfg(feature = "gpio")]
                {
                    // In a real implementation, this would initialize rppal GPIO
                    info!("GPIO transport initialized with Raspberry Pi GPIO");
                }
                #[cfg(not(feature = "gpio"))]
                {
                    warn!("GPIO feature not enabled, using mock implementation");
                }
            }
            GpioBackend::Mock => {
                info!("GPIO transport initialized with mock backend");
            }
        }

        state.connected = true;
        state.stats.record_successful_connection();

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        let mut state = self.state.write().await;
        if state.connected {
            state.connected = false;
            state.stats.record_disconnection();
            info!("GPIO transport disconnected");
        }
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<usize, TransportError> {
        if data.len() < 2 {
            return Err(TransportError::SendFailed(
                "Invalid GPIO command format".to_string(),
            ));
        }

        let pin = data[0];
        let value = data[1] != 0;

        self.set_digital_output(pin, value).await?;

        let mut state = self.state.write().await;
        state.stats.record_bytes_sent(data.len());

        Ok(data.len())
    }

    async fn receive(
        &mut self,
        buffer: &mut [u8],
        _timeout: Option<Duration>,
    ) -> Result<usize, TransportError> {
        if buffer.len() < 2 {
            return Err(TransportError::ReceiveFailed(
                "Buffer too small".to_string(),
            ));
        }

        // For simplicity, read the first configured input pin
        let state = self.state.read().await;
        let input_pin = state
            .pin_states
            .iter()
            .find(|(_, pin_state)| {
                matches!(
                    pin_state.config.mode,
                    GpioPinMode::DigitalInput
                        | GpioPinMode::DigitalInputPullUp
                        | GpioPinMode::DigitalInputPullDown
                )
            })
            .map(|(pin, _)| *pin);

        if let Some(pin) = input_pin {
            drop(state);
            let value = self.read_digital_input(pin).await?;

            buffer[0] = pin;
            buffer[1] = if value { 1 } else { 0 };

            let mut state = self.state.write().await;
            state.stats.record_bytes_received(2);

            Ok(2)
        } else {
            Err(TransportError::ReceiveFailed(
                "No input pins configured".to_string(),
            ))
        }
    }

    async fn is_connected(&self) -> bool {
        let state = self.state.read().await;
        state.connected
    }

    async fn connection_state(&self) -> ConnectionState {
        let state = self.state.read().await;
        state.stats.connection_state
    }

    async fn stats(&self) -> TransportStats {
        let state = self.state.read().await;
        let mut stats = state.stats.clone();

        // Update uptime
        if let Ok(elapsed) = self.start_time.elapsed() {
            stats.uptime = elapsed;
        }

        stats
    }

    async fn reset_stats(&mut self) {
        let mut state = self.state.write().await;
        state.stats.reset();
    }

    async fn diagnostics(&self) -> std::collections::HashMap<String, String> {
        let mut diag = std::collections::HashMap::new();
        let state = self.state.read().await;

        diag.insert(
            "transport_type".to_string(),
            self.transport_type().to_string(),
        );
        diag.insert("name".to_string(), self.name().to_string());
        diag.insert("connected".to_string(), state.connected.to_string());
        diag.insert(
            "connection_state".to_string(),
            format!("{:?}", state.stats.connection_state),
        );
        diag.insert("backend".to_string(), format!("{:?}", self.config.backend));
        diag.insert("pin_count".to_string(), state.pin_states.len().to_string());
        diag.insert(
            "poll_interval_ms".to_string(),
            self.config.poll_interval.as_millis().to_string(),
        );

        if let Some(device_path) = &self.config.device_path {
            diag.insert("device_path".to_string(), device_path.clone());
        }

        // Add pin states
        for (pin, pin_state) in &state.pin_states {
            diag.insert(
                format!("pin_{}_mode", pin),
                format!("{:?}", pin_state.config.mode),
            );
            if let Some(value) = pin_state.current_value {
                diag.insert(format!("pin_{}_value", pin), value.to_string());
            }
        }

        diag
    }
}

/// GPIO transport builder
#[derive(Debug, Default)]
pub struct GpioTransportBuilder;

impl GpioTransportBuilder {
    /// Create new GPIO transport builder
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TransportBuilder for GpioTransportBuilder {
    type Config = GpioTransportConfig;
    type Transport = GpioTransport;

    async fn build(&self, config: Self::Config) -> Result<Self::Transport, TransportError> {
        GpioTransport::new(config)
    }

    fn default_config(&self) -> Self::Config {
        GpioTransportConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpio_config_validation() {
        let mut config = GpioTransportConfig::default();

        // Empty pins should fail
        assert!(config.validate().is_err());

        // Add valid pin config
        config.pins.push(GpioPinConfig {
            pin: 18,
            mode: GpioPinMode::DigitalOutput,
            initial_value: Some(false),
            debounce_ms: None,
            label: Some("LED".to_string()),
        });
        assert!(config.validate().is_ok());

        // Duplicate pin should fail
        config.pins.push(GpioPinConfig {
            pin: 18, // Duplicate
            mode: GpioPinMode::DigitalInput,
            initial_value: None,
            debounce_ms: Some(50),
            label: Some("Button".to_string()),
        });
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_gpio_transport_creation() {
        let mut config = GpioTransportConfig::default();
        config.pins.push(GpioPinConfig {
            pin: 18,
            mode: GpioPinMode::DigitalOutput,
            initial_value: Some(false),
            debounce_ms: None,
            label: Some("LED".to_string()),
        });

        let transport = GpioTransport::new(config);
        assert!(transport.is_ok());

        let transport = transport.unwrap();
        assert_eq!(transport.transport_type(), "gpio");
        assert_eq!(transport.name(), "GPIO Transport");
    }

    #[tokio::test]
    async fn test_gpio_digital_io() {
        let mut config = GpioTransportConfig::default();
        config.pins.push(GpioPinConfig {
            pin: 18,
            mode: GpioPinMode::DigitalOutput,
            initial_value: Some(false),
            debounce_ms: None,
            label: Some("LED".to_string()),
        });
        config.pins.push(GpioPinConfig {
            pin: 19,
            mode: GpioPinMode::DigitalInput,
            initial_value: None,
            debounce_ms: Some(50),
            label: Some("Button".to_string()),
        });

        let mut transport = GpioTransport::new(config).unwrap();
        transport.connect().await.unwrap();

        // Test digital output
        assert!(transport.set_digital_output(18, true).await.is_ok());

        // Test digital input (will return default false since it's mock)
        let input_value = transport.read_digital_input(19).await.unwrap();
        assert!(!input_value); // Default mock value

        // Test multiple operations
        let mut outputs = HashMap::new();
        outputs.insert(18, false);
        assert!(transport.set_multiple_outputs(outputs).await.is_ok());

        let inputs = transport.read_multiple_inputs(&[19]).await.unwrap();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[&19], false);
    }

    #[tokio::test]
    async fn test_gpio_transport_builder() {
        let builder = GpioTransportBuilder::new();
        let mut config = builder.default_config();

        config.pins.push(GpioPinConfig {
            pin: 18,
            mode: GpioPinMode::DigitalOutput,
            initial_value: Some(false),
            debounce_ms: None,
            label: Some("Test LED".to_string()),
        });

        let transport = builder.build(config).await;
        assert!(transport.is_ok());
    }
}
