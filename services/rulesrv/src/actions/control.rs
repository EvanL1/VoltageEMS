use crate::error::{Result, RulesrvError};
use async_trait::async_trait;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

use crate::actions::ActionHandler;

/// 控制操作类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ControlOperationType {
    /// 启动操作
    Start,
    /// 停止操作
    Stop,
    /// 暂停操作
    Pause,
    /// 恢复操作
    Resume,
    /// 重置操作
    Reset,
    /// 调节参数操作
    Adjust,
    /// 自定义操作
    Custom(String),
}

/// 控制目标类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ControlTargetType {
    /// 设备控制
    Device,
    /// 系统控制
    System,
    /// 模型控制
    Model,
    /// 自定义控制目标
    Custom(String),
}

/// 控制命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCommand {
    /// 命令ID
    pub id: String,
    /// 通道ID
    pub channel_id: u16,
    /// 点位类型
    pub point_type: String,
    /// 点位ID
    pub point_id: u32,
    /// 控制值
    pub value: Value,
    /// 时间戳
    pub timestamp: i64,
}

/// 操作状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationStatus {
    Pending,
    Executing,
    Executed,
    Failed,
    Cancelled,
}

/// 操作状态记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStatusRecord {
    pub operation_id: String,
    pub status: OperationStatus,
    pub timestamp: i64,
    pub message: Option<String>,
}

/// 控制动作处理器
pub struct ControlActionHandler {
    redis_client: redis::Client,
    operation_status_cache: Arc<RwLock<HashMap<String, OperationStatusRecord>>>,
}

