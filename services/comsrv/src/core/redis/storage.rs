//! 极简的Redis存储实现

use super::types::*;
use crate::utils::error::{ComSrvError, Result};
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Pipeline};
use std::time::Instant;
use tracing::{debug, error, info};

/// Redis存储管理器
pub struct RedisStorage {
    conn: ConnectionManager,
}

impl RedisStorage {
    /// 创建新的存储实例
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| ComSrvError::Storage(format!("Failed to create Redis client: {}", e)))?;

        let conn = ConnectionManager::new(client)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to connect to Redis: {}", e)))?;

        Ok(Self { conn })
    }

    /// 设置单个点位值
    pub async fn set_point(
        &mut self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        let key = make_key(channel_id, point_type, point_id);
        let point_value = PointValue::new(value);
        let data = point_value.to_redis();

        self.conn
            .set(&key, &data)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to set point: {}", e)))?;

        debug!(
            "Set point {}:{}:{} = {}",
            channel_id, point_type, point_id, value
        );

        Ok(())
    }

    /// 获取单个点位值
    pub async fn get_point(
        &mut self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
    ) -> Result<Option<(f64, i64)>> {
        let key = make_key(channel_id, point_type, point_id);

        let data: Option<String> = self
            .conn
            .get(&key)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to get point: {}", e)))?;

        match data {
            Some(redis_str) => {
                if let Some(pv) = PointValue::from_redis(&redis_str) {
                    Ok(Some((pv.value, pv.timestamp)))
                } else {
                    error!("Failed to parse redis value: {}", redis_str);
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// 批量设置点位值（使用Pipeline）
    pub async fn set_points(&mut self, updates: &[PointUpdate]) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        let start = Instant::now();
        let mut pipe = Pipeline::new();
        let timestamp = chrono::Utc::now().timestamp_millis();

        for update in updates {
            let key = make_key(update.channel_id, update.point_type, update.point_id);
            let data = format!("{}:{}", update.value, timestamp);
            pipe.set(&key, &data);
        }

        pipe.query_async(&mut self.conn)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to set points: {}", e)))?;

        let elapsed = start.elapsed();
        info!("Batch updated {} points in {:?}", updates.len(), elapsed);

        Ok(())
    }

    /// 批量获取点位值
    pub async fn get_points(&mut self, keys: &[PointKey]) -> Result<Vec<Option<(f64, i64)>>> {
        if keys.is_empty() {
            return Ok(vec![]);
        }

        let redis_keys: Vec<String> = keys
            .iter()
            .map(|k| make_key(k.channel_id, k.point_type, k.point_id))
            .collect();

        let values: Vec<Option<String>> = self
            .conn
            .get(&redis_keys)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to get points: {}", e)))?;

        let results = values
            .into_iter()
            .map(|opt_str| {
                opt_str.and_then(|s| PointValue::from_redis(&s).map(|pv| (pv.value, pv.timestamp)))
            })
            .collect();

        Ok(results)
    }

    /// 设置点位配置
    pub async fn set_config(
        &mut self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        config: &PointConfig,
    ) -> Result<()> {
        let key = make_config_key(channel_id, point_type, point_id);
        let json = serde_json::to_string(config)
            .map_err(|e| ComSrvError::Storage(format!("Failed to serialize config: {}", e)))?;

        self.conn
            .set(&key, &json)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to set config: {}", e)))?;

        debug!("Set config for {}:{}:{}", channel_id, point_type, point_id);

        Ok(())
    }

    /// 获取点位配置
    pub async fn get_config(
        &mut self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
    ) -> Result<Option<PointConfig>> {
        let key = make_config_key(channel_id, point_type, point_id);

        let data: Option<String> = self
            .conn
            .get(&key)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to get config: {}", e)))?;

        match data {
            Some(json_str) => {
                let config = serde_json::from_str(&json_str)
                    .map_err(|e| ComSrvError::Storage(format!("Failed to parse config: {}", e)))?;
                Ok(Some(config))
            }
            None => Ok(None),
        }
    }

    /// 删除点位数据
    pub async fn delete_point(
        &mut self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
    ) -> Result<()> {
        let key = make_key(channel_id, point_type, point_id);

        self.conn
            .del(&key)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to delete point: {}", e)))?;

        Ok(())
    }

    /// 检查连接状态
    pub async fn ping(&mut self) -> Result<()> {
        redis::cmd("PING")
            .query_async(&mut self.conn)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Redis ping failed: {}", e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn test_redis_storage() {
        // 这是集成测试的占位符
        // 实际测试需要Redis实例
        assert!(true);
    }
}
