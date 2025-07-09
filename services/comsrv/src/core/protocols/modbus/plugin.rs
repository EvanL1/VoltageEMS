//! Modbus Protocol Plugin Implementation
//!
//! This module provides plugin implementations for Modbus TCP and RTU protocols,
//! enabling dynamic protocol loading through the plugin system.

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
use crate::core::transport::tcp::TcpTransportConfig;
use crate::core::transport::serial::SerialTransportConfig;

use super::client::{ModbusClient, ModbusChannelConfig};
use super::common::ModbusConfig;
use super::modbus_polling::ModbusPollingConfig;

/// Modbus TCP Protocol Plugin
pub struct ModbusTcpPlugin {
    metadata: ProtocolMetadata,
}

impl ModbusTcpPlugin {
    /// Create Modbus mapping table from combined points
    fn create_modbus_mapping_table(&self, config: &ChannelConfig) -> super::client::ProtocolMappingTable {
        use super::protocol_engine::{
            ModbusTelemetryMapping, ModbusSignalMapping,
            ModbusAdjustmentMapping, ModbusControlMapping
        };
        
        let mut mapping_table = super::client::ProtocolMappingTable::default();
        
        // Convert combined_points to protocol mappings
        for point in &config.combined_points {
            // Extract fields from CombinedPoint
            let point_id = point.point_id;
            let scale = point.scaling.as_ref().map(|s| s.scale).unwrap_or(1.0);
            let offset = point.scaling.as_ref().map(|s| s.offset).unwrap_or(0.0);
            
            // Parse address from protocol_params (format: "slave_id:function_code:register_address")
            let address = match point.protocol_params.get("address") {
                Some(addr) => addr,
                None => {
                    tracing::warn!("No address parameter for point {}", point_id);
                    continue;
                }
            };
                
            let address_parts: Vec<&str> = address.split(':').collect();
            if address_parts.len() < 3 {
                tracing::warn!("Invalid address format for point {}: {}", point_id, address);
                continue;
            }
            
            let slave_id = match address_parts[0].parse::<u8>() {
                Ok(id) => id,
                Err(_) => {
                    tracing::warn!("Invalid slave_id for point {}: {}", point_id, address_parts[0]);
                    continue;
                }
            };
            
            let function_code = match address_parts[1].parse::<u8>() {
                Ok(code) => code,
                Err(_) => {
                    tracing::warn!("Invalid function_code for point {}: {}", point_id, address_parts[1]);
                    continue;
                }
            };
            
            let register_address = match address_parts[2].parse::<u16>() {
                Ok(addr) => addr,
                Err(_) => {
                    tracing::warn!("Invalid register_address for point {}: {}", point_id, address_parts[2]);
                    continue;
                }
            };
            
            let data_type = point.data_type.clone();
            let bit_location = point.protocol_params.get("bit_location")
                .and_then(|v| v.parse::<u8>().ok());
            
            // Determine point type based on function code or telemetry type
            match function_code {
                3 | 4 => {
                    // Read Holding Registers or Input Registers - Telemetry (YC)
                    let mapping = ModbusTelemetryMapping {
                        point_id,
                        slave_id,
                        address: register_address,
                        data_type,
                        scale,
                        offset,
                    };
                    mapping_table.telemetry_mappings.insert(point_id, mapping);
                }
                1 | 2 => {
                    // Read Coils or Discrete Inputs - Signal (YX)
                    let mapping = ModbusSignalMapping {
                        point_id,
                        slave_id,
                        address: register_address,
                        bit_location,
                    };
                    mapping_table.signal_mappings.insert(point_id, mapping);
                }
                6 => {
                    // Write Single Register - Adjustment (YT)
                    let mapping = ModbusAdjustmentMapping {
                        point_id,
                        slave_id,
                        address: register_address,
                        data_type,
                        scale,
                        offset,
                    };
                    mapping_table.adjustment_mappings.insert(point_id, mapping);
                }
                5 => {
                    // Write Single Coil - Control (YK)
                    let mapping = ModbusControlMapping {
                        point_id,
                        slave_id,
                        address: register_address,
                        bit_location,
                        coil_number: Some(register_address),
                    };
                    mapping_table.control_mappings.insert(point_id, mapping);
                }
                _ => {
                    tracing::warn!("Unsupported function code {} for point {}", function_code, point_id);
                }
            }
        }
        
        let total = mapping_table.telemetry_mappings.len() 
            + mapping_table.signal_mappings.len()
            + mapping_table.adjustment_mappings.len()
            + mapping_table.control_mappings.len();
        tracing::info!("Created {} Modbus mappings (YC:{}, YX:{}, YT:{}, YK:{})", 
            total,
            mapping_table.telemetry_mappings.len(),
            mapping_table.signal_mappings.len(),
            mapping_table.adjustment_mappings.len(),
            mapping_table.control_mappings.len()
        );
        
        mapping_table
    }
}

