//! CAN Protocol Plugin Implementation
//!
//! This module provides plugin implementation for CAN bus protocol,
//! enabling vehicle and industrial CAN communication through the plugin system.

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
use crate::core::transport::factory::TransportFactory;
use crate::core::transport::can::CanTransportConfig;

use super::client::CanClientBase;
use super::config::CanConfig;

/// CAN Bus Protocol Plugin
pub struct CanPlugin {
    metadata: ProtocolMetadata,
}

impl Default for CanPlugin {
    fn default() -> Self {
        Self {
            metadata: ProtocolMetadata {
                id: "can".to_string(),
                name: "CAN Bus".to_string(),
                version: "1.0.0".to_string(),
                description: "Controller Area Network (CAN) bus protocol implementation".to_string(),
                author: "VoltageEMS Team".to_string(),
                license: "MIT".to_string(),
                features: vec![
                    "telemetry".to_string(), 
                    "control".to_string(), 
                    "diagnostics".to_string(),
                    "broadcast".to_string(),
                ],
                dependencies: HashMap::new(),
            },
        }
    }
}

#[async_trait]
impl ProtocolPlugin for CanPlugin {
    fn metadata(&self) -> ProtocolMetadata {
        self.metadata.clone()
    }
    
    fn config_template(&self) -> Vec<ConfigTemplate> {
        vec![
            // Interface parameters
            ConfigTemplate {
                name: "interface".to_string(),
                description: "CAN interface name (e.g., can0, vcan0)".to_string(),
                param_type: "string".to_string(),
                required: true,
                default_value: Some(json!("can0")),
                validation: Some(ValidationRule {
                    min: None,
                    max: None,
                    pattern: Some(r"^[a-zA-Z0-9]+$".to_string()),
                    allowed_values: None,
                }),
            },
            ConfigTemplate {
                name: "bitrate".to_string(),
                description: "CAN bus bitrate in bits per second".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(500000)),
                validation: Some(ValidationRule {
                    min: None,
                    max: None,
                    pattern: None,
                    allowed_values: Some(vec![
                        "10000".to_string(), "20000".to_string(), "50000".to_string(),
                        "100000".to_string(), "125000".to_string(), "250000".to_string(),
                        "500000".to_string(), "800000".to_string(), "1000000".to_string(),
                    ]),
                }),
            },
            // Frame type configuration
            ConfigTemplate {
                name: "use_extended_id".to_string(),
                description: "Use extended (29-bit) CAN IDs".to_string(),
                param_type: "bool".to_string(),
                required: false,
                default_value: Some(json!(false)),
                validation: None,
            },
            ConfigTemplate {
                name: "use_fd".to_string(),
                description: "Use CAN FD (Flexible Data-rate)".to_string(),
                param_type: "bool".to_string(),
                required: false,
                default_value: Some(json!(false)),
                validation: None,
            },
            // Filter configuration
            ConfigTemplate {
                name: "filters".to_string(),
                description: "CAN ID filters (JSON array of filter objects)".to_string(),
                param_type: "array".to_string(),
                required: false,
                default_value: Some(json!([])),
                validation: None,
            },
            // Error handling
            ConfigTemplate {
                name: "error_mask".to_string(),
                description: "CAN error frame mask".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(0)),
                validation: Some(ValidationRule {
                    min: Some(0.0),
                    max: Some(255.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
            // Timing parameters
            ConfigTemplate {
                name: "timeout_ms".to_string(),
                description: "Read timeout in milliseconds".to_string(),
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
                name: "send_queue_size".to_string(),
                description: "Size of the transmit queue".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(100)),
                validation: Some(ValidationRule {
                    min: Some(1.0),
                    max: Some(1000.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
        ]
    }
    
    fn validate_config(&self, config: &HashMap<String, Value>) -> Result<()> {
        // Check required parameters
        if !config.contains_key("interface") {
            return Err(Error::ConfigError("Missing required parameter: interface".to_string()));
        }
        
        // Validate interface
        if let Some(interface) = config.get("interface") {
            if !interface.is_string() {
                return Err(Error::ConfigError("Parameter 'interface' must be a string".to_string()));
            }
        }
        
        // Validate filters if provided
        if let Some(filters) = config.get("filters") {
            if !filters.is_array() {
                return Err(Error::ConfigError("Parameter 'filters' must be an array".to_string()));
            }
        }
        
        // Validate CAN FD data rate if FD is enabled
        if let Some(use_fd) = config.get("use_fd") {
            if use_fd.as_bool() == Some(true) {
                if let Some(bitrate) = config.get("bitrate") {
                    if let Some(rate) = bitrate.as_u64() {
                        if rate < 500000 {
                            return Err(Error::ConfigError(
                                "CAN FD requires bitrate of at least 500kbps".to_string()
                            ));
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn create_instance(
        &self,
        channel_config: ChannelConfig,
    ) -> Result<Box<dyn ComBase>> {
        // Extract CAN configuration from channel config
        let params = &channel_config.parameters;
        
        let interface = params.get("interface")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::ConfigError("Missing interface parameter".to_string()))?
            .to_string();
            
        let bitrate = params.get("bitrate")
            .and_then(|v| v.as_u64())
            .map(|b| b as u32)
            .unwrap_or(500000);
            
        let use_extended_id = params.get("use_extended_id")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
            
        let use_fd = params.get("use_fd")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
            
        let timeout_ms = params.get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);
            
        // Parse filters
        let filters = if let Some(filters_val) = params.get("filters") {
            if let Some(filters_arr) = filters_val.as_sequence() {
                filters_arr.iter()
                    .filter_map(|f| {
                        if let (Some(id), Some(mask)) = 
                            (f.get("id").and_then(|v| v.as_u64()),
                             f.get("mask").and_then(|v| v.as_u64())) {
                            Some((id as u32, mask as u32))
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
            
        // Create transport
        let factory = TransportFactory::new();
        let transport_config = CanTransportConfig {
            name: format!("can_{}", channel_config.id),
            interface: interface.clone(),
            bit_rate: crate::core::transport::can::CanBitRate::Custom(bitrate),
            can_fd: use_fd,
            timeout: std::time::Duration::from_millis(timeout_ms),
            max_retries: 3,
            recv_buffer_size: 1024,
            send_buffer_size: 1024,
            filters: filters.into_iter().map(|(id, mask)| crate::core::transport::can::CanFilter {
                id,
                mask,
                extended: use_extended_id,
            }).collect(),
        };
        
        let transport = factory.create_transport(
            crate::core::transport::factory::AnyTransportConfig::Can(transport_config)
        ).await?;
        
        // Create CAN configuration
        let can_config = CanConfig {
            interface,
            bitrate,
            use_extended_id,
            use_fd,
            timeout_ms,
            send_queue_size: params.get("send_queue_size")
                .and_then(|v| v.as_u64())
                .map(|s| s as usize)
                .unwrap_or(100),
        };
        
        // Create CAN client
        let client = CanClientBase::new(&channel_config.name, channel_config.clone());
        
        Ok(Box::new(client))
    }
    
    fn cli_commands(&self) -> Vec<CliCommand> {
        vec![
            CliCommand {
                name: "scan-bus".to_string(),
                description: "Scan CAN bus for active nodes".to_string(),
                args: vec![
                    CliArgument {
                        name: "interface".to_string(),
                        description: "CAN interface name".to_string(),
                        required: true,
                        default: None,
                    },
                    CliArgument {
                        name: "duration".to_string(),
                        description: "Scan duration in seconds".to_string(),
                        required: false,
                        default: Some("10".to_string()),
                    },
                ],
            },
            CliCommand {
                name: "send-frame".to_string(),
                description: "Send a CAN frame".to_string(),
                args: vec![
                    CliArgument {
                        name: "id".to_string(),
                        description: "CAN ID (hex)".to_string(),
                        required: true,
                        default: None,
                    },
                    CliArgument {
                        name: "data".to_string(),
                        description: "Frame data (hex bytes)".to_string(),
                        required: true,
                        default: None,
                    },
                ],
            },
            CliCommand {
                name: "monitor".to_string(),
                description: "Monitor CAN bus traffic".to_string(),
                args: vec![
                    CliArgument {
                        name: "interface".to_string(),
                        description: "CAN interface name".to_string(),
                        required: true,
                        default: None,
                    },
                    CliArgument {
                        name: "filter".to_string(),
                        description: "CAN ID filter (hex)".to_string(),
                        required: false,
                        default: None,
                    },
                ],
            },
        ]
    }
    
    fn documentation(&self) -> &str {
        r#"
# CAN Bus Protocol

The CAN bus protocol plugin provides communication with CAN (Controller Area Network) devices,
commonly used in automotive and industrial applications.

## Configuration Example

```yaml
channels:
  - id: 4
    name: "CAN Bus Device"
    protocol: "can"
    protocol_params:
      interface: "can0"
      bitrate: 500000
      use_extended_id: false
      use_fd: false
      timeout_ms: 1000
      filters:
        - id: 0x100
          mask: 0x7FF
        - id: 0x200
          mask: 0x7F0
```

## CAN Interface Setup

### Linux (SocketCAN)

```bash
# Load kernel modules
sudo modprobe can
sudo modprobe can_raw
sudo modprobe vcan  # For virtual CAN

# Create virtual CAN interface (for testing)
sudo ip link add dev vcan0 type vcan
sudo ip link set up vcan0

# Configure physical CAN interface
sudo ip link set can0 type can bitrate 500000
sudo ip link set up can0

# With CAN FD support
sudo ip link set can0 type can bitrate 500000 dbitrate 2000000 fd on
```

### Hardware Support

- Peak PCAN-USB
- Kvaser Leaf
- EMS CPC-USB
- Vector CANcaseXL
- Any SocketCAN compatible device

## Message Format

### Standard Frame (11-bit ID)
```
ID: 0x123
Data: [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
```

### Extended Frame (29-bit ID)
```
ID: 0x12345678
Data: [0x11, 0x22, 0x33, 0x44]
```

### CAN FD Frame
```
ID: 0x100
Data: up to 64 bytes
BRS: Bit Rate Switch enabled
```

## Point Configuration

Points are configured with CAN message IDs and signal definitions:

### Telemetry (YC) - telemetry.csv
```csv
point_id,name,description,unit,data_type,can_id,start_bit,bit_length,byte_order,scale,offset
1,engine_rpm,Engine RPM,rpm,uint16,0x100,0,16,motorola,0.25,0
2,vehicle_speed,Vehicle Speed,km/h,uint8,0x100,16,8,motorola,1.0,0
```

### Signal (YX) - signal.csv
```csv
point_id,name,description,normal_state,can_id,bit_position,reverse
1,engine_running,Engine Running Status,0,0x200,0,false
2,door_open,Door Open Status,0,0x201,3,true
```

## Filters

CAN filters reduce CPU load by hardware filtering:

```yaml
filters:
  - id: 0x100      # Exact match
    mask: 0x7FF
  - id: 0x200      # Range 0x200-0x20F
    mask: 0x7F0
  - id: 0x0        # Accept all
    mask: 0x0
```

## Troubleshooting

1. **No CAN interface**: Check if interface exists with `ip link show`
2. **Permission denied**: Add user to `dialout` group or use sudo
3. **Bus-off state**: Check termination resistors (120Ω on each end)
4. **No messages received**: Verify bitrate matches other nodes
5. **Buffer overruns**: Increase kernel buffer size or add filters
"#
    }
}