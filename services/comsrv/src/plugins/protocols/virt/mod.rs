//! Virtual Protocol for Testing
//!
//! This module provides a virtual protocol implementation for testing purposes.
//! It simulates a real protocol without requiring actual hardware or network connections.

pub mod plugin;

// 重新导出插件
pub use plugin::VirtualPlugin as VirtPlugin;

// 导出创建函数
pub fn create_plugin() -> Box<dyn crate::plugins::traits::ProtocolPlugin> {
    Box::new(VirtPlugin::new())
}

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::core::combase::{
    ChannelStatus, ComBase, PointData, PointDataMap, RedisValue, TelemetryType,
};
use crate::core::config::types::{ChannelConfig, UnifiedPointMapping};
use crate::plugins::core::{DefaultPluginStorage, PluginStorage};
use crate::utils::error::{ComSrvError, Result};

/// Virtual protocol client for testing
pub struct VirtualProtocol {
    name: String,
    channel_id: u16,
    running: Arc<RwLock<bool>>,
    // Simulated data storage
    telemetry_data: Arc<RwLock<Vec<f64>>>,
    signal_data: Arc<RwLock<Vec<bool>>>,
    // Plugin storage for data persistence
    storage: Arc<tokio::sync::Mutex<Option<Arc<dyn PluginStorage>>>>,
}

impl VirtualProtocol {
    pub fn new(channel_config: crate::core::config::types::ChannelConfig) -> Result<Self> {
        Ok(Self {
            name: channel_config.name.clone(),
            channel_id: channel_config.id,
            running: Arc::new(RwLock::new(false)),
            telemetry_data: Arc::new(RwLock::new(vec![0.0; 100])),
            signal_data: Arc::new(RwLock::new(vec![false; 100])),
            storage: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }

    /// Simulate data changes
    #[allow(dead_code)]
    async fn simulate_data(&self) {
        let mut data = self.telemetry_data.write().await;
        for (i, value) in data.iter_mut().enumerate() {
            // Generate sine wave data
            *value = (i as f64 * 0.1).sin() * 100.0;
        }
    }
}

impl std::fmt::Debug for VirtualProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualProtocol")
            .field("name", &self.name)
            .field("channel_id", &self.channel_id)
            .field("running", &self.running)
            .finish()
    }
}

#[async_trait]
impl ComBase for VirtualProtocol {
    fn name(&self) -> &str {
        &self.name
    }

    fn protocol_type(&self) -> &str {
        "virtual"
    }

    fn is_connected(&self) -> bool {
        true // 虚拟协议始终连接
    }

    async fn get_status(&self) -> ChannelStatus {
        ChannelStatus {
            is_connected: true,
            last_error: None,
            last_update: chrono::Utc::now().timestamp() as u64,
            success_count: 100,
            error_count: 0,
            reconnect_count: 0,
            points_count: 200, // 100 telemetry + 100 signal
            last_read_duration_ms: Some(1),
            average_read_duration_ms: Some(1.0),
        }
    }

    async fn initialize(&mut self, _channel_config: &ChannelConfig) -> Result<()> {
        // 初始化存储
        let storage = DefaultPluginStorage::from_env().await?;
        *self.storage.lock().await = Some(Arc::new(storage) as Arc<dyn PluginStorage>);
        Ok(())
    }

