//! modsrv Redis存储实现
//!
//! 提供统一的存储接口，支持监视值读取和控制命令写入

use super::types::*;
use crate::error::{ModelSrvError, Result};
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Pipeline};
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, error, info};

/// modsrv存储管理器
pub struct ModelStorage {
    conn: ConnectionManager,
}

impl ModelStorage {
    /// 创建新的存储实例
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to create Redis client: {}", e))
        })?;

        let conn = ConnectionManager::new(client)
            .await
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to connect to Redis: {}", e)))?;

        Ok(Self { conn })
    }

    /// 从环境变量创建
    pub async fn from_env() -> Result<Self> {
        let redis_url = std::env::var("MODSRV_REDIS_URL")
            .or_else(|_| std::env::var("REDIS_URL"))
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        Self::new(&redis_url).await
    }

    /// 读取单个监视值
    pub async fn get_monitor_value(
        &mut self,
        model_id: &str,
        monitor_type: MonitorType,
        point_id: u32,
    ) -> Result<Option<MonitorValue>> {
        let key = make_monitor_key(model_id, &monitor_type, point_id);

        let data: Option<String> = self.conn.get(&key).await.map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to get monitor value: {}", e))
        })?;

        match data {
            Some(redis_str) => {
                if let Some(mv) = MonitorValue::from_redis(&redis_str) {
                    Ok(Some(mv))
                } else {
                    error!("Failed to parse monitor value: {}", redis_str);
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// 批量读取监视值
    pub async fn get_monitor_values(
        &mut self,
        keys: &[MonitorKey],
    ) -> Result<Vec<Option<MonitorValue>>> {
        if keys.is_empty() {
            return Ok(vec![]);
        }

        let redis_keys: Vec<String> = keys
            .iter()
            .map(|k| make_monitor_key(&k.model_id, &k.monitor_type, k.point_id))
            .collect();

        let values: Vec<Option<String>> = self.conn.get(&redis_keys).await.map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to get monitor values: {}", e))
        })?;

        let results = values
            .into_iter()
            .map(|opt_str| opt_str.and_then(|s| MonitorValue::from_redis(&s)))
            .collect();

        Ok(results)
    }

    /// 写入单个监视值（用于模型输出）
    pub async fn set_monitor_value(
        &mut self,
        model_id: &str,
        monitor_type: MonitorType,
        point_id: u32,
        value: MonitorValue,
    ) -> Result<()> {
        let key = make_monitor_key(model_id, &monitor_type, point_id);
        let data = value.to_redis();

        self.conn.set::<_, _, ()>(&key, &data).await.map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to set monitor value: {}", e))
        })?;

        debug!(
            "Set monitor value {}:{}:{} = {}",
            model_id,
            monitor_type.to_redis(),
            point_id,
            value.value
        );

        Ok(())
    }

    /// 批量写入监视值
    pub async fn set_monitor_values(&mut self, updates: &[MonitorUpdate]) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        let start = Instant::now();
        let mut pipe = Pipeline::new();

        for update in updates {
            let key = make_monitor_key(&update.model_id, &update.monitor_type, update.point_id);
            let data = update.value.to_redis();
            pipe.set(&key, &data);
        }

        pipe.query_async::<()>(&mut self.conn).await.map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to set monitor values: {}", e))
        })?;

        let elapsed = start.elapsed();
        info!(
            "Batch updated {} monitor values in {:?}",
            updates.len(),
            elapsed
        );

        Ok(())
    }

    /// 创建控制命令
    pub async fn create_control_command(&mut self, command: &ControlCommand) -> Result<()> {
        let key = make_control_key(&command.id);
        let hash = command.to_hash();

        // 将命令存储为Hash
        for (field, value) in hash {
            self.conn
                .hset::<_, _, _, ()>(&key, field, value)
                .await
                .map_err(|e| {
                    ModelSrvError::RedisError(format!("Failed to set hash field: {}", e))
                })?;
        }

        // 设置过期时间（24小时）
        self.conn.expire::<_, ()>(&key, 86400).await.map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to set command expire: {}", e))
        })?;

        // 添加到模型的命令列表
        let list_key = make_control_list_key(&command.source_model);
        self.conn
            .lpush::<_, _, ()>(&list_key, &command.id)
            .await
            .map_err(|e| {
                ModelSrvError::RedisError(format!("Failed to add command to list: {}", e))
            })?;

        // 限制列表长度（保留最近1000条）
        self.conn
            .ltrim::<_, ()>(&list_key, 0, 999)
            .await
            .map_err(|e| {
                ModelSrvError::RedisError(format!("Failed to trim command list: {}", e))
            })?;

        // 发布命令到通道（供comsrv订阅）
        let channel = format!(
            "cmd:{}:{}",
            command.channel_id,
            command.command_type.to_redis()
        );
        let cmd_json =
            serde_json::to_string(command).map_err(|e| ModelSrvError::JsonError(e.to_string()))?;

        self.conn
            .publish::<_, _, ()>(&channel, &cmd_json)
            .await
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to publish command: {}", e)))?;

        info!(
            "Created control command {} for channel {} point {}",
            command.id, command.channel_id, command.point_id
        );

        Ok(())
    }

    /// 发送控制命令（简化接口）
    pub async fn send_control_command(&mut self, command: &ControlCommand) -> Result<()> {
        self.create_control_command(command).await
    }

    /// 获取模型配置
    pub async fn get_model_configs(&mut self, pattern: &str) -> Result<HashMap<String, String>> {
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut self.conn)
            .await
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to get model keys: {}", e)))?;

        let mut configs = HashMap::new();
        for key in keys {
            if let Ok(value) = self.conn.get::<_, String>(&key).await {
                configs.insert(key, value);
            }
        }

        Ok(configs)
    }

    /// 设置模型输出（JSON格式）
    pub async fn set_model_output_json(
        &mut self,
        model_id: &str,
        output: &serde_json::Value,
    ) -> Result<()> {
        let key = format!("model:output:{}", model_id);
        let value =
            serde_json::to_string(output).map_err(|e| ModelSrvError::JsonError(e.to_string()))?;

        self.conn
            .set_ex::<_, _, ()>(&key, &value, 300) // 5分钟过期
            .await
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to set model output: {}", e)))?;

        Ok(())
    }

    /// 获取控制命令
    pub async fn get_control_command(
        &mut self,
        command_id: &str,
    ) -> Result<Option<ControlCommand>> {
        let key = make_control_key(command_id);

        let hash: HashMap<String, String> = self.conn.hgetall(&key).await.map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to get control command: {}", e))
        })?;

        if hash.is_empty() {
            Ok(None)
        } else {
            Ok(ControlCommand::from_hash(hash))
        }
    }

    /// 更新控制命令状态
    pub async fn update_command_status(
        &mut self,
        command_id: &str,
        status: CommandStatus,
        message: Option<String>,
    ) -> Result<()> {
        let key = make_control_key(command_id);
        let now = chrono::Utc::now().timestamp_millis();

        let mut updates: Vec<(&str, String)> = vec![
            ("status", status.to_str().to_string()),
            ("updated_at", now.to_string()),
        ];

        if let Some(msg) = message {
            updates.push(("message", msg));
        }

        self.conn
            .hset_multiple::<_, _, _, ()>(&key, &updates)
            .await
            .map_err(|e| {
                ModelSrvError::RedisError(format!("Failed to update command status: {}", e))
            })?;

        Ok(())
    }

    /// 获取模型的最近控制命令
    pub async fn get_model_commands(
        &mut self,
        model_id: &str,
        limit: isize,
    ) -> Result<Vec<ControlCommand>> {
        let list_key = make_control_list_key(model_id);

        let command_ids: Vec<String> = self
            .conn
            .lrange(&list_key, 0, limit - 1)
            .await
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to get command list: {}", e)))?;

        let mut commands = Vec::new();
        for cmd_id in command_ids {
            if let Some(cmd) = self.get_control_command(&cmd_id).await? {
                commands.push(cmd);
            }
        }

        Ok(commands)
    }

    /// 写入模型输出
    pub async fn set_model_output(&mut self, output: &ModelOutput) -> Result<()> {
        let key = make_model_output_key(&output.model_id);
        let json =
            serde_json::to_string(output).map_err(|e| ModelSrvError::JsonError(e.to_string()))?;

        self.conn
            .set::<_, _, ()>(&key, &json)
            .await
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to set model output: {}", e)))?;

        // 设置过期时间（7天）
        self.conn.expire::<_, ()>(&key, 604800).await.map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to set output expire: {}", e))
        })?;

        Ok(())
    }

    /// 获取模型输出
    pub async fn get_model_output(&mut self, model_id: &str) -> Result<Option<ModelOutput>> {
        let key = make_model_output_key(model_id);

        let data: Option<String> =
            self.conn.get(&key).await.map_err(|e| {
                ModelSrvError::RedisError(format!("Failed to get model output: {}", e))
            })?;

        match data {
            Some(json_str) => {
                let output = serde_json::from_str(&json_str).map_err(|e| {
                    ModelSrvError::JsonError(format!("Failed to parse model output: {}", e))
                })?;
                Ok(Some(output))
            }
            None => Ok(None),
        }
    }

    /// 从comsrv读取点位值（只读）
    pub async fn read_comsrv_point(
        &mut self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
    ) -> Result<Option<(f64, i64)>> {
        // 使用comsrv的键格式
        let key = format!("{}:{}:{}", channel_id, point_type, point_id);

        let data: Option<String> = self.conn.get(&key).await.map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to read comsrv point: {}", e))
        })?;

        match data {
            Some(redis_str) => {
                // 解析comsrv的格式：value:timestamp
                let parts: Vec<&str> = redis_str.split(':').collect();
                if parts.len() == 2 {
                    if let (Ok(value), Ok(timestamp)) =
                        (parts[0].parse::<f64>(), parts[1].parse::<i64>())
                    {
                        return Ok(Some((value, timestamp)));
                    }
                }
                error!("Failed to parse comsrv value: {}", redis_str);
                Ok(None)
            }
            None => Ok(None),
        }
    }

    /// 批量读取comsrv点位值
    pub async fn read_comsrv_points(
        &mut self,
        points: &[(u16, &str, u32)],
    ) -> Result<Vec<Option<(f64, i64)>>> {
        if points.is_empty() {
            return Ok(vec![]);
        }

        let redis_keys: Vec<String> = points
            .iter()
            .map(|(channel_id, point_type, point_id)| {
                format!("{}:{}:{}", channel_id, point_type, point_id)
            })
            .collect();

        let values: Vec<Option<String>> = self.conn.get(&redis_keys).await.map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to read comsrv points: {}", e))
        })?;

        let results = values
            .into_iter()
            .map(|opt_str| {
                opt_str.and_then(|s| {
                    let parts: Vec<&str> = s.split(':').collect();
                    if parts.len() == 2 {
                        if let (Ok(value), Ok(timestamp)) =
                            (parts[0].parse::<f64>(), parts[1].parse::<i64>())
                        {
                            return Some((value, timestamp));
                        }
                    }
                    None
                })
            })
            .collect();

        Ok(results)
    }

    /// 检查连接状态
    pub async fn ping(&mut self) -> Result<()> {
        let _: String = redis::cmd("PING")
            .query_async(&mut self.conn)
            .await
            .map_err(|e| ModelSrvError::RedisError(format!("Redis ping failed: {}", e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_model_storage() {
        // 集成测试占位符
    }
}
