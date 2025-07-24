//! 实时数据库模块
//!
//! 提供高性能的实时数据存储和访问接口

use crate::error::{ModelSrvError, Result};
use redis::{AsyncCommands, Client};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

/// 模型存储管理器
pub struct ModelStorage {
    client: Client,
}

impl ModelStorage {
    /// 创建新的模型存储实例
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = Client::open(redis_url).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to create Redis client: {}", e))
        })?;

        // 测试连接
        let mut conn = client
            .get_async_connection()
            .await
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to connect to Redis: {}", e)))?;

        let _: String = conn
            .ping()
            .await
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to ping Redis: {}", e)))?;

        info!("Connected to Redis at {}", redis_url);

        Ok(Self { client })
    }

    /// 从环境变量创建
    pub async fn from_env() -> Result<Self> {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
        Self::new(&redis_url).await
    }

    /// 读取comsrv点位数据
    pub async fn read_comsrv_points(
        &self,
        points: &[(u16, &str, u32)],
    ) -> Result<Vec<Option<f64>>> {
        if points.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = self.client.get_async_connection().await.map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to get Redis connection: {}", e))
        })?;

        let mut results = Vec::with_capacity(points.len());

        for (channel_id, point_type, point_id) in points {
            let key = format!("comsrv:{}:{}", channel_id, point_type);
            let field = point_id.to_string();

            match conn.hget::<_, _, Option<String>>(&key, &field).await {
                Ok(Some(value_str)) => match value_str.parse::<f64>() {
                    Ok(value) => {
                        debug!("Read {}:{} = {}", key, field, value);
                        results.push(Some(value));
                    }
                    Err(e) => {
                        warn!(
                            "Failed to parse value '{}' for {}:{}: {}",
                            value_str, key, field, e
                        );
                        results.push(None);
                    }
                },
                Ok(None) => {
                    debug!("No value found for {}:{}", key, field);
                    results.push(None);
                }
                Err(e) => {
                    error!("Redis error reading {}:{}: {}", key, field, e);
                    results.push(None);
                }
            }
        }

        Ok(results)
    }

    /// 写入模型输出数据
    pub async fn write_model_output(
        &self,
        model_id: &str,
        outputs: &HashMap<String, f64>,
    ) -> Result<()> {
        if outputs.is_empty() {
            return Ok(());
        }

        let mut conn = self.client.get_async_connection().await.map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to get Redis connection: {}", e))
        })?;

        let key = format!("modsrv:output:{}", model_id);

        for (field, value) in outputs {
            let value_str = format!("{:.6}", value); // 6位小数精度
            let _: () = conn.hset(&key, field, &value_str).await.map_err(|e| {
                ModelSrvError::RedisError(format!("Failed to write {}.{}: {}", key, field, e))
            })?;

            debug!("Wrote {}.{} = {}", key, field, value_str);
        }

        info!("Wrote {} outputs for model {}", outputs.len(), model_id);
        Ok(())
    }

    /// 获取模型配置
    pub async fn get_model_configs(&self, pattern: &str) -> Result<HashMap<String, String>> {
        let mut conn = self.client.get_async_connection().await.map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to get Redis connection: {}", e))
        })?;

        let keys: Vec<String> = conn.keys(pattern).await.map_err(|e| {
            ModelSrvError::RedisError(format!(
                "Failed to get keys with pattern {}: {}",
                pattern, e
            ))
        })?;

        let mut configs = HashMap::new();

        for key in keys {
            match conn.get::<_, Option<String>>(&key).await {
                Ok(Some(config)) => {
                    configs.insert(key, config);
                }
                Ok(None) => {
                    warn!("Key {} exists but has no value", key);
                }
                Err(e) => {
                    error!("Failed to get config for key {}: {}", key, e);
                }
            }
        }

        debug!("Found {} model configurations", configs.len());
        Ok(configs)
    }

    /// 发送控制命令到comsrv
    pub async fn send_control_command(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        let mut conn = self.client.get_async_connection().await.map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to get Redis connection: {}", e))
        })?;

        let channel = format!("cmd:{}:{}", channel_id, point_type);
        let message = format!("{}:{:.6}", point_id, value);

        let _: () = conn.publish(&channel, &message).await.map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to publish control command: {}", e))
        })?;

        info!("Sent control command: {} -> {}", channel, message);
        Ok(())
    }

    /// 批量读取模型输出
    pub async fn read_model_outputs(
        &self,
        model_ids: &[&str],
    ) -> Result<HashMap<String, HashMap<String, f64>>> {
        if model_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut conn = self.client.get_async_connection().await.map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to get Redis connection: {}", e))
        })?;

        let mut results = HashMap::new();

        for model_id in model_ids {
            let key = format!("modsrv:output:{}", model_id);

            match conn.hgetall::<_, HashMap<String, String>>(&key).await {
                Ok(fields) => {
                    let mut model_outputs = HashMap::new();
                    for (field, value_str) in fields {
                        match value_str.parse::<f64>() {
                            Ok(value) => {
                                model_outputs.insert(field, value);
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to parse output value '{}' for {}.{}: {}",
                                    value_str, model_id, field, e
                                );
                            }
                        }
                    }
                    if !model_outputs.is_empty() {
                        results.insert(model_id.to_string(), model_outputs);
                    }
                }
                Err(e) => {
                    error!("Failed to read outputs for model {}: {}", model_id, e);
                }
            }
        }

        debug!("Read outputs for {} models", results.len());
        Ok(results)
    }
}
