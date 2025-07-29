//! 边端设备Redis连接管理
//!
//! 提供轻量级的Redis连接和Lua脚本管理功能

use crate::error::Result;
use redis::{aio::ConnectionManager, AsyncCommands, Client};

/// 边端设备Redis连接管理器
pub struct EdgeRedis {
    /// Redis连接管理器
    conn: ConnectionManager,
    /// 同步脚本SHA
    sync_script_sha: Option<String>,
}

impl EdgeRedis {
    /// 创建新的边端Redis连接
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;

        Ok(Self {
            conn,
            sync_script_sha: None,
        })
    }

    /// 初始化Lua脚本
    pub async fn init_scripts(&mut self) -> Result<()> {
        // 加载脚本并获取SHA
        let sha: String = redis::cmd("SCRIPT")
            .arg("LOAD")
            .arg(include_str!("../../../scripts/edge_sync.lua"))
            .query_async(&mut self.conn)
            .await?;

        self.sync_script_sha = Some(sha);
        tracing::info!(
            "Lua脚本加载成功，SHA: {}",
            self.sync_script_sha.as_ref().unwrap()
        );

        Ok(())
    }

    /// 同步测量数据
    pub async fn sync_measurement(&mut self, channel: u32, point: u32, value: f64) -> Result<()> {
        let args = vec![
            "sync_measurement".to_string(),
            channel.to_string(),
            point.to_string(),
            format!("{:.6}", value),
        ];

        self.execute_sync_script(args).await?;
        Ok(())
    }

    /// 发送控制命令
    pub async fn send_control(&mut self, model_id: &str, control: &str, value: f64) -> Result<()> {
        let args = vec![
            "send_control".to_string(),
            model_id.to_string(),
            control.to_string(),
            format!("{:.6}", value),
        ];

        self.execute_sync_script(args).await?;
        Ok(())
    }

    /// 获取模型所有值
    pub async fn get_model_values(&mut self, model_id: &str) -> Result<Vec<(String, String)>> {
        let args = vec!["get_values".to_string(), model_id.to_string()];

        let result: Vec<String> = if let Some(sha) = &self.sync_script_sha {
            redis::cmd("EVALSHA")
                .arg(sha)
                .arg(0)
                .arg(&args)
                .query_async(&mut self.conn)
                .await?
        } else {
            redis::cmd("EVAL")
                .arg(include_str!("../../../scripts/edge_sync.lua"))
                .arg(0)
                .arg(&args)
                .query_async(&mut self.conn)
                .await?
        };

        // 将结果转换为键值对
        let mut pairs = Vec::new();
        for chunk in result.chunks(2) {
            if chunk.len() == 2 {
                pairs.push((chunk[0].clone(), chunk[1].clone()));
            }
        }

        Ok(pairs)
    }

    /// 初始化映射
    pub async fn init_mapping(&mut self, mapping_type: &str, key: &str, value: &str) -> Result<()> {
        let args = vec![
            "init_mapping".to_string(),
            mapping_type.to_string(),
            key.to_string(),
            value.to_string(),
        ];

        self.execute_sync_script(args).await?;
        Ok(())
    }

    /// 清理所有映射
    pub async fn clear_mappings(&mut self) -> Result<i64> {
        let args = vec!["clear_mappings".to_string()];

        let count: i64 = if let Some(sha) = &self.sync_script_sha {
            redis::cmd("EVALSHA")
                .arg(sha)
                .arg(0)
                .arg(&args)
                .query_async(&mut self.conn)
                .await?
        } else {
            redis::cmd("EVAL")
                .arg(include_str!("../../../scripts/edge_sync.lua"))
                .arg(0)
                .arg(&args)
                .query_async(&mut self.conn)
                .await?
        };

        Ok(count)
    }

    /// 执行同步脚本
    async fn execute_sync_script(&mut self, args: Vec<String>) -> Result<String> {
        let result: String = if let Some(sha) = &self.sync_script_sha {
            redis::cmd("EVALSHA")
                .arg(sha)
                .arg(0)
                .arg(&args)
                .query_async(&mut self.conn)
                .await?
        } else {
            redis::cmd("EVAL")
                .arg(include_str!("../../../scripts/edge_sync.lua"))
                .arg(0)
                .arg(&args)
                .query_async(&mut self.conn)
                .await?
        };

        Ok(result)
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
        if let Ok(mut edge_redis) = EdgeRedis::new("redis://localhost:6379/0").await {
            assert!(edge_redis.init_scripts().await.is_ok());
            assert!(edge_redis.sync_script_sha.is_some());
        }
    }
}
