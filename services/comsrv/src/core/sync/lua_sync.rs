//! Lua 脚本同步管理器
//!
//! 管理 ComsRv 与其他服务间的双向数据同步

use crate::utils::error::{ComSrvError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use voltage_libs::redis::RedisClient;

/// Lua 同步配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuaSyncConfig {
    /// 是否启用同步
    pub enabled: bool,
    /// Lua 脚本路径
    pub script_path: String,
    /// 脚本 SHA（可选，如果提供则使用 EVALSHA）
    pub script_sha: Option<String>,
    /// 批量同步大小
    pub batch_size: usize,
    /// 同步重试次数
    pub retry_count: u32,
    /// 是否异步同步（不阻塞主流程）
    pub async_sync: bool,
}

impl Default for LuaSyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            script_path: "scripts/sync.lua".to_string(),
            script_sha: None,
            batch_size: 100,
            retry_count: 3,
            async_sync: true,
        }
    }
}

/// 同步统计信息
#[derive(Debug, Clone, Default)]
pub struct SyncStats {
    pub total_synced: u64,
    pub sync_success: u64,
    pub sync_failed: u64,
    pub no_mapping: u64,
    pub last_sync_error: Option<String>,
}

/// Lua 同步管理器
pub struct LuaSyncManager {
    config: LuaSyncConfig,
    redis_client: Arc<Mutex<RedisClient>>,
    script_sha: Option<String>,
    stats: Arc<Mutex<SyncStats>>,
}

impl std::fmt::Debug for LuaSyncManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LuaSyncManager")
            .field("config", &self.config)
            .field("redis_client", &"<RedisClient>")
            .field("script_sha", &self.script_sha)
            .field("stats", &"<Arc<Mutex<SyncStats>>")
            .finish()
    }
}

impl LuaSyncManager {
    /// 创建新的同步管理器
    pub async fn new(config: LuaSyncConfig, redis_client: RedisClient) -> Result<Self> {
        let mut manager = Self {
            config: config.clone(),
            redis_client: Arc::new(Mutex::new(redis_client)),
            script_sha: config.script_sha.clone(),
            stats: Arc::new(Mutex::new(SyncStats::default())),
        };

        // 如果没有提供 SHA，尝试加载脚本
        if manager.script_sha.is_none() && config.enabled {
            manager.load_script().await?;
        }

        Ok(manager)
    }

    /// 加载 Lua 脚本
    pub async fn load_script(&mut self) -> Result<()> {
        if let Ok(script_content) = tokio::fs::read_to_string(&self.config.script_path).await {
            let mut client = self.redis_client.lock().await;
            match client.script_load(&script_content).await {
                Ok(sha) => {
                    info!("Loaded Lua sync script, SHA: {}", sha);
                    self.script_sha = Some(sha.clone());
                    self.config.script_sha = Some(sha);
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to load Lua script: {}", e);
                    Err(ComSrvError::ConfigError(format!(
                        "Failed to load Lua script: {}",
                        e
                    )))
                }
            }
        } else {
            Err(ComSrvError::ConfigError(format!(
                "Failed to read Lua script from: {}",
                self.config.script_path
            )))
        }
    }

    /// 同步测量数据
    pub async fn sync_measurement(
        &self,
        channel_id: u16,
        telemetry_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let args = vec![
            "sync_measurement".to_string(),
            channel_id.to_string(),
            telemetry_type.to_string(),
            point_id.to_string(),
            format!("{:.6}", value),
            chrono::Utc::now().timestamp().to_string(),
        ];

        if self.config.async_sync {
            // 异步同步，不阻塞主流程
            let client = self.redis_client.clone();
            let script_sha = self.script_sha.clone();
            let stats = self.stats.clone();

            tokio::spawn(async move {
                let _ = Self::execute_sync(client, script_sha, args, stats).await;
            });

            Ok(())
        } else {
            // 同步执行
            Self::execute_sync(
                self.redis_client.clone(),
                self.script_sha.clone(),
                args,
                self.stats.clone(),
            )
            .await
        }
    }

    /// 批量同步数据
    pub async fn batch_sync_measurements(
        &self,
        updates: Vec<(u16, String, u32, f64)>, // (channel_id, telemetry_type, point_id, value)
    ) -> Result<()> {
        if !self.config.enabled || updates.is_empty() {
            return Ok(());
        }

        // 构建批量更新数据
        let batch_data: Vec<serde_json::Value> = updates
            .into_iter()
            .map(|(channel_id, telemetry_type, point_id, value)| {
                serde_json::json!({
                    "channel_id": channel_id,
                    "telemetry_type": telemetry_type,
                    "point_id": point_id,
                    "value": format!("{:.6}", value),
                })
            })
            .collect();

        let batch_json = serde_json::to_string(&batch_data)?;
        let args = vec!["batch_sync".to_string(), batch_json];

        if self.config.async_sync {
            let client = self.redis_client.clone();
            let script_sha = self.script_sha.clone();
            let stats = self.stats.clone();

            tokio::spawn(async move {
                let _ = Self::execute_sync(client, script_sha, args, stats).await;
            });

            Ok(())
        } else {
            Self::execute_sync(
                self.redis_client.clone(),
                self.script_sha.clone(),
                args,
                self.stats.clone(),
            )
            .await
        }
    }

