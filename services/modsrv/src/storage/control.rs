//! 控制模块
//!
//! 提供设备模型的控制命令发送功能

use super::rtdb::ModelStorage;
use crate::error::{ModelSrvError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

/// 控制命令类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlType {
    /// 遥控命令 (YK)
    RemoteControl,
    /// 遥调命令 (YT)  
    RemoteAdjustment,
}

/// 控制命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCommand {
    /// 命令ID
    pub id: String,
    /// 通道ID
    pub channel_id: u16,
    /// 点位ID
    pub point_id: u32,
    /// 控制类型
    pub control_type: ControlType,
    /// 控制值
    pub value: f64,
    /// 来源模型
    pub source_model: Option<String>,
    /// 创建时间戳
    pub timestamp: i64,
}

impl ControlCommand {
    /// 创建新的控制命令
    pub fn new(
        channel_id: u16,
        point_id: u32,
        control_type: ControlType,
        value: f64,
        source_model: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            channel_id,
            point_id,
            control_type,
            value,
            source_model,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    /// 获取点位类型字符串
    pub fn get_point_type(&self) -> &str {
        match self.control_type {
            ControlType::RemoteControl => "c",    // 遥控
            ControlType::RemoteAdjustment => "a", // 遥调
        }
    }
}

/// 控制管理器
pub struct ControlManager {
    storage: ModelStorage,
}

impl ControlManager {
    /// 创建新的控制管理器
    pub async fn new(redis_url: &str) -> Result<Self> {
        let storage = ModelStorage::new(redis_url).await?;
        Ok(Self { storage })
    }

    /// 从环境变量创建
    pub async fn from_env() -> Result<Self> {
        let storage = ModelStorage::from_env().await?;
        Ok(Self { storage })
    }

    /// 发送遥控命令 (YK)
    pub async fn send_remote_control(
        &self,
        channel_id: u16,
        point_id: u32,
        value: bool,
        source_model: Option<String>,
    ) -> Result<String> {
        let command = ControlCommand::new(
            channel_id,
            point_id,
            ControlType::RemoteControl,
            if value { 1.0 } else { 0.0 },
            source_model,
        );

        self.storage
            .send_control_command(
                command.channel_id,
                command.get_point_type(),
                command.point_id,
                command.value,
            )
            .await?;

        info!(
            "Sent remote control command {} to channel {} point {} value {}",
            command.id, channel_id, point_id, value
        );

        Ok(command.id)
    }

    /// 发送遥调命令 (YT)
    pub async fn send_remote_adjustment(
        &self,
        channel_id: u16,
        point_id: u32,
        value: f64,
        source_model: Option<String>,
    ) -> Result<String> {
        let command = ControlCommand::new(
            channel_id,
            point_id,
            ControlType::RemoteAdjustment,
            value,
            source_model,
        );

        self.storage
            .send_control_command(
                command.channel_id,
                command.get_point_type(),
                command.point_id,
                command.value,
            )
            .await?;

        info!(
            "Sent remote adjustment command {} to channel {} point {} value {}",
            command.id, channel_id, point_id, value
        );

        Ok(command.id)
    }

    /// 批量发送控制命令
    pub async fn send_batch_commands(&self, commands: Vec<ControlCommand>) -> Result<Vec<String>> {
        let mut command_ids = Vec::new();

        for command in commands {
            match self
                .storage
                .send_control_command(
                    command.channel_id,
                    command.get_point_type(),
                    command.point_id,
                    command.value,
                )
                .await
            {
                Ok(_) => {
                    info!("Sent batch command {}", command.id);
                    command_ids.push(command.id);
                }
                Err(e) => {
                    warn!("Failed to send batch command {}: {}", command.id, e);
                    return Err(e);
                }
            }
        }

        info!("Successfully sent {} batch commands", command_ids.len());
        Ok(command_ids)
    }

    /// 基于设备模型发送控制命令
    pub async fn send_model_control(
        &self,
        model_id: &str,
        command_name: &str,
        params: HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        // 这里可以根据设备模型定义解析命令参数
        // 现在先提供一个简化实现

        // 从参数中提取基本信息
        let channel_id = params
            .get("channel_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ModelSrvError::InvalidParameter("channel_id is required".to_string()))?
            as u16;

        let point_id = params
            .get("point_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ModelSrvError::InvalidParameter("point_id is required".to_string()))?
            as u32;

        let value = params
            .get("value")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| ModelSrvError::InvalidParameter("value is required".to_string()))?;

        // 根据命令名称确定控制类型
        let control_type = if command_name.contains("control") || command_name.contains("switch") {
            ControlType::RemoteControl
        } else if command_name.contains("adjust") || command_name.contains("set") {
            ControlType::RemoteAdjustment
        } else {
            // 默认为遥调
            ControlType::RemoteAdjustment
        };

        let command = ControlCommand::new(
            channel_id,
            point_id,
            control_type,
            value,
            Some(model_id.to_string()),
        );

        self.storage
            .send_control_command(
                command.channel_id,
                command.get_point_type(),
                command.point_id,
                command.value,
            )
            .await?;

        info!(
            "Sent model control command {} for model {} command {}",
            command.id, model_id, command_name
        );

        Ok(command.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_command_creation() {
        let command = ControlCommand::new(
            1001,
            30001,
            ControlType::RemoteControl,
            1.0,
            Some("test_model".to_string()),
        );

        assert_eq!(command.channel_id, 1001);
        assert_eq!(command.point_id, 30001);
        assert_eq!(command.value, 1.0);
        assert_eq!(command.get_point_type(), "c");
        assert!(!command.id.is_empty());
    }

    #[test]
    fn test_control_types() {
        let control_cmd = ControlCommand::new(1001, 30001, ControlType::RemoteControl, 1.0, None);
        assert_eq!(control_cmd.get_point_type(), "c");

        let adjust_cmd =
            ControlCommand::new(1001, 30002, ControlType::RemoteAdjustment, 25.5, None);
        assert_eq!(adjust_cmd.get_point_type(), "a");
    }
}
