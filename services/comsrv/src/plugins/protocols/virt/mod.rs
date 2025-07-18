//! Virtual Protocol for Testing
//!
//! This module provides a virtual protocol implementation for testing purposes.
//! It simulates a real protocol without requiring actual hardware or network connections.

pub mod plugin;

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::core::framework::traits::ComBase;
use crate::core::framework::{ChannelStatus, PointData, TelemetryType};
use crate::plugins::plugin_storage::{DefaultPluginStorage, PluginStorage};
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
    pub fn new(channel_config: crate::core::config::types::channel::ChannelConfig) -> Result<Self> {
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

        let mut signals = self.signal_data.write().await;
        for (i, signal) in signals.iter_mut().enumerate() {
            // Toggle some signals
            *signal = i % 3 == 0;
        }
    }
}

impl std::fmt::Debug for VirtualProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualProtocol")
            .field("name", &self.name)
            .field("channel_id", &self.channel_id)
            .field("running", &self.running)
            .field("telemetry_data", &"<telemetry_data>")
            .field("signal_data", &"<signal_data>")
            .field("storage", &"<storage>")
            .finish()
    }
}

#[async_trait]
impl ComBase for VirtualProtocol {
    fn name(&self) -> &str {
        &self.name
    }

    fn channel_id(&self) -> String {
        self.channel_id.to_string()
    }

    fn protocol_type(&self) -> &str {
        "virtual"
    }

    fn get_parameters(&self) -> HashMap<String, String> {
        HashMap::new()
    }

    async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    async fn start(&mut self) -> Result<()> {
        info!("Starting virtual protocol client: {}", self.name);

        // Initialize storage
        match DefaultPluginStorage::from_env().await {
            Ok(s) => {
                let mut storage = self.storage.lock().await;
                *storage = Some(Arc::new(s) as Arc<dyn PluginStorage>);
                info!("Virtual protocol storage initialized");
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to create storage: {}, data will not be persisted",
                    e
                );
            }
        }

        *self.running.write().await = true;

        // Start simulation task
        let running = self.running.clone();
        let telemetry_data = self.telemetry_data.clone();
        let signal_data = self.signal_data.clone();
        let storage = self.storage.clone();
        let channel_id = self.channel_id;

        tokio::spawn(async move {
            while *running.read().await {
                // Simulate data changes every second
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                let mut data = telemetry_data.write().await;
                for (i, value) in data.iter_mut().enumerate() {
                    *value = (chrono::Utc::now().timestamp() as f64 + i as f64 * 0.1).sin() * 100.0;
                }

                let mut signals = signal_data.write().await;
                for (i, signal) in signals.iter_mut().enumerate() {
                    *signal = (chrono::Utc::now().timestamp() + i as i64) % 3 == 0;
                }

                // Store data to plugin storage
                if let Some(storage) = &*storage.lock().await {
                    // Store some sample telemetry points
                    for i in 0..10 {
                        let _ = storage
                            .write_point(
                                channel_id,
                                &TelemetryType::Telemetry,
                                i as u32 + 1,
                                data[i],
                            )
                            .await;
                    }

                    // Store some sample signal points
                    for i in 0..10 {
                        let _ = storage
                            .write_point(
                                channel_id,
                                &TelemetryType::Signal,
                                i as u32 + 1001,
                                if signals[i] { 1.0 } else { 0.0 },
                            )
                            .await;
                    }
                }
            }
        });

        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        info!("Stopping virtual protocol client: {}", self.name);
        *self.running.write().await = false;
        Ok(())
    }

    async fn status(&self) -> ChannelStatus {
        ChannelStatus {
            id: self.channel_id.to_string(),
            connected: *self.running.read().await,
            last_response_time: 0.0,
            last_error: String::new(),
            last_update_time: chrono::Utc::now(),
            error_count: 0,
        }
    }

    async fn update_status(&mut self, status: ChannelStatus) -> Result<()> {
        debug!("Virtual protocol status update: {status:?}");
        Ok(())
    }

    async fn get_all_points(&self) -> Vec<PointData> {
        // Return simulated point data
        let data = self.telemetry_data.read().await;
        let signals = self.signal_data.read().await;

        let mut points = Vec::new();

        // Add telemetry points
        for (i, &value) in data.iter().enumerate() {
            points.push(PointData {
                id: format!("YC{}", i + 1),
                name: format!("Telemetry {}", i + 1),
                value: value.to_string(),
                timestamp: chrono::Utc::now(),
                unit: "V".to_string(),
                description: format!("Simulated telemetry point {}", i + 1),
                telemetry_type: Some(crate::core::framework::TelemetryType::Telemetry),
                channel_id: Some(self.channel_id),
            });
        }

        // Add signal points
        for (i, &signal) in signals.iter().enumerate() {
            points.push(PointData {
                id: format!("YX{}", i + 1),
                name: format!("Signal {}", i + 1),
                value: if signal { "1" } else { "0" }.to_string(),
                timestamp: chrono::Utc::now(),
                unit: "".to_string(),
                description: format!("Simulated signal point {}", i + 1),
                telemetry_type: Some(crate::core::framework::TelemetryType::Signal),
                channel_id: Some(self.channel_id),
            });
        }

        points
    }

    async fn read_point(&self, point_id: &str) -> Result<PointData> {
        if point_id.starts_with("YC") {
            let id: usize = point_id[2..].parse().unwrap_or(0);
            let data = self.telemetry_data.read().await;
            if id > 0 && id <= data.len() {
                Ok(PointData {
                    id: point_id.to_string(),
                    name: format!("Telemetry {id}"),
                    value: data[id - 1].to_string(),
                    timestamp: chrono::Utc::now(),
                    unit: "V".to_string(),
                    description: format!("Simulated telemetry point {id}"),
                    telemetry_type: Some(crate::core::framework::TelemetryType::Telemetry),
                    channel_id: Some(self.channel_id),
                })
            } else {
                Err(ComSrvError::InvalidParameter(
                    "Invalid point ID".to_string(),
                ))
            }
        } else if point_id.starts_with("YX") {
            let id: usize = point_id[2..].parse().unwrap_or(0);
            let signals = self.signal_data.read().await;
            if id > 0 && id <= signals.len() {
                Ok(PointData {
                    id: point_id.to_string(),
                    name: format!("Signal {id}"),
                    value: if signals[id - 1] { "1" } else { "0" }.to_string(),
                    timestamp: chrono::Utc::now(),
                    unit: "".to_string(),
                    description: format!("Simulated signal point {id}"),
                    telemetry_type: Some(crate::core::framework::TelemetryType::Signal),
                    channel_id: Some(self.channel_id),
                })
            } else {
                Err(ComSrvError::InvalidParameter(
                    "Invalid point ID".to_string(),
                ))
            }
        } else {
            Err(ComSrvError::InvalidParameter(
                "Unknown point type".to_string(),
            ))
        }
    }

    async fn write_point(&mut self, point_id: &str, value: &str) -> Result<()> {
        info!("Virtual protocol write: {}={value}", point_id);

        if point_id.starts_with("YK") || point_id.starts_with("YT") {
            // Simulated control/adjustment write
            Ok(())
        } else {
            Err(ComSrvError::InvalidParameter(format!(
                "Point {} is not writable",
                point_id
            )))
        }
    }

    async fn get_diagnostics(&self) -> HashMap<String, String> {
        let mut diag = HashMap::new();
        diag.insert("protocol".to_string(), "virtual".to_string());
        diag.insert("status".to_string(), "simulated".to_string());
        diag.insert("running".to_string(), self.is_running().await.to_string());
        diag
    }
}
