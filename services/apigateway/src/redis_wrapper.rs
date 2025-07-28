use crate::error::ApiGatewayError;
use std::collections::HashMap;
use voltage_libs::redis::RedisClient;

/// 对RedisClient的包装，处理mut self的问题
pub struct RedisWrapper {
    url: String,
}

impl RedisWrapper {
    pub fn new(url: String) -> Self {
        Self { url }
    }

    /// 获取字符串值
    pub async fn get(&self, key: &str) -> Result<Option<String>, ApiGatewayError> {
        let mut client = RedisClient::new(&self.url)
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))?;
        client
            .get(key)
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
    }

    /// 获取Hash所有字段
    pub async fn hgetall(&self, key: &str) -> Result<HashMap<String, String>, ApiGatewayError> {
        let mut client = RedisClient::new(&self.url)
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))?;
        client
            .hgetall(key)
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
    }

    /// Ping测试连接
    pub async fn ping(&self) -> Result<String, ApiGatewayError> {
        let mut client = RedisClient::new(&self.url)
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))?;
        client
            .ping()
            .await
            .map_err(|e| ApiGatewayError::RedisError(e.to_string()))
    }
}
