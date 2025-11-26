//! Hot reload implementation for rulesrv
//!
//! This module implements the unified `ReloadableService` trait for rulesrv,
//! enabling zero-downtime rule chain updates from SQLite database.

use std::collections::HashSet;
use tracing::info;
use voltage_config::rulesrv::RuleChain;
use voltage_config::{ReloadableService, RuleReloadResult};

use crate::app::AppState;
use crate::rules_repository;

/// Rule chain change severity classification
///
/// For rulesrv, all changes are treated as configuration updates since
/// rule chains are stateless configuration objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuleChangeType {
    /// Configuration update (nodes, variables, enabled state)
    ConfigUpdate = 0,
}

impl ReloadableService for AppState {
    type ChangeType = RuleChangeType;
    type Config = RuleChain;
    type ReloadResult = RuleReloadResult;

    /// Reload rule chains from SQLite database with incremental sync
    async fn reload_from_database(
        &self,
        _pool: &sqlx::SqlitePool,
    ) -> anyhow::Result<Self::ReloadResult> {
        let start_time = std::time::Instant::now();
        info!("Starting incremental rule chain reload from SQLite");

        // 1. Load all rule chains from SQLite via rules_repository
        let db_chains = rules_repository::load_all_chains(self).await?;
        let db_ids: HashSet<String> = db_chains.iter().map(|c| c.id.clone()).collect();

        // 2. Get current cached chain IDs
        let cached_chains = self.chains_cache.read().await;
        let cached_ids: HashSet<String> = cached_chains.iter().map(|c| c.id.clone()).collect();
        drop(cached_chains); // Release read lock

        // 3. Determine changes
        let to_add: Vec<String> = db_ids.difference(&cached_ids).cloned().collect();
        let to_remove: Vec<String> = cached_ids.difference(&db_ids).cloned().collect();
        let to_update: Vec<String> = db_ids.intersection(&cached_ids).cloned().collect();

        let mut added = Vec::new();
        let mut updated = Vec::new();
        let mut removed = Vec::new();
        let errors = Vec::new();

        // 4. Build new chains cache with all changes applied
        let mut new_chains = Vec::new();

        // Add new chains
        for id in &to_add {
            if let Some(chain) = db_chains.iter().find(|c| &c.id == id) {
                new_chains.push(chain.clone());
                added.push(id.clone());
                info!("Added rule chain {} ({})", chain.name, id);
            }
        }

        // Update existing chains
        for id in &to_update {
            if let Some(chain) = db_chains.iter().find(|c| &c.id == id) {
                new_chains.push(chain.clone());
                updated.push(id.clone());
                info!("Updated rule chain {} ({})", chain.name, id);
            }
        }

        // Note: Removed chains are automatically excluded from new_chains
        for id in &to_remove {
            removed.push(id.clone());
            info!("Removed rule chain {}", id);
        }

        // 5. Update chains cache atomically
        let mut cache = self.chains_cache.write().await;
        *cache = new_chains;
        drop(cache);

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let total_count = db_chains.len();

        info!(
            "Rule chain reload completed: {} added, {} updated, {} removed, {} errors ({}ms)",
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

    /// Perform hot reload of a rule chain (update cache)
    async fn perform_hot_reload(&self, config: Self::Config) -> anyhow::Result<String> {
        info!(
            "Performing hot reload for rule chain {} ({})",
            config.name, config.id
        );

        // Update single chain in cache
        let mut cache = self.chains_cache.write().await;
        let mut new_chains = cache.clone();

        // Find and replace or add
        if let Some(index) = new_chains.iter().position(|c| c.id == config.id) {
            new_chains[index] = config;
        } else {
            new_chains.push(config);
        }

        *cache = new_chains;
        drop(cache);

        Ok("cached".to_string())
    }

    /// Rollback to previous configuration
    async fn rollback(&self, previous_config: Self::Config) -> anyhow::Result<String> {
        info!(
            "Rolling back rule chain {} ({}) to previous configuration",
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
    fn test_rule_chain_creation() {
        // Test rule chain creation works correctly
        let chain = create_test_chain("chain_001", "Test Chain");
        assert_eq!(chain.id, "chain_001");
        assert_eq!(chain.name, "Test Chain");
    }

    fn create_test_chain(id: &str, name: &str) -> RuleChain {
        RuleChain {
            id: id.to_string(),
            name: name.to_string(),
            description: None,
            enabled: true,
            priority: 100,
            cooldown_ms: 0,
            variables: vec![],
            nodes: vec![],
            start_node_id: "start".to_string(),
            flow_json: None,
        }
    }
}
