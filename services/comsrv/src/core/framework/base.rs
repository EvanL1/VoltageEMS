//! Default Protocol Implementation
//!
//! This module contains the default reference implementation of the ComBase trait.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

use crate::core::config::ChannelConfig;
use crate::core::framework::combase_storage::{ComBaseStorage, DefaultComBaseStorage};
use crate::core::framework::manager::OptimizedPointManager;
use crate::core::framework::traits::{ComBase, ProtocolLogger};
use crate::core::framework::types::{
    ChannelCommand, ChannelStatus, PointData, PointValueType, RemoteOperationRequest,
    RemoteOperationResponse, TelemetryType,
};
use crate::plugins::plugin_storage::PluginPointUpdate;
use crate::utils::error::{ComSrvError, Result};

/// Packet parsing result
#[derive(Debug, Clone)]
pub struct PacketParseResult {
    pub success: bool,
    pub protocol: String,
    pub direction: String,
    pub hex_data: String,
    pub parsed_data: Option<String>,
    pub error_message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl PacketParseResult {
    pub fn success(protocol: &str, direction: &str, hex_data: &str, parsed_data: &str) -> Self {
        Self {
            success: true,
            protocol: protocol.to_string(),
            direction: direction.to_string(),
            hex_data: hex_data.to_string(),
            parsed_data: Some(parsed_data.to_string()),
            error_message: None,
            timestamp: Utc::now(),
        }
    }

    pub fn failure(protocol: &str, direction: &str, hex_data: &str, error: &str) -> Self {
        Self {
            success: false,
            protocol: protocol.to_string(),
            direction: direction.to_string(),
            hex_data: hex_data.to_string(),
            parsed_data: None,
            error_message: Some(error.to_string()),
            timestamp: Utc::now(),
        }
    }
}

/// Default implementation of ComBase trait
///
/// 现在集成了存储、pub/sub发布和命令订阅功能
pub struct DefaultProtocol {
    /// Service name
    name: String,
    /// Protocol type
    protocol_type: String,
    /// Channel configuration
    config: ChannelConfig,
    /// Running state
    running: Arc<RwLock<bool>>,
    /// Channel status
    status: Arc<RwLock<ChannelStatus>>,
    /// Protocol logger
    logger: Option<Arc<dyn ProtocolLogger>>,
    /// 统一存储接口（包含pub/sub发布功能）
    storage: Option<Arc<dyn ComBaseStorage>>,
    /// 命令接收器
    command_rx: Option<tokio::sync::mpsc::Receiver<ChannelCommand>>,
    /// 点位管理器
    point_manager: Option<OptimizedPointManager>,
}

impl std::fmt::Debug for DefaultProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultProtocol")
            .field("name", &self.name)
            .field("protocol_type", &self.protocol_type)
            .field("running", &self.running)
            .field("logger", &"<logger>")
            .finish()
    }
}

impl DefaultProtocol {
    /// Create a new default protocol implementation
    pub fn new(name: &str, protocol_type: &str, config: ChannelConfig) -> Self {
        let channel_id = config.id.to_string();
        Self {
            name: name.to_string(),
            protocol_type: protocol_type.to_string(),
            config,
            running: Arc::new(RwLock::new(false)),
            status: Arc::new(RwLock::new(ChannelStatus::new(&channel_id))),
            logger: None,
            storage: None,
            command_rx: None,
            point_manager: None,
        }
    }

    /// Set command receiver
    pub fn set_command_receiver(&mut self, rx: tokio::sync::mpsc::Receiver<ChannelCommand>) {
        self.command_rx = Some(rx);
    }

    /// 使用默认Redis存储创建实例
    pub async fn with_default_storage(
        name: &str,
        protocol_type: &str,
        config: ChannelConfig,
        redis_url: Option<&str>,
    ) -> Result<Self> {
        let mut protocol = Self::new(name, protocol_type, config);
        protocol.init_storage(redis_url).await?;
        Ok(protocol)
    }

    /// 初始化存储接口
    pub async fn init_storage(&mut self, redis_url: Option<&str>) -> Result<()> {
        let storage: Arc<dyn ComBaseStorage> = if let Some(url) = redis_url {
            Arc::new(DefaultComBaseStorage::new(url).await?)
        } else {
            Arc::new(DefaultComBaseStorage::from_env().await?)
        };
        self.storage = Some(storage);
        Ok(())
    }

    /// Set protocol logger
    pub fn set_logger(&mut self, logger: Arc<dyn ProtocolLogger>) {
        self.logger = Some(logger);
    }

