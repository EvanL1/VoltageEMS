//! Virtual Protocol Plugin Implementation
//!
//! This module provides plugin implementation for virtual protocol,
//! used for testing and simulation purposes.

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::{json, Value};

use crate::core::plugins::protocol_plugin::{
    ProtocolPlugin, ProtocolMetadata, ConfigTemplate, ValidationRule,
    CliCommand, CliArgument,
};
use crate::core::config::types::channel::ChannelConfig;
use crate::core::protocols::common::traits::ComBase;
use crate::utils::{Result, ComSrvError as Error};

use super::VirtualProtocol;

/// Virtual Protocol Plugin
pub struct VirtualPlugin {
    metadata: ProtocolMetadata,
}

impl Default for VirtualPlugin {
    fn default() -> Self {
        Self {
            metadata: ProtocolMetadata {
                id: "virtual".to_string(),
                name: "Virtual Protocol".to_string(),
                version: "1.0.0".to_string(),
                description: "Virtual protocol for testing and simulation".to_string(),
                author: "VoltageEMS Team".to_string(),
                license: "MIT".to_string(),
                features: vec![
                    "telemetry".to_string(), 
                    "control".to_string(), 
                    "signal".to_string(),
                    "adjustment".to_string(),
                    "simulation".to_string(),
                ],
                dependencies: HashMap::new(),
            },
        }
    }
}

#[async_trait]
impl ProtocolPlugin for VirtualPlugin {
    fn metadata(&self) -> ProtocolMetadata {
        self.metadata.clone()
    }
    
    fn config_template(&self) -> Vec<ConfigTemplate> {
        vec![
            ConfigTemplate {
                name: "update_interval".to_string(),
                description: "Data update interval in milliseconds".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(1000)),
                validation: Some(ValidationRule {
                    min: Some(100.0),
                    max: Some(60000.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
            ConfigTemplate {
                name: "simulation_mode".to_string(),
                description: "Simulation mode (random, sine, constant)".to_string(),
                param_type: "string".to_string(),
                required: false,
                default_value: Some(json!("random")),
                validation: Some(ValidationRule {
                    min: None,
                    max: None,
                    pattern: None,
                    allowed_values: Some(vec![
                        "random".to_string(),
                        "sine".to_string(),
                        "constant".to_string(),
                    ]),
                }),
            },
        ]
    }
    
    fn validate_config(&self, _config: &HashMap<String, Value>) -> Result<()> {
        // Virtual protocol accepts any configuration
        Ok(())
    }
    
    async fn create_instance(
        &self,
        channel_config: ChannelConfig,
    ) -> Result<Box<dyn ComBase>> {
        let protocol = VirtualProtocol::new(channel_config)?;
        Ok(Box::new(protocol))
    }
    
    fn cli_commands(&self) -> Vec<CliCommand> {
        vec![
            CliCommand {
                name: "test".to_string(),
                description: "Test virtual protocol".to_string(),
                args: vec![],
            },
        ]
    }
    
    fn documentation(&self) -> &str {
        r#"
# Virtual Protocol

The virtual protocol plugin provides a simulated protocol for testing purposes.

## Configuration Example

```yaml
channels:
  - id: 99
    name: "Virtual Device"
    protocol: "virtual"
    protocol_params:
      update_interval: 1000
      simulation_mode: "sine"
```
"#
    }
}