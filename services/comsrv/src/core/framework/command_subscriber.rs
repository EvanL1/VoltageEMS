//! 控制命令订阅器
//!
//! 负责从Redis订阅控制命令并分发给相应的通道处理

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use voltage_common::redis::async_client::RedisClient;

use crate::core::framework::traits::FourTelemetryOperations;
use crate::core::framework::types::RemoteOperationRequest;
use crate::utils::error::Result;

/// 控制命令类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandType {
    Control,
    Adjustment,
}

/// 控制命令消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCommand {
    /// 命令ID
    pub command_id: String,
    /// 通道ID
    pub channel_id: u16,
    /// 命令类型
    pub command_type: CommandType,
    /// 点位ID
    pub point_id: u32,
    /// 命令值
    pub value: f64,
    /// 时间戳
    pub timestamp: i64,
    /// 可选的元数据
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// 命令状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandStatus {
    pub command_id: String,
    pub status: String, // pending, executing, success, failed
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub timestamp: i64,
}

/// 命令订阅器配置
#[derive(Debug, Clone)]
pub struct CommandSubscriberConfig {
    pub channel_id: u16,
    pub redis_url: String,
}

/// 命令订阅器
pub struct CommandSubscriber {
    config: CommandSubscriberConfig,
    redis_client: Arc<RedisClient>,
    handler: Arc<dyn FourTelemetryOperations>,
    is_running: Arc<RwLock<bool>>,
    task_handle: Option<JoinHandle<()>>,
}

impl CommandSubscriber {
    /// 创建新的命令订阅器
    pub async fn new(
        config: CommandSubscriberConfig,
        handler: Arc<dyn FourTelemetryOperations>,
    ) -> Result<Self> {
        let redis_client = RedisClient::new(&config.redis_url).await?;

        Ok(Self {
            config,
            redis_client: Arc::new(redis_client),
            handler,
            is_running: Arc::new(RwLock::new(false)),
            task_handle: None,
        })
    }

    /// 启动订阅
    pub async fn start(&mut self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            if *running {
                warn!(
                    "Command subscriber already running for channel {}",
                    self.config.channel_id
                );
                return Ok(());
            }
            *running = true;
        }

        // 订阅控制和调节命令通道
        let control_channel = format!("cmd:{}:control", self.config.channel_id);
        let adjustment_channel = format!("cmd:{}:adjustment", self.config.channel_id);

        info!(
            "Starting command subscriber for channel {}, subscribing to: {} and {}",
            self.config.channel_id, control_channel, adjustment_channel
        );

        // 克隆必要的对象用于任务
        let redis_client = self.redis_client.clone();
        let handler = self.handler.clone();
        let is_running = self.is_running.clone();
        let channel_id = self.config.channel_id;

        // 启动订阅任务
        let task_handle = tokio::spawn(async move {
            if let Err(e) = Self::subscription_loop(
                redis_client,
                handler,
                is_running,
                channel_id,
                vec![control_channel, adjustment_channel],
            )
            .await
            {
                error!(
                    "Command subscription error for channel {}: {}",
                    channel_id, e
                );
            }
        });

