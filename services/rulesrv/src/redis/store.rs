use anyhow::Result;
use redis::{AsyncCommands, Pipeline};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info};

use crate::rules::{Rule, RuleGroup};

/// Redis 存储实现
pub struct RedisStore {
    redis_client: redis::Client,
    key_prefix: String,
}

impl RedisStore {
    /// 创建新的 Redis 存储
    pub fn new(redis_url: &str, key_prefix: Option<&str>) -> Result<Self> {
        let redis_client = redis::Client::open(redis_url)?;
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

    /// 获取字符串值
    pub async fn get_string(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        let value: Option<String> = conn.get(key).await?;
        Ok(value)
    }

    /// 设置字符串值
    pub async fn set_string(&self, key: &str, value: &str) -> Result<()> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        conn.set(key, value).await?;
        Ok(())
    }

    /// 保存规则
    pub async fn save_rule(&self, rule: &Rule) -> Result<()> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let key = self.make_key(&format!("rule:{}", rule.id));
        let value = serde_json::to_string(rule)?;

        conn.set(&key, &value).await?;

        // 将规则ID添加到规则列表
        let list_key = self.make_key("rules");
        conn.sadd(&list_key, &rule.id).await?;

        // 如果规则属于某个组，更新组的规则列表
        if let Some(group_id) = &rule.group_id {
            let group_rules_key = self.make_key(&format!("group:{}:rules", group_id));
            conn.sadd(&group_rules_key, &rule.id).await?;
        }

