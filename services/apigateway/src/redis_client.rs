use crate::error::ApiResult;
use redis::{aio::ConnectionManager, AsyncCommands};
use tokio::sync::Mutex;
use std::sync::Arc;

pub struct RedisClient {
    connection: Arc<Mutex<ConnectionManager>>,
}

impl RedisClient {
    pub async fn new(redis_url: &str) -> ApiResult<Self> {
        let client = redis::Client::open(redis_url)?;
        let connection = ConnectionManager::new(client).await?;
        
        Ok(Self { 
            connection: Arc::new(Mutex::new(connection))
        })
    }

    pub async fn get(&self, key: &str) -> ApiResult<Option<String>> {
        let mut conn = self.connection.lock().await;
        let value: Option<String> = conn.get(key).await?;
        Ok(value)
    }

    pub async fn set(&self, key: &str, value: &str) -> ApiResult<()> {
        let mut conn = self.connection.lock().await;
        let _: () = conn.set(key, value).await?;
        Ok(())
    }

    pub async fn set_ex(&self, key: &str, value: &str, seconds: u64) -> ApiResult<()> {
        let mut conn = self.connection.lock().await;
        let _: () = conn.set_ex(key, value, seconds as usize).await?;
        Ok(())
    }

    pub async fn del(&self, key: &str) -> ApiResult<()> {
        let mut conn = self.connection.lock().await;
        let _: () = conn.del(key).await?;
        Ok(())
    }

    pub async fn exists(&self, key: &str) -> ApiResult<bool> {
        let mut conn = self.connection.lock().await;
        let exists: bool = conn.exists(key).await?;
        Ok(exists)
    }

    pub async fn expire(&self, key: &str, seconds: i64) -> ApiResult<bool> {
        let mut conn = self.connection.lock().await;
        let result: bool = conn.expire(key, seconds as usize).await?;
        Ok(result)
    }

    pub async fn keys(&self, pattern: &str) -> ApiResult<Vec<String>> {
        let mut conn = self.connection.lock().await;
        let keys: Vec<String> = conn.keys(pattern).await?;
        Ok(keys)
    }

    pub async fn hget(&self, key: &str, field: &str) -> ApiResult<Option<String>> {
        let mut conn = self.connection.lock().await;
        let value: Option<String> = conn.hget(key, field).await?;
        Ok(value)
    }

    pub async fn hset(&self, key: &str, field: &str, value: &str) -> ApiResult<()> {
        let mut conn = self.connection.lock().await;
        let _: () = conn.hset(key, field, value).await?;
        Ok(())
    }

    pub async fn hgetall(&self, key: &str) -> ApiResult<Vec<(String, String)>> {
        let mut conn = self.connection.lock().await;
        let result: Vec<(String, String)> = conn.hgetall(key).await?;
        Ok(result)
    }

    pub async fn ping(&self) -> ApiResult<bool> {
        let mut conn = self.connection.lock().await;
        let pong: String = redis::cmd("PING").query_async(&mut *conn).await?;
        Ok(pong == "PONG")
    }

    pub async fn info(&self) -> ApiResult<String> {
        let mut conn = self.connection.lock().await;
        let info: String = redis::cmd("INFO").query_async(&mut *conn).await?;
        Ok(info)
    }
}