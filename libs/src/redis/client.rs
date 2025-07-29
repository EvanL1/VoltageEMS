//! Redis 异步客户端

use crate::error::Result;
use redis::{aio::ConnectionManager, AsyncCommands, Client};

/// Redis 异步客户端
pub struct RedisClient {
    pub(crate) conn: ConnectionManager,
    url: String,
}

impl std::fmt::Debug for RedisClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisClient")
            .field("url", &self.url)
            .field("conn", &"<ConnectionManager>")
            .finish()
    }
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
        let _: () = self.conn.set(key, value).await?;
        Ok(())
    }

    /// SET with expiration
    pub async fn setex<T: redis::ToRedisArgs + Send + Sync>(
        &mut self,
        key: &str,
        value: T,
        seconds: u64,
    ) -> Result<()> {
        let _: () = self.conn.set_ex(key, value, seconds).await?;
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
        let _: () = self.conn.mset(items).await?;
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

    /// Hash 操作 - 设置字段
    pub async fn hset(&mut self, key: &str, field: &str, value: String) -> Result<()> {
        redis::cmd("HSET")
            .arg(key)
            .arg(field)
            .arg(value)
            .query_async(&mut self.conn)
            .await
            .map_err(Into::into)
    }

    /// Hash 操作 - 获取字段
    pub async fn hget(&mut self, key: &str, field: &str) -> Result<Option<String>> {
        redis::cmd("HGET")
            .arg(key)
            .arg(field)
            .query_async(&mut self.conn)
            .await
            .map_err(Into::into)
    }

    /// Hash 操作 - 获取多个字段
    pub async fn hmget(&mut self, key: &str, fields: &[&str]) -> Result<Vec<Option<String>>> {
        redis::cmd("HMGET")
            .arg(key)
            .arg(fields)
            .query_async(&mut self.conn)
            .await
            .map_err(Into::into)
    }

    /// Hash 操作 - 获取所有字段
    pub async fn hgetall(
        &mut self,
        key: &str,
    ) -> Result<std::collections::HashMap<String, String>> {
        redis::cmd("HGETALL")
            .arg(key)
            .query_async(&mut self.conn)
            .await
            .map_err(Into::into)
    }

    /// 创建订阅连接
    pub async fn subscribe(&mut self, channels: &[&str]) -> Result<redis::aio::PubSub> {
        let client = Client::open(self.url.as_str())?;
        let mut pubsub = client.get_async_pubsub().await?;
        pubsub.subscribe(channels).await?;
        Ok(pubsub)
    }

    /// CONFIG SET 操作
    pub async fn config_set(&mut self, parameter: &str, value: &str) -> Result<String> {
        redis::cmd("CONFIG")
            .arg("SET")
            .arg(parameter)
            .arg(value)
            .query_async(&mut self.conn)
            .await
            .map_err(Into::into)
    }
}
