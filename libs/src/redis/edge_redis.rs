//! 边端设备Redis连接管理
//!
//! 提供轻量级的Redis连接和Lua脚本管理功能

use crate::error::Result;
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use serde_json;

/// 边端设备Redis连接管理器
pub struct EdgeRedis {
    /// Redis连接管理器
    conn: ConnectionManager,
}

impl EdgeRedis {
    /// 创建新的边端Redis连接
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;

        Ok(Self { conn })
    }

    /// 同步测量数据
    pub async fn sync_measurement(&mut self, channel: u32, point: u32, value: f64) -> Result<()> {
        let _: String = redis::cmd("FCALL")
            .arg("modsrv_sync_measurement")
            .arg(1) // key count
            .arg(channel.to_string()) // key
            .arg("m") // telemetry type
            .arg(point.to_string())
            .arg(format!("{value:.6}"))
            .query_async(&mut self.conn)
            .await?;
        Ok(())
    }

    /// 发送控制命令
    pub async fn send_control(&mut self, model_id: &str, control: &str, value: f64) -> Result<()> {
        let _: String = redis::cmd("FCALL")
            .arg("modsrv_send_control")
            .arg(1) // key count
            .arg(model_id) // key
            .arg(control)
            .arg(format!("{value:.6}"))
            .query_async(&mut self.conn)
            .await?;
        Ok(())
    }

    /// 获取模型所有值
    pub async fn get_model_values(&mut self, model_id: &str) -> Result<Vec<(String, String)>> {
        let result: String = redis::cmd("FCALL")
            .arg("modsrv_get_values")
            .arg(1) // key count
            .arg(model_id) // key
            .query_async(&mut self.conn)
            .await?;

        // 解析 JSON 结果
        if let Ok(json_data) = serde_json::from_str::<serde_json::Value>(&result) {
            if let Some(obj) = json_data.as_object() {
                return Ok(obj
                    .iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                    .collect());
            }
        }

        // 如果解析失败，尝试作为字符串数组处理
        let parts: Vec<&str> = result.split_whitespace().collect();
        let mut pairs = Vec::new();
        for chunk in parts.chunks(2) {
            if chunk.len() == 2 {
                pairs.push((chunk[0].to_string(), chunk[1].to_string()));
            }
        }
        Ok(pairs)
    }

    /// 初始化映射
    pub async fn init_mapping(&mut self, mapping_type: &str, key: &str, value: &str) -> Result<()> {
        let mapping = serde_json::json!([{
            "key": key,
            "value": value,
            "type": mapping_type,
            "source": "modsrv"
        }]);

        let _: String = redis::cmd("FCALL")
            .arg("modsrv_init_mappings")
            .arg(0) // no keys
            .arg(mapping.to_string())
            .query_async(&mut self.conn)
            .await?;
        Ok(())
    }

    /// 清理所有映射
    pub async fn clear_mappings(&mut self) -> Result<i64> {
        let count: i64 = redis::cmd("FCALL")
            .arg("modsrv_clear_mappings")
            .arg(0) // no keys
            .arg("*") // pattern
            .query_async(&mut self.conn)
            .await?;
        Ok(count)
    }

    /// 获取底层连接（用于兼容现有代码）
    pub fn get_connection(&mut self) -> &mut ConnectionManager {
        &mut self.conn
    }

    /// 基础Redis操作 - HSET
    pub async fn hset<K, F, V>(&mut self, key: K, field: F, value: V) -> Result<()>
    where
        K: redis::ToRedisArgs + Send + Sync,
        F: redis::ToRedisArgs + Send + Sync,
        V: redis::ToRedisArgs + Send + Sync,
    {
        let _: () = self.conn.hset(key, field, value).await?;
        Ok(())
    }

    /// 基础Redis操作 - HGET
    pub async fn hget<K, F, RV>(&mut self, key: K, field: F) -> Result<Option<RV>>
    where
        K: redis::ToRedisArgs + Send + Sync,
        F: redis::ToRedisArgs + Send + Sync,
        RV: redis::FromRedisValue,
    {
        Ok(self.conn.hget(key, field).await?)
    }

    /// 基础Redis操作 - HGETALL
    pub async fn hgetall<K, RV>(&mut self, key: K) -> Result<RV>
    where
        K: redis::ToRedisArgs + Send + Sync,
        RV: redis::FromRedisValue,
    {
        Ok(self.conn.hgetall(key).await?)
    }

    /// 基础Redis操作 - PUBLISH
    pub async fn publish<K, V>(&mut self, channel: K, message: V) -> Result<()>
    where
        K: redis::ToRedisArgs + Send + Sync,
        V: redis::ToRedisArgs + Send + Sync,
    {
        let _: () = self.conn.publish(channel, message).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_edge_redis_creation() {
        // 这个测试需要Redis运行
        if let Ok(_edge_redis) = EdgeRedis::new("redis://localhost:6379/0").await {
            // Connection manager created successfully, indicating Redis is accessible
            // This test passes if EdgeRedis creation succeeds
        }
    }
}
