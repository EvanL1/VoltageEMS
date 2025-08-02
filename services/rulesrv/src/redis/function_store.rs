use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info};

use crate::rules::{Rule, RuleGroup};

/// Redis 存储实现使用 Redis Functions
pub struct RedisFunctionStore {
    redis_client: voltage_libs::redis::RedisClient,
    key_prefix: String,
}

impl RedisFunctionStore {
    /// 创建新的 Redis 存储
    pub async fn new(redis_url: &str, key_prefix: Option<&str>) -> Result<Self> {
        let redis_client = voltage_libs::redis::RedisClient::new(redis_url).await?;
        let key_prefix = key_prefix.unwrap_or("rulesrv").to_string();

        Ok(Self {
            redis_client,
            key_prefix,
        })
    }

    /// 生成带前缀的键
    fn make_key(&self, key: &str) -> String {
        format!("{}:{}", self.key_prefix, key)
    }

    /// 获取字符串值 (保持向后兼容)
    pub async fn get_string(&mut self, key: &str) -> Result<Option<String>> {
        let value: Option<String> = self.redis_client.get(key).await?;
        Ok(value)
    }

    /// 设置字符串值 (保持向后兼容)
    pub async fn set_string(&mut self, key: &str, value: &str) -> Result<()> {
        self.redis_client.set(key, value).await?;
        Ok(())
    }

    /// 保存规则使用 Redis Function
    pub async fn save_rule(&mut self, rule: &Rule) -> Result<()> {
        let rule_json = serde_json::to_string(rule)?;
        let keys = vec![rule.id.clone()];
        let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        let args = vec![rule_json.as_str()];

        match self
            .redis_client
            .fcall::<String>("store_rule", &key_refs, &args)
            .await
        {
            Ok(_) => {
                info!(
                    "Saved rule: {} ({}) using Redis Function",
                    rule.name, rule.id
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to save rule {} using Redis Function: {}",
                    rule.id, e
                );
                Err(anyhow::anyhow!("Redis Function error: {}", e))
            }
        }
    }

    /// 获取规则
    pub async fn get_rule(&mut self, rule_id: &str) -> Result<Option<Rule>> {
        let keys = [rule_id.to_string()];
        let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        let args: Vec<&str> = vec![];

        match self
            .redis_client
            .fcall::<String>("get_rule", &key_refs, &args)
            .await
        {
            Ok(rule_json) => match serde_json::from_str::<Rule>(&rule_json) {
                Ok(rule) => {
                    debug!("Retrieved rule: {} using Redis Function", rule_id);
                    Ok(Some(rule))
                }
                Err(e) => {
                    error!("Failed to deserialize rule {}: {}", rule_id, e);
                    Err(anyhow::anyhow!("Deserialization error: {}", e))
                }
            },
            Err(e) => {
                let error_msg = format!("{}", e);
                if error_msg.contains("Rule not found") {
                    debug!("Rule not found: {}", rule_id);
                    Ok(None)
                } else {
                    error!("Failed to get rule {} using Redis Function: {}", rule_id, e);
                    Err(anyhow::anyhow!("Redis Function error: {}", e))
                }
            }
        }
    }

    /// 删除规则
    pub async fn delete_rule(&mut self, rule_id: &str) -> Result<bool> {
        let keys = [rule_id.to_string()];
        let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        let args: Vec<&str> = vec![];

        match self
            .redis_client
            .fcall::<String>("delete_rule", &key_refs, &args)
            .await
        {
            Ok(_) => {
                info!("Deleted rule: {} using Redis Function", rule_id);
                Ok(true)
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                if error_msg.contains("Rule not found") {
                    debug!("Rule not found for deletion: {}", rule_id);
                    Ok(false)
                } else {
                    error!(
                        "Failed to delete rule {} using Redis Function: {}",
                        rule_id, e
                    );
                    Err(anyhow::anyhow!("Redis Function error: {}", e))
                }
            }
        }
    }

