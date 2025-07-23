//! 优化的模型引擎
//!
//! 提供高性能的异步模型执行和缓存管理

use crate::cache::ModelCacheManager;
use crate::error::Result;
use crate::model::{ControlAction, ControlActionType, ModelDefinition};
use crate::storage::{ControlCommand, ModelStorage, MonitorKey, MonitorType};
use futures::future::join_all;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// 优化的模型引擎配置
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// 批处理大小
    pub batch_size: usize,
    /// 缓存TTL
    pub cache_ttl: Duration,
    /// 执行超时
    pub execution_timeout: Duration,
    /// 是否启用并行执行
    pub parallel_execution: bool,
    /// 最大并发模型数
    pub max_concurrent_models: usize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            cache_ttl: Duration::from_secs(5),
            execution_timeout: Duration::from_secs(30),
            parallel_execution: true,
            max_concurrent_models: 10,
        }
    }
}

/// 优化的模型引擎
pub struct OptimizedModelEngine {
    /// 模型定义
    models: Arc<RwLock<HashMap<String, ModelDefinition>>>,
    /// 控制动作
    actions: Arc<RwLock<HashMap<String, Vec<ControlAction>>>>,
    /// 缓存管理器
    cache: Arc<ModelCacheManager>,
    /// 存储接口
    storage: Arc<RwLock<ModelStorage>>,
    /// 配置
    config: EngineConfig,
    /// 执行统计
    stats: Arc<RwLock<ExecutionStats>>,
}

/// 执行统计
#[derive(Debug, Default)]
pub struct ExecutionStats {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub total_duration_ms: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

impl OptimizedModelEngine {
    /// 创建新的优化引擎
    pub async fn new(config: EngineConfig) -> Result<Self> {
        let storage = ModelStorage::from_env().await?;
        let cache = ModelCacheManager::new(config.cache_ttl);

        Ok(Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            actions: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(cache),
            storage: Arc::new(RwLock::new(storage)),
            config,
            stats: Arc::new(RwLock::new(ExecutionStats::default())),
        })
    }

    /// 加载模型（异步版本）
    pub async fn load_models(&self, pattern: &str) -> Result<()> {
        let mut storage = self.storage.write().await;
        let model_configs = storage.get_model_configs(pattern).await?;

        let mut models = self.models.write().await;
        let mut actions = self.actions.write().await;

        models.clear();
        actions.clear();

        for (key, config) in model_configs {
            match serde_json::from_str::<ModelDefinition>(&config) {
                Ok(model) => {
                    if model.enabled {
                        info!("Loaded model: {} ({})", model.name, model.id);
                        models.insert(model.id.clone(), model);
                    }
                }
                Err(e) => {
                    error!("Failed to parse model {}: {}", key, e);
                }
            }
        }

        info!("Loaded {} models", models.len());
        Ok(())
    }

    /// 执行所有模型（优化版本）
    pub async fn execute_all_models(&self) -> Result<()> {
        let start = Instant::now();
        let models = self.models.read().await.clone();

        if self.config.parallel_execution {
            // 并行执行
            let semaphore = Arc::new(tokio::sync::Semaphore::new(
                self.config.max_concurrent_models,
            ));
            let mut tasks = Vec::new();

            for (id, model) in models {
                let sem = semaphore.clone();
                let engine = self.clone_for_task();

                let task = tokio::spawn(async move {
                    let _permit = sem.acquire().await.unwrap();
                    if let Err(e) = engine.execute_single_model(&model).await {
                        error!("Failed to execute model {}: {}", id, e);
                    }
                });

                tasks.push(task);
            }

            // 等待所有任务完成
            join_all(tasks).await;
        } else {
            // 顺序执行
            for (id, model) in models {
                if let Err(e) = self.execute_single_model(&model).await {
                    error!("Failed to execute model {}: {}", id, e);
                }
            }
        }

        let duration = start.elapsed();
        let mut stats = self.stats.write().await;
        stats.total_duration_ms += duration.as_millis() as u64;

        info!("Executed all models in {:?}", duration);
        Ok(())
    }

    /// 执行单个模型
    async fn execute_single_model(&self, model: &ModelDefinition) -> Result<()> {
        let start = Instant::now();
        let mut stats = self.stats.write().await;
        stats.total_executions += 1;
        drop(stats);

        // 收集输入数据
        let inputs = self.collect_model_inputs(model).await?;

        // 处理模型（这里应该是实际的模型计算逻辑）
        let outputs = self.process_model(&model.id, &inputs).await?;

        // 存储结果
        self.store_model_outputs(&model.id, &model.output_key, outputs)
            .await?;

        // 检查并执行控制动作
        if let Some(actions) = self.actions.read().await.get(&model.id) {
            for action in actions {
                if action.enabled {
                    self.check_and_execute_action(action).await?;
                }
            }
        }

        let duration = start.elapsed();
        let mut stats = self.stats.write().await;
        stats.successful_executions += 1;
        stats.total_duration_ms += duration.as_millis() as u64;

        debug!("Model {} executed in {:?}", model.id, duration);
        Ok(())
    }

