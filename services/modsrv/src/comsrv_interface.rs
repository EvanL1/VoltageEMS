//! modsrv与comsrv的Redis接口实现
//!
//! 提供高效的点位数据读取和控制命令发送功能

use crate::error::{ModelSrvError, Result};
use crate::redis_handler::RedisConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
// use tokio::sync::mpsc; // 保留以备后续异步实现
use tracing::{debug, error, info, warn};

/// 点位类型常量（与comsrv保持一致）
pub const TYPE_MEASUREMENT: &str = "m"; // 遥测 YC
pub const TYPE_SIGNAL: &str = "s"; // 遥信 YX
pub const TYPE_CONTROL: &str = "c"; // 遥控 YK
pub const TYPE_ADJUSTMENT: &str = "a"; // 遥调 YT

/// 点位值结构（与comsrv格式兼容）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointValue {
    pub value: f64,
    pub timestamp: i64,
}

impl PointValue {
    /// 从Redis字符串解析（格式: value:timestamp）
    pub fn from_redis(data: &str) -> Option<Self> {
        let parts: Vec<&str> = data.split(':').collect();
        if parts.len() == 2 {
            if let (Ok(value), Ok(timestamp)) = (parts[0].parse::<f64>(), parts[1].parse::<i64>()) {
                return Some(Self { value, timestamp });
            }
        }
        None
    }

    /// 转换为Redis字符串
    pub fn to_redis(&self) -> String {
        format!("{}:{}", self.value, self.timestamp)
    }
}

/// 控制命令结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCommand {
    pub channel_id: u16,
    pub point_type: String,
    pub point_id: u32,
    pub value: f64,
    pub command_id: String,
    pub timestamp: i64,
    pub source: String,
}

impl ControlCommand {
    pub fn new(channel_id: u16, point_type: &str, point_id: u32, value: f64) -> Self {
        Self {
            channel_id,
            point_type: point_type.to_string(),
            point_id,
            value,
            command_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            source: "modsrv".to_string(),
        }
    }
}

/// 命令状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandStatus {
    pub command_id: String,
    pub status: String, // pending, executing, success, failed
    pub message: Option<String>,
    pub timestamp: i64,
}

/// ComSrv接口 - 提供与comsrv的交互功能
pub struct ComSrvInterface {
    redis: RedisConnection,
    command_channel: String,
    _status_channel: String,
    _cache_ttl: Duration,
}

impl ComSrvInterface {
    /// 创建新的接口实例
    pub fn new(redis: RedisConnection) -> Self {
        Self {
            redis,
            command_channel: "comsrv:commands".to_string(),
            _status_channel: "comsrv:status".to_string(),
            _cache_ttl: Duration::from_secs(1),
        }
    }

    /// 创建带自定义配置的接口实例
    pub fn with_config(
        redis: RedisConnection,
        command_channel: String,
        status_channel: String,
        cache_ttl: Duration,
    ) -> Self {
        Self {
            redis,
            command_channel,
            _status_channel: status_channel,
            _cache_ttl: cache_ttl,
        }
    }

    // ===== 数据读取接口 =====

