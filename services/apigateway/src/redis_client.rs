use crate::error::{ApiGatewayError, ApiResult};

// Re-export RedisClient from voltage-common
pub use voltage_common::redis::RedisClient;

// Extension trait to adapt voltage_common::Result to ApiResult
pub trait RedisClientExt {
    async fn get_api(&self, key: &str) -> ApiResult<Option<String>>;
    async fn set_api(&self, key: &str, value: &str) -> ApiResult<()>;
    async fn set_ex_api(&self, key: &str, value: &str, seconds: u64) -> ApiResult<()>;
    async fn del_api(&self, key: &str) -> ApiResult<()>;
    async fn exists_api(&self, key: &str) -> ApiResult<bool>;
    async fn expire_api(&self, key: &str, seconds: i64) -> ApiResult<bool>;
    async fn keys_api(&self, pattern: &str) -> ApiResult<Vec<String>>;
    async fn hget_api(&self, key: &str, field: &str) -> ApiResult<Option<String>>;
    async fn hset_api(&self, key: &str, field: &str, value: &str) -> ApiResult<()>;
    async fn hgetall_api(&self, key: &str) -> ApiResult<Vec<(String, String)>>;
    async fn ping_api(&self) -> ApiResult<bool>;
    async fn info_api(&self) -> ApiResult<String>;
}

impl RedisClientExt for RedisClient {
    async fn get_api(&self, key: &str) -> ApiResult<Option<String>> {
        self.get(key)
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
    }

    async fn set_api(&self, key: &str, value: &str) -> ApiResult<()> {
        self.set(key, value)
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
    }

    async fn set_ex_api(&self, key: &str, value: &str, seconds: u64) -> ApiResult<()> {
        self.set_ex(key, value, seconds)
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
    }

    async fn del_api(&self, key: &str) -> ApiResult<()> {
        self.del(key)
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
            .map(|_| ())
    }

    async fn exists_api(&self, key: &str) -> ApiResult<bool> {
        self.exists(key)
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
    }

    async fn expire_api(&self, key: &str, seconds: i64) -> ApiResult<bool> {
        self.expire(key, seconds)
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
    }

    async fn keys_api(&self, pattern: &str) -> ApiResult<Vec<String>> {
        self.keys(pattern)
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
    }

    async fn hget_api(&self, key: &str, field: &str) -> ApiResult<Option<String>> {
        self.hget(key, field)
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
    }

    async fn hset_api(&self, key: &str, field: &str, value: &str) -> ApiResult<()> {
        self.hset(key, field, value)
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
            .map(|_| ())
    }

    async fn hgetall_api(&self, key: &str) -> ApiResult<Vec<(String, String)>> {
        self.hgetall(key)
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
            .map(|hm| hm.into_iter().collect())
    }

    async fn ping_api(&self) -> ApiResult<bool> {
        self.ping()
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
            .map(|result| result == "PONG")
    }

    async fn info_api(&self) -> ApiResult<String> {
        self.info()
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
    }
}

// Backward compatibility aliases
pub async fn new_redis_client(redis_url: &str) -> ApiResult<RedisClient> {
    RedisClient::new(redis_url)
        .await
        .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
}
