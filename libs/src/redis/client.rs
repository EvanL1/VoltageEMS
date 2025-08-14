//! Redis asynchronousclient

use crate::error::Result;
use redis::{aio::ConnectionManager, AsyncCommands, Client};

/// Redis asynchronousclient
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
    /// Create新的client
    pub async fn new(url: &str) -> Result<Self> {
        let client = Client::open(url)?;

        // Add timeout for connection
        let conn = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            ConnectionManager::new(client),
        )
        .await
        .map_err(|_| {
            crate::error::Error::Redis("Redis connection timeout after 5 seconds".into())
        })??;

        Ok(Self {
            conn,
            url: url.into(),
        })
    }

    /// GET operation
    pub async fn get<T: redis::FromRedisValue>(&mut self, key: &str) -> Result<Option<T>> {
        Ok(self.conn.get(key).await?)
    }

    /// SET operation
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

    /// DELETE operation
    pub async fn del(&mut self, keys: &[&str]) -> Result<u32> {
        Ok(self.conn.del(keys).await?)
    }

    /// EXISTS operation
    pub async fn exists(&mut self, key: &str) -> Result<bool> {
        Ok(self.conn.exists(key).await?)
    }

    /// EXPIRE operation
    pub async fn expire(&mut self, key: &str, seconds: i64) -> Result<bool> {
        Ok(self.conn.expire(key, seconds).await?)
    }

    /// PUBLISH operation
    pub async fn publish(&mut self, channel: &str, message: &str) -> Result<u32> {
        Ok(self.conn.publish(channel, message).await?)
    }

    /// PING operation
    pub async fn ping(&mut self) -> Result<String> {
        let pong: String = redis::cmd("PING").query_async(&mut self.conn).await?;
        Ok(pong)
    }

    /// batch GET
    pub async fn mget<T: redis::FromRedisValue>(
        &mut self,
        keys: &[&str],
    ) -> Result<Vec<Option<T>>> {
        Ok(self.conn.get(keys).await?)
    }

    /// BLPOP operation - 阻塞式列表弹出
    /// 返回 Some((key, value)) 或 None（超时）
    pub async fn blpop(
        &mut self,
        keys: &[&str],
        timeout: usize,
    ) -> Result<Option<(String, String)>> {
        let result: Option<(String, String)> = redis::cmd("BLPOP")
            .arg(keys)
            .arg(timeout)
            .query_async(&mut self.conn)
            .await?;
        Ok(result)
    }

    /// RPUSH operation - 向列表尾部添加元素
    pub async fn rpush(&mut self, key: &str, value: &str) -> Result<u32> {
        Ok(self.conn.rpush(key, value).await?)
    }

    /// batch SET
    pub async fn mset<T: redis::ToRedisArgs + Send + Sync>(
        &mut self,
        items: &[(String, T)],
    ) -> Result<()> {
        let _: () = self.conn.mset(items).await?;
        Ok(())
    }

    /// Getallmatch的key
    pub async fn keys(&mut self, pattern: &str) -> Result<Vec<String>> {
        Ok(self.conn.keys(pattern).await?)
    }

    /// Getconnectionmanaging器的mutablereference
    pub fn get_connection_mut(&mut self) -> &mut ConnectionManager {
        &mut self.conn
    }

    /// Hash operation - settingfield
    pub async fn hset(&mut self, key: &str, field: &str, value: String) -> Result<()> {
        redis::cmd("HSET")
            .arg(key)
            .arg(field)
            .arg(value)
            .query_async(&mut self.conn)
            .await
            .map_err(Into::into)
    }

    /// Hash operation - batch setting multiple fields
    pub async fn hmset(&mut self, key: &str, fields: &[(String, String)]) -> Result<()> {
        let mut cmd = redis::cmd("HMSET");
        cmd.arg(key);
        for (field, value) in fields {
            cmd.arg(field).arg(value);
        }
        cmd.query_async(&mut self.conn).await.map_err(Into::into)
    }

    /// Hash operation - acquiringfield
    pub async fn hget(&mut self, key: &str, field: &str) -> Result<Option<String>> {
        redis::cmd("HGET")
            .arg(key)
            .arg(field)
            .query_async(&mut self.conn)
            .await
            .map_err(Into::into)
    }

    /// Hash operation - acquiring多个field
    pub async fn hmget(&mut self, key: &str, fields: &[&str]) -> Result<Vec<Option<String>>> {
        redis::cmd("HMGET")
            .arg(key)
            .arg(fields)
            .query_async(&mut self.conn)
            .await
            .map_err(Into::into)
    }

    /// Hash operation - acquiringallfield
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

    /// Hash operation - acquiringallfield名
    pub async fn hkeys(&mut self, key: &str) -> Result<Vec<String>> {
        redis::cmd("HKEYS")
            .arg(key)
            .query_async(&mut self.conn)
            .await
            .map_err(Into::into)
    }

    /// Createsubscribingconnection
    pub async fn subscribe(&mut self, channels: &[&str]) -> Result<redis::aio::PubSub> {
        let client = Client::open(self.url.as_str())?;
        let mut pubsub = client.get_async_pubsub().await?;
        pubsub.subscribe(channels).await?;
        Ok(pubsub)
    }

    /// CONFIG SET operation
    pub async fn config_set(&mut self, parameter: &str, value: &str) -> Result<String> {
        redis::cmd("CONFIG")
            .arg("SET")
            .arg(parameter)
            .arg(value)
            .query_async(&mut self.conn)
            .await
            .map_err(Into::into)
    }

    /// Load Lua 脚本并return SHA
    pub async fn script_load(&mut self, script: &str) -> Result<String> {
        redis::cmd("SCRIPT")
            .arg("LOAD")
            .arg(script)
            .query_async(&mut self.conn)
            .await
            .map_err(Into::into)
    }

    /// Execute Lua 脚本（通过 SHA）
    pub async fn evalsha(
        &mut self,
        sha: &str,
        keys: &[&str],
        args: &[&str],
    ) -> Result<redis::Value> {
        let mut cmd = redis::cmd("EVALSHA");
        cmd.arg(sha).arg(keys.len());

        for key in keys {
            cmd.arg(key);
        }

        for arg in args {
            cmd.arg(arg);
        }

        cmd.query_async(&mut self.conn).await.map_err(Into::into)
    }

    /// Check脚本yesnoexists
    pub async fn script_exists(&mut self, shas: &[&str]) -> Result<Vec<bool>> {
        redis::cmd("SCRIPT")
            .arg("EXISTS")
            .arg(shas)
            .query_async(&mut self.conn)
            .await
            .map_err(Into::into)
    }

    /// Execute Lua 脚本（直接executing）
    pub async fn eval(
        &mut self,
        script: &str,
        keys: &[&str],
        args: &[&str],
    ) -> Result<redis::Value> {
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(script).arg(keys.len());

        for key in keys {
            cmd.arg(key);
        }

        for arg in args {
            cmd.arg(arg);
        }

        cmd.query_async(&mut self.conn).await.map_err(Into::into)
    }

    /// clearall Lua 脚本cache
    pub async fn script_flush(&mut self) -> Result<String> {
        redis::cmd("SCRIPT")
            .arg("FLUSH")
            .query_async(&mut self.conn)
            .await
            .map_err(Into::into)
    }

    /// call Redis Function
    pub async fn fcall<T: redis::FromRedisValue>(
        &mut self,
        function: &str,
        keys: &[&str],
        args: &[&str],
    ) -> Result<T> {
        let mut cmd = redis::cmd("FCALL");
        cmd.arg(function).arg(keys.len());

        for key in keys {
            cmd.arg(key);
        }

        for arg in args {
            cmd.arg(arg);
        }

        cmd.query_async(&mut self.conn).await.map_err(Into::into)
    }

    /// call Redis Function returnprimal Redis Value
    pub async fn fcall_raw(
        &mut self,
        function: &str,
        keys: &[&str],
        args: &[&str],
    ) -> Result<redis::Value> {
        let mut cmd = redis::cmd("FCALL");
        cmd.arg(function).arg(keys.len());

        for key in keys {
            cmd.arg(key);
        }

        for arg in args {
            cmd.arg(arg);
        }

        cmd.query_async(&mut self.conn).await.map_err(Into::into)
    }

    /// column出all已loading的 Redis Functions
    pub async fn function_list(&mut self) -> Result<redis::Value> {
        redis::cmd("FUNCTION")
            .arg("LIST")
            .query_async(&mut self.conn)
            .await
            .map_err(Into::into)
    }

    /// Load Redis Function
    pub async fn function_load(&mut self, script: &str, replace: bool) -> Result<String> {
        let mut cmd = redis::cmd("FUNCTION");
        cmd.arg("LOAD");
        if replace {
            cmd.arg("REPLACE");
        }
        cmd.arg(script);

        cmd.query_async(&mut self.conn).await.map_err(Into::into)
    }

    /// Delete Redis Function library
    pub async fn function_delete(&mut self, library_name: &str) -> Result<String> {
        redis::cmd("FUNCTION")
            .arg("DELETE")
            .arg(library_name)
            .query_async(&mut self.conn)
            .await
            .map_err(Into::into)
    }
}