    /// 查询规则使用 Redis Function
    pub async fn query_rules(&mut self, query: &RuleQuery) -> Result<RuleQueryResult> {
        let query_json = serde_json::to_string(query)?;
        let keys: Vec<String> = vec![];
        let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        let args = vec![query_json.as_str()];

        match self
            .redis_client
            .fcall::<String>("query_rules", &key_refs, &args)
            .await
        {
            Ok(result_json) => match serde_json::from_str::<RuleQueryResult>(&result_json) {
                Ok(result) => {
                    debug!("Queried {} rules using Redis Function", result.total);
                    Ok(result)
                }
                Err(e) => {
                    error!("Failed to deserialize query result: {}", e);
                    Err(anyhow::anyhow!("Deserialization error: {}", e))
                }
            },
            Err(e) => {
                error!("Failed to query rules using Redis Function: {}", e);
                Err(anyhow::anyhow!("Redis Function error: {}", e))
            }
        }
    }

    /// 获取所有规则 (便利方法)
    pub async fn list_rules(&mut self) -> Result<Vec<Rule>> {
        let query = RuleQuery::default();
        let result = self.query_rules(&query).await?;
        Ok(result.data)
    }

    /// 执行 DAG 规则
    pub async fn execute_dag_rule(
        &mut self,
        rule_id: &str,
        context: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult> {
        let context_json = serde_json::to_string(context)?;
        let keys = [rule_id.to_string()];
        let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        let args = vec![context_json.as_str()];

        match self
            .redis_client
            .fcall::<String>("execute_dag_rule", &key_refs, &args)
            .await
        {
            Ok(result_json) => match serde_json::from_str::<ExecutionResult>(&result_json) {
                Ok(result) => {
                    debug!("Executed DAG rule: {} using Redis Function", rule_id);
                    Ok(result)
                }
                Err(e) => {
                    error!("Failed to deserialize execution result: {}", e);
                    Err(anyhow::anyhow!("Deserialization error: {}", e))
                }
            },
            Err(e) => {
                error!(
                    "Failed to execute DAG rule {} using Redis Function: {}",
                    rule_id, e
                );
                Err(anyhow::anyhow!("Redis Function error: {}", e))
            }
        }
    }

    /// 更新规则状态
    pub async fn update_rule_state(
        &mut self,
        rule_id: &str,
        state_update: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let state_json = serde_json::to_string(state_update)?;
        let keys = [rule_id.to_string()];
        let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        let args = vec![state_json.as_str()];

        match self
            .redis_client
            .fcall::<String>("update_rule_state", &key_refs, &args)
            .await
        {
            Ok(_) => {
                debug!("Updated rule state: {} using Redis Function", rule_id);
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to update rule state {} using Redis Function: {}",
                    rule_id, e
                );
                Err(anyhow::anyhow!("Redis Function error: {}", e))
            }
        }
    }

    /// 获取规则统计
    pub async fn get_rule_stats(&mut self) -> Result<RuleStats> {
        let keys: Vec<String> = vec![];
        let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        let args: Vec<&str> = vec![];

        match self
            .redis_client
            .fcall::<String>("get_rule_stats", &key_refs, &args)
            .await
        {
            Ok(stats_json) => match serde_json::from_str::<RuleStats>(&stats_json) {
                Ok(stats) => {
                    debug!("Retrieved rule statistics using Redis Function");
                    Ok(stats)
                }
                Err(e) => {
                    error!("Failed to deserialize rule stats: {}", e);
                    Err(anyhow::anyhow!("Deserialization error: {}", e))
                }
            },
            Err(e) => {
                error!("Failed to get rule stats using Redis Function: {}", e);
                Err(anyhow::anyhow!("Redis Function error: {}", e))
            }
        }
    }

    // Rule Group operations (using traditional Redis commands for now)
    // These could be moved to Redis Functions in a future iteration