    /// 收集模型输入（使用缓存）
    async fn collect_model_inputs(
        &self,
        model: &ModelDefinition,
    ) -> Result<HashMap<String, Value>> {
        let mut inputs = HashMap::new();
        let mut keys_to_fetch = Vec::new();

        // 收集需要获取的键
        for mapping in &model.input_mappings {
            let cache_key = format!("{}:{}", mapping.source_key, mapping.source_field);

            // 尝试从缓存获取
            if let Some(cached) = self.cache.get_model_output(&cache_key).await {
                inputs.insert(mapping.target_field.clone(), cached);
            } else {
                keys_to_fetch.push(mapping);
            }
        }

        // 批量从Redis获取缺失的数据
        if !keys_to_fetch.is_empty() {
            let monitor_keys: Vec<MonitorKey> = keys_to_fetch
                .iter()
                .filter_map(|mapping| {
                    // 解析键格式：model_id:type:point_id
                    let parts: Vec<&str> = mapping.source_key.split(':').collect();
                    if parts.len() >= 3 {
                        let model_id = parts[0].to_string();
                        let monitor_type = match parts[1] {
                            "m" => MonitorType::Measurement,
                            "s" => MonitorType::Signal,
                            _ => return None,
                        };
                        let field_name = parts[2].to_string();
                        return Some(MonitorKey {
                            model_id,
                            monitor_type,
                            field_name,
                        });
                    }
                    None
                })
                .collect();

            let mut storage = self.storage.write().await;
            let values = storage.get_monitor_values(&monitor_keys).await?;

            // 更新缓存并收集输入
            for (i, value) in values.iter().enumerate() {
                if let Some(mv) = value {
                    let mapping = &keys_to_fetch[i];
                    let json_value =
                        Value::Number(serde_json::Number::from_f64(mv.raw_value()).unwrap());

                    // 更新缓存
                    let cache_key = format!("{}:{}", mapping.source_key, mapping.source_field);
                    self.cache
                        .update_model_output(cache_key, json_value.clone())
                        .await;

                    // 应用转换
                    let transformed = self.apply_transform(json_value, &mapping.transform)?;
                    inputs.insert(mapping.target_field.clone(), transformed);
                }
            }
        }

        Ok(inputs)
    }

    /// 应用数据转换
    fn apply_transform(&self, value: Value, transform: &Option<String>) -> Result<Value> {
        if let Some(expr) = transform {
            // 这里应该实现表达式计算逻辑
            // 暂时只返回原值
            warn!("Transform expression not implemented: {}", expr);
        }
        Ok(value)
    }

    /// 处理模型（实际的模型计算）
    async fn process_model(
        &self,
        model_id: &str,
        inputs: &HashMap<String, Value>,
    ) -> Result<Value> {
        // 这里应该是实际的模型处理逻辑
        // 现在只是返回一个示例输出
        let output = serde_json::json!({
            "model_id": model_id,
            "timestamp": chrono::Utc::now().timestamp_millis(),
            "inputs": inputs,
            "result": 42.0,
        });

        Ok(output)
    }

    /// 存储模型输出
    async fn store_model_outputs(
        &self,
        model_id: &str,
        output_key: &str,
        output: Value,
    ) -> Result<()> {
        // 更新缓存
        self.cache
            .update_model_output(output_key.to_string(), output.clone())
            .await;

        // 存储到Redis
        let mut storage = self.storage.write().await;
        storage.set_model_output_json(model_id, &output).await?;

        Ok(())
    }

    /// 检查并执行控制动作
    async fn check_and_execute_action(&self, action: &ControlAction) -> Result<()> {
        // 检查条件
        let should_execute = self.evaluate_conditions(&action.conditions).await?;

        if should_execute {
            // 创建控制命令
            let command_type = match action.action_type {
                ControlActionType::RemoteControl => crate::storage::ControlType::RemoteControl,
                ControlActionType::RemoteAdjust => crate::storage::ControlType::RemoteAdjust,
            };

            let command = ControlCommand::new(
                action.channel.parse().unwrap_or(0),
                action.point.parse().unwrap_or(0),
                command_type,
                action.value.parse().unwrap_or(0.0),
                format!("modsrv:{}", action.id),
            );

            // 发送命令
            let mut storage = self.storage.write().await;
            storage.send_control_command(&command).await?;

            info!("Executed control action: {} for model", action.name);
        }

        Ok(())
    }

    /// 评估条件
    async fn evaluate_conditions(
        &self,
        _conditions: &[crate::model::ControlActionCondition],
    ) -> Result<bool> {
        // 暂时返回true，实际应该评估条件
        Ok(true)
    }

    /// 克隆用于并行任务
    fn clone_for_task(&self) -> Self {
        Self {
            models: self.models.clone(),
            actions: self.actions.clone(),
            cache: self.cache.clone(),
            storage: self.storage.clone(),
            config: self.config.clone(),
            stats: self.stats.clone(),
        }
    }

    /// 获取执行统计
    pub async fn get_stats(&self) -> ExecutionStats {
        let stats = self.stats.read().await;
        let cache_info = self.cache.get_cache_info().await;

        ExecutionStats {
            total_executions: stats.total_executions,
            successful_executions: stats.successful_executions,
            failed_executions: stats.failed_executions,
            total_duration_ms: stats.total_duration_ms,
            cache_hits: cache_info.stats.hits,
            cache_misses: cache_info.stats.misses,
        }
    }

    /// 清理过期缓存
    pub async fn cleanup_cache(&self) {
        self.cache.cleanup_expired().await;
    }
}
