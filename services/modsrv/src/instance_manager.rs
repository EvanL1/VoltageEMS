//! Instance Manager - Core Lifecycle Operations
//!
//! This module provides the core instance lifecycle management.
//! Extended functionality is provided in separate modules:
//! - `instance_routing.rs` - Routing CRUD operations
//! - `instance_redis_sync.rs` - Redis synchronization
//! - `instance_data.rs` - Data loading and querying

use anyhow::{anyhow, Result};
use dashmap::DashMap;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use voltage_model::{validate_instance_name, KeySpaceConfig};
use voltage_rtdb::Rtdb;
use voltage_rtdb_shm::{InstanceIndex, SlotBitmap};

use crate::config::TopologyNode;
use crate::product_loader::{CreateInstanceRequest, Instance, ProductLoader};

/// Row type returned by SQLite instance queries
type InstanceRow = (u32, String, String, Option<u32>, Option<String>, String);

#[derive(serde::Deserialize)]
struct ComsrvRoutingReloadResult {
    errors: Vec<String>,
}

#[derive(serde::Deserialize)]
struct ComsrvRoutingReloadResponse {
    success: bool,
    data: ComsrvRoutingReloadResult,
}

/// Parse properties JSON string into HashMap
fn parse_properties_json(
    json: Option<String>,
    instance_id: u32,
) -> Result<HashMap<String, serde_json::Value>> {
    match json {
        Some(s) => serde_json::from_str(&s).map_err(|e| {
            anyhow!(
                "Invalid properties JSON for instance {}: {}",
                instance_id,
                e
            )
        }),
        None => Ok(HashMap::new()),
    }
}

/// Build Instance from database row tuple
fn build_instance_from_row(
    row: (u32, String, String, Option<u32>, Option<String>, String),
) -> Result<Instance> {
    let (instance_id, instance_name, product_name, parent_id, properties_json, _created_at) = row;
    let properties = parse_properties_json(properties_json, instance_id)?;
    Ok(Instance {
        core: crate::config::InstanceCore {
            instance_id,
            instance_name,
            product_name,
            parent_id,
            properties,
        },
        measurement_mappings: None,
        action_mappings: None,
        created_at: None,
    })
}

/// Instance Manager handles runtime instance lifecycle
pub struct InstanceManager<R: Rtdb> {
    pub pool: SqlitePool,
    pub rtdb: Arc<R>,
    pub(crate) routing_cache: Arc<voltage_routing::RoutingCache>,
    pub(crate) product_loader: Arc<ProductLoader>,
    /// Instance name → instance_id cache (for fast API lookups)
    pub(crate) name_cache: DashMap<String, u16>,
    // ========== Dynamic Slot Allocation ==========
    /// Dynamic instance index for unified pool architecture (optional)
    /// Supports hot add/remove with ArcSwap for lock-free reads
    dynamic_instance_index: Option<Arc<InstanceIndex>>,
    /// Slot bitmap for dynamic allocation (optional, requires RwLock for &mut access)
    slot_bitmap: Option<Arc<parking_lot::RwLock<SlotBitmap>>>,
    // ========== SHM Action Writer (M2C via SHM) ==========
    /// UnifiedWriter for writing Control/Adjustment points to SHM
    /// When set, M2C commands go directly to SHM (primary path)
    /// Redis TODO queue remains as fallback
    /// Uses ArcSwapOption for runtime hot-swap on routing reload
    pub(crate) shm_action_writer: arc_swap::ArcSwapOption<voltage_rtdb_shm::UnifiedWriter>,
    /// SharedConfig for SHM file paths and sizing (needed for writer rebuild)
    pub(crate) shm_config: std::sync::OnceLock<voltage_rtdb_shm::SharedConfig>,
    // ========== UDS Notifier (Event-driven M2C) ==========
    /// ShmNotifier for sending UDS notifications to comsrv
    /// When SHM write succeeds, send notification to trigger immediate dispatch
    /// Uses OnceLock for delayed initialization (set after `Arc<InstanceManager>` is created)
    /// Protected by tokio Mutex for async &mut access
    pub(crate) shm_notifier:
        std::sync::OnceLock<Arc<tokio::sync::Mutex<voltage_rtdb_shm::ShmNotifier>>>,
}