    /// 设置点位管理器
    pub fn set_point_manager(&mut self, point_manager: OptimizedPointManager) {
        self.point_manager = Some(point_manager);
    }

    /// 获取存储接口引用
    pub fn storage(&self) -> Option<&Arc<dyn ComBaseStorage>> {
        self.storage.as_ref()
    }

    /// 处理接收到的命令 - 现在通过trait实现

    /// Measure execution time for an async operation
    pub async fn measure_execution<F, Fut, R>(&self, operation: F) -> (R, Duration)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        let start = Instant::now();
        let result = operation().await;
        let duration = start.elapsed();
        (result, duration)
    }

    /// Measure execution time and return result
    pub async fn measure_result_execution<F, Fut, R>(&self, operation: F) -> Result<(R, Duration)>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<R>>,
    {
        let start = Instant::now();
        let result = operation().await?;
        let duration = start.elapsed();
        Ok((result, duration))
    }

    /// Set error status
    pub async fn set_error(&mut self, error_message: &str) {
        let mut status = self.status.write();
        status.last_error = error_message.to_string();
        status.last_update_time = Utc::now();
    }
}

#[async_trait]
impl ComBase for DefaultProtocol {
    fn name(&self) -> &str {
        &self.name
    }

    fn channel_id(&self) -> String {
        self.config.id.to_string()
    }

    fn protocol_type(&self) -> &str {
        &self.protocol_type
    }

    fn get_parameters(&self) -> HashMap<String, String> {
        // Extract parameters from config
        let mut params = HashMap::new();
        params.insert("name".to_string(), self.name.clone());
        params.insert("protocol".to_string(), self.protocol_type.clone());
        params.insert("channel_id".to_string(), self.config.id.to_string());
        params
    }

    async fn is_running(&self) -> bool {
        *self.running.read()
    }

    async fn start(&mut self) -> Result<()> {
        info!("Starting communication service: {}", self.name);

        // 检查存储连接（如果已配置）
        if let Some(ref storage) = self.storage {
            if !storage.is_connected().await {
                return Err(ComSrvError::Storage("Storage not connected".to_string()));
            }
        }

        // 命令订阅现在通过外部的CommandSubscriber处理，不在这里启动

        *self.running.write() = true;

        // Update status
        let mut status = self.status.write();
        status.connected = true;
        status.last_update_time = Utc::now();
        status.last_error.clear();

        info!("Communication service started: {}", self.name);
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        info!("Stopping communication service: {}", self.name);

        // 命令订阅现在通过外部处理，不在这里停止

        *self.running.write() = false;

        // Update status
        let mut status = self.status.write();
        status.connected = false;
        status.last_update_time = Utc::now();

        info!("Communication service stopped: {}", self.name);
        Ok(())
    }

    async fn status(&self) -> ChannelStatus {
        self.status.read().clone()
    }

    async fn update_status(&mut self, new_status: ChannelStatus) -> Result<()> {
        *self.status.write() = new_status;
        Ok(())
    }

    async fn get_all_points(&self) -> Vec<PointData> {
        // Base implementation returns empty list
        // Protocol-specific implementations should override this
        Vec::new()
    }

    async fn read_point(&self, _point_id: &str) -> Result<PointData> {
        Err(ComSrvError::InvalidOperation(
            "Base implementation does not support point reading".to_string(),
        ))
    }

    async fn write_point(&mut self, _point_id: &str, _value: &str) -> Result<()> {
        Err(ComSrvError::InvalidOperation(
            "Base implementation does not support point writing".to_string(),
        ))
    }

    async fn get_diagnostics(&self) -> HashMap<String, String> {
        let mut diagnostics = HashMap::new();
        diagnostics.insert("service_name".to_string(), self.name.to_string());
        diagnostics.insert("protocol_type".to_string(), self.protocol_type.to_string());
        diagnostics.insert("running".to_string(), self.is_running().await.to_string());

        // 先从status中获取需要的值，然后释放锁
        let (connected, response_time, last_error, last_update) = {
            let status = self.status.read();
            (
                status.is_connected(),
                status.response_time(),
                status.error_ref().to_string(),
                status.last_update(),
            )
        };

        diagnostics.insert("connected".to_string(), connected.to_string());
        diagnostics.insert("last_response_time".to_string(), response_time.to_string());
        diagnostics.insert("last_error".to_string(), last_error);
        diagnostics.insert("last_update".to_string(), last_update.to_rfc3339());

        // 添加存储和命令订阅状态
        if let Some(ref storage) = self.storage {
            diagnostics.insert(
                "storage_connected".to_string(),
                storage.is_connected().await.to_string(),
            );
        }
        // 命令订阅状态现在由外部管理
        diagnostics.insert(
            "command_subscription".to_string(),
            self.command_rx.is_some().to_string(),
        );

        diagnostics
    }

