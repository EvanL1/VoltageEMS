//! RTU monitoring module
//!
//! Provides real-time monitoring of RTU communication including:
//! - Connection status
//! - Communication statistics
//! - Data quality analysis
//! - Alarm and diagnostics

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, SystemTime};
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tokio::time::interval;
use serde::{Deserialize, Serialize};

use crate::utils::error::{ComSrvError, Result};
use super::super::protocols::modbus::{ModbusClient, ModbusClientStats, ModbusConnectionState};

// Serde helper module for SystemTime serialization
mod timestamp_as_seconds {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time.duration_since(UNIX_EPOCH)
            .map_err(serde::ser::Error::custom)?;
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seconds = u64::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + std::time::Duration::from_secs(seconds))
    }
}

/// RTU monitor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtuMonitorConfig {
    /// Monitoring interval in seconds
    pub monitor_interval: u64,
    /// History retention time in minutes
    pub history_retention_minutes: u64,
    /// Alarm thresholds
    pub alarm_thresholds: RtuAlarmThresholds,
    /// Whether to enable detailed logging
    pub detailed_logging: bool,
}

impl Default for RtuMonitorConfig {
    fn default() -> Self {
        Self {
            monitor_interval: 10,
            history_retention_minutes: 60,
            alarm_thresholds: RtuAlarmThresholds::default(),
            detailed_logging: false,
        }
    }
}

/// RTU alarm thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtuAlarmThresholds {
    /// Communication quality low threshold (%)
    pub communication_quality_low: f64,
    /// High average response time threshold (ms)
    pub avg_response_time_high: f64,
    /// Consecutive failures threshold
    pub consecutive_failures_threshold: u32,
    /// High CRC error rate threshold (%)
    pub crc_error_rate_high: f64,
}

impl Default for RtuAlarmThresholds {
    fn default() -> Self {
        Self {
            communication_quality_low: 90.0,
            avg_response_time_high: 1000.0,
            consecutive_failures_threshold: 5,
            crc_error_rate_high: 5.0,
        }
    }
}

/// RTU monitoring data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtuMonitorPoint {
    /// Timestamp
    #[serde(with = "timestamp_as_seconds")]
    pub timestamp: SystemTime,
    /// Connection state
    pub connection_state: String,
    /// Communication quality (%)
    pub communication_quality: f64,
    /// Average response time (ms)
    pub avg_response_time_ms: f64,
    /// Successful request count
    pub successful_requests: u64,
    /// Failed request count
    pub failed_requests: u64,
    /// CRC error count
    pub crc_errors: u64,
    /// Timeout request count
    pub timeout_requests: u64,
    /// Exception response count
    pub exception_responses: u64,
}

/// RTU alarm information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtuAlarm {
    /// Alarm ID
    pub id: String,
    /// Alarm type
    pub alarm_type: RtuAlarmType,
    /// Alarm severity
    pub severity: RtuAlarmSeverity,
    /// Alarm message
    pub message: String,
    /// Alarm timestamp
    #[serde(with = "timestamp_as_seconds")]
    pub timestamp: SystemTime,
    /// Whether the alarm is acknowledged
    pub acknowledged: bool,
    /// Channel ID
    pub channel_id: u16,
}

/// RTU alarm type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RtuAlarmType {
    /// Connection lost
    ConnectionLost,
    /// Communication quality low
    CommunicationQualityLow,
    /// High response time
    HighResponseTime,
    /// High CRC error rate
    HighCrcErrorRate,
    /// Consecutive failures
    ConsecutiveFailures,
    /// Device not responding
    DeviceNotResponding,
}

/// RTU alarm severity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RtuAlarmSeverity {
    /// Info
    Info,
    /// Warning
    Warning,
    /// Error
    Error,
    /// Critical
    Critical,
}

/// RTU monitoring status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtuMonitorStatus {
    /// Number of monitored channels
    pub monitored_channels: u32,
    /// Number of online channels
    pub online_channels: u32,
    /// Number of offline channels
    pub offline_channels: u32,
    /// Number of active alarms
    pub active_alarms: u32,
    /// Overall communication quality
    pub overall_communication_quality: f64,
    /// Last update time
    #[serde(with = "timestamp_as_seconds")]
    pub last_update: SystemTime,
}