    /// 保存规则组
    pub async fn save_rule_group(&mut self, group: &RuleGroup) -> Result<()> {
        let key = self.make_key(&format!("group:{}", group.id));
        let value = serde_json::to_string(group)?;

        self.redis_client.set(&key, value).await?;

        // 将组ID添加到组列表 - 使用直接的 Redis 命令
        let list_key = self.make_key("groups");
        {
            let conn = self.redis_client.get_connection_mut();
            let _: () = redis::AsyncCommands::sadd(conn, &list_key, &group.id).await?;
        }

        info!("Saved rule group: {} ({})", group.name, group.id);
        Ok(())
    }

    /// 获取规则组
    pub async fn get_rule_group(&mut self, group_id: &str) -> Result<Option<RuleGroup>> {
        let key = self.make_key(&format!("group:{}", group_id));
        let value: Option<String> = self.redis_client.get(&key).await?;

        match value {
            Some(json) => {
                let group: RuleGroup = serde_json::from_str(&json)?;
                Ok(Some(group))
            }
            None => Ok(None),
        }
    }

    /// 删除规则组
    pub async fn delete_rule_group(&mut self, group_id: &str) -> Result<bool> {
        // 检查组内是否还有规则
        let group_rules_key = self.make_key(&format!("group:{}:rules", group_id));
        let rule_count: usize = {
            let conn = self.redis_client.get_connection_mut();
            redis::AsyncCommands::scard(conn, &group_rules_key).await?
        };

        if rule_count > 0 {
            return Err(anyhow::anyhow!(
                "Cannot delete group {} with {} rules",
                group_id,
                rule_count
            ));
        }

        // 删除组
        let key = self.make_key(&format!("group:{}", group_id));
        let keys = vec![key.as_str()];
        let deleted_count = self.redis_client.del(&keys).await?;
        let deleted = deleted_count > 0;

        if deleted {
            // 从组列表中移除和删除组的规则列表键
            let list_key = self.make_key("groups");
            let group_rule_keys = vec![group_rules_key.as_str()];

            {
                let conn = self.redis_client.get_connection_mut();
                let _: () = redis::AsyncCommands::srem(conn, &list_key, group_id).await?;
            }

            let _: u32 = self.redis_client.del(&group_rule_keys).await?;

            info!("Deleted rule group: {}", group_id);
        }

        Ok(deleted)
    }

    /// 获取所有规则组
    pub async fn list_rule_groups(&mut self) -> Result<Vec<RuleGroup>> {
        // 获取所有组ID
        let list_key = self.make_key("groups");
        let group_ids: Vec<String> = {
            let conn = self.redis_client.get_connection_mut();
            redis::AsyncCommands::smembers(conn, &list_key).await?
        };

        // 批量获取组
        if group_ids.is_empty() {
            return Ok(Vec::new());
        }

        let keys: Vec<String> = group_ids
            .iter()
            .map(|id| self.make_key(&format!("group:{}", id)))
            .collect();
        let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();

        let values: Vec<Option<String>> = self.redis_client.mget(&key_refs).await?;

        let mut groups = Vec::new();
        for (i, value) in values.iter().enumerate() {
            if let Some(json) = value {
                match serde_json::from_str::<RuleGroup>(json) {
                    Ok(group) => groups.push(group),
                    Err(e) => {
                        error!("Failed to deserialize group {}: {}", group_ids[i], e);
                    }
                }
            }
        }

        Ok(groups)
    }

    /// 获取组内的规则
    pub async fn get_group_rules(&mut self, group_id: &str) -> Result<Vec<Rule>> {
        // 使用查询功能获取组内规则
        let query = RuleQuery {
            group_id: Some(group_id.to_string()),
            ..Default::default()
        };
        let result = self.query_rules(&query).await?;
        Ok(result.data)
    }

