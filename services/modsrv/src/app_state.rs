//! Application State Management
//!
//! Central application state that is shared across all API handlers

use std::sync::Arc;

use dashmap::DashMap;
use tracing::info;

use crate::calculation_engine::CalculationEngine;
use crate::error::ModSrvError;
use crate::instance_manager::InstanceManager;
use crate::product_loader::ProductLoader;
use common::sqlite::SqliteClient;
use voltage_config::modsrv::ModsrvConfig;
#[cfg(test)]
use voltage_rtdb::MemoryRtdb as TestRtdb;
#[cfg(not(test))]
use voltage_rtdb::RedisRtdb as TestRtdb;

/// Application state containing shared resources
pub struct AppState {
    /// Configuration loaded from database
    #[allow(dead_code)]
    pub config: Arc<ModsrvConfig>,

    /// SQLite client for configuration storage
    pub sqlite_client: Option<Arc<SqliteClient>>,

    /// Product template loader
    pub product_loader: Arc<ProductLoader>,

    /// Instance lifecycle manager (uses Redis RTDB in production)
    pub instance_manager: Arc<InstanceManager<TestRtdb>>,

    /// Calculation execution engine
    pub calculation_engine: Arc<CalculationEngine>,

    /// Instance name → instance_id cache (for fast API lookups)
    /// Updated on: create, delete, rename operations
    pub name_to_id_cache: Arc<DashMap<String, u16>>,
}

impl AppState {
    /// Create new application state
    pub fn new(
        config: Arc<ModsrvConfig>,
        sqlite_client: Option<Arc<SqliteClient>>,
        product_loader: Arc<ProductLoader>,
        instance_manager: Arc<InstanceManager<TestRtdb>>,
        calculation_engine: Arc<CalculationEngine>,
    ) -> Self {
        Self {
            config,
            sqlite_client,
            product_loader,
            instance_manager,
            calculation_engine,
            name_to_id_cache: Arc::new(DashMap::new()),
        }
    }

    // ============================================================================
    // Instance name → ID translation methods
    // ============================================================================

    /// Get instance_id by instance_name (with caching)
    ///
    /// This method provides fast lookup of instance IDs from names, using a
    /// DashMap cache for sub-microsecond performance on cache hits.
    ///
    /// # Cache Strategy
    /// - Cache hit: Returns immediately (~100ns)
    /// - Cache miss: Queries SQLite and updates cache
    ///
    /// # Errors
    /// Returns `ModSrvError::InstanceNotFound` if the instance doesn't exist.
    pub async fn get_instance_id(&self, instance_name: &str) -> Result<u16, ModSrvError> {
        // 1. Fast path: Check cache first
        if let Some(id) = self.name_to_id_cache.get(instance_name) {
            return Ok(*id);
        }

        // 2. Slow path: Query database
        let id: u16 =
            sqlx::query_scalar("SELECT instance_id FROM instances WHERE instance_name = ?")
                .bind(instance_name)
                .fetch_optional(&self.instance_manager.pool)
                .await
                .map_err(|e| ModSrvError::InternalError(format!("Database query failed: {}", e)))?
                .ok_or_else(|| ModSrvError::InstanceNotFound(instance_name.to_string()))?;

        // 3. Update cache for next time
        self.name_to_id_cache.insert(instance_name.to_string(), id);

        Ok(id)
    }

    /// Populate the name→id cache from database at startup
    ///
    /// This should be called once after creating the AppState to pre-warm
    /// the cache with all existing instances.
    ///
    /// # Errors
    /// Returns an error if the database query fails.
    pub async fn populate_name_cache(&self) -> Result<(), ModSrvError> {
        let instances: Vec<(String, u16)> =
            sqlx::query_as("SELECT instance_name, instance_id FROM instances")
                .fetch_all(&self.instance_manager.pool)
                .await
                .map_err(|e| {
                    ModSrvError::InternalError(format!("Failed to populate cache: {}", e))
                })?;

        for (name, id) in instances {
            self.name_to_id_cache.insert(name, id);
        }

        info!(
            "Name→ID cache populated with {} entries",
            self.name_to_id_cache.len()
        );
        Ok(())
    }

    /// Update cache entry (called on instance create/rename)
    ///
    /// This method ensures the cache stays consistent with database changes.
    ///
    /// # Example
    /// ```rust,ignore
    /// // After creating instance
    /// state.update_name_cache("pv_inverter_01".to_string(), 100);
    /// ```
    pub fn update_name_cache(&self, instance_name: String, instance_id: u16) {
        self.name_to_id_cache.insert(instance_name, instance_id);
    }

    /// Remove entry from cache (called on instance delete or rename-old-name)
    ///
    /// # Example
    /// ```rust,ignore
    /// // After deleting instance
    /// state.remove_from_cache("old_instance_name");
    ///
    /// // During rename (remove old, add new)
    /// state.remove_from_cache("old_name");
    /// state.update_name_cache("new_name".to_string(), instance_id);
    /// ```
    pub fn remove_from_cache(&self, instance_name: &str) {
        self.name_to_id_cache.remove(instance_name);
    }
}
