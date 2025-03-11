//! Hot reload implementation for rulesrv
//!
//! This module implements the unified `ReloadableService` trait for rulesrv,
//! enabling zero-downtime rule updates from SQLite database.

use std::collections::HashSet;
use std::sync::Arc;
use tracing::{error, info};
use voltage_config::{ReloadableService, RuleReloadResult};

use crate::app::AppState;
use crate::rule_engine::Rule;
use crate::rules_repository;

/// Rule change severity classification
///
/// For rulesrv, all changes are treated as configuration updates since
/// rules are stateless configuration objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuleChangeType {
    /// Configuration update (condition, action, priority, enabled)
    ConfigUpdate = 0,
}

impl ReloadableService for AppState {
    type ChangeType = RuleChangeType;
    type Config = Rule;
    type ReloadResult = RuleReloadResult;

    /// Reload rules from SQLite database with incremental sync
    async fn reload_from_database(
        &self,
        _pool: &sqlx::SqlitePool,
    ) -> anyhow::Result<Self::ReloadResult> {
        let start_time = std::time::Instant::now();
        info!("Starting incremental rule reload from SQLite");

        // 1. Load all rules from SQLite via rules_repository
        let db_rules_json = rules_repository::list_rules(self).await?;

        // Convert JSON rules to Rule structs
        let mut db_rules = Vec::new();
        for rule_json in db_rules_json {
            match serde_json::from_value::<Rule>(rule_json) {
                Ok(rule) => db_rules.push(rule),
                Err(e) => {
                    error!("Failed to deserialize rule from SQLite: {}", e);
                    continue;
                },
            }
        }

        let db_ids: HashSet<String> = db_rules.iter().map(|r| r.id.clone()).collect();

        // 2. Get current cached rule IDs
        let cached_rules = self.rules_cache.read().await;
        let cached_ids: HashSet<String> = cached_rules.iter().map(|r| r.id.clone()).collect();
        drop(cached_rules); // Release read lock

        // 3. Determine changes
        let to_add: Vec<String> = db_ids.difference(&cached_ids).cloned().collect();
        let to_remove: Vec<String> = cached_ids.difference(&db_ids).cloned().collect();
        let to_update: Vec<String> = db_ids.intersection(&cached_ids).cloned().collect();

        let mut added = Vec::new();
        let mut updated = Vec::new();
        let mut removed = Vec::new();
        let errors = Vec::new();

        // 4. Build new rules cache with all changes applied
        let mut new_rules = Vec::new();

        // Add new rules
        for id in &to_add {
            if let Some(rule) = db_rules.iter().find(|r| &r.id == id) {
                new_rules.push(rule.clone());
                added.push(id.clone());
                info!("Added rule {} ({})", rule.name, id);
            }
        }

        // Update existing rules
        for id in &to_update {
            if let Some(rule) = db_rules.iter().find(|r| &r.id == id) {
                new_rules.push(rule.clone());
                updated.push(id.clone());
                info!("Updated rule {} ({})", rule.name, id);
            }
        }

        // Note: Removed rules are automatically excluded from new_rules
        for id in &to_remove {
            removed.push(id.clone());
            info!("Removed rule {}", id);
        }

        // 5. Update rules cache atomically
        let mut cache = self.rules_cache.write().await;
        *cache = Arc::new(new_rules);
        drop(cache);

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let total_count = db_rules.len();

        info!(
            "Rule reload completed: {} added, {} updated, {} removed, {} errors ({}ms)",
            added.len(),
            updated.len(),
            removed.len(),
            errors.len(),
            duration_ms
        );

        Ok(RuleReloadResult {
            total_count,
            added,
            updated,
            removed,
            errors,
            duration_ms,
        })
    }

    /// Analyze changes between old and new configuration
    fn analyze_changes(
        &self,
        _old_config: &Self::Config,
        _new_config: &Self::Config,
    ) -> Self::ChangeType {
        // For rulesrv, all changes are config updates
        RuleChangeType::ConfigUpdate
    }

    /// Perform hot reload of a rule (update cache)
    async fn perform_hot_reload(&self, config: Self::Config) -> anyhow::Result<String> {
        info!(
            "Performing hot reload for rule {} ({})",
            config.name, config.id
        );

        // Update single rule in cache
        let mut cache = self.rules_cache.write().await;
        let mut new_rules = (**cache).clone();

        // Find and replace or add
        if let Some(index) = new_rules.iter().position(|r| r.id == config.id) {
            new_rules[index] = config;
        } else {
            new_rules.push(config);
        }

        *cache = Arc::new(new_rules);
        drop(cache);

        Ok("cached".to_string())
    }

    /// Rollback to previous configuration
    async fn rollback(&self, previous_config: Self::Config) -> anyhow::Result<String> {
        info!(
            "Rolling back rule {} ({}) to previous configuration",
            previous_config.name, previous_config.id
        );

        self.perform_hot_reload(previous_config).await?;
        Ok("restored".to_string())
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_rule_change_type() {
        // For rulesrv, all changes are classified as ConfigUpdate
        let change_type = RuleChangeType::ConfigUpdate;
        assert_eq!(change_type, RuleChangeType::ConfigUpdate);

        // Test ordering
        assert!(RuleChangeType::ConfigUpdate == RuleChangeType::ConfigUpdate);
    }

    #[test]
    fn test_rule_creation() {
        // Test rule creation works correctly
        let rule = create_test_rule("rule_001", "Test Rule");
        assert_eq!(rule.id, "rule_001");
        assert_eq!(rule.name, "Test Rule");
    }

    fn create_test_rule(id: &str, name: &str) -> Rule {
        use crate::rule_engine::{ConditionGroup, LogicalOperator, RuleMetadata};

        Rule {
            id: id.to_string(),
            name: name.to_string(),
            category: "test".to_string(),
            description: None,
            priority: 100,
            enabled: true,
            triggers: vec![],
            conditions: ConditionGroup::Group {
                logic: LogicalOperator::And,
                rules: vec![],
            },
            actions: vec![],
            metadata: RuleMetadata::default(),
        }
    }
}