impl Default for ModbusTcpPlugin {
    fn default() -> Self {
        Self {
            metadata: ProtocolMetadata {
                id: "modbus_tcp".to_string(),
                name: "Modbus TCP".to_string(),
                version: "1.0.0".to_string(),
                description: "Modbus TCP protocol implementation".to_string(),
                author: "VoltageEMS Team".to_string(),
                license: "MIT".to_string(),
                features: vec!["telemetry".to_string(), "control".to_string(), "adjustment".to_string(), "signal".to_string()],
                dependencies: HashMap::new(),
            },
        }
    }
}

#[async_trait]
impl ProtocolPlugin for ModbusTcpPlugin {
    fn metadata(&self) -> ProtocolMetadata {
        self.metadata.clone()
    }
    
    fn config_template(&self) -> Vec<ConfigTemplate> {
        vec![
            ConfigTemplate {
                name: "host".to_string(),
                description: "Modbus TCP server host address".to_string(),
                param_type: "string".to_string(),
                required: true,
                default_value: Some(json!("127.0.0.1")),
                validation: Some(ValidationRule {
                    min: None,
                    max: None,
                    pattern: Some(r"^[a-zA-Z0-9\.\-]+$".to_string()),
                    allowed_values: None,
                }),
            },
            ConfigTemplate {
                name: "port".to_string(),
                description: "Modbus TCP server port".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(502)),
                validation: Some(ValidationRule {
                    min: Some(1.0),
                    max: Some(65535.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
            ConfigTemplate {
                name: "timeout_ms".to_string(),
                description: "Connection timeout in milliseconds".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(5000)),
                validation: Some(ValidationRule {
                    min: Some(100.0),
                    max: Some(60000.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
            ConfigTemplate {
                name: "slave_id".to_string(),
                description: "Default Modbus slave ID (unit identifier)".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(1)),
                validation: Some(ValidationRule {
                    min: Some(0.0),
                    max: Some(255.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
        ]
    }
    
    fn validate_config(&self, config: &HashMap<String, Value>) -> Result<()> {
        // Check required parameters
        if !config.contains_key("host") {
            return Err(Error::ConfigError("Missing required parameter: host".to_string()));
        }
        
        // Validate host
        if let Some(host) = config.get("host") {
            if !host.is_string() {
                return Err(Error::ConfigError("Parameter 'host' must be a string".to_string()));
            }
        }
        
        // Validate port
        if let Some(port) = config.get("port") {
            if let Some(port_num) = port.as_u64() {
                if port_num == 0 || port_num > 65535 {
                    return Err(Error::ConfigError("Port must be between 1 and 65535".to_string()));
                }
            } else {
                return Err(Error::ConfigError("Parameter 'port' must be a number".to_string()));
            }
        }
        
        Ok(())
    }
    
    async fn create_instance(
        &self,
        channel_config: ChannelConfig,
    ) -> Result<Box<dyn ComBase>> {
        tracing::info!("ModbusTcpPlugin: Starting to create instance for channel {}", channel_config.name);
        
        // Extract Modbus configuration from channel config
        let params = &channel_config.parameters;
        tracing::debug!("ModbusTcpPlugin: Parameters: {:?}", params);
        
        let host = params.get("host")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::ConfigError("Missing host parameter".to_string()))?
            .to_string();
            
        let port = params.get("port")
            .and_then(|v| v.as_u64())
            .map(|p| p as u16)
            .unwrap_or(502);
            
        let timeout_ms = params.get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(5000);
            
        // Create transport
        let factory = TransportFactory::new();
        let transport_config = TcpTransportConfig {
            host: host.clone(),
            port,
            timeout: std::time::Duration::from_millis(timeout_ms),
            max_retries: 3,
            keep_alive: Some(std::time::Duration::from_secs(60)),
            recv_buffer_size: None,
            send_buffer_size: None,
            no_delay: true,
        };
        
        tracing::info!("ModbusTcpPlugin: Creating TCP transport to {}:{}", host, port);
        let transport = factory.create_tcp_transport(transport_config).await
            .map_err(|e| {
                tracing::error!("ModbusTcpPlugin: Failed to create TCP transport: {}", e);
                e
            })?;
        tracing::info!("ModbusTcpPlugin: TCP transport created successfully");
        
        // Create Modbus configuration
        let modbus_config = ModbusConfig {
            protocol_type: "ModbusTcp".to_string(),
            host: Some(host),
            port: Some(port),
            device_path: None,
            baud_rate: None,
            data_bits: None,
            stop_bits: None,
            parity: None,
            timeout_ms: Some(timeout_ms),
            points: vec![], // Points will be configured later
        };
        
        // Create channel configuration
        let modbus_channel_config = ModbusChannelConfig {
            channel_id: channel_config.id,
            channel_name: channel_config.name.clone(),
            connection: modbus_config,
            request_timeout: std::time::Duration::from_millis(timeout_ms),
            max_retries: 3,
            retry_delay: std::time::Duration::from_millis(1000),
            polling: ModbusPollingConfig {
                default_interval_ms: params.get("polling_interval")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1000), // Default 1 second
                enable_batch_reading: params.get("enable_batch_reading")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
                max_batch_size: params.get("batch_size")
                    .and_then(|v| v.as_u64())
                    .map(|s| s as u16)
                    .unwrap_or(125),
                read_timeout_ms: timeout_ms,
                slave_configs: HashMap::new(),
            },
        };
        
        // Create Modbus client
        tracing::info!("ModbusTcpPlugin: Creating Modbus client for channel {}", modbus_channel_config.channel_name);
        let mut client = ModbusClient::new(modbus_channel_config, transport).await
            .map_err(|e| {
                tracing::error!("ModbusTcpPlugin: Failed to create Modbus client: {}", e);
                e
            })?;
        tracing::info!("ModbusTcpPlugin: Modbus client created successfully");
        
        // Load protocol mappings if combined_points are available
        if !channel_config.combined_points.is_empty() {
            tracing::info!("ModbusTcpPlugin: Loading {} protocol mappings for channel {}", 
                channel_config.combined_points.len(), channel_config.name);
            
            // Create mapping table from combined points
            let mapping_table = self.create_modbus_mapping_table(&channel_config);
            
            if let Err(e) = client.load_protocol_mappings(mapping_table).await {
                tracing::warn!("ModbusTcpPlugin: Failed to load protocol mappings: {}", e);
            } else {
                tracing::info!("ModbusTcpPlugin: Successfully loaded protocol mappings");
            }
        } else {
            tracing::info!("ModbusTcpPlugin: No combined_points to load for channel {}", channel_config.name);
        }
        
        Ok(Box::new(client))
    }
    
    fn cli_commands(&self) -> Vec<CliCommand> {
        vec![
            CliCommand {
                name: "test-connection".to_string(),
                description: "Test connection to Modbus TCP server".to_string(),
                args: vec![
                    CliArgument {
                        name: "host".to_string(),
                        description: "Server host address".to_string(),
                        required: true,
                        default: None,
                    },
                    CliArgument {
                        name: "port".to_string(),
                        description: "Server port".to_string(),
                        required: false,
                        default: Some("502".to_string()),
                    },
                ],
            },
            CliCommand {
                name: "read-register".to_string(),
                description: "Read a holding register".to_string(),
                args: vec![
                    CliArgument {
                        name: "address".to_string(),
                        description: "Register address".to_string(),
                        required: true,
                        default: None,
                    },
                    CliArgument {
                        name: "count".to_string(),
                        description: "Number of registers to read".to_string(),
                        required: false,
                        default: Some("1".to_string()),
                    },
                ],
            },
        ]
    }
    
    fn documentation(&self) -> &str {
        r#"
# Modbus TCP Protocol

The Modbus TCP protocol plugin provides communication with Modbus TCP servers.

## Configuration Example

```yaml
channels:
  - id: 1
    name: "Modbus TCP Device"
    protocol: "modbus_tcp"
    protocol_params:
      host: "192.168.1.100"
      port: 502
      timeout_ms: 5000
      slave_id: 1
```

## Supported Features

- Read holding registers (Function Code 0x03)
- Read input registers (Function Code 0x04)
- Read coils (Function Code 0x01)
- Read discrete inputs (Function Code 0x02)
- Write single coil (Function Code 0x05)
- Write single register (Function Code 0x06)
- Write multiple coils (Function Code 0x0F)
- Write multiple registers (Function Code 0x10)

## Point Configuration

Points are configured in CSV files with the following format:

### Telemetry (YC) - telemetry.csv
```csv
point_id,name,description,unit,data_type,range_min,range_max,scale,offset
1,voltage,Phase A Voltage,V,float32,0,500,1.0,0.0
```

### Signal (YX) - signal.csv
```csv
point_id,name,description,normal_state,alarm_delay,reverse
1,breaker_status,Circuit Breaker Status,0,5,false
```

### Control (YK) - control.csv
```csv
point_id,name,description,control_type,reverse
1,breaker_control,Circuit Breaker Control,toggle,false
```

### Adjustment (YT) - adjustment.csv
```csv
point_id,name,description,unit,min,max,step,data_type
1,power_setpoint,Power Setpoint,kW,0,1000,0.1,float32
```
"#
    }
}

/// Modbus RTU Protocol Plugin
pub struct ModbusRtuPlugin {
    metadata: ProtocolMetadata,
}

impl Default for ModbusRtuPlugin {
    fn default() -> Self {
        Self {
            metadata: ProtocolMetadata {
                id: "modbus_rtu".to_string(),
                name: "Modbus RTU".to_string(),
                version: "1.0.0".to_string(),
                description: "Modbus RTU protocol implementation".to_string(),
                author: "VoltageEMS Team".to_string(),
                license: "MIT".to_string(),
                features: vec!["telemetry".to_string(), "control".to_string(), "adjustment".to_string(), "signal".to_string()],
                dependencies: HashMap::new(),
            },
        }
    }
}

#[async_trait]
impl ProtocolPlugin for ModbusRtuPlugin {
    fn metadata(&self) -> ProtocolMetadata {
        self.metadata.clone()
    }
    
    fn config_template(&self) -> Vec<ConfigTemplate> {
        vec![
            ConfigTemplate {
                name: "device_path".to_string(),
                description: "Serial device path (e.g., /dev/ttyUSB0 or COM1)".to_string(),
                param_type: "string".to_string(),
                required: true,
                default_value: Some(json!("/dev/ttyUSB0")),
                validation: None,
            },
            ConfigTemplate {
                name: "baud_rate".to_string(),
                description: "Serial baud rate".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(9600)),
                validation: Some(ValidationRule {
                    min: None,
                    max: None,
                    pattern: None,
                    allowed_values: Some(vec!["1200".to_string(), "2400".to_string(), "4800".to_string(), 
                                           "9600".to_string(), "19200".to_string(), "38400".to_string(),
                                           "57600".to_string(), "115200".to_string()]),
                }),
            },
            ConfigTemplate {
                name: "data_bits".to_string(),
                description: "Serial data bits".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(8)),
                validation: Some(ValidationRule {
                    min: Some(5.0),
                    max: Some(8.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
            ConfigTemplate {
                name: "stop_bits".to_string(),
                description: "Serial stop bits".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(1)),
                validation: Some(ValidationRule {
                    min: None,
                    max: None,
                    pattern: None,
                    allowed_values: Some(vec!["1".to_string(), "2".to_string()]),
                }),
            },
            ConfigTemplate {
                name: "parity".to_string(),
                description: "Serial parity".to_string(),
                param_type: "string".to_string(),
                required: false,
                default_value: Some(json!("None")),
                validation: Some(ValidationRule {
                    min: None,
                    max: None,
                    pattern: None,
                    allowed_values: Some(vec!["None".to_string(), "Even".to_string(), "Odd".to_string()]),
                }),
            },
            ConfigTemplate {
                name: "timeout_ms".to_string(),
                description: "Serial timeout in milliseconds".to_string(),
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
                name: "slave_id".to_string(),
                description: "Default Modbus slave ID".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(1)),
                validation: Some(ValidationRule {
                    min: Some(1.0),
                    max: Some(247.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
        ]
    }
    
    fn validate_config(&self, config: &HashMap<String, Value>) -> Result<()> {
        // Check required parameters
        if !config.contains_key("device_path") {
            return Err(Error::ConfigError("Missing required parameter: device_path".to_string()));
        }
        
        // Validate device_path
        if let Some(path) = config.get("device_path") {
            if !path.is_string() {
                return Err(Error::ConfigError("Parameter 'device_path' must be a string".to_string()));
            }
        }
        
        // Validate baud_rate
        if let Some(baud) = config.get("baud_rate") {
            if !baud.is_u64() {
                return Err(Error::ConfigError("Parameter 'baud_rate' must be a number".to_string()));
            }
        }
        
        Ok(())
    }
    
    async fn create_instance(
        &self,
        channel_config: ChannelConfig,
    ) -> Result<Box<dyn ComBase>> {
        // Extract Modbus RTU configuration from channel config
        let params = &channel_config.parameters;
        
        let device_path = params.get("device_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::ConfigError("Missing device_path parameter".to_string()))?
            .to_string();
            
        let baud_rate = params.get("baud_rate")
            .and_then(|v| v.as_u64())
            .map(|b| b as u32)
            .unwrap_or(9600);
            
        let data_bits = params.get("data_bits")
            .and_then(|v| v.as_u64())
            .map(|d| d as u8)
            .unwrap_or(8);
            
        let stop_bits = params.get("stop_bits")
            .and_then(|v| v.as_u64())
            .map(|s| s as u8)
            .unwrap_or(1);
            
        let parity = params.get("parity")
            .and_then(|v| v.as_str())
            .unwrap_or("None")
            .to_string();
            
        let timeout_ms = params.get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);
            
        // Create transport
        let factory = TransportFactory::new();
        let transport_config = SerialTransportConfig {
            port: device_path.clone(),
            baud_rate,
            data_bits,
            stop_bits,
            parity: parity.clone(),
            flow_control: "None".to_string(),
            timeout: std::time::Duration::from_millis(timeout_ms),
            max_retries: 3,
            read_timeout: std::time::Duration::from_millis(timeout_ms),
            write_timeout: std::time::Duration::from_millis(timeout_ms),
        };
        
        let transport = factory.create_serial_transport(transport_config).await?;
        
        // Create Modbus configuration
        let modbus_config = ModbusConfig {
            protocol_type: "ModbusRtu".to_string(),
            host: None,
            port: None,
            device_path: Some(device_path),
            baud_rate: Some(baud_rate),
            data_bits: Some(data_bits),
            stop_bits: Some(stop_bits as u8),
            parity: Some(parity),
            timeout_ms: Some(timeout_ms),
            points: vec![], // Points will be configured later
        };
        
        // Create channel configuration
        let rtu_channel_config = ModbusChannelConfig {
            channel_id: channel_config.id,
            channel_name: channel_config.name.clone(),
            connection: modbus_config,
            request_timeout: std::time::Duration::from_millis(timeout_ms),
            max_retries: 3,
            retry_delay: std::time::Duration::from_millis(1000),
            polling: ModbusPollingConfig {
                default_interval_ms: params.get("polling_interval")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1000), // Default 1 second
                enable_batch_reading: params.get("enable_batch_reading")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
                max_batch_size: params.get("batch_size")
                    .and_then(|v| v.as_u64())
                    .map(|s| s as u16)
                    .unwrap_or(125),
                read_timeout_ms: timeout_ms,
                slave_configs: HashMap::new(),
            },
        };
        
        // Create Modbus client
        let mut client = ModbusClient::new(rtu_channel_config, transport).await?;
        
        // Load protocol mappings if combined_points are available
        if !channel_config.combined_points.is_empty() {
            tracing::info!("ModbusRtuPlugin: Loading {} protocol mappings for channel {}", 
                channel_config.combined_points.len(), channel_config.name);
            
            // Create mapping table from combined points
            let mapping_table = ModbusTcpPlugin::create_modbus_mapping_table(&Default::default(), &channel_config);
            
            if let Err(e) = client.load_protocol_mappings(mapping_table).await {
                tracing::warn!("ModbusRtuPlugin: Failed to load protocol mappings: {}", e);
            } else {
                tracing::info!("ModbusRtuPlugin: Successfully loaded protocol mappings");
            }
        }
        
        Ok(Box::new(client))
    }
    
    fn cli_commands(&self) -> Vec<CliCommand> {
        vec![
            CliCommand {
                name: "scan-devices".to_string(),
                description: "Scan for Modbus RTU devices on the bus".to_string(),
                args: vec![
                    CliArgument {
                        name: "start-id".to_string(),
                        description: "Starting slave ID".to_string(),
                        required: false,
                        default: Some("1".to_string()),
                    },
                    CliArgument {
                        name: "end-id".to_string(),
                        description: "Ending slave ID".to_string(),
                        required: false,
                        default: Some("247".to_string()),
                    },
                ],
            },
        ]
    }
    
    fn documentation(&self) -> &str {
        r#"
# Modbus RTU Protocol

The Modbus RTU protocol plugin provides communication with Modbus RTU devices over serial connections.

## Configuration Example

```yaml
channels:
  - id: 2
    name: "Modbus RTU Device"
    protocol: "modbus_rtu"
    protocol_params:
      device_path: "/dev/ttyUSB0"
      baud_rate: 9600
      data_bits: 8
      stop_bits: 1
      parity: "None"
      timeout_ms: 1000
      slave_id: 1
```

## Serial Port Configuration

### Linux
- Use device paths like `/dev/ttyUSB0`, `/dev/ttyS0`
- Ensure user has permissions: `sudo usermod -a -G dialout $USER`

### Windows
- Use device paths like `COM1`, `COM2`
- May require administrator privileges

### macOS
- Use device paths like `/dev/cu.usbserial-*`
- Check available ports: `ls /dev/cu.*`

## Troubleshooting

1. **Permission Denied**: Add user to dialout group (Linux)
2. **Device Not Found**: Check if device is connected with `ls /dev/tty*`
3. **Timeout Errors**: Increase timeout_ms or check baud rate settings
4. **CRC Errors**: Verify parity and stop bits match device settings
"#
    }
}