    /// 保存执行历史
    pub async fn save_execution_history(
        &mut self,
        rule_id: &str,
        execution_result: &ExecutionHistory,
    ) -> Result<()> {
        let key = self.make_key(&format!("history:{}", rule_id));
        let value = serde_json::to_string(execution_result)?;

        // 使用列表存储历史，新的在前
        {
            let conn = self.redis_client.get_connection_mut();
            let _: () = redis::AsyncCommands::lpush(conn, &key, &value).await?;
            let _: () = redis::AsyncCommands::ltrim(conn, &key, 0, 999).await?;
        }

        // 设置过期时间（7天）
        self.redis_client.expire(&key, 604800).await?;

        debug!("Saved execution history for rule: {}", rule_id);
        Ok(())
    }

    /// 获取执行历史
    pub async fn get_execution_history(
        &mut self,
        rule_id: &str,
        limit: usize,
    ) -> Result<Vec<ExecutionHistory>> {
        let key = self.make_key(&format!("history:{}", rule_id));
        let values: Vec<String> = {
            let conn = self.redis_client.get_connection_mut();
            redis::AsyncCommands::lrange(conn, &key, 0, limit as isize - 1).await?
        };

        let mut history = Vec::new();
        for value in values {
            match serde_json::from_str::<ExecutionHistory>(&value) {
                Ok(h) => history.push(h),
                Err(e) => {
                    error!("Failed to deserialize execution history: {}", e);
                }
            }
        }

        Ok(history)
    }

    /// 清理过期数据
    pub async fn cleanup_expired_data(&self) -> Result<()> {
        info!("Starting cleanup of expired data");

        // Redis 的过期机制会自动处理过期的键
        // 这里可以添加其他清理逻辑

        Ok(())
    }

    /// Publish a message to a Redis channel
    pub async fn publish(&mut self, channel: &str, message: &str) -> Result<()> {
        self.redis_client.publish(channel, message).await?;
        Ok(())
    }
}

/// 规则查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleQuery {
    pub group_id: Option<String>,
    pub rule_type: Option<String>,
    pub enabled: Option<bool>,
    pub min_priority: Option<i32>,
    pub sort_by: Option<String>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

impl Default for RuleQuery {
    fn default() -> Self {
        Self {
            group_id: None,
            rule_type: None,
            enabled: None,
            min_priority: None,
            sort_by: None,
            offset: Some(0),
            limit: Some(100),
        }
    }
}

/// 规则查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleQueryResult {
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub data: Vec<Rule>,
}

/// 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub rule_id: String,
    pub conditions_met: Option<bool>,
    pub actions_executed: Option<Vec<serde_json::Value>>,
    pub node_results: Option<Vec<serde_json::Value>>,
    pub error: Option<String>,
}

/// 规则统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleStats {
    pub total_rules: usize,
    pub enabled_rules: usize,
    pub disabled_rules: usize,
    pub total_executions: Option<usize>,
    pub simple_executions: Option<usize>,
    pub dag_executions: Option<usize>,
    pub by_type: HashMap<String, usize>,
}

/// 执行历史记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionHistory {
    /// 执行ID
    pub id: String,
    /// 规则ID
    pub rule_id: String,
    /// 执行时间
    pub timestamp: i64,
    /// 是否触发
    pub triggered: bool,
    /// 执行的动作
    pub actions_executed: Vec<String>,
    /// 执行结果
    pub success: bool,
    /// 错误信息
    pub error: Option<String>,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
    /// 上下文数据快照
    pub context: HashMap<String, serde_json::Value>,
}

// 为了向后兼容，重新导出原始 RedisStore 的类型别名
pub type RedisStore = RedisFunctionStore;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rule_crud() {
        // 这里应该使用 mock Redis 进行测试
        // 暂时跳过需要真实 Redis 的测试
    }

    #[tokio::test]
    async fn test_key_generation() {
        let store = RedisFunctionStore {
            redis_client: voltage_libs::redis::RedisClient::new("redis://localhost")
                .await
                .unwrap(),
            key_prefix: "test".to_string(),
        };

        assert_eq!(store.make_key("rule:123"), "test:rule:123");
        assert_eq!(store.make_key("group:abc"), "test:group:abc");
    }
}
