//! 监视值管理模块
//!
//! 提供对监视值的高级操作接口

use super::rtdb::ModelStorage;
use super::types::*;
use crate::error::Result;
use std::collections::HashMap;
use tracing::{debug, info};

/// 监视值管理器
pub struct MonitorManager {
    storage: ModelStorage,
}

impl MonitorManager {
    /// 创建新的监视值管理器
    pub async fn new(redis_url: &str) -> Result<Self> {
        let storage = ModelStorage::new(redis_url).await?;
        Ok(Self { storage })
    }

    /// 从环境变量创建
    pub async fn from_env() -> Result<Self> {
        let storage = ModelStorage::from_env().await?;
        Ok(Self { storage })
    }

    /// 读取模型的输入数据（从comsrv）
    pub async fn read_model_inputs(
        &mut self,
        input_mappings: &[(u16, &str, u32, String)], // (channel_id, point_type, point_id, field_name)
    ) -> Result<HashMap<String, f64>> {
        if input_mappings.is_empty() {
            return Ok(HashMap::new());
        }

        // 构建查询列表
        let points: Vec<(u16, &str, u32)> = input_mappings
            .iter()
            .map(|(ch, pt, pid, _)| (*ch, *pt, *pid))
            .collect();

        // 批量读取
        let values = self.storage.read_comsrv_points(&points).await?;

        // 构建结果映射
        let mut results = HashMap::new();
        for (idx, (_, _, _, field_name)) in input_mappings.iter().enumerate() {
            if let Some(Some(value)) = values.get(idx) {
                results.insert(field_name.clone(), *value);
            }
        }

        debug!("Read {} model inputs", results.len());
        Ok(results)
    }

    /// 写入模型的计算结果
    pub async fn write_model_outputs(
        &mut self,
        model_id: &str,
        outputs: HashMap<String, f64>,
    ) -> Result<()> {
        let timestamp = chrono::Utc::now().timestamp_millis();

        // 构建批量更新
        let mut updates = Vec::new();
        for (field_name, value) in outputs.iter() {
            let monitor_value = MonitorValue::new(*value);

            updates.push(MonitorUpdate {
                model_id: model_id.to_string(),
                monitor_type: MonitorType::ModelOutput,
                field_name: field_name.clone(),
                value: monitor_value,
            });
        }

        // 批量写入
        self.storage.set_monitor_values(&updates).await?;

        // 同时保存完整的模型输出记录，转换数值格式
        let std_outputs: HashMap<String, voltage_libs::types::StandardFloat> = outputs
            .into_iter()
            .map(|(k, v)| (k, voltage_libs::types::StandardFloat::new(v)))
            .collect();

        let model_output = ModelOutput {
            model_id: model_id.to_string(),
            outputs: std_outputs,
            timestamp,
            execution_time_ms: 0, // 调用方可以设置实际执行时间
        };
        self.storage.set_model_output(&model_output).await?;

        info!("Wrote {} model outputs for {}", updates.len(), model_id);
        Ok(())
    }

    /// 读取模型的中间计算值
    pub async fn read_intermediate_values(
        &mut self,
        model_id: &str,
        field_names: &[String],
    ) -> Result<HashMap<String, Option<MonitorValue>>> {
        let mut results = HashMap::new();

        // 构建查询键
        let keys: Vec<MonitorKey> = field_names
            .iter()
            .map(|name| MonitorKey {
                model_id: model_id.to_string(),
                monitor_type: MonitorType::Intermediate,
                field_name: name.clone(),
            })
            .collect();

        // 批量读取
        let values = self.storage.get_monitor_values(&keys).await?;

        // 构建结果
        for (idx, field_name) in field_names.iter().enumerate() {
            results.insert(field_name.clone(), values.get(idx).cloned().flatten());
        }

        Ok(results)
    }

    /// 写入中间计算值
    pub async fn write_intermediate_value(
        &mut self,
        model_id: &str,
        field_name: &str,
        value: f64,
    ) -> Result<()> {
        let monitor_value = MonitorValue::new(value);

        self.storage
            .set_monitor_value(
                model_id,
                MonitorType::Intermediate,
                field_name,
                monitor_value,
            )
            .await
    }

    /// 获取模型的最后输出
    pub async fn get_last_model_output(&mut self, model_id: &str) -> Result<Option<ModelOutput>> {
        self.storage.get_model_output(model_id).await
    }
}

/// 监视值订阅器（用于实时监听变化）
pub struct MonitorSubscriber {
    pubsub: redis::aio::PubSub,
}

impl MonitorSubscriber {
    /// 创建订阅器
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| crate::error::ModelSrvError::RedisError(e.to_string()))?;

        let pubsub = client
            .get_async_pubsub()
            .await
            .map_err(|e| crate::error::ModelSrvError::RedisError(e.to_string()))?;

        Ok(Self { pubsub })
    }

    /// 订阅comsrv的点位更新
    pub async fn subscribe_comsrv_updates(&mut self, patterns: &[String]) -> Result<()> {
        // use redis::AsyncCommands;

        for pattern in patterns {
            self.pubsub
                .psubscribe(pattern)
                .await
                .map_err(|e| crate::error::ModelSrvError::RedisError(e.to_string()))?;
        }
        Ok(())
    }

    /// 接收更新消息
    pub async fn receive_update(&mut self) -> Result<Option<(String, String)>> {
        // use redis::AsyncCommands;
        use futures_util::StreamExt;

        let msg = self.pubsub.on_message().next().await;

        match msg {
            Some(msg) => {
                let channel = msg.get_channel_name().to_string();
                let payload: String = msg
                    .get_payload()
                    .map_err(|e| crate::error::ModelSrvError::RedisError(e.to_string()))?;
                Ok(Some((channel, payload)))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_name_to_point_id() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // 直接测试哈希逻辑
        let field1 = "temperature";
        let field2 = "pressure";

        let mut hasher1 = DefaultHasher::new();
        field1.hash(&mut hasher1);
        let id1 = hasher1.finish() as u32;

        let mut hasher2 = DefaultHasher::new();
        field2.hash(&mut hasher2);
        let id2 = hasher2.finish() as u32;

        let mut hasher3 = DefaultHasher::new();
        field1.hash(&mut hasher3);
        let id3 = hasher3.finish() as u32;

        assert_ne!(id1, id2); // 不同字段名应该有不同ID
        assert_eq!(id1, id3); // 相同字段名应该有相同ID
    }
}
