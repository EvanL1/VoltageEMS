//! IEC 60870-5-104 Protocol Plugin Implementation
//!
//! This module provides plugin implementation for IEC 60870-5-104 protocol,
//! enabling SCADA communication through the plugin system.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::core::config::types::channel::ChannelConfig;
use crate::core::plugins::protocol_plugin::{
    CliArgument, CliCommand, ConfigTemplate, ProtocolMetadata, ProtocolPlugin, ValidationRule,
};
use crate::core::protocols::common::traits::ComBase;
use crate::core::transport::factory::TransportFactory;
use crate::core::transport::tcp::TcpTransportConfig;
use crate::utils::{ComSrvError as Error, Result};

use super::config::Iec104Config;
use super::iec104::Iec104Client;

/// IEC 60870-5-104 Protocol Plugin
pub struct Iec104Plugin {
    metadata: ProtocolMetadata,
}

impl Default for Iec104Plugin {
    fn default() -> Self {
        Self {
            metadata: ProtocolMetadata {
                id: "iec104".to_string(),
                name: "IEC 60870-5-104".to_string(),
                version: "1.0.0".to_string(),
                description: "IEC 60870-5-104 protocol for SCADA systems".to_string(),
                author: "VoltageEMS Team".to_string(),
                license: "MIT".to_string(),
                features: vec![
                    "telemetry".to_string(),
                    "control".to_string(),
                    "signal".to_string(),
                    "time_sync".to_string(),
                    "file_transfer".to_string(),
                ],
                dependencies: HashMap::new(),
            },
        }
    }
}

#[async_trait]
impl ProtocolPlugin for Iec104Plugin {
    fn metadata(&self) -> ProtocolMetadata {
        self.metadata.clone()
    }