/// RTU monitor
pub struct RtuMonitor {
    /// Configuration
    config: RtuMonitorConfig,
    /// Monitored clients
    clients: Arc<RwLock<HashMap<u16, Arc<Mutex<ModbusClient>>>>>,
    /// Historical monitoring data
    history: Arc<RwLock<HashMap<u16, VecDeque<RtuMonitorPoint>>>>,
    /// Active alarms
    active_alarms: Arc<RwLock<HashMap<String, RtuAlarm>>>,
    /// Monitoring status
    status: Arc<RwLock<RtuMonitorStatus>>,
    /// Whether the monitor is running
    is_running: Arc<RwLock<bool>>,
}

impl RtuMonitor {
    /// Create a new RTU monitor
    pub fn new(config: RtuMonitorConfig) -> Self {
        Self {
            config,
            clients: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(HashMap::new())),
            active_alarms: Arc::new(RwLock::new(HashMap::new())),
            status: Arc::new(RwLock::new(RtuMonitorStatus {
                monitored_channels: 0,
                online_channels: 0,
                offline_channels: 0,
                active_alarms: 0,
                overall_communication_quality: 0.0,
                last_update: SystemTime::now(),
            })),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start monitoring
    pub async fn start(&self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            if *running {
                return Err(ComSrvError::StateError("Monitor is already running".to_string()));
            }
            *running = true;
        }

        // Spawn the monitoring task
        self.start_monitoring_task().await;

        log::info!("RTU monitor started with {} second interval", self.config.monitor_interval);
        Ok(())
    }

    /// Stop monitoring
    pub async fn stop(&self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }

        log::info!("RTU monitor stopped");
        Ok(())
    }

    /// Add a client to the monitor
    pub async fn add_client(&self, channel_id: u16, client: Arc<Mutex<ModbusClient>>) {
        {
            let mut clients = self.clients.write().await;
            clients.insert(channel_id, client);
        }

        {
            let mut history = self.history.write().await;
            history.insert(channel_id, VecDeque::new());
        }

        log::info!("Added channel {} to RTU monitoring", channel_id);
    }

    /// Remove a client from the monitor
    pub async fn remove_client(&self, channel_id: u16) {
        {
            let mut clients = self.clients.write().await;
            clients.remove(&channel_id);
        }

        {
            let mut history = self.history.write().await;
            history.remove(&channel_id);
        }

        log::info!("Removed channel {} from RTU monitoring", channel_id);
    }

    /// Spawn the monitoring task
    async fn start_monitoring_task(&self) {
        let config = self.config.clone();
        let clients = self.clients.clone();
        let history = self.history.clone();
        let active_alarms = self.active_alarms.clone();
        let status = self.status.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            let mut monitor_interval = interval(Duration::from_secs(config.monitor_interval));

            while *is_running.read().await {
                monitor_interval.tick().await;

                // Collect monitoring data from all clients
                Self::collect_monitoring_data(
                    &config,
                    &clients,
                    &history,
                    &active_alarms,
                    &status,
                ).await;
            }
        });
    }

    /// Collect monitoring data
    async fn collect_monitoring_data(
        config: &RtuMonitorConfig,
        clients: &Arc<RwLock<HashMap<u16, Arc<Mutex<ModbusClient>>>>>,
        history: &Arc<RwLock<HashMap<u16, VecDeque<RtuMonitorPoint>>>>,
        active_alarms: &Arc<RwLock<HashMap<String, RtuAlarm>>>,
        status: &Arc<RwLock<RtuMonitorStatus>>,
    ) {
        let client_map = clients.read().await;
        let mut total_quality = 0.0;
        let mut online_count = 0u32;
        let mut offline_count = 0u32;

        for (&channel_id, client) in client_map.iter() {
            let client_guard = client.lock().await;
            
            // Obtain connection state and statistics
            let connection_state = client_guard.get_connection_state().await;
            let stats = client_guard.get_stats().await;

            // Create monitoring data point
            let monitor_point = RtuMonitorPoint {
                timestamp: SystemTime::now(),
                connection_state: format!("{:?}", connection_state),
                communication_quality: stats.communication_quality,
                avg_response_time_ms: stats.avg_response_time_ms,
                successful_requests: stats.successful_requests,
                failed_requests: stats.failed_requests,
                crc_errors: stats.crc_errors,
                timeout_requests: stats.timeout_requests,
                exception_responses: stats.exception_responses,
            };

            // Update historical data
            {
                let mut history_map = history.write().await;
                if let Some(channel_history) = history_map.get_mut(&channel_id) {
                    channel_history.push_back(monitor_point.clone());
                    
                    // Remove expired entries
                    let retention_duration = Duration::from_secs(config.history_retention_minutes * 60);
                    let cutoff_time = SystemTime::now() - retention_duration;
                    
                    while let Some(front) = channel_history.front() {
                        if front.timestamp < cutoff_time {
                            channel_history.pop_front();
                        } else {
                            break;
                        }
                    }
                }
            }

            // Update counters
            match connection_state {
                ModbusConnectionState::Connected => {
                    online_count += 1;
                    total_quality += stats.communication_quality;
                }
                _ => {
                    offline_count += 1;
                }
            }

            // Check alarm conditions
            Self::check_alarms(config, channel_id, &monitor_point, &stats, active_alarms).await;

            if config.detailed_logging {
                log::debug!(
                    "Channel {}: State={:?}, Quality={:.1}%, ResponseTime={:.1}ms",
                    channel_id, connection_state, stats.communication_quality, stats.avg_response_time_ms
                );
            }
        }

        // Update monitoring status
        {
            let mut status_guard = status.write().await;
            status_guard.monitored_channels = client_map.len() as u32;
            status_guard.online_channels = online_count;
            status_guard.offline_channels = offline_count;
            status_guard.overall_communication_quality = if online_count > 0 {
                total_quality / online_count as f64
            } else {
                0.0
            };
            status_guard.active_alarms = active_alarms.read().await.len() as u32;
            status_guard.last_update = SystemTime::now();
        }
    }

    /// Check alarm conditions
    async fn check_alarms(
        config: &RtuMonitorConfig,
        channel_id: u16,
        monitor_point: &RtuMonitorPoint,
        stats: &ModbusClientStats,
        active_alarms: &Arc<RwLock<HashMap<String, RtuAlarm>>>,
    ) {
        let mut new_alarms = Vec::new();

        // Verify communication quality
        if monitor_point.communication_quality < config.alarm_thresholds.communication_quality_low {
            let alarm = RtuAlarm {
                id: format!("ch{}_comm_quality_low", channel_id),
                alarm_type: RtuAlarmType::CommunicationQualityLow,
                severity: RtuAlarmSeverity::Warning,
                message: format!(
                    "Channel {} communication quality is low: {:.1}%",
                    channel_id, monitor_point.communication_quality
                ),
                timestamp: SystemTime::now(),
                acknowledged: false,
                channel_id,
            };
            new_alarms.push(alarm);
        }

        // Verify response time
        if monitor_point.avg_response_time_ms > config.alarm_thresholds.avg_response_time_high {
            let alarm = RtuAlarm {
                id: format!("ch{}_high_response_time", channel_id),
                alarm_type: RtuAlarmType::HighResponseTime,
                severity: RtuAlarmSeverity::Warning,
                message: format!(
                    "Channel {} average response time is high: {:.1}ms",
                    channel_id, monitor_point.avg_response_time_ms
                ),
                timestamp: SystemTime::now(),
                acknowledged: false,
                channel_id,
            };
            new_alarms.push(alarm);
        }

        // Verify CRC error rate
        if stats.total_requests > 0 {
            let crc_error_rate = (stats.crc_errors as f64 / stats.total_requests as f64) * 100.0;
            if crc_error_rate > config.alarm_thresholds.crc_error_rate_high {
                let alarm = RtuAlarm {
                    id: format!("ch{}_high_crc_error_rate", channel_id),
                    alarm_type: RtuAlarmType::HighCrcErrorRate,
                    severity: RtuAlarmSeverity::Error,
                    message: format!(
                        "Channel {} CRC error rate is high: {:.1}%",
                        channel_id, crc_error_rate
                    ),
                    timestamp: SystemTime::now(),
                    acknowledged: false,
                    channel_id,
                };
                new_alarms.push(alarm);
            }
        }

        // Verify connection state
        if monitor_point.connection_state != "Connected" {
            let alarm = RtuAlarm {
                id: format!("ch{}_connection_lost", channel_id),
                alarm_type: RtuAlarmType::ConnectionLost,
                severity: RtuAlarmSeverity::Error,
                message: format!(
                    "Channel {} connection lost: {}",
                    channel_id, monitor_point.connection_state
                ),
                timestamp: SystemTime::now(),
                acknowledged: false,
                channel_id,
            };
            new_alarms.push(alarm);
        }

        // Add new alarms to the active list
        {
            let mut alarms = active_alarms.write().await;
            for alarm in new_alarms {
                let existing_alarm = alarms.get(&alarm.id);
                
                // Only insert if alarm does not exist or has been acknowledged
                if existing_alarm.is_none() || existing_alarm.unwrap().acknowledged {
                    log::warn!("RTU Alarm: {}", alarm.message);
                    alarms.insert(alarm.id.clone(), alarm);
                }
            }
        }
    }

    /// Get monitoring status
    pub async fn get_status(&self) -> RtuMonitorStatus {
        self.status.read().await.clone()
    }

    /// Get active alarms
    pub async fn get_active_alarms(&self) -> Vec<RtuAlarm> {
        self.active_alarms.read().await.values().cloned().collect()
    }

    /// Acknowledge an alarm
    pub async fn acknowledge_alarm(&self, alarm_id: &str) -> Result<()> {
        let mut alarms = self.active_alarms.write().await;
        if let Some(alarm) = alarms.get_mut(alarm_id) {
            alarm.acknowledged = true;
            log::info!("Alarm acknowledged: {}", alarm_id);
            Ok(())
        } else {
            Err(ComSrvError::NotFound(format!("Alarm not found: {}", alarm_id)))
        }
    }

    /// Clear acknowledged alarms
    pub async fn clear_acknowledged_alarms(&self) {
        let mut alarms = self.active_alarms.write().await;
        alarms.retain(|_, alarm| !alarm.acknowledged);
        log::info!("Cleared acknowledged alarms");
    }

    /// Get channel history data
    pub async fn get_channel_history(&self, channel_id: u16, limit: Option<usize>) -> Vec<RtuMonitorPoint> {
        let history_map = self.history.read().await;
        if let Some(channel_history) = history_map.get(&channel_id) {
            let mut points: Vec<_> = channel_history.iter().cloned().collect();
            points.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            
            if let Some(limit) = limit {
                points.into_iter().rev().take(limit).rev().collect()
            } else {
                points
            }
        } else {
            Vec::new()
        }
    }

    /// Get current channel status
    pub async fn get_channel_status(&self, channel_id: u16) -> Option<RtuMonitorPoint> {
        let history_map = self.history.read().await;
        if let Some(channel_history) = history_map.get(&channel_id) {
            channel_history.back().cloned()
        } else {
            None
        }
    }

    /// Generate a monitoring report
    pub async fn generate_report(&self) -> RtuMonitorReport {
        let status = self.get_status().await;
        let alarms = self.get_active_alarms().await;
        let clients = self.clients.read().await;
        
        let mut channel_summaries = Vec::new();
        
        for &channel_id in clients.keys() {
            if let Some(current_status) = self.get_channel_status(channel_id).await {
                let summary = RtuChannelSummary {
                    channel_id,
                    connection_state: current_status.connection_state,
                    communication_quality: current_status.communication_quality,
                    avg_response_time_ms: current_status.avg_response_time_ms,
                    total_requests: current_status.successful_requests + current_status.failed_requests,
                    successful_requests: current_status.successful_requests,
                    failed_requests: current_status.failed_requests,
                    active_alarms: alarms.iter()
                        .filter(|a| a.channel_id == channel_id && !a.acknowledged)
                        .count() as u32,
                };
                channel_summaries.push(summary);
            }
        }

        RtuMonitorReport {
            generated_at: SystemTime::now(),
            overall_status: status,
            channel_summaries,
            active_alarms: alarms,
        }
    }
}

