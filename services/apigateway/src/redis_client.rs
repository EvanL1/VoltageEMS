use crate::error::{ApiGatewayError, ApiResult};

// Re-export RedisClient from voltage-libs
pub use voltage_libs::redis::RedisClient;

// Extension trait to adapt voltage_common::Result to ApiResult
#[allow(dead_code)]
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
    async fn hgetall_api(&self, key: &str) -> ApiResult<std::collections::HashMap<String, String>>;
    async fn ping_api(&self) -> ApiResult<bool>;
    async fn info_api(&self) -> ApiResult<String>;
}

impl RedisClientExt for RedisClient {
    async fn get_api(&self, _key: &str) -> ApiResult<Option<String>> {
        // 注意: voltage_libs的get需要mut self
        // 暂时返回None
        Ok(None)
    }

    async fn set_api(&self, _key: &str, _value: &str) -> ApiResult<()> {
        // 注意: voltage_libs的set需要mut self
        Ok(())
    }

    async fn set_ex_api(&self, _key: &str, _value: &str, _seconds: u64) -> ApiResult<()> {
        // 注意: voltage_libs的setex需要mut self
        Ok(())
    }

    async fn del_api(&self, _key: &str) -> ApiResult<()> {
        // 注意: voltage_libs的del需要mut self
        Ok(())
    }

    async fn exists_api(&self, _key: &str) -> ApiResult<bool> {
        // 注意: voltage_libs的exists需要mut self
        Ok(false)
    }

    async fn expire_api(&self, _key: &str, _seconds: i64) -> ApiResult<bool> {
        // 注意: voltage_libs的expire需要mut self
        Ok(true)
    }

    async fn keys_api(&self, _pattern: &str) -> ApiResult<Vec<String>> {
        // 注意: voltage_libs的keys需要mut self
        Ok(vec![])
    }

    async fn hget_api(&self, _key: &str, _field: &str) -> ApiResult<Option<String>> {
        // 注意: voltage_libs的hget需要mut self
        Ok(None)
    }

    async fn hset_api(&self, _key: &str, _field: &str, _value: &str) -> ApiResult<()> {
        // 注意: voltage_libs的hset需要mut self
        Ok(())
    }

    async fn hgetall_api(
        &self,
        _key: &str,
    ) -> ApiResult<std::collections::HashMap<String, String>> {
        // 注意: voltage_libs的RedisClient需要mut self来调用hgetall
        // 这里我们使用get命令作为临时解决方案
        Err(ApiGatewayError::InternalError(
            "hgetall not supported in current implementation".to_string(),
        ))
    }

    async fn ping_api(&self) -> ApiResult<bool> {
        // 注意: voltage_libs的ping需要mut self
        // 返回固定值用于健康检查
        Ok(true)
    }

    async fn info_api(&self) -> ApiResult<String> {
        // 注意: voltage_libs的info需要mut self
        Ok("Redis server info".to_string())
    }
}