    fn config_template(&self) -> Vec<ConfigTemplate> {
        vec![
            // Connection parameters
            ConfigTemplate {
                name: "host".to_string(),
                description: "IEC 104 server host address".to_string(),
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
                description: "IEC 104 server port".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(2404)),
                validation: Some(ValidationRule {
                    min: Some(1.0),
                    max: Some(65535.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
            // IEC 104 specific parameters
            ConfigTemplate {
                name: "common_addr".to_string(),
                description: "Common Address of ASDU (station address)".to_string(),
                param_type: "int".to_string(),
                required: true,
                default_value: Some(json!(1)),
                validation: Some(ValidationRule {
                    min: Some(0.0),
                    max: Some(65535.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
            ConfigTemplate {
                name: "cot_size".to_string(),
                description: "Cause of Transmission field size (1 or 2 bytes)".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(2)),
                validation: Some(ValidationRule {
                    min: None,
                    max: None,
                    pattern: None,
                    allowed_values: Some(vec!["1".to_string(), "2".to_string()]),
                }),
            },
            ConfigTemplate {
                name: "coa_size".to_string(),
                description: "Common Address field size (1 or 2 bytes)".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(2)),
                validation: Some(ValidationRule {
                    min: None,
                    max: None,
                    pattern: None,
                    allowed_values: Some(vec!["1".to_string(), "2".to_string()]),
                }),
            },
            ConfigTemplate {
                name: "ioa_size".to_string(),
                description: "Information Object Address field size (1, 2 or 3 bytes)".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(3)),
                validation: Some(ValidationRule {
                    min: None,
                    max: None,
                    pattern: None,
                    allowed_values: Some(vec!["1".to_string(), "2".to_string(), "3".to_string()]),
                }),
            },
            // Timing parameters
            ConfigTemplate {
                name: "t0_timeout".to_string(),
                description: "Connection establishment timeout (ms)".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(30000)),
                validation: Some(ValidationRule {
                    min: Some(1000.0),
                    max: Some(255000.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
            ConfigTemplate {
                name: "t1_timeout".to_string(),
                description: "Send or test APDU timeout (ms)".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(15000)),
                validation: Some(ValidationRule {
                    min: Some(1000.0),
                    max: Some(255000.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
            ConfigTemplate {
                name: "t2_timeout".to_string(),
                description: "Acknowledgement timeout when no data (ms)".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(10000)),
                validation: Some(ValidationRule {
                    min: Some(1000.0),
                    max: Some(255000.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
            ConfigTemplate {
                name: "t3_timeout".to_string(),
                description: "Test frame timeout (ms)".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(20000)),
                validation: Some(ValidationRule {
                    min: Some(1000.0),
                    max: Some(172800000.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
            // APDU parameters
            ConfigTemplate {
                name: "k_value".to_string(),
                description: "Maximum number of outstanding I format APDUs".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(12)),
                validation: Some(ValidationRule {
                    min: Some(1.0),
                    max: Some(32767.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
            ConfigTemplate {
                name: "w_value".to_string(),
                description: "Latest acknowledgement after receiving w I format APDUs".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(8)),
                validation: Some(ValidationRule {
                    min: Some(1.0),
                    max: Some(32767.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
        ]
    }

    fn validate_config(&self, config: &HashMap<String, Value>) -> Result<()> {
        // Check required parameters
        if !config.contains_key("host") {
            return Err(Error::ConfigError(
                "Missing required parameter: host".to_string(),
            ));
        }

        if !config.contains_key("common_addr") {
            return Err(Error::ConfigError(
                "Missing required parameter: common_addr".to_string(),
            ));
        }

        // Validate host
        if let Some(host) = config.get("host") {
            if !host.is_string() {
                return Err(Error::ConfigError(
                    "Parameter 'host' must be a string".to_string(),
                ));
            }
        }

        // Validate timing parameters relationship
        if let (Some(t1), Some(t2)) = (config.get("t1_timeout"), config.get("t2_timeout")) {
            if let (Some(t1_val), Some(t2_val)) = (t1.as_u64(), t2.as_u64()) {
                if t2_val >= t1_val {
                    return Err(Error::ConfigError(
                        "t2_timeout must be less than t1_timeout".to_string(),
                    ));
                }
            }
        }

        // Validate k and w relationship
        if let (Some(k), Some(w)) = (config.get("k_value"), config.get("w_value")) {
            if let (Some(k_val), Some(w_val)) = (k.as_u64(), w.as_u64()) {
                if w_val > k_val {
                    return Err(Error::ConfigError(
                        "w_value must be less than or equal to k_value".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    async fn create_instance(&self, channel_config: ChannelConfig) -> Result<Box<dyn ComBase>> {
        // Extract IEC 104 configuration from channel config
        let params = &channel_config.parameters;

        let host = params
            .get("host")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::ConfigError("Missing host parameter".to_string()))?
            .to_string();

        let port = params
            .get("port")
            .and_then(|v| v.as_u64())
            .map(|p| p as u16)
            .unwrap_or(2404);

        let common_addr = params
            .get("common_addr")
            .and_then(|v| v.as_u64())
            .map(|a| a as u16)
            .ok_or_else(|| Error::ConfigError("Missing common_addr parameter".to_string()))?;

        // Create transport
        let factory = TransportFactory::new();
        let transport_config = TcpTransportConfig {
            host: host.clone(),
            port,
            timeout: std::time::Duration::from_millis(
                params
                    .get("t0_timeout")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(30000),
            ),
            max_retries: 3,
            keep_alive: Some(std::time::Duration::from_secs(60)),
            recv_buffer_size: None,
            send_buffer_size: None,
            no_delay: true,
        };

        let _transport = factory.create_tcp_transport(transport_config).await?;

        // Create IEC 104 configuration
        let _iec104_config = Iec104Config {
            host,
            port,
            common_addr,
            cot_size: params
                .get("cot_size")
                .and_then(|v| v.as_u64())
                .map(|s| s as u8)
                .unwrap_or(2),
            coa_size: params
                .get("coa_size")
                .and_then(|v| v.as_u64())
                .map(|s| s as u8)
                .unwrap_or(2),
            ioa_size: params
                .get("ioa_size")
                .and_then(|v| v.as_u64())
                .map(|s| s as u8)
                .unwrap_or(3),
            t0_timeout: params
                .get("t0_timeout")
                .and_then(|v| v.as_u64())
                .unwrap_or(30000),
            t1_timeout: params
                .get("t1_timeout")
                .and_then(|v| v.as_u64())
                .unwrap_or(15000),
            t2_timeout: params
                .get("t2_timeout")
                .and_then(|v| v.as_u64())
                .unwrap_or(10000),
            t3_timeout: params
                .get("t3_timeout")
                .and_then(|v| v.as_u64())
                .unwrap_or(20000),
            k_value: params
                .get("k_value")
                .and_then(|v| v.as_u64())
                .map(|k| k as u16)
                .unwrap_or(12),
            w_value: params
                .get("w_value")
                .and_then(|v| v.as_u64())
                .map(|w| w as u16)
                .unwrap_or(8),
        };

        // Create IEC 104 client
        let client = Iec104Client::new(channel_config);

        Ok(Box::new(client))
    }

    fn cli_commands(&self) -> Vec<CliCommand> {
        vec![
            CliCommand {
                name: "interrogation".to_string(),
                description: "Send general interrogation command".to_string(),
                args: vec![
                    CliArgument {
                        name: "host".to_string(),
                        description: "Server host address".to_string(),
                        required: true,
                        default: None,
                    },
                    CliArgument {
                        name: "common-addr".to_string(),
                        description: "Common address of ASDU".to_string(),
                        required: false,
                        default: Some("1".to_string()),
                    },
                ],
            },
            CliCommand {
                name: "time-sync".to_string(),
                description: "Send time synchronization command".to_string(),
                args: vec![],
            },
            CliCommand {
                name: "test-link".to_string(),
                description: "Test IEC 104 connection".to_string(),
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
                        default: Some("2404".to_string()),
                    },
                ],
            },
        ]
    }

    fn documentation(&self) -> &str {
        r#"
# IEC 60870-5-104 Protocol

The IEC 60870-5-104 protocol plugin provides communication with IEC 104 servers,
commonly used in electrical power systems and SCADA applications.

## Configuration Example

```yaml
channels:
  - id: 3
    name: "IEC 104 RTU"
    protocol: "iec104"
    protocol_params:
      host: "192.168.1.50"
      port: 2404
      common_addr: 1
      # APDU structure
      cot_size: 2      # Cause of transmission size
      coa_size: 2      # Common address size
      ioa_size: 3      # Information object address size
      # Timing parameters
      t0_timeout: 30000  # Connection timeout
      t1_timeout: 15000  # Send/test timeout
      t2_timeout: 10000  # Ack timeout
      t3_timeout: 20000  # Test frame timeout
      # Flow control
      k_value: 12        # Max outstanding I-frames
      w_value: 8         # Ack after w I-frames
```

## Information Objects

### Monitoring Direction (M_xx_xx)
- M_SP_NA_1 (1): Single-point information
- M_DP_NA_1 (3): Double-point information
- M_ME_NA_1 (9): Measured value, normalized
- M_ME_NB_1 (11): Measured value, scaled
- M_ME_NC_1 (13): Measured value, short floating point

### Control Direction (C_xx_xx)
- C_SC_NA_1 (45): Single command
- C_DC_NA_1 (46): Double command
- C_SE_NA_1 (48): Set-point command, normalized
- C_SE_NB_1 (49): Set-point command, scaled
- C_SE_NC_1 (50): Set-point command, short floating point

### System Information
- C_IC_NA_1 (100): Interrogation command
- C_CI_NA_1 (101): Counter interrogation command
- C_CS_NA_1 (103): Clock synchronization command
- C_TS_NA_1 (104): Test command

## Point Configuration

Points are configured in CSV files with information object addresses:

### Telemetry (YC) - telemetry.csv
```csv
point_id,name,description,unit,data_type,range_min,range_max,scale,offset,ioa
1,voltage_a,Phase A Voltage,V,float32,0,500,1.0,0.0,1001
2,current_a,Phase A Current,A,float32,0,100,0.1,0.0,1002
```

### Signal (YX) - signal.csv
```csv
point_id,name,description,normal_state,alarm_delay,reverse,ioa
1,cb_status,Circuit Breaker Status,0,5,false,2001
2,alarm_signal,Alarm Signal,0,0,false,2002
```

## Troubleshooting

1. **Connection Timeout**: Check t0_timeout and network connectivity
2. **No Data Received**: Verify common_addr matches server configuration
3. **APDU Errors**: Ensure cot_size, coa_size, ioa_size match server
4. **Test Frames**: Monitor t3_timeout for keep-alive issues
"#
    }
}