impl<R: Rtdb + 'static> InstanceManager<R> {
    pub fn new(
        pool: SqlitePool,
        rtdb: Arc<R>,
        routing_cache: Arc<voltage_routing::RoutingCache>,
        product_loader: Arc<ProductLoader>,
    ) -> Self {
        Self {
            pool,
            rtdb,
            routing_cache,
            product_loader,
            name_cache: DashMap::new(),
            dynamic_instance_index: None,
            slot_bitmap: None,
            shm_action_writer: arc_swap::ArcSwapOption::empty(),
            shm_config: std::sync::OnceLock::new(),
            shm_notifier: std::sync::OnceLock::new(),
        }
    }

    /// Configure dynamic slot allocation (optional feature)
    ///
    /// Call this after construction to enable unified pool architecture.
    /// If not called, dynamic features are disabled (backward compatible).
    ///
    /// # Arguments
    /// * `dynamic_index` - InstanceIndex for dynamic instance management
    /// * `slot_bitmap` - SlotBitmap for slot allocation (wrapped in RwLock)
    pub fn with_dynamic_allocation(
        mut self,
        dynamic_index: Arc<InstanceIndex>,
        slot_bitmap: Arc<parking_lot::RwLock<SlotBitmap>>,
    ) -> Self {
        self.dynamic_instance_index = Some(dynamic_index);
        self.slot_bitmap = Some(slot_bitmap);
        self
    }

    /// Configure SHM action writer for M2C via shared memory
    ///
    /// When set, action commands (Control/Adjustment) are written to SHM
    /// in addition to Redis TODO queue. SHM serves as the primary path
    /// for comsrv's ShmCommandPoller, with Redis as fallback.
    ///
    /// Uses ArcSwapOption for runtime-swappable initialization.
    /// Also stores SharedConfig via OnceLock for future rebuilds.
    pub fn set_shm_action_writer(
        &self,
        writer: Arc<voltage_rtdb_shm::UnifiedWriter>,
        config: voltage_rtdb_shm::SharedConfig,
    ) {
        self.shm_action_writer.store(Some(writer));
        let _ = self.shm_config.set(config); // ignore if already set
    }

    /// Re-open the SHM action writer after routing changes
    ///
    /// Called after `refresh_routing_cache()` to ensure the writer's slot layout
    /// matches the updated routing. The old writer is atomically swapped out
    /// via ArcSwapOption and dropped after all in-flight reads complete.
    pub fn rebuild_shm_action_writer(&self) {
        let Some(config) = self.shm_config.get() else {
            return; // SHM not configured
        };
        match voltage_rtdb_shm::UnifiedWriter::open_for_actions(config, &self.routing_cache) {
            Ok(writer) => {
                self.shm_action_writer
                    .store(Some(std::sync::Arc::new(writer)));
                tracing::info!("SHM action writer rebuilt after routing change");
            },
            Err(e) => {
                tracing::warn!("SHM action writer rebuild failed: {}", e);
            },
        }
    }

    /// Configure UDS notifier for event-driven M2C command dispatch
    ///
    /// When set, after SHM write succeeds, send UDS notification to comsrv
    /// to trigger immediate command processing (~1-2ms latency).
    /// Falls back gracefully if notification fails.
    ///
    /// Uses OnceLock for delayed initialization - can be called after
    /// `Arc<InstanceManager>` is created. Returns true if set successfully,
    /// false if already set.
    pub fn set_shm_notifier(
        &self,
        notifier: Arc<tokio::sync::Mutex<voltage_rtdb_shm::ShmNotifier>>,
    ) -> bool {
        self.shm_notifier.set(notifier).is_ok()
    }

    /// Get dynamic InstanceIndex (for external access, e.g., API stats)
    pub fn dynamic_instance_index(&self) -> Option<&Arc<InstanceIndex>> {
        self.dynamic_instance_index.as_ref()
    }

    /// Get SlotBitmap stats (for monitoring)
    pub fn slot_bitmap_stats(&self) -> Option<voltage_rtdb_shm::BitmapStats> {
        self.slot_bitmap.as_ref().map(|b| b.read().stats())
    }

    /// Get the routing cache reference
    ///
    /// Returns a reference to the shared routing cache for use in API handlers
    /// that need to refresh the cache after routing management operations.
    pub fn routing_cache(&self) -> &Arc<voltage_routing::RoutingCache> {
        &self.routing_cache
    }

    /// Refresh routing cache from database and rebuild SHM action writer
    ///
    /// Refreshes local routing cache, asks comsrv to reload its routing/SHM view,
    /// then re-opens the local SHM action writer against the updated header.
    pub async fn refresh_routing_and_shm(&self) -> anyhow::Result<usize> {
        let count =
            crate::bootstrap::refresh_routing_cache(&self.pool, &self.routing_cache).await?;
        self.reload_comsrv_routing().await?;
        self.rebuild_shm_action_writer();
        Ok(count)
    }

    async fn reload_comsrv_routing(&self) -> anyhow::Result<()> {
        let base_url = std::env::var(common::ENV_COMSRV_URL)
            .unwrap_or_else(|_| common::DEFAULT_COMSRV_URL.to_string());
        let url = format!("{}/api/routing/reload", base_url.trim_end_matches('/'));

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| anyhow!("Failed to build comsrv reload client: {}", e))?;

        let response = client
            .post(&url)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to call comsrv routing reload at {}: {}", url, e))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "comsrv routing reload returned HTTP {}: {}",
                status,
                body
            ));
        }

        let payload: ComsrvRoutingReloadResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Invalid comsrv routing reload response: {}", e))?;

        if !payload.success {
            return Err(anyhow!("comsrv routing reload reported success=false"));
        }

        if !payload.data.errors.is_empty() {
            return Err(anyhow!(
                "comsrv routing reload completed with errors: {}",
                payload.data.errors.join("; ")
            ));
        }

        info!("comsrv routing reload completed via {}", url);
        Ok(())
    }

    /// Get the product loader reference
    ///
    /// Returns a reference to the product loader for accessing product templates.
    pub fn product_loader(&self) -> &ProductLoader {
        &self.product_loader
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
    pub async fn get_instance_id(&self, instance_name: &str) -> Result<u16> {
        // 1. Fast path: Check cache first
        if let Some(id) = self.name_cache.get(instance_name) {
            return Ok(*id);
        }

        // 2. Slow path: Query database
        let id: u16 =
            sqlx::query_scalar("SELECT instance_id FROM instances WHERE instance_name = ?")
                .bind(instance_name)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| anyhow!("Instance not found: {}", instance_name))?;

        // 3. Update cache for next time
        self.name_cache.insert(instance_name.to_string(), id);

        Ok(id)
    }

    /// Populate the name→id cache from database at startup
    ///
    /// This should be called once after creating the InstanceManager to pre-warm
    /// the cache with all existing instances.
    pub async fn populate_name_cache(&self) -> Result<()> {
        let instances: Vec<(String, u16)> =
            sqlx::query_as("SELECT instance_name, instance_id FROM instances")
                .fetch_all(&self.pool)
                .await?;

        for (name, id) in instances {
            self.name_cache.insert(name, id);
        }

        debug!("Name->ID cache: {} entries", self.name_cache.len());
        Ok(())
    }

    /// Update cache entry (called on instance create/rename)
    pub fn update_name_cache(&self, instance_name: String, instance_id: u16) {
        self.name_cache.insert(instance_name, instance_id);
    }

    /// Remove entry from cache (called on instance delete)
    pub fn remove_from_name_cache(&self, instance_name: &str) {
        self.name_cache.remove(instance_name);
    }

    /// Create a new instance based on a product template
    ///
    /// @input req: CreateInstanceRequest - Instance configuration
    /// @output `Result<Instance>` - Created instance with all point routings
    /// @throws anyhow::Error - Instance exists, product not found, database error
    /// @side-effects Creates instance in SQLite, initializes Redis keys
    /// @transaction Full creation is atomic
    pub async fn create_instance(&self, req: CreateInstanceRequest) -> Result<Instance> {
        // Use user-provided instance ID and name
        let instance_id = req.instance_id;

        info!(
            "Creating instance: {} (id: {}) for product: {}",
            req.instance_name, instance_id, req.product_name
        );

        // 1. Validate instance name format
        if let Err(e) = validate_instance_name(&req.instance_name) {
            return Err(anyhow!("Invalid instance name: {}", e));
        }

        // 2. Verify product exists (products are compile-time constants)
        // Note: Name uniqueness is enforced by database UNIQUE constraint.
        // We rely on the constraint rather than check-then-act to avoid race conditions.
        let product = self.product_loader.get_product(&req.product_name)?;

        // 3. Hierarchy validation: soft check on pName (warn only, never block)
        //    Product JSON defines pName for documentation, but we don't enforce it
        //    since real-world topologies may differ from the product library defaults.
        let parent_name = self
            .product_loader
            .get_product_parent_name(&req.product_name);
        match (&parent_name, req.parent_id) {
            (None, Some(_)) => {
                warn!(
                    "Root product '{}' typically has no parent, but parent_id was provided",
                    req.product_name
                );
            },
            (None, None) => {},
            (Some(expected_parent), None) => {
                warn!(
                    "Product '{}' has pName='{}' but no parent_id provided — creating as standalone",
                    req.product_name, expected_parent
                );
            },
            (Some(expected_parent), Some(pid)) => {
                // Validate parent exists (hard check — referential integrity)
                let parent_product: Option<String> =
                    sqlx::query_scalar("SELECT product_name FROM instances WHERE instance_id = ?")
                        .bind(pid as i64)
                        .fetch_optional(&self.pool)
                        .await?;

                let parent_product =
                    parent_product.ok_or_else(|| anyhow!("Parent instance {} not found", pid))?;

                if parent_product != *expected_parent {
                    warn!(
                        "Parent instance {} is '{}', but '{}' pName suggests '{}' — allowing anyway",
                        pid, parent_product, req.product_name, expected_parent
                    );
                }
            },
        }

        // 4. Begin transaction for atomic creation
        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                error!(
                    "Failed to begin transaction for instance {}: {}",
                    req.instance_name, e
                );
                return Err(anyhow!("Database transaction failed: {}", e));
            },
        };

        // 4. Create instance in SQLite within transaction
        let properties_json = serde_json::to_string(&req.properties)?;

        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO instances (instance_id, instance_name, product_name, parent_id, properties)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(instance_id as i64)
        .bind(&req.instance_name)
        .bind(&req.product_name)
        .bind(req.parent_id.map(|id| id as i64))
        .bind(&properties_json)
        .execute(&mut *tx)
        .await
        {
            error!("Failed to insert instance {}: {}", req.instance_name, e);
            if let Err(rb_err) = tx.rollback().await {
                error!("Transaction rollback failed: {}", rb_err);
            }
            return Err(anyhow!("Failed to create instance: {}", e));
        }

        // 5. Create point routings for measurement and action points within transaction
        // Measurement point routing - maps point IDs to Redis keys
        let mut measurement_point_routings = HashMap::new();
        // Action point routing - maps point IDs to Redis keys
        let mut action_point_routings = HashMap::new();

        // Build point routing maps for Redis registration (generate Redis keys)
        // Note: Routing configuration is managed by routing_loader.rs, not stored here
        let mut ibuf = itoa::Buffer::new();
        for point in &product.measurements {
            let redis_key = KeySpaceConfig::production_cached()
                .instance_measurement_point_key(instance_id, ibuf.format(point.measurement_id));
            measurement_point_routings.insert(point.measurement_id, redis_key);
        }

        for point in &product.actions {
            let redis_key = KeySpaceConfig::production_cached()
                .instance_action_point_key(instance_id, ibuf.format(point.action_id));
            action_point_routings.insert(point.action_id, redis_key);
        }

        // 6. Commit transaction first (ensure database persistence)
        if let Err(e) = tx.commit().await {
            error!(
                "Failed to commit transaction for instance {}: {}",
                req.instance_name, e
            );
            return Err(anyhow!("Database transaction commit failed: {}", e));
        }

        // 7. Best effort register instance in Redis (after commit, allow failure)
        info!("Registering instance {} in Redis", req.instance_name);
        if let Err(e) = self
            .register_instance_in_redis(
                req.instance_id,
                &req.instance_name,
                &req.product_name,
                &req.properties,
                &product.measurements,
                &product.actions,
                &measurement_point_routings,
                &action_point_routings,
            )
            .await
        {
            warn!(
                "Instance {} created in SQLite but Redis registration failed: {}. Will register on next reload.",
                req.instance_name, e
            );
        } else {
            info!(
                "Successfully registered instance {} in Redis after creation",
                req.instance_name
            );
        }

        info!("Successfully created instance {}", req.instance_name);

        // 8. Dynamic Slot Allocation: Add instance to InstanceIndex
        if let (Some(index), Some(bitmap)) = (&self.dynamic_instance_index, &self.slot_bitmap) {
            // Instance has M (Measurement) and A (Action) own_counts
            let own_counts = [
                product.measurements.len() as u32,
                product.actions.len() as u32,
            ];
            let total: u32 = own_counts.iter().sum();
            if total > 0 {
                let mut bitmap_guard = bitmap.write();
                match index.add_instance(instance_id, own_counts, Some(&mut bitmap_guard)) {
                    Ok(layout) => {
                        debug!(
                            "Inst{} slot allocated: base={}, total={}",
                            instance_id, layout.own_base, layout.own_total
                        );
                    },
                    Err(e) => {
                        // Log warning but don't fail - dynamic allocation is optional
                        warn!("Inst{} slot allocation failed: {}", instance_id, e);
                    },
                }
            }
        }

        // 9. Return created instance - move req fields into result (avoid clone)
        Ok(Instance {
            core: crate::config::InstanceCore {
                instance_id,
                instance_name: req.instance_name,
                product_name: req.product_name,
                parent_id: req.parent_id,
                properties: req.properties,
            },
            measurement_mappings: Some(measurement_point_routings),
            action_mappings: Some(action_point_routings),
            created_at: Some(chrono::Utc::now()),
        })
    }

    /// List all instances, optionally filtered by product_name
    pub async fn list_instances(&self, product_name: Option<&str>) -> Result<Vec<Instance>> {
        let (_, instances) = self
            .list_instances_paginated(product_name, 1, u32::MAX)
            .await?;
        Ok(instances)
    }

    /// List instances with pagination
    ///
    /// Uses SQL `? IS NULL OR product_name = ?` pattern to handle optional filter
    /// in a single query without Rust-side branching.
    pub async fn list_instances_paginated(
        &self,
        product_name: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<(u32, Vec<Instance>)> {
        let offset = (page - 1) * page_size;

        let (total,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM instances WHERE (? IS NULL OR product_name = ?)")
                .bind(product_name)
                .bind(product_name)
                .fetch_one(&self.pool)
                .await?;

        let rows: Vec<InstanceRow> = sqlx::query_as(
            r#"SELECT instance_id, instance_name, product_name, parent_id, properties, created_at
               FROM instances
               WHERE (? IS NULL OR product_name = ?)
               ORDER BY instance_id ASC
               LIMIT ? OFFSET ?"#,
        )
        .bind(product_name)
        .bind(product_name)
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        let instances = rows
            .into_iter()
            .map(build_instance_from_row)
            .collect::<Result<Vec<_>>>()?;

        Ok((u32::try_from(total).unwrap_or(u32::MAX), instances))
    }

    /// Search instances by name with fuzzy matching
    ///
    /// Uses SQL `? IS NULL OR product_name = ?` pattern to handle optional filter
    /// in a single query without Rust-side branching.
    pub async fn search_instances(
        &self,
        keyword: &str,
        product_name: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<(u32, Vec<Instance>)> {
        let offset = (page - 1) * page_size;
        let like_pattern = format!("%{}%", keyword);

        let (total,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM instances WHERE instance_name LIKE ? AND (? IS NULL OR product_name = ?)",
        )
        .bind(&like_pattern)
        .bind(product_name)
        .bind(product_name)
        .fetch_one(&self.pool)
        .await?;

        let rows: Vec<InstanceRow> = sqlx::query_as(
            r#"SELECT instance_id, instance_name, product_name, parent_id, properties, created_at
               FROM instances
               WHERE instance_name LIKE ? AND (? IS NULL OR product_name = ?)
               ORDER BY instance_id ASC
               LIMIT ? OFFSET ?"#,
        )
        .bind(&like_pattern)
        .bind(product_name)
        .bind(product_name)
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        let instances = rows
            .into_iter()
            .map(build_instance_from_row)
            .collect::<Result<Vec<_>>>()?;

        Ok((u32::try_from(total).unwrap_or(u32::MAX), instances))
    }

    /// Rename an instance
    pub async fn rename_instance(&self, instance_id: u32, new_name: &str) -> Result<()> {
        // Validate instance name format before checking uniqueness
        if let Err(e) = validate_instance_name(new_name) {
            anyhow::bail!("Invalid instance name: {}", e);
        }

        // Check if new name already exists
        let (count,): (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM instances WHERE instance_name = ? AND instance_id != ?"#,
        )
        .bind(new_name)
        .bind(instance_id as i64)
        .fetch_one(&self.pool)
        .await?;

        if count > 0 {
            anyhow::bail!("Instance name '{}' already exists, cannot rename", new_name);
        }

        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Update instances table
        sqlx::query(
            r#"UPDATE instances SET instance_name = ?, updated_at = CURRENT_TIMESTAMP WHERE instance_id = ?"#,
        )
        .bind(new_name)
        .bind(instance_id as i64)
        .execute(&mut *tx)
        .await?;

        // Update measurement_routing table (redundant field)
        sqlx::query(r#"UPDATE measurement_routing SET instance_name = ? WHERE instance_id = ?"#)
            .bind(new_name)
            .bind(instance_id as i64)
            .execute(&mut *tx)
            .await?;

        // Update action_routing table (redundant field)
        sqlx::query(r#"UPDATE action_routing SET instance_name = ? WHERE instance_id = ?"#)
            .bind(new_name)
            .bind(instance_id as i64)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        info!(
            "Instance {} renamed to '{}' in SQLite",
            instance_id, new_name
        );
        Ok(())
    }

    /// Get next available instance ID
    pub async fn get_next_instance_id(&self) -> Result<u32> {
        let row = sqlx::query_as::<_, (Option<i32>,)>("SELECT MAX(instance_id) FROM instances")
            .fetch_one(&self.pool)
            .await?;

        match row.0 {
            Some(max_id) => {
                let next_id = max_id + 1;
                if next_id < 0 {
                    anyhow::bail!("Instance ID overflow: negative value");
                }
                Ok(next_id as u32)
            },
            None => Ok(1), // First instance
        }
    }

    /// Get instance by ID
    pub async fn get_instance(&self, instance_id: u32) -> Result<Instance> {
        let row = sqlx::query_as::<_, (String, String, Option<u32>, Option<String>, String)>(
            r#"
            SELECT instance_name, product_name, parent_id, properties, created_at
            FROM instances
            WHERE instance_id = ?
            "#,
        )
        .bind(instance_id as i64)
        .fetch_optional(&self.pool)
        .await?;

        let row = row.ok_or_else(|| anyhow!("Instance not found: {}", instance_id))?;

        let (instance_name, product_name, parent_id, properties_json, _created_at) = row;
        let properties = parse_properties_json(properties_json, instance_id)?;

        // Load point routings from routing tables and generate Redis keys dynamically
        let mut measurement_point_routings = HashMap::new();
        let mut action_point_routings = HashMap::new();

        // Query measurement routing
        let measurement_points = sqlx::query_as::<_, (u32,)>(
            r#"
            SELECT measurement_id
            FROM measurement_routing
            WHERE instance_id = ?
            "#,
        )
        .bind(instance_id as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut ibuf = itoa::Buffer::new();
        for (point_id,) in measurement_points {
            let redis_key = KeySpaceConfig::production_cached()
                .instance_measurement_point_key(instance_id, ibuf.format(point_id));
            measurement_point_routings.insert(point_id, redis_key);
        }

        // Query action routing
        let action_points = sqlx::query_as::<_, (u32,)>(
            r#"
            SELECT action_id
            FROM action_routing
            WHERE instance_id = ?
            "#,
        )
        .bind(instance_id as i64)
        .fetch_all(&self.pool)
        .await?;

        for (point_id,) in action_points {
            let redis_key = KeySpaceConfig::production_cached()
                .instance_action_point_key(instance_id, ibuf.format(point_id));
            action_point_routings.insert(point_id, redis_key);
        }

        Ok(Instance {
            core: crate::config::InstanceCore {
                instance_id,
                instance_name,
                product_name,
                parent_id,
                properties,
            },
            measurement_mappings: Some(measurement_point_routings),
            action_mappings: Some(action_point_routings),
            created_at: None,
        })
    }

    /// Load all routing mappings for multiple instances in batch (O(1) queries instead of O(N))
    ///
    /// Returns a tuple of:
    /// - HashMap<instance_id, HashMap<point_id, redis_key>> for measurements
    /// - HashMap<instance_id, HashMap<point_id, redis_key>> for actions
    pub(crate) async fn load_all_routings_batch(
        &self,
    ) -> Result<(
        HashMap<u32, HashMap<u32, String>>,
        HashMap<u32, HashMap<u32, String>>,
    )> {
        // Query all measurement routings in one batch
        let all_measurements: Vec<(i32, i32)> =
            sqlx::query_as("SELECT instance_id, measurement_id FROM measurement_routing")
                .fetch_all(&self.pool)
                .await?;

        // Query all action routings in one batch
        let all_actions: Vec<(i32, i32)> =
            sqlx::query_as("SELECT instance_id, action_id FROM action_routing")
                .fetch_all(&self.pool)
                .await?;

        // Group measurements by instance_id
        let mut measurement_routings: HashMap<u32, HashMap<u32, String>> = HashMap::new();
        let mut ibuf = itoa::Buffer::new();
        for (instance_id, point_id) in all_measurements {
            let instance_id = instance_id as u32;
            let point_id = point_id as u32;
            let redis_key = KeySpaceConfig::production_cached()
                .instance_measurement_point_key(instance_id, ibuf.format(point_id));
            measurement_routings
                .entry(instance_id)
                .or_default()
                .insert(point_id, redis_key);
        }

        // Group actions by instance_id
        let mut action_routings: HashMap<u32, HashMap<u32, String>> = HashMap::new();
        for (instance_id, point_id) in all_actions {
            let instance_id = instance_id as u32;
            let point_id = point_id as u32;
            let redis_key = KeySpaceConfig::production_cached()
                .instance_action_point_key(instance_id, ibuf.format(point_id));
            action_routings
                .entry(instance_id)
                .or_default()
                .insert(point_id, redis_key);
        }

        Ok((measurement_routings, action_routings))
    }

    /// Collect all descendant instance IDs (BFS), returning them in leaf-first order
    ///
    /// Used for cascade delete: descendants must be deleted before the parent.
    async fn collect_descendants(&self, instance_id: u32) -> Result<Vec<u32>> {
        let mut all = Vec::new();
        let mut queue = vec![instance_id];
        while let Some(parent) = queue.pop() {
            let children: Vec<(u32,)> =
                sqlx::query_as("SELECT instance_id FROM instances WHERE parent_id = ?")
                    .bind(parent as i64)
                    .fetch_all(&self.pool)
                    .await?;
            for (child_id,) in children {
                all.push(child_id);
                queue.push(child_id);
            }
        }
        all.reverse(); // Leaf nodes first, parent nodes last
        Ok(all)
    }

    /// Delete a single instance by ID (internal — no cascade)
    ///
    /// Handles SQLite deletion, Redis cleanup, and dynamic slot deallocation.
    async fn delete_single_instance(&self, instance_id: u32) -> Result<()> {
        // 1. Query instance_name before deletion (needed for Redis cleanup and logging)
        let instance_name: String =
            sqlx::query_scalar("SELECT instance_name FROM instances WHERE instance_id = ?")
                .bind(instance_id as i64)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        // 2. Begin transaction for atomic deletion
        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                error!(
                    "Failed to begin transaction for deleting instance {} ({}): {}",
                    instance_id, instance_name, e
                );
                return Err(anyhow!("Database transaction failed: {}", e));
            },
        };

        // 3. Delete from SQLite within transaction (cascade will handle point routings)
        let result = sqlx::query("DELETE FROM instances WHERE instance_id = ?")
            .bind(instance_id as i64)
            .execute(&mut *tx)
            .await;

        match result {
            Ok(res) => {
                if res.rows_affected() == 0 {
                    // Rollback transaction
                    if let Err(rb_err) = tx.rollback().await {
                        error!("Transaction rollback failed: {}", rb_err);
                    }
                    return Err(anyhow!("Instance not found: {}", instance_id));
                }
            },
            Err(e) => {
                error!(
                    "Failed to delete instance {} ({}) from SQLite: {}",
                    instance_id, instance_name, e
                );
                if let Err(rb_err) = tx.rollback().await {
                    error!("Transaction rollback failed: {}", rb_err);
                }
                return Err(anyhow!("Failed to delete instance: {}", e));
            },
        }

        // 4. Commit transaction first (ensure database persistence)
        if let Err(e) = tx.commit().await {
            error!(
                "Failed to commit transaction for deleting instance {} ({}): {}",
                instance_id, instance_name, e
            );
            return Err(anyhow!("Database transaction commit failed: {}", e));
        }

        // 5. Best effort remove from Redis (after commit, allow failure)
        if let Err(e) = self
            .unregister_instance_from_redis(instance_id, &instance_name)
            .await
        {
            warn!(
                "Instance {} ({}) deleted from SQLite but Redis cleanup failed: {}. Will be cleaned up on next reload.",
                instance_id, instance_name, e
            );
        } else {
            info!(
                "Successfully unregistered instance {} ({}) from Redis after deletion",
                instance_id, instance_name
            );
        }

        // 6. Dynamic Slot Deallocation: Remove instance from InstanceIndex and free slots
        if let (Some(index), Some(bitmap)) = (&self.dynamic_instance_index, &self.slot_bitmap) {
            let mut bitmap_guard = bitmap.write();
            match index.remove_instance(instance_id, Some(&mut bitmap_guard)) {
                Ok(layout) => {
                    debug!(
                        "Inst{} slot freed: base={}, count={}",
                        instance_id, layout.own_base, layout.own_total
                    );
                },
                Err(e) => {
                    // Log warning but don't fail - dynamic allocation is optional
                    warn!("Inst{} slot deallocation failed: {}", instance_id, e);
                },
            }
        }

        // 7. Remove from name cache
        self.remove_from_name_cache(&instance_name);

        info!(
            "Successfully deleted instance: {} ({})",
            instance_id, instance_name
        );
        Ok(())
    }

    /// Delete an instance by ID with cascade delete of all descendants
    ///
    /// Collects all descendant instances (children, grandchildren, etc.),
    /// deletes them leaf-first to ensure proper Redis cleanup for each,
    /// then deletes the target instance itself.
    pub async fn delete_instance(&self, instance_id: u32) -> Result<()> {
        // 1. Collect all descendants (leaf-first order)
        let descendants = self.collect_descendants(instance_id).await?;

        if !descendants.is_empty() {
            info!(
                "Cascade delete: instance {} has {} descendants",
                instance_id,
                descendants.len()
            );
        }

        // 2. Delete descendants leaf-first (each goes through full cleanup)
        for desc_id in &descendants {
            if let Err(e) = self.delete_single_instance(*desc_id).await {
                warn!(
                    "Failed to cascade-delete descendant instance {}: {}",
                    desc_id, e
                );
                // Continue deleting other descendants
            }
        }

        // 3. Delete the target instance itself
        self.delete_single_instance(instance_id).await
    }

    // ============================================================================
    // Topology Query Methods
    // ============================================================================

    /// Get direct child instances of a given parent
    pub async fn get_children(&self, instance_id: u32) -> Result<Vec<Instance>> {
        let rows: Vec<InstanceRow> = sqlx::query_as(
            r#"
                SELECT instance_id, instance_name, product_name, parent_id, properties, created_at
                FROM instances
                WHERE parent_id = ?
                ORDER BY instance_id ASC
                "#,
        )
        .bind(instance_id as i64)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(build_instance_from_row)
            .collect::<Result<Vec<_>>>()
    }

    /// Get full topology tree starting from all root instances (Station)
    ///
    /// Returns a flat list of topology nodes with parent_id for tree reconstruction.
    pub async fn get_topology_tree(&self) -> Result<Vec<TopologyNode>> {
        let rows: Vec<(u32, String, String, Option<u32>)> = sqlx::query_as(
            r#"
            SELECT instance_id, instance_name, product_name, parent_id
            FROM instances
            ORDER BY parent_id NULLS FIRST, instance_id ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(instance_id, instance_name, product_name, parent_id)| TopologyNode {
                    instance_id,
                    instance_name,
                    product_name,
                    parent_id,
                },
            )
            .collect())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[path = "instance_manager_tests.rs"]
mod tests;
