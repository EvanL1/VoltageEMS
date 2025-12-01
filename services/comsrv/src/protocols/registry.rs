//! Protocol Registry Module
//!
//! Provides a registry pattern for protocol factories, enabling single-point
//! extension when adding new protocols (OPC-UA, BACnet, etc.)
//!
//! ## Usage
//!
//! Adding a new protocol requires only 2 changes:
//! 1. Implement `ProtocolFactory` for the new protocol
//! 2. Register it in `create_default_registry()`

use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;

use crate::core::channels::traits::ComClient;
use crate::core::config::RuntimeChannelConfig;
use crate::error::{ComSrvError, Result};

// Re-export normalize_protocol_name from utils for consistency
pub use crate::utils::normalize_protocol_name;

// ============================================================================
// Protocol Factory Trait
// ============================================================================

/// Protocol factory trait for creating protocol client instances
///
/// Each protocol implementation should provide a factory that implements this trait.
/// The factory is responsible for:
/// - Declaring supported protocol names (aliases)
/// - Creating configured protocol client instances
#[async_trait]
pub trait ProtocolFactory: Send + Sync {
    /// Returns the list of protocol names this factory handles
    ///
    /// Multiple names can be returned to support aliases (e.g., "modbus_tcp", "modbustcp")
    fn protocol_names(&self) -> &'static [&'static str];

    /// Creates a new protocol client instance from runtime configuration
    ///
    /// # Arguments
    /// * `config` - Runtime channel configuration containing connection parameters
    ///
    /// # Returns
    /// * `Ok(Box<dyn ComClient>)` - Configured protocol client
    /// * `Err(ComSrvError)` - If configuration is invalid or creation fails
    async fn create(&self, config: &RuntimeChannelConfig) -> Result<Box<dyn ComClient>>;
}

// ============================================================================
// Protocol Registry
// ============================================================================

/// Protocol registry for managing protocol factories
///
/// Uses DashMap for thread-safe concurrent access during protocol creation.
pub struct ProtocolRegistry {
    factories: DashMap<String, Arc<dyn ProtocolFactory>>,
}

impl Default for ProtocolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            factories: DashMap::new(),
        }
    }

    /// Register a protocol factory
    ///
    /// All protocol names declared by the factory are registered.
    pub fn register(&self, factory: Arc<dyn ProtocolFactory>) {
        for name in factory.protocol_names() {
            self.factories.insert(name.to_string(), factory.clone());
        }
    }

    /// Create a protocol client by name
    ///
    /// # Arguments
    /// * `protocol_name` - Protocol name (will be normalized to lowercase)
    /// * `config` - Runtime channel configuration
    ///
    /// # Returns
    /// * `Ok(Box<dyn ComClient>)` - Created protocol client
    /// * `Err(ComSrvError::InvalidProtocol)` - If protocol is not registered
    pub async fn create(
        &self,
        protocol_name: &str,
        config: &RuntimeChannelConfig,
    ) -> Result<Box<dyn ComClient>> {
        let normalized = normalize_protocol_name(protocol_name);

        let factory = self
            .factories
            .get(&normalized)
            .ok_or_else(|| ComSrvError::InvalidProtocol(protocol_name.to_string()))?;

        factory.create(config).await
    }

    /// Check if a protocol is registered
    pub fn is_registered(&self, protocol_name: &str) -> bool {
        let normalized = normalize_protocol_name(protocol_name);
        self.factories.contains_key(&normalized)
    }

    /// Get list of registered protocol names
    pub fn registered_protocols(&self) -> Vec<String> {
        self.factories.iter().map(|r| r.key().clone()).collect()
    }
}

// ============================================================================
// Built-in Protocol Factories
// ============================================================================

/// Modbus protocol factory
#[cfg(feature = "modbus")]
pub struct ModbusFactory;

#[cfg(feature = "modbus")]
#[async_trait]
impl ProtocolFactory for ModbusFactory {
    fn protocol_names(&self) -> &'static [&'static str] {
        &["modbus_tcp", "modbus_rtu", "modbustcp", "modbusrtu"]
    }

    async fn create(&self, config: &RuntimeChannelConfig) -> Result<Box<dyn ComClient>> {
        use crate::protocols::modbus::ModbusProtocol;
        Ok(Box::new(ModbusProtocol::from_runtime_config(config)?))
    }
}

/// Virtual protocol factory (always available)
pub struct VirtualFactory;

#[async_trait]
impl ProtocolFactory for VirtualFactory {
    fn protocol_names(&self) -> &'static [&'static str] {
        &["virtual", "virt"]
    }

    async fn create(&self, config: &RuntimeChannelConfig) -> Result<Box<dyn ComClient>> {
        use crate::protocols::virt::VirtualProtocol;
        Ok(Box::new(VirtualProtocol::from_runtime_config(config)?))
    }
}

// ============================================================================
// Registry Initialization
// ============================================================================

/// Create a registry with all default protocols registered
///
/// This is the single point where new protocols should be added.
/// To add a new protocol:
/// 1. Create a new factory struct implementing `ProtocolFactory`
/// 2. Add it to this function with appropriate feature flag
pub fn create_default_registry() -> ProtocolRegistry {
    let registry = ProtocolRegistry::new();

    // Register Modbus protocol (feature-gated)
    #[cfg(feature = "modbus")]
    registry.register(Arc::new(ModbusFactory));

    // Register Virtual protocol (always available)
    registry.register(Arc::new(VirtualFactory));

    // Future protocols:
    // #[cfg(feature = "opcua")]
    // registry.register(Arc::new(OpcUaFactory));
    //
    // #[cfg(feature = "bacnet")]
    // registry.register(Arc::new(BacnetFactory));

    registry
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = create_default_registry();

        // Virtual should always be registered
        assert!(registry.is_registered("virtual"));
        assert!(registry.is_registered("virt"));

        // Check Modbus registration based on feature
        #[cfg(feature = "modbus")]
        {
            assert!(registry.is_registered("modbus_tcp"));
            assert!(registry.is_registered("modbus_rtu"));
        }
    }

    #[test]
    fn test_registered_protocols() {
        let registry = create_default_registry();
        let protocols = registry.registered_protocols();

        assert!(protocols.contains(&"virtual".to_string()));
        assert!(protocols.contains(&"virt".to_string()));
    }
}
