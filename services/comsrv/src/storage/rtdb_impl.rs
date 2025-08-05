//! real-timedatalibrary(RTDB) implement

use async_trait::async_trait;
use voltage_libs::redis::RedisClient;

use super::{PointData, PointStorage, PointUpdate};
use crate::utils::error::{ComSrvError, Result};

/// retryconfiguring
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// maxretry次数
    pub max_retries: u32,
    /// 初始retrylatency（毫秒）
    pub initial_delay_ms: u64,
    /// maxretrylatency（毫秒）
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 2000,
        }
    }
}

/// real-timedatalibrarystorageimplement
#[derive(Debug)]
pub struct RtdbStorage {
    redis_url: String,
    #[allow(dead_code)]
    retry_config: RetryConfig,
}

impl RtdbStorage {
    /// Create新的real-timedatalibraryinstance
    pub async fn new(redis_url: &str) -> Result<Self> {
        // testingconnection
        let mut client = RedisClient::new(redis_url)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to connect to Redis: {e}")))?;
        client
            .ping()
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to ping Redis: {e}")))?;

        Ok(Self {
            redis_url: redis_url.to_string(),
            retry_config: RetryConfig::default(),
        })
    }

    /// 带configuringcreate
    pub async fn with_config(redis_url: &str, retry_config: RetryConfig) -> Result<Self> {
        // testingconnection
        let mut client = RedisClient::new(redis_url)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to connect to Redis: {e}")))?;
        client
            .ping()
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to ping Redis: {e}")))?;

        Ok(Self {
            redis_url: redis_url.to_string(),
            retry_config,
        })
    }

    /// Get Redis client
    async fn get_client(&self) -> Result<RedisClient> {
        RedisClient::new(&self.redis_url)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to connect to Redis: {e}")))
    }
}

#[async_trait]
impl PointStorage for RtdbStorage {
    async fn write_point(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        let hash_key = format!("comsrv:{channel_id}:{point_type}");
        let field = point_id.to_string();
        let data = PointData::new(value);

        let mut client = self.get_client().await?;
        client
            .hset(&hash_key, &field, data.to_redis_value())
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to write point: {e}")))?;

        Ok(())
    }

    async fn write_point_with_metadata(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
        raw_value: Option<f64>,
    ) -> Result<()> {
        let hash_key = format!("comsrv:{channel_id}:{point_type}");
        let field = point_id.to_string();
        let data = PointData::new(value);

        let mut client = self.get_client().await?;

        // usingtransactionwrite多个value
        let mut pipe = redis::pipe();
        pipe.atomic();

        // writemasterHashvalue
        pipe.hset(&hash_key, &field, data.to_redis_value());

        // writemetadata（仍using单独的key）
        if let Some(raw) = raw_value {
            pipe.hset(format!("{hash_key}:raw"), &field, format!("{raw:.6}"));
            pipe.hset(format!("{hash_key}:ts"), &field, data.timestamp.to_string());
        }

        let conn = client.get_connection_mut();
        let _: () = pipe
            .query_async(conn)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to write with metadata: {e}")))?;

        Ok(())
    }

    async fn write_batch(&self, updates: Vec<PointUpdate>) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        let mut client = self.get_client().await?;
        let mut pipe = redis::pipe();
        pipe.atomic();

        // 按channel和typegrouping
        use std::collections::HashMap;
        let mut grouped: HashMap<String, Vec<&PointUpdate>> = HashMap::new();

        for update in &updates {
            let hash_key = format!("comsrv:{}:{}", update.channel_id, update.point_type);
            grouped.entry(hash_key).or_default().push(update);
        }

        // batchwriteeachHash
        for (hash_key, updates) in grouped {
            for update in updates {
                let field = update.point_id.to_string();
                pipe.hset(&hash_key, &field, update.data.to_redis_value());

                if let Some(raw) = update.raw_value {
                    pipe.hset(format!("{hash_key}:raw"), &field, format!("{raw:.6}"));
                    pipe.hset(
                        format!("{hash_key}:ts"),
                        &field,
                        update.data.timestamp.to_string(),
                    );
                }
            }
        }

        let conn = client.get_connection_mut();
        let _: () = pipe
            .query_async(conn)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to write batch: {e}")))?;

        Ok(())
    }

    async fn read_point(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
    ) -> Result<Option<PointData>> {
        let hash_key = format!("comsrv:{channel_id}:{point_type}");
        let field = point_id.to_string();

        let mut client = self.get_client().await?;
        let data: Option<String> = client
            .hget(&hash_key, &field)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to read point: {e}")))?;

        match data {
            Some(value) => {
                let point = PointData::from_redis_value(&value).map_err(|e| {
                    ComSrvError::Storage(format!("Failed to parse point data: {e}"))
                })?;
                Ok(Some(point))
            },
            None => Ok(None),
        }
    }

    async fn read_points(&self, keys: Vec<String>) -> Result<Vec<Option<PointData>>> {
        if keys.is_empty() {
            return Ok(vec![]);
        }

        let mut client = self.get_client().await?;
        let mut results = Vec::new();

        // parsekey并按Hashgrouping
        for key in keys {
            // 期望格式: "comsrv:{channel_id}:{type}:{point_id}"
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() == 4 {
                let hash_key = format!("{}:{}:{}", parts[0], parts[1], parts[2]);
                let field = parts[3];

                let data: Option<String> = client
                    .hget(&hash_key, field)
                    .await
                    .map_err(|e| ComSrvError::Storage(format!("Failed to read point: {e}")))?;

                match data {
                    Some(value) => {
                        let point = PointData::from_redis_value(&value).map_err(|e| {
                            ComSrvError::Storage(format!("Failed to parse point data: {e}"))
                        })?;
                        results.push(Some(point));
                    },
                    None => results.push(None),
                }
            } else {
                results.push(None);
            }
        }

        Ok(results)
    }

    async fn get_channel_points(
        &self,
        channel_id: u16,
        point_type: &str,
    ) -> Result<Vec<(u32, PointData)>> {
        let hash_key = format!("comsrv:{channel_id}:{point_type}");

        let mut client = self.get_client().await?;

        // usingHGETALLacquiringallfield
        let data: std::collections::HashMap<String, String> = client
            .hgetall(&hash_key)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to get channel points: {e}")))?;

        let mut results = Vec::new();
        for (field, value) in data {
            if let Ok(point_id) = field.parse::<u32>() {
                if let Ok(point_data) = PointData::from_redis_value(&value) {
                    results.push((point_id, point_data));
                }
            }
        }

        Ok(results)
    }
}
