//! Redis 异步客户端

use crate::error::Result;
use redis::{aio::ConnectionManager, AsyncCommands, Client};

/// Redis 异步客户端
pub struct RedisClient {
    pub(crate) conn: ConnectionManager,
    url: String,
}

impl RedisClient {
    /// 创建新的客户端
    pub async fn new(url: &str) -> Result<Self> {
        let client = Client::open(url)?;
        let conn = ConnectionManager::new(client).await?;
        Ok(Self {
            conn,
            url: url.to_string(),
        })
    }

    /// GET 操作
    pub async fn get<T: redis::FromRedisValue>(&mut self, key: &str) -> Result<Option<T>> {
        Ok(self.conn.get(key).await?)
    }

    /// SET 操作
    pub async fn set<T: redis::ToRedisArgs + Send + Sync>(
        &mut self,
        key: &str,
        value: T,
    ) -> Result<()> {
        self.conn.set(key, value).await?;
        Ok(())
    }

    /// SET with expiration
    pub async fn setex<T: redis::ToRedisArgs + Send + Sync>(
        &mut self,
        key: &str,
        value: T,
        seconds: u64,
    ) -> Result<()> {
        self.conn.set_ex(key, value, seconds).await?;
        Ok(())
    }

    /// DELETE 操作
    pub async fn del(&mut self, keys: &[&str]) -> Result<u32> {
        Ok(self.conn.del(keys).await?)
    }

    /// EXISTS 操作
    pub async fn exists(&mut self, key: &str) -> Result<bool> {
        Ok(self.conn.exists(key).await?)
    }

    /// EXPIRE 操作
    pub async fn expire(&mut self, key: &str, seconds: i64) -> Result<bool> {
        Ok(self.conn.expire(key, seconds).await?)
    }

    /// PUBLISH 操作
    pub async fn publish(&mut self, channel: &str, message: &str) -> Result<u32> {
        Ok(self.conn.publish(channel, message).await?)
    }

    /// PING 操作
    pub async fn ping(&mut self) -> Result<String> {
        let pong: String = redis::cmd("PING").query_async(&mut self.conn).await?;
        Ok(pong)
    }

    /// 批量 GET
    pub async fn mget<T: redis::FromRedisValue>(
        &mut self,
        keys: &[&str],
    ) -> Result<Vec<Option<T>>> {
        Ok(self.conn.get(keys).await?)
    }

    /// 批量 SET
    pub async fn mset<T: redis::ToRedisArgs + Send + Sync>(
        &mut self,
        items: &[(String, T)],
    ) -> Result<()> {
        self.conn.mset(items).await?;
        Ok(())
    }

    /// 获取所有匹配的键
    pub async fn keys(&mut self, pattern: &str) -> Result<Vec<String>> {
        Ok(self.conn.keys(pattern).await?)
    }

    /// 获取连接管理器的可变引用
    pub fn get_connection_mut(&mut self) -> &mut ConnectionManager {
        &mut self.conn
    }

    /// 创建订阅连接
    pub async fn subscribe(&mut self, channels: &[&str]) -> Result<redis::aio::PubSub> {
        let client = Client::open(self.url.as_str())?;
        let mut pubsub = client.get_async_pubsub().await?;
        pubsub.subscribe(channels).await?;
        Ok(pubsub)
    }
}
