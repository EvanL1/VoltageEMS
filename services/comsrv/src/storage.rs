//! 简化的 Redis 存储模块
//!
//! 直接提供 Redis 操作函数，无需复杂的 trait 和抽象层

use crate::utils::error::{ComSrvError, Result};
use std::collections::HashMap;
use voltage_libs::redis::RedisClient;

/// 点位更新数据
#[derive(Debug, Clone)]
pub struct PointUpdate {
    pub channel_id: u16,
    pub point_type: String,
    pub point_id: u32,
    pub value: f64,
}

/// 写入单个点位到 Redis
pub async fn write_point(
    client: &mut RedisClient,
    channel_id: u16,
    point_type: &str,
    point_id: u32,
    value: f64,
) -> Result<()> {
    let hash_key = format!("comsrv:{channel_id}:{point_type}");
    let field = point_id.to_string();
    let value_str = format!("{:.6}", value);

    client
        .hset(&hash_key, &field, value_str)
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to write point: {e}")))?;

    Ok(())
}

/// 批量写入点位到 Redis
pub async fn write_batch(client: &mut RedisClient, updates: Vec<PointUpdate>) -> Result<()> {
    if updates.is_empty() {
        return Ok(());
    }

    // 按 hash key 分组
    let mut grouped: HashMap<String, Vec<(String, String)>> = HashMap::new();

    for update in updates {
        let hash_key = format!("comsrv:{}:{}", update.channel_id, update.point_type);
        let field = update.point_id.to_string();
        let value = format!("{:.6}", update.value);

        grouped.entry(hash_key).or_default().push((field, value));
    }

    // 批量写入每个 hash
    for (hash_key, fields) in grouped {
        for (field, value) in fields {
            client
                .hset(&hash_key, &field, value)
                .await
                .map_err(|e| ComSrvError::Storage(format!("Batch write failed: {e}")))?;
        }
    }

    Ok(())
}

/// 读取单个点位
pub async fn read_point(
    client: &mut RedisClient,
    channel_id: u16,
    point_type: &str,
    point_id: u32,
) -> Result<Option<f64>> {
    let hash_key = format!("comsrv:{channel_id}:{point_type}");
    let field = point_id.to_string();

    let value: Option<String> = client
        .hget(&hash_key, &field)
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to read point: {e}")))?;

    Ok(value.and_then(|v| v.parse::<f64>().ok()))
}

/// 读取多个点位
pub async fn read_points(
    client: &mut RedisClient,
    channel_id: u16,
    point_type: &str,
    point_ids: &[u32],
) -> Result<Vec<Option<f64>>> {
    if point_ids.is_empty() {
        return Ok(vec![]);
    }

    let hash_key = format!("comsrv:{channel_id}:{point_type}");
    let fields: Vec<String> = point_ids.iter().map(|id| id.to_string()).collect();
    let field_refs: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();

    let values: Vec<Option<String>> = client
        .hmget(&hash_key, &field_refs)
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to read points: {e}")))?;

    Ok(values
        .into_iter()
        .map(|opt| opt.and_then(|v| v.parse::<f64>().ok()))
        .collect())
}

/// 获取通道的所有点位
pub async fn get_channel_points(
    client: &mut RedisClient,
    channel_id: u16,
    point_type: &str,
) -> Result<HashMap<u32, f64>> {
    let hash_key = format!("comsrv:{channel_id}:{point_type}");

    let all: HashMap<String, String> = client
        .hgetall(&hash_key)
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to get all points: {e}")))?;

    let mut result = HashMap::new();
    for (field, value) in all {
        if let (Ok(point_id), Ok(val)) = (field.parse::<u32>(), value.parse::<f64>()) {
            result.insert(point_id, val);
        }
    }

    Ok(result)
}

/// 发布点位更新到 Redis Pub/Sub
pub async fn publish_update(
    client: &mut RedisClient,
    channel_id: u16,
    point_type: &str,
    point_id: u32,
    value: f64,
) -> Result<()> {
    let topic = format!("comsrv:{}:{}", channel_id, point_type);
    let message = serde_json::json!({
        "point_id": point_id,
        "value": value,
        "timestamp": chrono::Utc::now().timestamp()
    });

    client
        .publish(&topic, &message.to_string())
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to publish: {e}")))?;

    Ok(())
}

/// 创建 Redis 客户端
pub async fn create_redis_client(redis_url: &str) -> Result<RedisClient> {
    RedisClient::new(redis_url)
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to connect to Redis: {e}")))
}