/// RTU channel summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtuChannelSummary {
    /// Channel ID
    pub channel_id: u16,
    /// Connection state
    pub connection_state: String,
    /// Communication quality
    pub communication_quality: f64,
    /// Average response time
    pub avg_response_time_ms: f64,
    /// Total request count
    pub total_requests: u64,
    /// Successful request count
    pub successful_requests: u64,
    /// Failed request count
    pub failed_requests: u64,
    /// Active alarm count
    pub active_alarms: u32,
}

/// RTU monitor report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtuMonitorReport {
    /// Generation time
    #[serde(with = "timestamp_as_seconds")]
    pub generated_at: SystemTime,
    /// Overall status
    pub overall_status: RtuMonitorStatus,
    /// Channel summaries
    pub channel_summaries: Vec<RtuChannelSummary>,
    /// Active alarms
    pub active_alarms: Vec<RtuAlarm>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::protocols::modbus::{ModbusClient, ModbusCommunicationMode, ModbusClientConfig};
    use std::time::Duration;
    use tokio::time::sleep;

    /// Create a test RTU monitor
    fn create_test_rtu_monitor() -> RtuMonitor {
        let config = RtuMonitorConfig {
            monitor_interval: 1, // 1-second interval for fast tests
            history_retention_minutes: 5,
            alarm_thresholds: RtuAlarmThresholds::default(),
            detailed_logging: false,
        };
        RtuMonitor::new(config)
    }

    /// Create a test Modbus client
    fn create_test_modbus_client() -> ModbusClient {
        let config = crate::core::config::config_manager::ChannelConfig {
            id: 1,
            name: "Test Channel".to_string(),
            description: "Test Modbus Channel".to_string(),
            protocol: crate::core::config::config_manager::ProtocolType::ModbusRtu,
            parameters: crate::core::config::config_manager::ChannelParameters::ModbusRtu {
                port: "/dev/ttyUSB0".to_string(),
                baud_rate: 9600,
                data_bits: 8,
                stop_bits: 1,
                parity: "None".to_string(),
                timeout: 1000,
                max_retries: 3,
                point_tables: std::collections::HashMap::new(),
                poll_rate: 1000,
                slave_id: 1,
            },
        };
        
        ModbusClient::new(config.into(), ModbusCommunicationMode::Rtu).unwrap()
    }

    #[test]
    fn test_rtu_monitor_creation() {
        let monitor = create_test_rtu_monitor();
        assert_eq!(monitor.config.monitor_interval, 1);
        assert_eq!(monitor.config.history_retention_minutes, 5);
    }

    #[test]
    fn test_rtu_monitor_config_default() {
        let config = RtuMonitorConfig::default();
        assert_eq!(config.monitor_interval, 10);  // Match actual default value
        assert_eq!(config.history_retention_minutes, 60);
        assert!(!config.detailed_logging);
    }

    #[test]
    fn test_rtu_alarm_thresholds_default() {
        let thresholds = RtuAlarmThresholds::default();
        assert_eq!(thresholds.communication_quality_low, 90.0);  // Match actual default value
        assert_eq!(thresholds.avg_response_time_high, 1000.0);
        assert_eq!(thresholds.consecutive_failures_threshold, 5);
        assert_eq!(thresholds.crc_error_rate_high, 5.0);
    }

    #[tokio::test]
    async fn test_rtu_monitor_start_stop() {
        let monitor = create_test_rtu_monitor();
        
        // Initially not running
        assert!(!*monitor.is_running.read().await);
        
        // Start monitoring
        let result = monitor.start().await;
        assert!(result.is_ok());
        assert!(*monitor.is_running.read().await);
        
        // Stop monitoring
        let result = monitor.stop().await;
        assert!(result.is_ok());
        assert!(!*monitor.is_running.read().await);
    }

    #[tokio::test]
    async fn test_add_remove_client() {
        let monitor = create_test_rtu_monitor();
        let client = Arc::new(Mutex::new(create_test_modbus_client()));
        
        // Add client
        monitor.add_client(1, client.clone()).await;
        
        // Verify client was added
        {
            let clients = monitor.clients.read().await;
            assert!(clients.contains_key(&1));
            assert_eq!(clients.len(), 1);
        }
        
        // Remove client
        monitor.remove_client(1).await;
        
        // Verify client was removed
        {
            let clients = monitor.clients.read().await;
            assert!(!clients.contains_key(&1));
            assert_eq!(clients.len(), 0);
        }
    }

    #[tokio::test]
    async fn test_get_status() {
        let monitor = create_test_rtu_monitor();
        let status = monitor.get_status().await;
        
        assert_eq!(status.monitored_channels, 0);
        assert_eq!(status.online_channels, 0);
        assert_eq!(status.offline_channels, 0);
        assert_eq!(status.active_alarms, 0);
        assert_eq!(status.overall_communication_quality, 0.0);
    }

    #[tokio::test]
    async fn test_get_active_alarms_empty() {
        let monitor = create_test_rtu_monitor();
        let alarms = monitor.get_active_alarms().await;
        
        assert!(alarms.is_empty());
    }

    #[tokio::test]
    async fn test_acknowledge_alarm_not_found() {
        let monitor = create_test_rtu_monitor();
        let result = monitor.acknowledge_alarm("non_existent_alarm").await;
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ComSrvError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_clear_acknowledged_alarms_empty() {
        let monitor = create_test_rtu_monitor();
        
        // This should not panic even with no alarms
        monitor.clear_acknowledged_alarms().await;
        
        let alarms = monitor.get_active_alarms().await;
        assert!(alarms.is_empty());
    }

    #[tokio::test]
    async fn test_get_channel_history_empty() {
        let monitor = create_test_rtu_monitor();
        let history = monitor.get_channel_history(1, None).await;
        
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_get_channel_history_with_limit() {
        let monitor = create_test_rtu_monitor();
        let history = monitor.get_channel_history(1, Some(10)).await;
        
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_get_channel_status_not_found() {
        let monitor = create_test_rtu_monitor();
        let status = monitor.get_channel_status(1).await;
        
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn test_generate_report_empty() {
        let monitor = create_test_rtu_monitor();
        let report = monitor.generate_report().await;
        
        assert!(report.channel_summaries.is_empty());
        assert!(report.active_alarms.is_empty());
        assert_eq!(report.overall_status.monitored_channels, 0);
    }

    #[tokio::test]
    async fn test_alarm_creation_and_acknowledgment() {
        let monitor = create_test_rtu_monitor();
        
        // Manually create an alarm for testing
        let alarm = RtuAlarm {
            id: "test_alarm".to_string(),
            alarm_type: RtuAlarmType::ConnectionLost,
            severity: RtuAlarmSeverity::Error,
            message: "Test alarm message".to_string(),
            timestamp: SystemTime::now(),
            acknowledged: false,
            channel_id: 1,
        };
        
        // Add alarm manually
        {
            let mut alarms = monitor.active_alarms.write().await;
            alarms.insert(alarm.id.clone(), alarm.clone());
        }
        
        // Verify alarm exists
        let active_alarms = monitor.get_active_alarms().await;
        assert_eq!(active_alarms.len(), 1);
        assert!(!active_alarms[0].acknowledged);
        
        // Acknowledge alarm
        let result = monitor.acknowledge_alarm("test_alarm").await;
        assert!(result.is_ok());
        
        // Verify alarm is acknowledged
        let active_alarms = monitor.get_active_alarms().await;
        assert_eq!(active_alarms.len(), 1);
        assert!(active_alarms[0].acknowledged);
        
        // Clear acknowledged alarms
        monitor.clear_acknowledged_alarms().await;
        
        // Verify alarm is removed
        let active_alarms = monitor.get_active_alarms().await;
        assert!(active_alarms.is_empty());
    }

    #[test]
    fn test_rtu_alarm_types() {
        let alarm_types = vec![
            RtuAlarmType::ConnectionLost,
            RtuAlarmType::CommunicationQualityLow,
            RtuAlarmType::HighResponseTime,
            RtuAlarmType::HighCrcErrorRate,
            RtuAlarmType::ConsecutiveFailures,
            RtuAlarmType::DeviceNotResponding,
        ];
        
        // Ensure all alarm types can be created
        for alarm_type in alarm_types {
            let alarm = RtuAlarm {
                id: "test".to_string(),
                alarm_type,
                severity: RtuAlarmSeverity::Info,
                message: "Test".to_string(),
                timestamp: SystemTime::now(),
                acknowledged: false,
                channel_id: 1,
            };
            assert_eq!(alarm.channel_id, 1);
        }
    }

    #[test]
    fn test_rtu_alarm_severities() {
        let severities = vec![
            RtuAlarmSeverity::Info,
            RtuAlarmSeverity::Warning,
            RtuAlarmSeverity::Error,
            RtuAlarmSeverity::Critical,
        ];
        
        // Ensure all severity levels can be created
        for severity in severities {
            let alarm = RtuAlarm {
                id: "test".to_string(),
                alarm_type: RtuAlarmType::ConnectionLost,
                severity,
                message: "Test".to_string(),
                timestamp: SystemTime::now(),
                acknowledged: false,
                channel_id: 1,
            };
            assert_eq!(alarm.channel_id, 1);
        }
    }

    #[test]
    fn test_rtu_monitor_point_creation() {
        let point = RtuMonitorPoint {
            timestamp: SystemTime::now(),
            connection_state: "Connected".to_string(),
            communication_quality: 95.5,
            avg_response_time_ms: 123.4,
            successful_requests: 100,
            failed_requests: 5,
            crc_errors: 2,
            timeout_requests: 1,
            exception_responses: 2,
        };
        
        assert_eq!(point.connection_state, "Connected");
        assert_eq!(point.communication_quality, 95.5);
        assert_eq!(point.avg_response_time_ms, 123.4);
        assert_eq!(point.successful_requests, 100);
        assert_eq!(point.failed_requests, 5);
    }

    #[test]
    fn test_rtu_channel_summary_creation() {
        let summary = RtuChannelSummary {
            channel_id: 1,
            connection_state: "Connected".to_string(),
            communication_quality: 95.0,
            avg_response_time_ms: 100.0,
            total_requests: 200,
            successful_requests: 190,
            failed_requests: 10,
            active_alarms: 2,
        };
        
        assert_eq!(summary.channel_id, 1);
        assert_eq!(summary.connection_state, "Connected");
        assert_eq!(summary.communication_quality, 95.0);
        assert_eq!(summary.total_requests, 200);
        assert_eq!(summary.active_alarms, 2);
    }

    #[test]
    fn test_rtu_monitor_report_creation() {
        let status = RtuMonitorStatus {
            monitored_channels: 3,
            online_channels: 2,
            offline_channels: 1,
            active_alarms: 5,
            overall_communication_quality: 85.0,
            last_update: SystemTime::now(),
        };
        
        let report = RtuMonitorReport {
            generated_at: SystemTime::now(),
            overall_status: status,
            channel_summaries: Vec::new(),
            active_alarms: Vec::new(),
        };
        
        assert_eq!(report.overall_status.monitored_channels, 3);
        assert_eq!(report.overall_status.online_channels, 2);
        assert_eq!(report.overall_status.offline_channels, 1);
        assert_eq!(report.overall_status.active_alarms, 5);
        assert!(report.channel_summaries.is_empty());
        assert!(report.active_alarms.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_clients_management() {
        let monitor = create_test_rtu_monitor();
        
        // Add multiple clients
        for i in 1..=3 {
            let client = Arc::new(Mutex::new(create_test_modbus_client()));
            monitor.add_client(i, client).await;
        }
        
        // Verify all clients are added
        {
            let clients = monitor.clients.read().await;
            assert_eq!(clients.len(), 3);
            for i in 1..=3 {
                assert!(clients.contains_key(&i));
            }
        }
        
        // Remove one client
        monitor.remove_client(2).await;
        
        // Verify client was removed
        {
            let clients = monitor.clients.read().await;
            assert_eq!(clients.len(), 2);
            assert!(clients.contains_key(&1));
            assert!(!clients.contains_key(&2));
            assert!(clients.contains_key(&3));
        }
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let monitor = Arc::new(create_test_rtu_monitor());
        let mut handles = Vec::new();
        
        // Spawn multiple tasks that access the monitor concurrently
        for i in 0..10 {
            let monitor_clone = Arc::clone(&monitor);
            let handle = tokio::spawn(async move {
                let client = Arc::new(Mutex::new(create_test_modbus_client()));
                monitor_clone.add_client(i, client).await;
                let _status = monitor_clone.get_status().await;
                let _alarms = monitor_clone.get_active_alarms().await;
                monitor_clone.remove_client(i).await;
            });
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Verify no clients remain
        {
            let clients = monitor.clients.read().await;
            assert_eq!(clients.len(), 0);
        }
    }

    #[test]
    fn test_serialization_deserialization() {
        let alarm = RtuAlarm {
            id: "test_alarm".to_string(),
            alarm_type: RtuAlarmType::ConnectionLost,
            severity: RtuAlarmSeverity::Error,
            message: "Test alarm message".to_string(),
            timestamp: SystemTime::now(),
            acknowledged: false,
            channel_id: 1,
        };
        
        // Test JSON serialization
        let json_str = serde_json::to_string(&alarm).unwrap();
        let deserialized_alarm: RtuAlarm = serde_json::from_str(&json_str).unwrap();
        
        assert_eq!(alarm.id, deserialized_alarm.id);
        assert_eq!(alarm.message, deserialized_alarm.message);
        assert_eq!(alarm.acknowledged, deserialized_alarm.acknowledged);
        assert_eq!(alarm.channel_id, deserialized_alarm.channel_id);
    }
} 