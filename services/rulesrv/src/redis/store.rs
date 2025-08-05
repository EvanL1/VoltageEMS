use anyhow::Result;
use redis::AsyncCommands;

/// Redis storageimplement
pub struct RedisStore {
    redis_client: redis::Client,
}

impl RedisStore {
    /// Create新的 Redis storage
    pub fn new(redis_url: &str, _key_prefix: Option<&str>) -> Result<Self> {
        let redis_client = redis::Client::open(redis_url)?;

        Ok(Self { redis_client })
    }

    /// Get字符串value
    pub async fn get_string(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        let value: Option<String> = conn.get(key).await?;
        Ok(value)
    }

    /// Set字符串value
    pub async fn set_string(&self, key: &str, value: &str) -> Result<()> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        let _: () = conn.set(key, value).await?;
        Ok(())
    }

    /// Deletekey
    pub async fn delete(&self, key: &str) -> Result<()> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        let _: () = conn.del(key).await?;
        Ok(())
    }

    /// Gethashfieldvalue
    pub async fn get_hash_field(&self, key: &str, field: &str) -> Result<Option<String>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        let value: Option<String> = conn.hget(key, field).await?;
        Ok(value)
    }

    /// releasingmessage
    pub async fn publish(&self, channel: &str, message: &str) -> Result<()> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        let _: () = conn.publish(channel, message).await?;
        Ok(())
    }

    /// call Redis Function
    pub async fn call_function(
        &self,
        function_name: &str,
        keys: &[&str],
        args: &[&str],
    ) -> Result<String> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        // using Redis 的 FCALL 命令callfunction
        // 格式: FCALL function_name numkeys key1 key2 ... arg1 arg2 ...
        let mut cmd = redis::cmd("FCALL");
        cmd.arg(function_name).arg(keys.len());

        for key in keys {
            cmd.arg(*key);
        }

        for arg in args {
            cmd.arg(*arg);
        }

        let result: String = cmd.query_async(&mut conn).await?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_redis_store() {
        // testing需要on-premiserunning的 Redis instance
        let store = RedisStore::new("redis://127.0.0.1/", None)
            .expect("test Redis connection should succeed");

        // testing字符串operation
        let key = "test:key";
        let value = "test_value";

        store
            .set_string(key, value)
            .await
            .expect("set_string should succeed");
        let retrieved = store
            .get_string(key)
            .await
            .expect("get_string should succeed");
        assert_eq!(retrieved, Some(value.to_string()));

        // cleaning
        store.delete(key).await.expect("delete should succeed");
        let deleted = store
            .get_string(key)
            .await
            .expect("get_string after delete should succeed");
        assert_eq!(deleted, None);
    }
}