        self.task_handle = Some(task_handle);
        Ok(())
    }

    /// 停止订阅
    pub async fn stop(&mut self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }

        // 等待任务结束
        if let Some(handle) = self.task_handle.take() {
            // 给任务一些时间优雅退出
            match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                Ok(Ok(())) => info!(
                    "Command subscriber stopped for channel {}",
                    self.config.channel_id
                ),
                Ok(Err(e)) => warn!("Command subscriber task error: {}", e),
                Err(_) => warn!("Command subscriber task timeout, forcing stop"),
            }
        }

        Ok(())
    }

    /// 订阅循环
    async fn subscription_loop(
        redis_client: Arc<RedisClient>,
        handler: Arc<dyn FourTelemetryOperations>,
        is_running: Arc<RwLock<bool>>,
        channel_id: u16,
        channels: Vec<String>,
    ) -> Result<()> {
        // 创建订阅
        let channel_refs: Vec<&str> = channels.iter().map(|s| s.as_str()).collect();
        let mut pubsub = redis_client.subscribe(&channel_refs).await?;

        // 订阅频道
        for channel in &channels {
            pubsub.subscribe(channel).await.map_err(|e| {
                crate::error::ComSrvError::InternalError(format!(
                    "Failed to subscribe to {}: {}",
                    channel, e
                ))
            })?;
        }

        info!(
            "Command subscription established for channel {}",
            channel_id
        );

        loop {
            // 检查是否应该停止
            if !*is_running.read().await {
                info!("Stopping command subscription for channel {}", channel_id);
                break;
            }

            // 接收消息（带超时）
            match tokio::time::timeout(
                std::time::Duration::from_secs(1),
                pubsub.on_message().next(),
            )
            .await
            {
                Ok(Some(msg)) => {
                    // 处理消息
                    if let Err(e) =
                        Self::process_message(&redis_client, &handler, channel_id, msg).await
                    {
                        error!("Failed to process command message: {}", e);
                    }
                }
                Ok(None) => {
                    warn!("Subscription closed for channel {}", channel_id);
                    break;
                }
                Err(_) => {
                    // 超时，继续循环
                    continue;
                }
            }
        }

        Ok(())
    }

    /// 处理单个消息
    async fn process_message(
        redis_client: &Arc<RedisClient>,
        handler: &Arc<dyn FourTelemetryOperations>,
        channel_id: u16,
        msg: redis::Msg,
    ) -> Result<()> {
        // 获取消息内容
        let payload: String = msg.get_payload().map_err(|e| {
            crate::error::ComSrvError::InternalError(format!(
                "Failed to get message payload: {}",
                e
            ))
        })?;

        debug!(
            "Received command message on channel {}: {}",
            channel_id, payload
        );

        // 解析命令
        let command: ControlCommand = serde_json::from_str(&payload).map_err(|e| {
            crate::error::ComSrvError::ParsingError(format!("Failed to parse command: {}", e))
        })?;

        // 确保命令是发给正确的通道
        if command.channel_id != channel_id {
            warn!(
                "Received command for wrong channel: expected {}, got {}",
                channel_id, command.channel_id
            );
            return Ok(());
        }

        // 更新命令状态为执行中
        let mut status = CommandStatus {
            command_id: command.command_id.clone(),
            status: "executing".to_string(),
            result: None,
            error: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        // 发布执行状态
        let status_key = format!("cmd_status:{}:{}", channel_id, command.command_id);
        redis_client
            .set(&status_key, &serde_json::to_string(&status)?)
            .await?;

        // 创建远程操作请求
        let operation_type = match command.command_type {
            CommandType::Control => crate::core::framework::types::RemoteOperationType::Control {
                value: command.value != 0.0, // 将非零值转换为true
            },
            CommandType::Adjustment => {
                crate::core::framework::types::RemoteOperationType::Regulation {
                    value: command.value,
                }
            }
        };

        let request = RemoteOperationRequest {
            operation_id: command.command_id.clone(),
            point_name: command.point_id.to_string(),
            operation_type,
        };

        // 执行命令
        let result = match command.command_type {
            CommandType::Control => handler.remote_control(request).await,
            CommandType::Adjustment => handler.remote_regulation(request).await,
        };

        // 更新命令状态
        match result {
            Ok(response) => {
                status.status = if response.success {
                    "success"
                } else {
                    "failed"
                }
                .to_string();
                status.result = Some(serde_json::json!({
                    "operation_id": response.operation_id,
                    "success": response.success,
                    "timestamp": response.timestamp,
                }));
                if let Some(ref error_msg) = response.error_message {
                    status.error = Some(error_msg.clone());
                }
                info!(
                    "Command {} executed on channel {} with result: {}",
                    command.command_id,
                    channel_id,
                    if response.success {
                        "success"
                    } else {
                        "failed"
                    }
                );
            }
            Err(e) => {
                status.status = "failed".to_string();
                status.error = Some(format!("{}", e));
                error!(
                    "Command {} failed on channel {}: {}",
                    command.command_id, channel_id, e
                );
            }
        }

        status.timestamp = chrono::Utc::now().timestamp_millis();

        // 发布最终状态
        redis_client
            .set(&status_key, &serde_json::to_string(&status)?)
            .await?;

        // 设置状态键的过期时间（24小时）
        if let Err(e) = redis_client.expire(&status_key, 86400).await {
            warn!("Failed to set expiry for command status: {}", e);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::framework::types::PointValueType;

    struct MockHandler;

    #[async_trait]
    impl FourTelemetryOperations for MockHandler {
        async fn remote_measurement(
            &self,
            _point_names: &[String],
        ) -> Result<Vec<(String, PointValueType)>> {
            Ok(vec![])
        }

        async fn remote_signaling(
            &self,
            _point_names: &[String],
        ) -> Result<Vec<(String, PointValueType)>> {
            Ok(vec![])
        }

        async fn remote_control(
            &self,
            request: RemoteOperationRequest,
        ) -> Result<RemoteOperationResponse> {
            Ok(RemoteOperationResponse {
                operation_id: request.operation_id.clone(),
                success: true,
                error_message: None,
                timestamp: chrono::Utc::now(),
            })
        }

        async fn remote_regulation(
            &self,
            request: RemoteOperationRequest,
        ) -> Result<RemoteOperationResponse> {
            Ok(RemoteOperationResponse {
                operation_id: request.operation_id.clone(),
                success: true,
                error_message: None,
                timestamp: chrono::Utc::now(),
            })
        }

        async fn get_control_points(&self) -> Vec<String> {
            vec![]
        }

        async fn get_regulation_points(&self) -> Vec<String> {
            vec![]
        }

        async fn get_measurement_points(&self) -> Vec<String> {
            vec![]
        }

        async fn get_signaling_points(&self) -> Vec<String> {
            vec![]
        }
    }

    #[tokio::test]
    async fn test_command_parsing() {
        let json = r#"{
            "command_id": "test-123",
            "channel_id": 1,
            "command_type": "control",
            "point_id": 1001,
            "value": 1.0,
            "timestamp": 1234567890,
            "metadata": {}
        }"#;

        let command: ControlCommand = serde_json::from_str(json).unwrap();
        assert_eq!(command.command_id, "test-123");
        assert_eq!(command.channel_id, 1);
        assert!(matches!(command.command_type, CommandType::Control));
        assert_eq!(command.point_id, 1001);
        assert_eq!(command.value, 1.0);
    }
}