    async fn get_point_manager(&self) -> Option<OptimizedPointManager> {
        self.point_manager.clone()
    }

    // ========== 四遥功能实现 ==========

    async fn remote_measurement(
        &self,
        point_names: &[String],
    ) -> Result<Vec<(String, PointValueType)>> {
        // 默认实现：委托给子类
        let _ = point_names;
        Ok(Vec::new())
    }

    async fn remote_signaling(
        &self,
        point_names: &[String],
    ) -> Result<Vec<(String, PointValueType)>> {
        // 默认实现：委托给子类
        let _ = point_names;
        Ok(Vec::new())
    }

    async fn remote_control(
        &mut self,
        request: RemoteOperationRequest,
    ) -> Result<RemoteOperationResponse> {
        // 默认实现：委托给子类
        let _ = request;
        Err(ComSrvError::NotImplemented(
            "Remote control not implemented".to_string(),
        ))
    }

    async fn remote_regulation(
        &mut self,
        request: RemoteOperationRequest,
    ) -> Result<RemoteOperationResponse> {
        // 默认实现：委托给子类
        let _ = request;
        Err(ComSrvError::NotImplemented(
            "Remote regulation not implemented".to_string(),
        ))
    }

    // ========== 存储接口实现 ==========

    async fn store_point_data(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        if let Some(ref storage) = self.storage {
            storage
                .store_point(channel_id, telemetry_type, point_id, value)
                .await
        } else {
            Err(ComSrvError::Storage("Storage not initialized".to_string()))
        }
    }

    async fn store_batch_data(&self, updates: Vec<PluginPointUpdate>) -> Result<()> {
        if let Some(ref storage) = self.storage {
            storage.store_batch(updates).await
        } else {
            Err(ComSrvError::Storage("Storage not initialized".to_string()))
        }
    }

    // ========== 命令处理 ==========

    /// Set command receiver for handling incoming commands
    async fn set_command_receiver(
        &mut self,
        rx: tokio::sync::mpsc::Receiver<ChannelCommand>,
    ) -> Result<()> {
        self.command_rx = Some(rx);
        Ok(())
    }

    /// Handle channel command - trait implementation
    async fn handle_channel_command(&mut self, command: ChannelCommand) -> Result<()> {
        // 处理命令逻辑（从之前定义的内部方法移过来）
        match command {
            ChannelCommand::Control {
                command_id,
                point_id,
                value,
                ..
            } => {
                let request = RemoteOperationRequest {
                    operation_id: command_id,
                    point_name: point_id.to_string(),
                    operation_type: crate::core::framework::types::RemoteOperationType::Control {
                        value: value != 0.0,
                    },
                };
                self.remote_control(request).await?;
            }
            ChannelCommand::Adjustment {
                command_id,
                point_id,
                value,
                ..
            } => {
                let request = RemoteOperationRequest {
                    operation_id: command_id,
                    point_name: point_id.to_string(),
                    operation_type:
                        crate::core::framework::types::RemoteOperationType::Regulation { value },
                };
                self.remote_regulation(request).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::types::ChannelLoggingConfig;
    use crate::core::config::ChannelConfig;

    fn create_test_config() -> ChannelConfig {
        ChannelConfig {
            id: 1,
            name: "test_channel".to_string(),
            description: Some("Test channel".to_string()),
            protocol: "Virtual".to_string(),
            parameters: HashMap::new(),
            logging: ChannelLoggingConfig::default(),
            table_config: None,
            points: Vec::new(),
            combined_points: Vec::new(),
        }
    }

    #[tokio::test]
    async fn test_combase_impl_creation() {
        let config = create_test_config();
        let service = DefaultProtocol::new("Test Service", "test_protocol", config);

        assert_eq!(service.name(), "Test Service");
        assert_eq!(service.protocol_type(), "test_protocol");
        assert_eq!(service.channel_id(), "1");
        assert!(!service.is_running().await);
    }

    #[tokio::test]
    async fn test_combase_impl_lifecycle() {
        let config = create_test_config();
        let mut service = DefaultProtocol::new("Test Service", "test_protocol", config);

        // Test start
        service.start().await.unwrap();
        assert!(service.is_running().await);

        let status = service.status().await;
        assert!(status.connected);

        // Test stop
        service.stop().await.unwrap();
        assert!(!service.is_running().await);

        let status = service.status().await;
        assert!(!status.connected);
    }
}
