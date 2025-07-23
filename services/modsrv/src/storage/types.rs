//! modsrv存储类型定义
//!
//! 定义模型服务的核心数据结构

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use voltage_libs::types::StandardFloat;

/// 模型监视数据类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MonitorType {
    /// 来自comsrv的实时测量值 (YC)
    Measurement,
    /// 来自comsrv的实时信号值 (YX)
    Signal,
    /// 模型计算输出值
    ModelOutput,
    /// 中间计算值
    Intermediate,
}

impl MonitorType {
    /// 转换为Redis存储的类型缩写
    pub fn to_redis(&self) -> &'static str {
        match self {
            MonitorType::Measurement => "mv:m", // monitor value: measurement
            MonitorType::Signal => "mv:s",      // monitor value: signal
            MonitorType::ModelOutput => "mo",   // model output
            MonitorType::Intermediate => "mi",  // model intermediate
        }
    }
}

/// 控制命令类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ControlType {
    /// 遥控命令 (YK)
    RemoteControl,
    /// 遥调命令 (YT)
    RemoteAdjust,
}

impl ControlType {
    /// 转换为Redis存储的类型缩写
    pub fn to_redis(&self) -> &'static str {
        match self {
            ControlType::RemoteControl => "cc:c", // control command: control
            ControlType::RemoteAdjust => "cc:a",  // control command: adjust
        }
    }
}

/// 监视值数据（modsrv格式：仅存储计算值，6位小数精度，不含时间戳）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorValue {
    /// 数值（标准化6位小数）
    pub value: StandardFloat,
}

impl MonitorValue {
    /// 创建新的监视值
    pub fn new(value: f64) -> Self {
        Self {
            value: StandardFloat::new(value),
        }
    }

    /// 从Redis字符串解析（格式：仅数值，6位小数精度）
    pub fn from_redis(data: &str) -> Option<Self> {
        if let Ok(std_float) = StandardFloat::from_redis(data) {
            Some(Self { value: std_float })
        } else {
            None
        }
    }

    /// 转换为Redis字符串（标准化6位小数格式）
    pub fn to_redis(&self) -> String {
        self.value.to_redis()
    }

    /// 获取原始数值
    pub fn raw_value(&self) -> f64 {
        self.value.value()
    }
}

/// 控制命令状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandStatus {
    /// 等待执行
    Pending,
    /// 正在执行
    Executing,
    /// 执行成功
    Success,
    /// 执行失败
    Failed,
    /// 已取消
    Cancelled,
    /// 超时
    Timeout,
}

impl CommandStatus {
    pub fn to_str(&self) -> &'static str {
        match self {
            CommandStatus::Pending => "pending",
            CommandStatus::Executing => "executing",
            CommandStatus::Success => "success",
            CommandStatus::Failed => "failed",
            CommandStatus::Cancelled => "cancelled",
            CommandStatus::Timeout => "timeout",
        }
    }
}

impl std::str::FromStr for CommandStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(CommandStatus::Pending),
            "executing" => Ok(CommandStatus::Executing),
            "success" => Ok(CommandStatus::Success),
            "failed" => Ok(CommandStatus::Failed),
            "cancelled" => Ok(CommandStatus::Cancelled),
            "timeout" => Ok(CommandStatus::Timeout),
            _ => Err(format!("Unknown command status: {}", s)),
        }
    }
}

/// 控制命令数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCommand {
    /// 命令ID
    pub id: String,
    /// 目标通道ID
    pub channel_id: u16,
    /// 点位ID
    pub point_id: u32,
    /// 命令类型
    pub command_type: ControlType,
    /// 命令值（对于YK为0/1，对于YT为具体数值）
    pub value: StandardFloat,
    /// 命令状态
    pub status: CommandStatus,
    /// 创建时间戳
    pub created_at: i64,
    /// 更新时间戳
    pub updated_at: i64,
    /// 执行结果消息
    pub message: Option<String>,
    /// 命令来源（模型ID）
    pub source_model: String,
}

