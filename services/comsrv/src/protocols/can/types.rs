//! CAN Protocol Type Definitions

use crate::core::config::types::ChannelConfig;
use crate::utils::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Re-export shared types
pub use crate::protocols::can_common::types::{ByteOrder, CanFilter, CanMessage, SignalDataType};

/// CAN polling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanPollingConfig {
    pub enabled: bool,
    #[serde(default = "default_interval_ms")]
    pub interval_ms: u64,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_retry_interval_ms")]
    pub retry_interval_ms: u64,
    #[serde(default)]
    pub batch_config: CanBatchConfig,
}

/// CAN batch processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanBatchConfig {
    #[serde(default = "default_batch_enabled")]
    pub enabled: bool,
    #[serde(default = "default_max_batch_size")]
    pub max_batch_size: u16,
    #[serde(default = "default_max_wait_ms")]
    pub max_wait_ms: u64,
}

/// CAN configuration
#[derive(Debug, Clone)]
pub struct CanConfig {
    pub interface: String,
    pub polling: CanPollingConfig,
    pub filters: Vec<CanFilter>,
    pub bitrate: Option<u32>,
}

/// Four-remote mapping types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MappingType {
    Telemetry,  // Telemetry point.
    Signal,     // Signal point.
    Control,    // Control point.
    Adjustment, // Adjustment point.
}

/// CAN to four-remote mapping (CSV-based configuration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanMapping {
    pub point_id: u32,
    pub can_id: u32,
    pub msg_name: String,
    pub signal_name: String,
    pub start_bit: u8,
    pub bit_length: u8,
    pub byte_order: String, // "ABCD", "DCBA", "BA", "AB", etc.
    pub data_type: SignalDataType,
    pub signed: bool,
    pub mapping_type: MappingType,
    #[serde(default = "default_scale")]
    pub scale: f64,
    #[serde(default = "default_offset")]
    pub offset: f64,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub unit: Option<String>,
    pub description: Option<String>,
}

fn default_scale() -> f64 {
    1.0
}

fn default_offset() -> f64 {
    0.0
}

/// CAN mapping collection
#[derive(Debug, Clone, Default)]
pub struct CanMappingCollection {
    pub telemetry: Vec<CanMapping>,
    pub signal: Vec<CanMapping>,
    pub control: Vec<CanMapping>,
    pub adjustment: Vec<CanMapping>,
    // Index for fast lookup by CAN ID
    pub by_can_id: HashMap<u32, Vec<CanMapping>>,
}

impl CanConfig {
    /// Create from channel configuration
    pub fn from_channel_config(channel_config: &ChannelConfig) -> Result<Self> {
        let params = &channel_config.parameters;

        // Extract interface
        let interface = params
            .get("interface")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                crate::utils::error::ComSrvError::InvalidParameter(
                    "CAN interface is required".to_string(),
                )
            })?
            .to_string();

        // Extract polling configuration
        let polling = if let Some(polling_value) = params.get("polling") {
            serde_json::from_value(polling_value.clone())
                .unwrap_or_else(|_| CanPollingConfig::default())
        } else {
            CanPollingConfig::default()
        };

        // Extract filters
        let filters = extract_filters(params);

        // Extract bitrate
        let bitrate = params
            .get("bitrate")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        Ok(Self {
            interface,
            polling,
            filters,
            bitrate,
        })
    }
}

fn extract_filters(params: &HashMap<String, serde_json::Value>) -> Vec<CanFilter> {
    let mut filters = Vec::new();

    if let Some(filter_value) = params.get("filters") {
        if let Some(arr) = filter_value.as_array() {
            for item in arr {
                if let Some(s) = item.as_str() {
                    // Parse filter string, e.g., "0x100-0x1FF" or "0x200:0xFF00"
                    if let Some(filter) = parse_filter_string(s) {
                        filters.push(filter);
                    }
                }
            }
        }
    }

    filters
}

pub fn parse_filter_string(s: &str) -> Option<CanFilter> {
    if s.contains('-') {
        // Range filter, e.g., "0x100-0x1FF"
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() == 2 {
            let start = parse_hex_id(parts[0])?;
            let end = parse_hex_id(parts[1])?;
            // Create mask for range
            let mask = create_range_mask(start, end);
            return Some(CanFilter {
                can_id: start,
                mask,
            });
        }
    } else if s.contains(':') {
        // ID:Mask filter, e.g., "0x200:0xFF00"
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            let can_id = parse_hex_id(parts[0])?;
            let mask = parse_hex_id(parts[1])?;
            return Some(CanFilter { can_id, mask });
        }
    } else {
        // Single ID filter
        let can_id = parse_hex_id(s)?;
        return Some(CanFilter {
            can_id,
            mask: 0xFFFFFFFF, // Exact match
        });
    }

    None
}

pub fn parse_hex_id(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.starts_with("0x") || s.starts_with("0X") {
        u32::from_str_radix(&s[2..], 16).ok()
    } else {
        s.parse().ok()
    }
}

fn create_range_mask(start: u32, end: u32) -> u32 {
    // Simple implementation: create a mask that covers the range
    // This is a simplified version; a more sophisticated implementation
    // would create optimal masks for the range
    let diff = end ^ start;
    let mut mask = 0xFFFFFFFF;
    let mut bit = 0x80000000;

    while bit > 0 {
        if (diff & bit) != 0 {
            mask &= !bit;
        }
        bit >>= 1;
    }

    mask
}

impl Default for CanPollingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_ms: default_interval_ms(),
            timeout_ms: default_timeout_ms(),
            max_retries: default_max_retries(),
            retry_interval_ms: default_retry_interval_ms(),
            batch_config: CanBatchConfig::default(),
        }
    }
}

impl Default for CanBatchConfig {
    fn default() -> Self {
        Self {
            enabled: default_batch_enabled(),
            max_batch_size: default_max_batch_size(),
            max_wait_ms: default_max_wait_ms(),
        }
    }
}

// Default value functions
fn default_interval_ms() -> u64 {
    1000
}
fn default_timeout_ms() -> u64 {
    5000
}
fn default_max_retries() -> u32 {
    3
}
fn default_retry_interval_ms() -> u64 {
    1000
}
fn default_batch_enabled() -> bool {
    true
}
fn default_max_batch_size() -> u16 {
    20
}
fn default_max_wait_ms() -> u64 {
    100
}