impl ControlActionHandler {
    /// 创建新的控制动作处理器
    #[allow(dead_code)]
    pub fn new(redis_url: &str) -> Result<Self> {
        let redis_client = redis::Client::open(redis_url)?;

        Ok(Self {
            redis_client,
            operation_status_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 发送设备控制命令
    async fn send_device_control(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: &Value,
    ) -> Result<String> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let command_id = format!("cmd:{}", Uuid::new_v4());
        let command = ControlCommand {
            id: command_id.clone(),
            channel_id,
            point_type: point_type.to_string(),
            point_id,
            value: value.clone(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        // 发布到 comsrv 的控制命令通道
        let channel = format!("cmd:{}:{}", channel_id, point_type);
        let command_json = serde_json::to_string(&command)?;
        let _: () = conn.publish(&channel, &command_json).await?;

        info!(
            "Published control command {} to channel {}",
            command_id, channel
        );

        // 记录命令状态
        self.update_operation_status(&command_id, OperationStatus::Executing, None)
            .await?;

        Ok(command_id)
    }

    /// 发送模型控制命令
    async fn send_model_control(&self, model_id: &str, action: &str) -> Result<String> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let command_id = format!("model_cmd:{}", Uuid::new_v4());

        // 发布到 modsrv 的控制通道
        let channel = "modsrv:control";
        let command = serde_json::json!({
            "id": command_id,
            "model_id": model_id,
            "action": action,
            "timestamp": chrono::Utc::now().timestamp_millis(),
        });

        let _: () = conn.publish(channel, command.to_string()).await?;

        info!(
            "Published model control command {} for model {}",
            command_id, model_id
        );

        // 记录命令状态
        self.update_operation_status(&command_id, OperationStatus::Executing, None)
            .await?;

        Ok(command_id)
    }

    /// 更新操作状态
    async fn update_operation_status(
        &self,
        operation_id: &str,
        status: OperationStatus,
        message: Option<String>,
    ) -> Result<()> {
        let status_record = OperationStatusRecord {
            operation_id: operation_id.to_string(),
            status: status.clone(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            message,
        };

        // 更新缓存
        let mut cache = self.operation_status_cache.write().await;
        cache.insert(operation_id.to_string(), status_record.clone());

        // 更新 Redis
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        let status_key = format!("rulesrv:operation:status:{}", operation_id);
        let status_json = serde_json::to_string(&status_record)?;
        let _: () = conn.set_ex(&status_key, &status_json, 86400).await?; // 24小时过期

        Ok(())
    }

    /// 获取操作状态
    #[allow(dead_code)]
    pub async fn get_operation_status(
        &self,
        operation_id: &str,
    ) -> Result<Option<OperationStatusRecord>> {
        // 先检查缓存
        let cache = self.operation_status_cache.read().await;
        if let Some(status) = cache.get(operation_id) {
            return Ok(Some(status.clone()));
        }
        drop(cache);

        // 从 Redis 获取
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        let status_key = format!("rulesrv:operation:status:{}", operation_id);
        let status_json: Option<String> = conn.get(&status_key).await?;

        if let Some(json) = status_json {
            let status: OperationStatusRecord = serde_json::from_str(&json)?;

            // 更新缓存
            let mut cache = self.operation_status_cache.write().await;
            cache.insert(operation_id.to_string(), status.clone());

            Ok(Some(status))
        } else {
            Ok(None)
        }
    }
}

#[async_trait]
impl ActionHandler for ControlActionHandler {
    fn can_handle(&self, action_type: &str) -> bool {
        matches!(action_type, "control" | "device_control" | "model_control")
    }

    fn name(&self) -> &str {
        "ControlActionHandler"
    }

    fn handler_type(&self) -> String {
        "control".to_string()
    }

    async fn execute_action(&self, action_type: &str, config: &Value) -> Result<String> {
        match action_type {
            "control" => {
                // 通用控制动作
                if let Some(_control_id) = config.get("control_id").and_then(|v| v.as_str()) {
                    // 基于预定义的控制操作执行
                    let target_type = config
                        .get("target_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("device");

                    match target_type {
                        "device" => {
                            let channel_id = config
                                .get("channel_id")
                                .and_then(|v| v.as_u64())
                                .ok_or_else(|| {
                                    RulesrvError::ActionExecutionError(
                                        "Missing channel_id".to_string(),
                                    )
                                })? as u16;

                            let point_type = config
                                .get("point_type")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    RulesrvError::ActionExecutionError(
                                        "Missing point_type".to_string(),
                                    )
                                })?;

                            let point_id = config
                                .get("point_id")
                                .and_then(|v| v.as_u64())
                                .ok_or_else(|| {
                                    RulesrvError::ActionExecutionError(
                                        "Missing point_id".to_string(),
                                    )
                                })? as u32;

                            let value = config.get("value").ok_or_else(|| {
                                RulesrvError::ActionExecutionError("Missing value".to_string())
                            })?;

                            self.send_device_control(channel_id, point_type, point_id, value)
                                .await
                        }
                        "model" => {
                            let model_id = config
                                .get("model_id")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    RulesrvError::ActionExecutionError(
                                        "Missing model_id".to_string(),
                                    )
                                })?;

                            let action =
                                config
                                    .get("action")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| {
                                        RulesrvError::ActionExecutionError(
                                            "Missing action".to_string(),
                                        )
                                    })?;

                            self.send_model_control(model_id, action).await
                        }
                        _ => Err(RulesrvError::ActionExecutionError(format!(
                            "Unsupported target type: {}",
                            target_type
                        ))),
                    }
                } else {
                    Err(RulesrvError::ActionExecutionError(
                        "Missing control_id in config".to_string(),
                    ))
                }
            }
            "device_control" => {
                // 直接设备控制
                let channel_id = config
                    .get("channel_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        RulesrvError::ActionExecutionError("Missing channel_id".to_string())
                    })? as u16;

                let point_type = config
                    .get("point_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("c"); // 默认为控制点

                let point_id = config
                    .get("point_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        RulesrvError::ActionExecutionError("Missing point_id".to_string())
                    })? as u32;

                let value = config.get("value").ok_or_else(|| {
                    RulesrvError::ActionExecutionError("Missing value".to_string())
                })?;

                self.send_device_control(channel_id, point_type, point_id, value)
                    .await
            }
            "model_control" => {
                // 模型控制
                let model_id =
                    config
                        .get("model_id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            RulesrvError::ActionExecutionError("Missing model_id".to_string())
                        })?;

                let action = config
                    .get("action")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        RulesrvError::ActionExecutionError("Missing action".to_string())
                    })?;

                self.send_model_control(model_id, action).await
            }
            _ => Err(RulesrvError::ActionExecutionError(format!(
                "Unsupported action type: {}",
                action_type
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_control_action_handler() {
        // 这里应该使用 mock Redis 进行测试
        // 暂时跳过需要真实 Redis 的测试
    }

    #[test]
    fn test_operation_status_serialization() {
        let status = OperationStatusRecord {
            operation_id: "test_op_1".to_string(),
            status: OperationStatus::Executed,
            timestamp: 1234567890,
            message: Some("Success".to_string()),
        };

        let json = serde_json::to_string(&status).unwrap();
        let deserialized: OperationStatusRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(status.operation_id, deserialized.operation_id);
        assert_eq!(status.status, deserialized.status);
        assert_eq!(status.timestamp, deserialized.timestamp);
        assert_eq!(status.message, deserialized.message);
    }
}