    async fn connect(&mut self) -> Result<()> {
        info!("Starting virtual protocol client: {}", self.name);
        *self.running.write().await = true;

        // Start simulation task
        let telemetry_data = self.telemetry_data.clone();
        let signal_data = self.signal_data.clone();
        let running = self.running.clone();
        let channel_id = self.channel_id;
        let storage = self.storage.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

            while *running.read().await {
                interval.tick().await;

                // Update telemetry data
                {
                    let mut data = telemetry_data.write().await;
                    for (i, value) in data.iter_mut().enumerate() {
                        // Generate sine wave data
                        let t = chrono::Utc::now().timestamp() as f64;
                        *value = ((t + i as f64) * 0.1).sin() * 100.0;
                    }
                }

                // Update signal data
                {
                    let mut signals = signal_data.write().await;
                    for (i, signal) in signals.iter_mut().enumerate() {
                        // Toggle some signals randomly
                        if i % 10 == 0 {
                            *signal = !*signal;
                        }
                    }
                }

                // Write to storage if available
                if let Some(storage) = &*storage.lock().await {
                    let data = telemetry_data.read().await;
                    for (i, &value) in data.iter().enumerate().take(10) {
                        let _ = storage
                            .write_point(
                                channel_id,
                                &crate::core::config::TelemetryType::Telemetry,
                                i as u32 + 1,
                                value,
                            )
                            .await;
                    }
                }
            }
        });

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("Stopping virtual protocol client: {}", self.name);
        *self.running.write().await = false;
        Ok(())
    }

    async fn read_four_telemetry(&self, telemetry_type: &str) -> Result<PointDataMap> {
        let mut data_map = PointDataMap::new();

        match telemetry_type {
            "m" | "telemetry" => {
                let data = self.telemetry_data.read().await;
                for (i, &value) in data.iter().enumerate() {
                    data_map.insert(
                        i as u32 + 1,
                        PointData {
                            value: RedisValue::Float(value),
                            quality: 192,
                            timestamp: chrono::Utc::now().timestamp() as u64,
                        },
                    );
                }
            }
            "s" | "signal" => {
                let signals = self.signal_data.read().await;
                for (i, &signal) in signals.iter().enumerate() {
                    data_map.insert(
                        i as u32 + 1,
                        PointData {
                            value: RedisValue::Bool(signal),
                            quality: 192,
                            timestamp: chrono::Utc::now().timestamp() as u64,
                        },
                    );
                }
            }
            _ => {}
        }

        Ok(data_map)
    }

    async fn control(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>> {
        let mut results = Vec::new();
        let mut signals = self.signal_data.write().await;

        for (point_id, value) in commands {
            if point_id > 0 && point_id <= signals.len() as u32 {
                let idx = (point_id - 1) as usize;
                signals[idx] = match value {
                    RedisValue::Bool(b) => b,
                    RedisValue::Integer(i) => i != 0,
                    RedisValue::Float(f) => f != 0.0,
                    _ => false,
                };
                results.push((point_id, true));
            } else {
                results.push((point_id, false));
            }
        }

        Ok(results)
    }

    async fn adjustment(
        &mut self,
        adjustments: Vec<(u32, RedisValue)>,
    ) -> Result<Vec<(u32, bool)>> {
        let mut results = Vec::new();
        let mut data = self.telemetry_data.write().await;

        for (point_id, value) in adjustments {
            if point_id > 0 && point_id <= data.len() as u32 {
                let idx = (point_id - 1) as usize;
                data[idx] = match value {
                    RedisValue::Float(f) => f,
                    RedisValue::Integer(i) => i as f64,
                    RedisValue::String(s) => s.parse().unwrap_or(0.0),
                    _ => 0.0,
                };
                results.push((point_id, true));
            } else {
                results.push((point_id, false));
            }
        }

        Ok(results)
    }

    async fn update_points(&mut self, _mappings: Vec<UnifiedPointMapping>) -> Result<()> {
        // 虚拟协议不需要更新点位映射
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_virtual_protocol() {
        let config = ChannelConfig {
            id: 1,
            name: "Test Channel".to_string(),
            description: None,
            protocol: "virtual".to_string(),
            parameters: HashMap::new(),
            logging: Default::default(),
            points_config: None,
            combined_points: vec![],
        };

        let mut protocol = VirtualProtocol::new(config).unwrap();

        // Test connection
        assert!(protocol.connect().await.is_ok());
        assert!(protocol.is_connected());

        // Test reading telemetry
        let telemetry = protocol.read_four_telemetry("m").await.unwrap();
        assert_eq!(telemetry.len(), 100);

        // Test control
        let commands = vec![(1, RedisValue::Bool(true))];
        let results = protocol.control(commands).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].1);

        // Test disconnection
        assert!(protocol.disconnect().await.is_ok());
    }
}