impl ControlCommand {
    /// 创建新的控制命令
    pub fn new(
        channel_id: u16,
        point_id: u32,
        command_type: ControlType,
        value: f64,
        source_model: String,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            channel_id,
            point_id,
            command_type,
            value: StandardFloat::new(value),
            status: CommandStatus::Pending,
            created_at: now,
            updated_at: now,
            message: None,
            source_model,
        }
    }

    /// 转换为Redis Hash存储格式
    pub fn to_hash(&self) -> HashMap<String, String> {
        let mut hash = HashMap::new();
        hash.insert("id".to_string(), self.id.clone());
        hash.insert("channel_id".to_string(), self.channel_id.to_string());
        hash.insert("point_id".to_string(), self.point_id.to_string());
        hash.insert(
            "command_type".to_string(),
            self.command_type.to_redis().to_string(),
        );
        hash.insert("value".to_string(), self.value.to_redis());
        hash.insert("status".to_string(), self.status.to_str().to_string());
        hash.insert("created_at".to_string(), self.created_at.to_string());
        hash.insert("updated_at".to_string(), self.updated_at.to_string());
        if let Some(msg) = &self.message {
            hash.insert("message".to_string(), msg.clone());
        }
        hash.insert("source_model".to_string(), self.source_model.clone());
        hash
    }

    /// 从Redis Hash解析
    pub fn from_hash(hash: HashMap<String, String>) -> Option<Self> {
        Some(Self {
            id: hash.get("id")?.clone(),
            channel_id: hash.get("channel_id")?.parse().ok()?,
            point_id: hash.get("point_id")?.parse().ok()?,
            command_type: match hash.get("command_type")?.as_str() {
                "cc:c" => ControlType::RemoteControl,
                "cc:a" => ControlType::RemoteAdjust,
                _ => return None,
            },
            value: StandardFloat::from_redis(hash.get("value")?).ok()?,
            status: hash.get("status")?.parse().ok()?,
            created_at: hash.get("created_at")?.parse().ok()?,
            updated_at: hash.get("updated_at")?.parse().ok()?,
            message: hash.get("message").cloned(),
            source_model: hash.get("source_model")?.clone(),
        })
    }
}

/// 模型输出数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelOutput {
    /// 模型ID
    pub model_id: String,
    /// 输出数据（键值对）
    pub outputs: HashMap<String, StandardFloat>,
    /// 时间戳
    pub timestamp: i64,
    /// 执行耗时（毫秒）
    pub execution_time_ms: u64,
}

/// 批量监视更新
#[derive(Debug, Clone)]
pub struct MonitorUpdate {
    pub model_id: String,
    pub monitor_type: MonitorType,
    pub field_name: String, // 有意义的字段名替代point_id
    pub value: MonitorValue,
}

/// 批量监视查询键
#[derive(Debug, Clone)]
pub struct MonitorKey {
    pub model_id: String,
    pub monitor_type: MonitorType,
    pub field_name: String, // 有意义的字段名替代point_id
}

/// 生成监视值Redis Hash键 (符合规范：modsrv:{modelname}:{type})
pub fn make_monitor_key(model_id: &str, monitor_type: &MonitorType) -> String {
    let type_str = match monitor_type {
        MonitorType::Measurement | MonitorType::ModelOutput | MonitorType::Intermediate => {
            "measurement"
        }
        MonitorType::Signal => "signal",
    };
    format!("modsrv:{}:{}", model_id, type_str)
}

/// 生成控制命令Redis键
pub fn make_control_key(command_id: &str) -> String {
    format!("cmd:{}", command_id)
}

/// 生成控制命令列表键（按模型）
pub fn make_control_list_key(model_id: &str) -> String {
    format!("cmd:list:{}", model_id)
}

/// 生成模型输出Redis键
pub fn make_model_output_key(model_id: &str) -> String {
    format!("modsrv:{}:output", model_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_value() {
        let mv = MonitorValue::new(25.6);
        assert_eq!(mv.raw_value(), 25.6);

        let redis_str = mv.to_redis();
        assert_eq!(redis_str, "25.600000");

        let parsed = MonitorValue::from_redis(&redis_str).unwrap();
        assert_eq!(parsed.raw_value(), 25.6);
    }

    #[test]
    fn test_control_command() {
        let cmd = ControlCommand::new(
            1001,
            20001,
            ControlType::RemoteControl,
            1.0,
            "model_123".to_string(),
        );
        assert_eq!(cmd.status, CommandStatus::Pending);

        let hash = cmd.to_hash();
        let parsed = ControlCommand::from_hash(hash).unwrap();
        assert_eq!(parsed.channel_id, 1001);
        assert_eq!(parsed.point_id, 20001);
    }

    #[test]
    fn test_make_keys() {
        assert_eq!(
            make_monitor_key("model_123", &MonitorType::Measurement),
            "modsrv:model_123:measurement"
        );
        assert_eq!(
            make_monitor_key("model_123", &MonitorType::Signal),
            "modsrv:model_123:signal"
        );
        assert_eq!(make_control_key("cmd_123"), "cmd:cmd_123");
    }
}
