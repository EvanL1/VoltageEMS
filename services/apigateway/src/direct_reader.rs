use crate::error::ApiGatewayError;
use crate::redis_wrapper::RedisWrapper;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// 直接读取的数据类型（硬编码）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DirectReadType {
    Measurements, // comsrv:{channel}:m
    Signals,      // comsrv:{channel}:s
    Models,       // modsrv:{model}:measurement
    Status,       // status:{service}
}

impl DirectReadType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "measurements" => Some(Self::Measurements),
            "signals" => Some(Self::Signals),
            "models" => Some(Self::Models),
            "status" => Some(Self::Status),
            _ => None,
        }
    }
}

/// 批量读取请求
#[derive(Debug, Deserialize)]
pub struct BatchReadRequest {
    #[serde(default)]
    pub measurements: Vec<u32>,
    #[serde(default)]
    pub signals: Vec<u32>,
    #[serde(default)]
    pub models: Vec<String>,
}

/// 批量读取响应
#[derive(Debug, Serialize)]
pub struct BatchReadResponse {
    pub measurements: HashMap<String, HashMap<String, f64>>,
    pub signals: HashMap<String, HashMap<String, bool>>,
    pub models: HashMap<String, HashMap<String, f64>>,
}

/// 直接读取器
pub struct DirectReader {
    redis: Arc<RedisWrapper>,
}

impl DirectReader {
    pub fn new(redis: Arc<RedisWrapper>) -> Self {
        Self { redis }
    }

    /// 读取数据
    pub async fn read(
        &self,
        read_type: DirectReadType,
        id: &str,
    ) -> Result<serde_json::Value, ApiGatewayError> {
        let key = match read_type {
            DirectReadType::Measurements => format!("comsrv:{}:m", id),
            DirectReadType::Signals => format!("comsrv:{}:s", id),
            DirectReadType::Models => format!("modsrv:{}:measurement", id),
            DirectReadType::Status => format!("status:{}", id),
        };

        match read_type {
            DirectReadType::Measurements | DirectReadType::Models => {
                self.read_float_hash(&key).await
            },
            DirectReadType::Signals => self.read_bool_hash(&key).await,
            DirectReadType::Status => self.read_string(&key).await,
        }
    }

    /// 批量读取
    pub async fn batch_read(
        &self,
        request: BatchReadRequest,
    ) -> Result<BatchReadResponse, ApiGatewayError> {
        let mut response = BatchReadResponse {
            measurements: HashMap::new(),
            signals: HashMap::new(),
            models: HashMap::new(),
        };

        // 读取测量数据
        for channel_id in request.measurements {
            let key = format!("comsrv:{}:m", channel_id);
            if let Ok(data) = self.read_float_hash_raw(&key).await {
                response.measurements.insert(channel_id.to_string(), data);
            }
        }

        // 读取信号数据
        for channel_id in request.signals {
            let key = format!("comsrv:{}:s", channel_id);
            if let Ok(data) = self.read_bool_hash_raw(&key).await {
                response.signals.insert(channel_id.to_string(), data);
            }
        }

        // 读取模型数据
        for model_name in request.models {
            let key = format!("modsrv:{}:measurement", model_name);
            if let Ok(data) = self.read_float_hash_raw(&key).await {
                response.models.insert(model_name, data);
            }
        }

        Ok(response)
    }

    /// 读取浮点数Hash
    async fn read_float_hash(&self, key: &str) -> Result<serde_json::Value, ApiGatewayError> {
        let data = self.read_float_hash_raw(key).await?;
        Ok(serde_json::to_value(data)?)
    }

    /// 读取浮点数Hash（原始格式）
    async fn read_float_hash_raw(
        &self,
        key: &str,
    ) -> Result<HashMap<String, f64>, ApiGatewayError> {
        let redis_data = self.redis.hgetall(key).await?;

        let mut result = HashMap::new();
        for (field, value) in redis_data {
            if let Ok(f) = value.parse::<f64>() {
                result.insert(field, f);
            }
        }

        if result.is_empty() {
            return Err(ApiGatewayError::NotFound(format!(
                "No data found for key: {}",
                key
            )));
        }

        Ok(result)
    }

    /// 读取布尔值Hash
    async fn read_bool_hash(&self, key: &str) -> Result<serde_json::Value, ApiGatewayError> {
        let data = self.read_bool_hash_raw(key).await?;
        Ok(serde_json::to_value(data)?)
    }

    /// 读取布尔值Hash（原始格式）
    async fn read_bool_hash_raw(
        &self,
        key: &str,
    ) -> Result<HashMap<String, bool>, ApiGatewayError> {
        let redis_data = self.redis.hgetall(key).await?;

        let mut result = HashMap::new();
        for (field, value) in redis_data {
            let bool_val = match value.as_str() {
                "0" | "false" | "False" | "FALSE" => false,
                "1" | "true" | "True" | "TRUE" => true,
                _ => continue,
            };
            result.insert(field, bool_val);
        }

        if result.is_empty() {
            return Err(ApiGatewayError::NotFound(format!(
                "No data found for key: {}",
                key
            )));
        }

        Ok(result)
    }

    /// 读取字符串值
    async fn read_string(&self, key: &str) -> Result<serde_json::Value, ApiGatewayError> {
        let value = self.redis.get(key).await?;

        match value {
            Some(v) => Ok(serde_json::json!({ "value": v })),
            None => Err(ApiGatewayError::NotFound(format!(
                "No data found for key: {}",
                key
            ))),
        }
    }
}

/// 检查直读权限（简化版）
pub fn check_direct_read_permission(user_roles: &[String], read_type: DirectReadType) -> bool {
    let has_role = |role: &str| user_roles.iter().any(|r| r == role);

    match read_type {
        DirectReadType::Measurements | DirectReadType::Signals => {
            has_role("operator") || has_role("admin")
        },
        DirectReadType::Models => has_role("viewer") || has_role("operator") || has_role("admin"),
        DirectReadType::Status => {
            // 所有登录用户都可以查看状态
            true
        },
    }
}
