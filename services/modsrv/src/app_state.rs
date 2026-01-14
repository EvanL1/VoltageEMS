//! Application State Management
//!
//! Central application state that is shared across all API handlers

use std::sync::Arc;

use crate::config::ModsrvConfig;
use crate::error::ModSrvError;
use crate::instance_manager::InstanceManager;

#[cfg(test)]
use voltage_rtdb::MemoryRtdb as TestRtdb;
#[cfg(not(test))]
use voltage_rtdb::RedisRtdb as TestRtdb;

/// Application state containing shared resources
pub struct AppState {
    /// Configuration loaded from database
    pub config: Arc<ModsrvConfig>,

    /// Instance lifecycle manager (uses Redis RTDB in production)
    pub instance_manager: Arc<InstanceManager<TestRtdb>>,
}

impl AppState {
    /// Create new application state
    pub fn new(
        config: Arc<ModsrvConfig>,
        instance_manager: Arc<InstanceManager<TestRtdb>>,
    ) -> Self {
        Self {
            config,
            instance_manager,
        }
    }

    // ============================================================================
    // Instance name → ID translation methods (delegates to InstanceManager)
    // ============================================================================

    /// Get instance_id by instance_name (with caching)
    ///
    /// Delegates to InstanceManager for the actual lookup.
    pub async fn get_instance_id(&self, instance_name: &str) -> Result<u16, ModSrvError> {
        self.instance_manager
            .get_instance_id(instance_name)
            .await
            .map_err(|e| ModSrvError::InstanceNotFound(e.to_string()))
    }

    /// Populate the name→id cache from database at startup
    ///
    /// Delegates to InstanceManager.
    pub async fn populate_name_cache(&self) -> Result<(), ModSrvError> {
        self.instance_manager
            .populate_name_cache()
            .await
            .map_err(|e| ModSrvError::InternalError(format!("Failed to populate cache: {}", e)))
    }

    /// Update cache entry (called on instance create/rename)
    pub fn update_name_cache(&self, instance_name: String, instance_id: u16) {
        self.instance_manager
            .update_name_cache(instance_name, instance_id);
    }

    /// Remove entry from cache (called on instance delete)
    pub fn remove_from_cache(&self, instance_name: &str) {
        self.instance_manager.remove_from_name_cache(instance_name);
    }
}