    /// 执行同步操作
    async fn execute_sync(
        redis_client: Arc<Mutex<RedisClient>>,
        script_sha: Option<String>,
        args: Vec<String>,
        stats: Arc<Mutex<SyncStats>>,
    ) -> Result<()> {
        let mut client = redis_client.lock().await;

        let result = if let Some(sha) = script_sha {
            let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            client.evalsha(&sha, &[], &args_refs).await
        } else {
            return Err(ComSrvError::ConfigError(
                "Lua script not loaded".to_string(),
            ));
        };

        let mut stats = stats.lock().await;
        stats.total_synced += 1;

        match result {
            Ok(redis::Value::BulkString(data)) => {
                let response = String::from_utf8_lossy(&data);
                if response == "OK" {
                    stats.sync_success += 1;
                    debug!("Sync successful");
                    Ok(())
                } else if response == "NO_MAPPING" {
                    stats.no_mapping += 1;
                    debug!("No mapping found for sync");
                    Ok(())
                } else if response.starts_with("SUCCESS:") {
                    // 批量同步响应
                    stats.sync_success += 1;
                    debug!("Batch sync result: {}", response);
                    Ok(())
                } else {
                    stats.sync_failed += 1;
                    stats.last_sync_error = Some(response.to_string());
                    warn!("Sync returned unexpected response: {}", response);
                    Ok(())
                }
            }
            Ok(redis::Value::Okay) => {
                stats.sync_success += 1;
                Ok(())
            }
            Ok(redis::Value::Nil) => {
                stats.no_mapping += 1;
                Ok(())
            }
            Ok(other) => {
                stats.sync_failed += 1;
                stats.last_sync_error = Some(format!("Unexpected response: {:?}", other));
                warn!("Unexpected sync response: {:?}", other);
                Ok(())
            }
            Err(e) => {
                stats.sync_failed += 1;
                stats.last_sync_error = Some(e.to_string());
                error!("Sync failed: {}", e);
                Err(ComSrvError::RedisError(e.to_string()))
            }
        }
    }

    /// 获取同步统计信息
    pub async fn get_stats(&self) -> SyncStats {
        self.stats.lock().await.clone()
    }

    /// 重置统计信息
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.lock().await;
        *stats = SyncStats::default();
    }

    /// 检查脚本是否已加载
    pub async fn is_script_loaded(&self) -> bool {
        if let Some(sha) = &self.script_sha {
            let mut client = self.redis_client.lock().await;
            if let Ok(exists) = client.script_exists(&[sha.as_str()]).await {
                return exists.get(0).copied().unwrap_or(false);
            }
        }
        false
    }

    /// 启用/禁用同步
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    /// 获取配置
    pub fn config(&self) -> &LuaSyncConfig {
        &self.config
    }
}

/// 同步 trait（供其他模块使用）
#[async_trait]
pub trait DataSync: Send + Sync {
    /// 同步单个测量点
    async fn sync_measurement(
        &self,
        channel_id: u16,
        telemetry_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()>;

    /// 批量同步
    async fn batch_sync(&self, updates: Vec<(u16, String, u32, f64)>) -> Result<()>;
}

#[async_trait]
impl DataSync for LuaSyncManager {
    async fn sync_measurement(
        &self,
        channel_id: u16,
        telemetry_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        self.sync_measurement(channel_id, telemetry_type, point_id, value)
            .await
    }

    async fn batch_sync(&self, updates: Vec<(u16, String, u32, f64)>) -> Result<()> {
        self.batch_sync_measurements(updates).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_config_default() {
        let config = LuaSyncConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.script_path, "scripts/sync.lua");
        assert_eq!(config.batch_size, 100);
        assert_eq!(config.retry_count, 3);
        assert!(config.async_sync);
    }

    #[test]
    fn test_sync_stats_default() {
        let stats = SyncStats::default();
        assert_eq!(stats.total_synced, 0);
        assert_eq!(stats.sync_success, 0);
        assert_eq!(stats.sync_failed, 0);
        assert_eq!(stats.no_mapping, 0);
        assert!(stats.last_sync_error.is_none());
    }
}