    /// 读取单个点位值
    pub fn get_point_value(
        &mut self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
    ) -> Result<Option<PointValue>> {
        let key = self.make_point_key(channel_id, point_type, point_id);

        match self.redis.get_string(&key) {
            Ok(data) => Ok(PointValue::from_redis(&data)),
            Err(ModelSrvError::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// 批量读取点位值（优化性能）
    pub fn batch_get_points(
        &mut self,
        points: &[(u16, &str, u32)],
    ) -> Result<HashMap<String, Option<PointValue>>> {
        let start = Instant::now();
        let mut results = HashMap::new();

        // 构建所有键
        let keys: Vec<String> = points
            .iter()
            .map(|(ch, pt, id)| self.make_point_key(*ch, pt, *id))
            .collect();

        // 批量获取（需要扩展redis_handler支持mget）
        for key in keys.iter() {
            let value = match self.redis.get_string(key) {
                Ok(data) => PointValue::from_redis(&data),
                Err(_) => None,
            };
            results.insert(key.clone(), value);
        }

        let elapsed = start.elapsed();
        if elapsed.as_millis() > 100 {
            warn!("Batch read {} points took {:?}", points.len(), elapsed);
        }

        Ok(results)
    }

    /// 读取通道下所有点位（使用通配符）
    pub fn get_channel_points(
        &mut self,
        channel_id: u16,
        point_type: Option<&str>,
    ) -> Result<HashMap<String, PointValue>> {
        let pattern = match point_type {
            Some(pt) => format!("{}:{}:*", channel_id, pt),
            None => format!("{}:*", channel_id),
        };

        let keys = self.redis.get_keys(&pattern)?;
        let mut results = HashMap::new();

        for key in keys {
            if let Ok(data) = self.redis.get_string(&key) {
                if let Some(value) = PointValue::from_redis(&data) {
                    results.insert(key, value);
                }
            }
        }

        Ok(results)
    }

    /// 监听点位变化（使用Redis keyspace notifications）
    pub async fn watch_points(
        &mut self,
        _patterns: Vec<String>,
        _callback: impl Fn(String, PointValue) + Send + 'static,
    ) -> Result<()> {
        // 注意：需要Redis开启keyspace notifications
        // CONFIG SET notify-keyspace-events KEA

        warn!("Point watching requires Redis keyspace notifications enabled");

        // 这里需要使用异步Redis客户端进行订阅
        // 由于当前使用同步客户端，这个功能需要额外实现

        Err(ModelSrvError::ConfigError(
            "Async subscription not yet implemented in sync client".to_string(),
        ))
    }

    // ===== 控制命令接口 =====

    /// 发送控制命令
    pub fn send_control_command(
        &mut self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<String> {
        let command = ControlCommand::new(channel_id, point_type, point_id, value);
        let command_id = command.command_id.clone();

        // 序列化命令
        let cmd_json = serde_json::to_string(&command)
            .map_err(|e| ModelSrvError::SerializationError(e.to_string()))?;

        // 发布到命令通道
        self.redis.publish(&self.command_channel, &cmd_json)?;

        // 记录命令状态
        let status_key = format!("cmd:status:{}", command_id);
        let status = CommandStatus {
            command_id: command_id.clone(),
            status: "pending".to_string(),
            message: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        let status_json = serde_json::to_string(&status)
            .map_err(|e| ModelSrvError::SerializationError(e.to_string()))?;

        self.redis.set_string(&status_key, &status_json)?;

        info!(
            "Sent control command {} to {}:{}:{} = {}",
            command_id, channel_id, point_type, point_id, value
        );

        Ok(command_id)
    }

    /// 批量发送控制命令
    pub fn batch_send_commands(
        &mut self,
        commands: Vec<(u16, &str, u32, f64)>,
    ) -> Result<Vec<String>> {
        let mut command_ids = Vec::new();

        for (channel_id, point_type, point_id, value) in commands {
            match self.send_control_command(channel_id, point_type, point_id, value) {
                Ok(id) => command_ids.push(id),
                Err(e) => {
                    error!(
                        "Failed to send command to {}:{}:{}: {}",
                        channel_id, point_type, point_id, e
                    );
                }
            }
        }

        Ok(command_ids)
    }

    /// 查询命令状态
    pub fn get_command_status(&mut self, command_id: &str) -> Result<Option<CommandStatus>> {
        let status_key = format!("cmd:status:{}", command_id);

        match self.redis.get_string(&status_key) {
            Ok(json) => {
                let status: CommandStatus = serde_json::from_str(&json)
                    .map_err(|e| ModelSrvError::SerializationError(e.to_string()))?;
                Ok(Some(status))
            }
            Err(ModelSrvError::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// 等待命令完成（带超时）
    pub fn wait_for_command(
        &mut self,
        command_id: &str,
        timeout: Duration,
    ) -> Result<CommandStatus> {
        let start = Instant::now();
        let poll_interval = Duration::from_millis(100);

        loop {
            if start.elapsed() > timeout {
                return Err(ModelSrvError::TimeoutError(format!(
                    "Command {} timeout after {:?}",
                    command_id, timeout
                )));
            }

            if let Some(status) = self.get_command_status(command_id)? {
                match status.status.as_str() {
                    "success" | "failed" => return Ok(status),
                    _ => {
                        std::thread::sleep(poll_interval);
                        continue;
                    }
                }
            }

            std::thread::sleep(poll_interval);
        }
    }

    // ===== 辅助方法 =====

    /// 生成点位键
    fn make_point_key(&self, channel_id: u16, point_type: &str, point_id: u32) -> String {
        format!("{}:{}:{}", channel_id, point_type, point_id)
    }

    /// 批量更新modsrv计算结果到Redis（供其他服务使用）
    pub fn publish_calculation_results(
        &mut self,
        module_id: &str,
        results: HashMap<String, f64>,
    ) -> Result<()> {
        let timestamp = chrono::Utc::now().timestamp_millis();

        // 批量更新到Hash结构
        let hash_key = format!("modsrv:calc:{}", module_id);
        let mut fields = HashMap::new();

        for (point_name, value) in results.iter() {
            let data = format!("{}:{}", value, timestamp);
            fields.insert(point_name.clone(), data);
        }

        self.redis.set_hash(&hash_key, fields)?;

        // 发布更新通知
        let notification = serde_json::json!({
            "module_id": module_id,
            "timestamp": timestamp,
            "point_count": results.len()
        });

        self.redis.publish(
            &format!("modsrv:outputs:{}", module_id),
            &notification.to_string(),
        )?;

        debug!(
            "Published {} calculation results for module {}",
            results.len(),
            module_id
        );

        Ok(())
    }
}

/// 点位缓存（用于减少Redis访问）
pub struct PointCache {
    cache: HashMap<String, (PointValue, Instant)>,
    ttl: Duration,
}

impl PointCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: HashMap::new(),
            ttl,
        }
    }

    pub fn get(&self, key: &str) -> Option<&PointValue> {
        self.cache.get(key).and_then(|(value, time)| {
            if time.elapsed() < self.ttl {
                Some(value)
            } else {
                None
            }
        })
    }

    pub fn set(&mut self, key: String, value: PointValue) {
        self.cache.insert(key, (value, Instant::now()));
    }

    pub fn clear_expired(&mut self) {
        let now = Instant::now();
        self.cache
            .retain(|_, (_, time)| now.duration_since(*time) < self.ttl);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_value_parsing() {
        let redis_str = "25.6:1234567890";
        let pv = PointValue::from_redis(redis_str).unwrap();
        assert_eq!(pv.value, 25.6);
        assert_eq!(pv.timestamp, 1234567890);
    }

    #[test]
    fn test_control_command_creation() {
        let cmd = ControlCommand::new(1001, TYPE_CONTROL, 30001, 1.0);
        assert_eq!(cmd.channel_id, 1001);
        assert_eq!(cmd.point_type, "c");
        assert_eq!(cmd.point_id, 30001);
        assert_eq!(cmd.value, 1.0);
        assert_eq!(cmd.source, "modsrv");
    }

    #[test]
    fn test_point_cache() {
        let mut cache = PointCache::new(Duration::from_secs(1));
        let pv = PointValue {
            value: 42.0,
            timestamp: 123456,
        };

        cache.set("test_key".to_string(), pv.clone());
        assert!(cache.get("test_key").is_some());

        std::thread::sleep(Duration::from_secs(2));
        assert!(cache.get("test_key").is_none());
    }
}
