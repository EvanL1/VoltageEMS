use crate::error::{HisSrvError, Result};
use crate::storage::StorageManager;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration as TokioDuration};
use tracing::{debug, error, info, warn};

/// 保留策略类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RetentionType {
    /// 基于时间的保留
    TimeBased {
        /// 保留时长（秒）
        duration_seconds: u64,
    },
    /// 基于空间的保留
    SpaceBased {
        /// 最大存储大小（字节）
        max_size_bytes: u64,
    },
    /// 基于记录数的保留
    CountBased {
        /// 最大记录数
        max_count: u64,
    },
    /// 混合策略（满足任一条件即触发清理）
    Hybrid {
        duration_seconds: Option<u64>,
        max_size_bytes: Option<u64>,
        max_count: Option<u64>,
    },
}

/// 降采样策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownsamplingPolicy {
    /// 源数据保留时长（秒）
    source_retention_seconds: u64,
    /// 降采样间隔（秒）
    interval_seconds: u64,
    /// 聚合方法
    aggregation_method: AggregationMethod,
    /// 目标测量名称后缀
    target_suffix: String,
}

/// 聚合方法
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationMethod {
    /// 平均值
    Mean,
    /// 最大值
    Max,
    /// 最小值
    Min,
    /// 总和
    Sum,
    /// 计数
    Count,
    /// 第一个值
    First,
    /// 最后一个值
    Last,
    /// 中位数
    Median,
    /// 标准差
    StdDev,
}

/// 保留策略配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicyConfig {
    /// 策略名称
    pub name: String,
    /// 是否启用
    pub enabled: bool,
    /// 应用到的测量名称模式（支持通配符）
    pub measurement_patterns: Vec<String>,
    /// 保留类型
    pub retention_type: RetentionType,
    /// 降采样策略（可选）
    pub downsampling: Option<Vec<DownsamplingPolicy>>,
    /// 执行间隔（秒）
    pub execution_interval_seconds: u64,
}

/// 策略执行统计
#[derive(Debug, Clone, Default)]
pub struct PolicyStatistics {
    /// 上次执行时间
    pub last_execution: Option<DateTime<Utc>>,
    /// 总执行次数
    pub total_executions: u64,
    /// 成功执行次数
    pub successful_executions: u64,
    /// 失败执行次数  
    pub failed_executions: u64,
    /// 删除的记录数
    pub total_records_deleted: u64,
    /// 降采样的记录数
    pub total_records_downsampled: u64,
    /// 上次执行耗时（毫秒）
    pub last_execution_duration_ms: u64,
}

/// 保留策略管理器
pub struct RetentionPolicyManager {
    /// 策略配置
    policies: Arc<RwLock<HashMap<String, RetentionPolicyConfig>>>,
    /// 策略统计
    statistics: Arc<RwLock<HashMap<String, PolicyStatistics>>>,
    /// 存储管理器
    storage_manager: Arc<RwLock<StorageManager>>,
    /// 是否正在运行
    running: Arc<RwLock<bool>>,
}

