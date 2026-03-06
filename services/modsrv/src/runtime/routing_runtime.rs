//! Routing Runtime
//!
//! Wraps the shared RoutingCache with domain-level refresh logic.
//! The RoutingCache itself uses ArcSwap<FxHashMap> for ~25ns lock-free reads.

use sqlx::SqlitePool;
use std::sync::Arc;
use tracing::{debug, info};
use voltage_routing::RoutingCache;

/// Domain-friendly wrapper around the shared RoutingCache
pub struct RoutingRuntime {
    cache: Arc<RoutingCache>,
}

impl RoutingRuntime {
    pub fn new(cache: Arc<RoutingCache>) -> Self {
        Self { cache }
    }

    /// Access the inner cache (for handlers that need direct access)
    pub fn cache(&self) -> &Arc<RoutingCache> {
        &self.cache
    }

    /// Refresh routing cache from SQLite database
    ///
    /// Reloads routing data and atomically swaps the in-memory cache.
    /// Returns the total number of routes loaded (c2m + m2c).
    pub async fn refresh(&self, pool: &SqlitePool) -> anyhow::Result<usize> {
        debug!("Refreshing routes");
        let maps = voltage_routing::load_routing_maps(pool).await?;
        let total_routes = maps.c2m.len() + maps.m2c.len();
        self.cache.update(maps.c2m, maps.m2c, maps.c2c);
        info!("Routes refreshed: {}", total_routes);
        Ok(total_routes)
    }

    /// Get routing cache statistics
    pub fn stats(&self) -> voltage_routing::RoutingCacheStats {
        self.cache.stats()
    }
}