        info!("Saved rule: {} ({})", rule.name, rule.id);
        Ok(())
    }

    /// 获取规则
    pub async fn get_rule(&self, rule_id: &str) -> Result<Option<Rule>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let key = self.make_key(&format!("rule:{}", rule_id));
        let value: Option<String> = conn.get(&key).await?;

        match value {
            Some(json) => {
                let rule: Rule = serde_json::from_str(&json)?;
                Ok(Some(rule))
            }
            None => Ok(None),
        }
    }

    /// 删除规则
    pub async fn delete_rule(&self, rule_id: &str) -> Result<bool> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        // 获取规则以查找组ID
        let rule = self.get_rule(rule_id).await?;

        // 删除规则
        let key = self.make_key(&format!("rule:{}", rule_id));
        let deleted: bool = conn.del(&key).await?;

        if deleted {
            // 从规则列表中移除
            let list_key = self.make_key("rules");
            conn.srem(&list_key, rule_id).await?;

            // 如果规则属于某个组，从组的规则列表中移除
            if let Some(rule) = rule {
                if let Some(group_id) = &rule.group_id {
                    let group_rules_key = self.make_key(&format!("group:{}:rules", group_id));
                    conn.srem(&group_rules_key, rule_id).await?;
                }
            }

            info!("Deleted rule: {}", rule_id);
        }

        Ok(deleted)
    }

    /// 获取所有规则
    pub async fn list_rules(&self) -> Result<Vec<Rule>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        // 获取所有规则ID
        let list_key = self.make_key("rules");
        let rule_ids: Vec<String> = conn.smembers(&list_key).await?;

        // 批量获取规则
        if rule_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut pipe = Pipeline::new();
        for rule_id in &rule_ids {
            let key = self.make_key(&format!("rule:{}", rule_id));
            pipe.get(&key);
        }

        let values: Vec<Option<String>> = pipe.query_async(&mut conn).await?;

        let mut rules = Vec::new();
        for (i, value) in values.iter().enumerate() {
            if let Some(json) = value {
                match serde_json::from_str::<Rule>(json) {
                    Ok(rule) => rules.push(rule),
                    Err(e) => {
                        error!("Failed to deserialize rule {}: {}", rule_ids[i], e);
                    }
                }
            }
        }

        Ok(rules)
    }

    /// 保存规则组
    pub async fn save_rule_group(&self, group: &RuleGroup) -> Result<()> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let key = self.make_key(&format!("group:{}", group.id));
        let value = serde_json::to_string(group)?;

        conn.set(&key, &value).await?;

        // 将组ID添加到组列表
        let list_key = self.make_key("groups");
        conn.sadd(&list_key, &group.id).await?;

        info!("Saved rule group: {} ({})", group.name, group.id);
        Ok(())
    }

    /// 获取规则组
    pub async fn get_rule_group(&self, group_id: &str) -> Result<Option<RuleGroup>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let key = self.make_key(&format!("group:{}", group_id));
        let value: Option<String> = conn.get(&key).await?;

        match value {
            Some(json) => {
                let group: RuleGroup = serde_json::from_str(&json)?;
                Ok(Some(group))
            }
            None => Ok(None),
        }
    }

    /// 删除规则组
    pub async fn delete_rule_group(&self, group_id: &str) -> Result<bool> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        // 检查组内是否还有规则
        let group_rules_key = self.make_key(&format!("group:{}:rules", group_id));
        let rule_count: usize = conn.scard(&group_rules_key).await?;

        if rule_count > 0 {
            return Err(anyhow::anyhow!(
                "Cannot delete group {} with {} rules",
                group_id,
                rule_count
            ));
        }

        // 删除组
        let key = self.make_key(&format!("group:{}", group_id));
        let deleted: bool = conn.del(&key).await?;

        if deleted {
            // 从组列表中移除
            let list_key = self.make_key("groups");
            conn.srem(&list_key, group_id).await?;

            // 删除组的规则列表键
            conn.del(&group_rules_key).await?;

            info!("Deleted rule group: {}", group_id);
        }

        Ok(deleted)
    }

    /// 获取所有规则组
    pub async fn list_rule_groups(&self) -> Result<Vec<RuleGroup>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        // 获取所有组ID
        let list_key = self.make_key("groups");
        let group_ids: Vec<String> = conn.smembers(&list_key).await?;

        // 批量获取组
        if group_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut pipe = Pipeline::new();
        for group_id in &group_ids {
            let key = self.make_key(&format!("group:{}", group_id));
            pipe.get(&key);
        }

        let values: Vec<Option<String>> = pipe.query_async(&mut conn).await?;

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
    pub async fn get_group_rules(&self, group_id: &str) -> Result<Vec<Rule>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        // 获取组内的规则ID
        let group_rules_key = self.make_key(&format!("group:{}:rules", group_id));
        let rule_ids: Vec<String> = conn.smembers(&group_rules_key).await?;

        // 批量获取规则
        if rule_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut pipe = Pipeline::new();
        for rule_id in &rule_ids {
            let key = self.make_key(&format!("rule:{}", rule_id));
            pipe.get(&key);
        }

        let values: Vec<Option<String>> = pipe.query_async(&mut conn).await?;

        let mut rules = Vec::new();
        for (i, value) in values.iter().enumerate() {
            if let Some(json) = value {
                match serde_json::from_str::<Rule>(json) {
                    Ok(rule) => rules.push(rule),
                    Err(e) => {
                        error!("Failed to deserialize rule {}: {}", rule_ids[i], e);
                    }
                }
            }
        }

        Ok(rules)
    }

    /// 保存执行历史
    pub async fn save_execution_history(
        &self,
        rule_id: &str,
        execution_result: &ExecutionHistory,
    ) -> Result<()> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let key = self.make_key(&format!("history:{}", rule_id));
        let value = serde_json::to_string(execution_result)?;

        // 使用列表存储历史，新的在前
        conn.lpush(&key, &value).await?;

        // 限制历史记录数量（保留最近1000条）
        conn.ltrim(&key, 0, 999).await?;

        // 设置过期时间（7天）
        conn.expire(&key, 604800).await?;

        debug!("Saved execution history for rule: {}", rule_id);
        Ok(())
    }

    /// 获取执行历史
    pub async fn get_execution_history(
        &self,
        rule_id: &str,
        limit: usize,
    ) -> Result<Vec<ExecutionHistory>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let key = self.make_key(&format!("history:{}", rule_id));
        let values: Vec<String> = conn.lrange(&key, 0, limit as isize - 1).await?;

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
    pub async fn publish(&self, channel: &str, message: &str) -> Result<()> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        redis::AsyncCommands::publish(&mut conn, channel, message).await?;
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rule_crud() {
        // 这里应该使用 mock Redis 进行测试
        // 暂时跳过需要真实 Redis 的测试
    }

    #[test]
    fn test_key_generation() {
        let store = RedisStore {
            redis_client: redis::Client::open("redis://localhost").unwrap(),
            key_prefix: "test".to_string(),
        };

        assert_eq!(store.make_key("rule:123"), "test:rule:123");
        assert_eq!(store.make_key("group:abc"), "test:group:abc");
    }
}