impl RetentionPolicyManager {
    /// 创建新的保留策略管理器
    pub fn new(storage_manager: Arc<RwLock<StorageManager>>) -> Self {
        Self {
            policies: Arc::new(RwLock::new(HashMap::new())),
            statistics: Arc::new(RwLock::new(HashMap::new())),
            storage_manager,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// 添加策略
    pub async fn add_policy(&self, policy: RetentionPolicyConfig) -> Result<()> {
        let name = policy.name.clone();
        info!("Adding retention policy: {}", name);

        self.policies.write().await.insert(name.clone(), policy);
        self.statistics
            .write()
            .await
            .insert(name, PolicyStatistics::default());

        Ok(())
    }

    /// 移除策略
    pub async fn remove_policy(&self, name: &str) -> Result<()> {
        info!("Removing retention policy: {}", name);

        self.policies.write().await.remove(name);
        self.statistics.write().await.remove(name);

        Ok(())
    }

    /// 获取策略
    pub async fn get_policy(&self, name: &str) -> Option<RetentionPolicyConfig> {
        self.policies.read().await.get(name).cloned()
    }

    /// 获取所有策略
    pub async fn get_all_policies(&self) -> Vec<RetentionPolicyConfig> {
        self.policies.read().await.values().cloned().collect()
    }

    /// 获取策略统计
    pub async fn get_statistics(&self, name: &str) -> Option<PolicyStatistics> {
        self.statistics.read().await.get(name).cloned()
    }

    /// 获取所有统计
    pub async fn get_all_statistics(&self) -> HashMap<String, PolicyStatistics> {
        self.statistics.read().await.clone()
    }

    /// 启动策略执行器
    pub async fn start(&self) -> Result<()> {
        if *self.running.read().await {
            return Err(HisSrvError::ValidationError(
                "Retention policy manager already running".to_string(),
            ));
        }

        *self.running.write().await = true;
        info!("Starting retention policy manager");

        // 为每个策略启动独立的执行器
        let policies = self.policies.read().await.clone();
        for (name, policy) in policies {
            if policy.enabled {
                let manager = self.clone_for_task();
                let policy_name = name.clone();

                tokio::spawn(async move {
                    manager.run_policy_executor(policy_name, policy).await;
                });
            }
        }

        Ok(())
    }

    /// 停止策略执行器
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping retention policy manager");
        *self.running.write().await = false;
        Ok(())
    }

    /// 立即执行指定策略
    pub async fn execute_policy(&self, name: &str) -> Result<()> {
        let policy = self
            .get_policy(name)
            .await
            .ok_or_else(|| HisSrvError::NotFound(format!("Policy {} not found", name)))?;

        if !policy.enabled {
            return Err(HisSrvError::ValidationError(format!(
                "Policy {} is disabled",
                name
            )));
        }

        self.execute_single_policy(&name, &policy).await
    }

    /// 执行单个策略
    async fn execute_single_policy(
        &self,
        name: &str,
        policy: &RetentionPolicyConfig,
    ) -> Result<()> {
        info!("Executing retention policy: {}", name);
        let start_time = Utc::now();

        let result = match &policy.retention_type {
            RetentionType::TimeBased { duration_seconds } => {
                self.execute_time_based_retention(policy, *duration_seconds)
                    .await
            }
            RetentionType::SpaceBased { max_size_bytes } => {
                self.execute_space_based_retention(policy, *max_size_bytes)
                    .await
            }
            RetentionType::CountBased { max_count } => {
                self.execute_count_based_retention(policy, *max_count).await
            }
            RetentionType::Hybrid {
                duration_seconds,
                max_size_bytes,
                max_count,
            } => {
                self.execute_hybrid_retention(
                    policy,
                    *duration_seconds,
                    *max_size_bytes,
                    *max_count,
                )
                .await
            }
        };

        // 更新统计信息
        let duration_ms = (Utc::now() - start_time).num_milliseconds() as u64;
        let mut stats = self.statistics.write().await;
        if let Some(stat) = stats.get_mut(name) {
            stat.last_execution = Some(Utc::now());
            stat.total_executions += 1;
            stat.last_execution_duration_ms = duration_ms;

            match result {
                Ok(deleted_count) => {
                    stat.successful_executions += 1;
                    stat.total_records_deleted += deleted_count;
                    info!(
                        "Policy {} executed successfully, deleted {} records",
                        name, deleted_count
                    );
                }
                Err(ref e) => {
                    stat.failed_executions += 1;
                    error!("Policy {} execution failed: {}", name, e);
                }
            }
        }

        // 执行降采样（如果配置了）
        if let Some(downsampling_policies) = &policy.downsampling {
            for ds_policy in downsampling_policies {
                if let Err(e) = self.execute_downsampling(policy, ds_policy).await {
                    error!("Downsampling failed for policy {}: {}", name, e);
                }
            }
        }

        result.map(|_| ())
    }

    /// 执行基于时间的保留策略
    async fn execute_time_based_retention(
        &self,
        policy: &RetentionPolicyConfig,
        duration_seconds: u64,
    ) -> Result<u64> {
        let cutoff_time = Utc::now() - Duration::seconds(duration_seconds as i64);
        let mut total_deleted = 0u64;

        let storage_manager = self.storage_manager.read().await;

        // 对每个匹配的测量执行删除
        for pattern in &policy.measurement_patterns {
            debug!(
                "Processing pattern: {} with cutoff time: {}",
                pattern, cutoff_time
            );

            // 这里需要根据实际的存储后端实现删除逻辑
            // 示例：使用 InfluxDB 的删除功能
            if let Some(backend) = storage_manager.get_backend(Some("influxdb")) {
                // 构造删除查询
                let delete_query = format!(
                    r#"DELETE FROM "{}" WHERE time < '{}'"#,
                    pattern,
                    cutoff_time.to_rfc3339()
                );

                // 注意：实际实现需要在 StorageBackend trait 中添加删除方法
                // 这里仅作示例
                debug!("Would execute delete query: {}", delete_query);

                // total_deleted += backend.delete_before(pattern, cutoff_time).await?;
            }
        }

        Ok(total_deleted)
    }

    /// 执行基于空间的保留策略
    async fn execute_space_based_retention(
        &self,
        policy: &RetentionPolicyConfig,
        max_size_bytes: u64,
    ) -> Result<u64> {
        // 实现基于空间的清理逻辑
        // 这需要查询当前存储使用情况，然后删除最旧的数据直到满足空间限制

        warn!(
            "Space-based retention not yet implemented for policy: {}",
            policy.name
        );
        Ok(0)
    }

    /// 执行基于记录数的保留策略
    async fn execute_count_based_retention(
        &self,
        policy: &RetentionPolicyConfig,
        max_count: u64,
    ) -> Result<u64> {
        // 实现基于记录数的清理逻辑
        // 这需要查询当前记录数，然后删除最旧的记录直到满足数量限制

        warn!(
            "Count-based retention not yet implemented for policy: {}",
            policy.name
        );
        Ok(0)
    }

    /// 执行混合保留策略
    async fn execute_hybrid_retention(
        &self,
        policy: &RetentionPolicyConfig,
        duration_seconds: Option<u64>,
        max_size_bytes: Option<u64>,
        max_count: Option<u64>,
    ) -> Result<u64> {
        let mut total_deleted = 0u64;

        // 执行时间保留
        if let Some(duration) = duration_seconds {
            match self.execute_time_based_retention(policy, duration).await {
                Ok(deleted) => total_deleted += deleted,
                Err(e) => warn!("Time-based retention failed: {}", e),
            }
        }

        // 执行空间保留
        if let Some(max_size) = max_size_bytes {
            match self.execute_space_based_retention(policy, max_size).await {
                Ok(deleted) => total_deleted += deleted,
                Err(e) => warn!("Space-based retention failed: {}", e),
            }
        }

        // 执行计数保留
        if let Some(max_count_val) = max_count {
            match self
                .execute_count_based_retention(policy, max_count_val)
                .await
            {
                Ok(deleted) => total_deleted += deleted,
                Err(e) => warn!("Count-based retention failed: {}", e),
            }
        }

        Ok(total_deleted)
    }

    /// 执行降采样
    async fn execute_downsampling(
        &self,
        policy: &RetentionPolicyConfig,
        downsampling: &DownsamplingPolicy,
    ) -> Result<()> {
        info!(
            "Executing downsampling for policy {} with interval {} seconds",
            policy.name, downsampling.interval_seconds
        );

        // 实现降采样逻辑
        // 这需要查询原始数据，应用聚合函数，然后写入降采样后的数据

        warn!(
            "Downsampling not yet implemented for policy: {}",
            policy.name
        );
        Ok(())
    }

    /// 策略执行器循环
    async fn run_policy_executor(self, name: String, policy: RetentionPolicyConfig) {
        let mut interval_timer =
            interval(TokioDuration::from_secs(policy.execution_interval_seconds));

        loop {
            interval_timer.tick().await;

            if !*self.running.read().await {
                info!("Policy executor {} stopping", name);
                break;
            }

            if let Err(e) = self.execute_single_policy(&name, &policy).await {
                error!("Failed to execute policy {}: {}", name, e);
            }
        }
    }

    /// 为任务克隆管理器
    fn clone_for_task(&self) -> Self {
        Self {
            policies: Arc::clone(&self.policies),
            statistics: Arc::clone(&self.statistics),
            storage_manager: Arc::clone(&self.storage_manager),
            running: Arc::clone(&self.running),
        }
    }
}

/// 默认保留策略
pub fn default_retention_policies() -> Vec<RetentionPolicyConfig> {
    vec![
        // 原始数据保留7天
        RetentionPolicyConfig {
            name: "raw_data_7d".to_string(),
            enabled: true,
            measurement_patterns: vec!["*".to_string()],
            retention_type: RetentionType::TimeBased {
                duration_seconds: 7 * 24 * 60 * 60, // 7天
            },
            downsampling: Some(vec![
                // 1小时后降采样到5分钟
                DownsamplingPolicy {
                    source_retention_seconds: 60 * 60, // 1小时
                    interval_seconds: 5 * 60,          // 5分钟
                    aggregation_method: AggregationMethod::Mean,
                    target_suffix: "_5m".to_string(),
                },
                // 1天后降采样到1小时
                DownsamplingPolicy {
                    source_retention_seconds: 24 * 60 * 60, // 1天
                    interval_seconds: 60 * 60,              // 1小时
                    aggregation_method: AggregationMethod::Mean,
                    target_suffix: "_1h".to_string(),
                },
            ]),
            execution_interval_seconds: 60 * 60, // 每小时执行一次
        },
        // 事件数据保留30天
        RetentionPolicyConfig {
            name: "events_30d".to_string(),
            enabled: true,
            measurement_patterns: vec!["events*".to_string()],
            retention_type: RetentionType::TimeBased {
                duration_seconds: 30 * 24 * 60 * 60, // 30天
            },
            downsampling: None,
            execution_interval_seconds: 24 * 60 * 60, // 每天执行一次
        },
        // 系统状态数据保留1天
        RetentionPolicyConfig {
            name: "system_status_1d".to_string(),
            enabled: true,
            measurement_patterns: vec!["system*".to_string()],
            retention_type: RetentionType::TimeBased {
                duration_seconds: 24 * 60 * 60, // 1天
            },
            downsampling: None,
            execution_interval_seconds: 60 * 60, // 每小时执行一次
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retention_policy_manager() {
        // 创建模拟的存储管理器
        let storage_manager = Arc::new(RwLock::new(StorageManager::new()));
        let manager = RetentionPolicyManager::new(storage_manager);

        // 添加策略
        let policy = RetentionPolicyConfig {
            name: "test_policy".to_string(),
            enabled: true,
            measurement_patterns: vec!["test_*".to_string()],
            retention_type: RetentionType::TimeBased {
                duration_seconds: 3600,
            },
            downsampling: None,
            execution_interval_seconds: 60,
        };

        manager.add_policy(policy.clone()).await.unwrap();

        // 验证策略已添加
        let retrieved = manager.get_policy("test_policy").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test_policy");

        // 获取统计信息
        let stats = manager.get_statistics("test_policy").await;
        assert!(stats.is_some());
        assert_eq!(stats.unwrap().total_executions, 0);

        // 移除策略
        manager.remove_policy("test_policy").await.unwrap();
        assert!(manager.get_policy("test_policy").await.is_none());
    }
}
