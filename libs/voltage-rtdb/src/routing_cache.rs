//! Application-layer routing cache
//!
//! Provides in-memory caching of routing tables for high-performance lookups.
//! This is a pure data structure without external dependencies.

use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;

/// Application-layer routing cache for C2M, C2C and M2C routing
///
/// Uses `Arc<str>` for values to avoid string clones on lookup - only reference count increments.
#[derive(Debug, Clone)]
pub struct RoutingCache {
    /// C2M routing: channel_key -> instance_key
    /// Example: "2:T:1" -> "23:M:1"
    c2m: Arc<DashMap<String, Arc<str>>>,
    /// C2C routing: channel_key -> channel_key
    /// Example: "2:T:1" -> "2:T:2"
    /// Enables direct channel-to-channel data forwarding
    c2c: Arc<DashMap<String, Arc<str>>>,
    /// M2C routing: instance_key -> channel_key
    /// Example: "23:A:4" -> "2:A:1"
    m2c: Arc<DashMap<String, Arc<str>>>,
}

impl RoutingCache {
    /// Create an empty routing cache
    pub fn new() -> Self {
        Self {
            c2m: Arc::new(DashMap::new()),
            c2c: Arc::new(DashMap::new()),
            m2c: Arc::new(DashMap::new()),
        }
    }

    /// Construct routing cache from raw HashMap data
    ///
    /// Services should load data from SQLite/Redis, then construct the cache.
    ///
    /// ## Example
    /// ```rust
    /// use voltage_rtdb::RoutingCache;
    /// use std::collections::HashMap;
    ///
    /// let c2m_data: HashMap<String, String> = HashMap::new(); // load from SQLite
    /// let m2c_data: HashMap<String, String> = HashMap::new(); // load from SQLite
    /// let c2c_data: HashMap<String, String> = HashMap::new(); // load from SQLite
    /// let cache = RoutingCache::from_maps(c2m_data, m2c_data, c2c_data);
    /// ```
    pub fn from_maps(
        c2m_data: HashMap<String, String>,
        m2c_data: HashMap<String, String>,
        c2c_data: HashMap<String, String>,
    ) -> Self {
        // Initialize C2M routing (convert values to Arc<str>)
        let c2m = Arc::new(DashMap::new());
        for (k, v) in c2m_data {
            c2m.insert(k, Arc::from(v));
        }

        // Initialize M2C routing (convert values to Arc<str>)
        let m2c = Arc::new(DashMap::new());
        for (k, v) in m2c_data {
            m2c.insert(k, Arc::from(v));
        }

        // Initialize C2C routing (convert values to Arc<str>)
        let c2c = Arc::new(DashMap::new());
        for (k, v) in c2c_data {
            c2c.insert(k, Arc::from(v));
        }

        Self { c2c, c2m, m2c }
    }

    /// Update routing cache with new data
    ///
    /// Clears existing cache and loads new data. Used during hot-reload.
    pub fn update(
        &self,
        c2m_data: HashMap<String, String>,
        m2c_data: HashMap<String, String>,
        c2c_data: HashMap<String, String>,
    ) {
        self.c2m.clear();
        for (k, v) in c2m_data {
            self.c2m.insert(k, Arc::from(v));
        }

        self.m2c.clear();
        for (k, v) in m2c_data {
            self.m2c.insert(k, Arc::from(v));
        }

        self.c2c.clear();
        for (k, v) in c2c_data {
            self.c2c.insert(k, Arc::from(v));
        }
    }

    /// Lookup C2M routing (Channel to Model)
    ///
    /// Returns `Arc<str>` - clone is just a reference count increment (cheap).
    ///
    /// ## Example
    /// ```rust
    /// use voltage_rtdb::RoutingCache;
    /// use std::collections::HashMap;
    ///
    /// let mut c2m = HashMap::new();
    /// c2m.insert("2:T:1".to_string(), "23:M:1".to_string());
    /// let cache = RoutingCache::from_maps(c2m, HashMap::new(), HashMap::new());
    ///
    /// if let Some(target) = cache.lookup_c2m("2:T:1") {
    ///     // target = "23:M:1" (Instance 23, Measurement, Point 1)
    ///     assert_eq!(target.as_ref(), "23:M:1");
    /// }
    /// ```
    pub fn lookup_c2m(&self, key: &str) -> Option<Arc<str>> {
        self.c2m.get(key).map(|v| Arc::clone(v.value()))
    }

    /// Lookup M2C routing (Model to Channel)
    ///
    /// Returns `Arc<str>` - clone is just a reference count increment (cheap).
    ///
    /// ## Example
    /// ```rust
    /// use voltage_rtdb::RoutingCache;
    /// use std::collections::HashMap;
    ///
    /// let mut m2c = HashMap::new();
    /// m2c.insert("23:A:4".to_string(), "2:A:1".to_string());
    /// let cache = RoutingCache::from_maps(HashMap::new(), m2c, HashMap::new());
    ///
    /// if let Some(target) = cache.lookup_m2c("23:A:4") {
    ///     // target = "2:A:1" (Channel 2, Adjustment, Point 1)
    ///     assert_eq!(target.as_ref(), "2:A:1");
    /// }
    /// ```
    pub fn lookup_m2c(&self, key: &str) -> Option<Arc<str>> {
        self.m2c.get(key).map(|v| Arc::clone(v.value()))
    }

    /// Lookup C2C routing (Channel to Channel)
    ///
    /// Returns `Arc<str>` - clone is just a reference count increment (cheap).
    ///
    /// ## Example
    /// ```rust
    /// use voltage_rtdb::RoutingCache;
    /// use std::collections::HashMap;
    ///
    /// let mut c2c = HashMap::new();
    /// c2c.insert("1001:T:1".to_string(), "1002:T:5".to_string());
    /// let cache = RoutingCache::from_maps(HashMap::new(), HashMap::new(), c2c);
    ///
    /// if let Some(target) = cache.lookup_c2c("1001:T:1") {
    ///     // target = "1002:T:5" (Target: Channel 1002, Telemetry, Point 5)
    ///     assert_eq!(target.as_ref(), "1002:T:5");
    /// }
    /// ```
    pub fn lookup_c2c(&self, key: &str) -> Option<Arc<str>> {
        self.c2c.get(key).map(|v| Arc::clone(v.value()))
    }

    /// Insert C2C routing entry
    pub fn insert_c2c(&self, source_key: String, target_key: String) {
        self.c2c.insert(source_key, Arc::from(target_key));
    }

    /// Remove C2C routing entry
    pub fn remove_c2c(&self, source_key: &str) -> Option<(String, Arc<str>)> {
        self.c2c.remove(source_key)
    }

    /// Get all C2C routing entries matching a prefix
    pub fn get_c2c_by_prefix(&self, prefix: &str) -> Vec<(String, Arc<str>)> {
        self.c2c
            .iter()
            .filter(|entry| entry.key().starts_with(prefix))
            .map(|entry| (entry.key().clone(), Arc::clone(entry.value())))
            .collect()
    }

    /// Get all C2M routing entries matching a prefix
    pub fn get_c2m_by_prefix(&self, prefix: &str) -> Vec<(String, Arc<str>)> {
        self.c2m
            .iter()
            .filter(|entry| entry.key().starts_with(prefix))
            .map(|entry| (entry.key().clone(), Arc::clone(entry.value())))
            .collect()
    }

    /// Get all M2C routing entries matching a prefix
    pub fn get_m2c_by_prefix(&self, prefix: &str) -> Vec<(String, Arc<str>)> {
        self.m2c
            .iter()
            .filter(|entry| entry.key().starts_with(prefix))
            .map(|entry| (entry.key().clone(), Arc::clone(entry.value())))
            .collect()
    }

    /// Get cache statistics
    pub fn stats(&self) -> RoutingCacheStats {
        RoutingCacheStats {
            c2m_count: self.c2m.len(),
            m2c_count: self.m2c.len(),
            c2c_count: self.c2c.len(),
        }
    }
}

impl Default for RoutingCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Routing cache statistics
#[derive(Debug, Clone)]
pub struct RoutingCacheStats {
    pub c2m_count: usize,
    pub m2c_count: usize,
    pub c2c_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_cache_creation() {
        let cache = RoutingCache::new();
        let stats = cache.stats();
        assert_eq!(stats.c2m_count, 0);
        assert_eq!(stats.m2c_count, 0);
    }

    #[test]
    fn test_from_maps() {
        let mut c2m_data = HashMap::new();
        c2m_data.insert("2:T:1".to_string(), "23:M:1".to_string());

        let mut m2c_data = HashMap::new();
        m2c_data.insert("23:A:4".to_string(), "2:A:1".to_string());

        let mut c2c_data = HashMap::new();
        c2c_data.insert("1001:T:1".to_string(), "1002:T:5".to_string());

        let cache = RoutingCache::from_maps(c2m_data, m2c_data, c2c_data);

        assert_eq!(cache.lookup_c2m("2:T:1"), Some(Arc::from("23:M:1")));
        assert_eq!(cache.lookup_m2c("23:A:4"), Some(Arc::from("2:A:1")));
        assert_eq!(cache.lookup_c2c("1001:T:1"), Some(Arc::from("1002:T:5")));

        let stats = cache.stats();
        assert_eq!(stats.c2m_count, 1);
        assert_eq!(stats.m2c_count, 1);
        assert_eq!(stats.c2c_count, 1);
    }

    #[test]
    fn test_update() {
        let cache = RoutingCache::new();

        let mut c2m_data = HashMap::new();
        c2m_data.insert("2:T:1".to_string(), "23:M:1".to_string());

        let mut m2c_data = HashMap::new();
        m2c_data.insert("23:A:4".to_string(), "2:A:1".to_string());

        let mut c2c_data = HashMap::new();
        c2c_data.insert("1001:S:2".to_string(), "1002:S:3".to_string());

        cache.update(c2m_data, m2c_data, c2c_data);

        assert_eq!(cache.lookup_c2m("2:T:1"), Some(Arc::from("23:M:1")));
        assert_eq!(cache.lookup_m2c("23:A:4"), Some(Arc::from("2:A:1")));
        assert_eq!(cache.lookup_c2c("1001:S:2"), Some(Arc::from("1002:S:3")));
    }

    #[test]
    fn test_prefix_filtering() {
        let mut c2m_data = HashMap::new();
        c2m_data.insert("2:T:1".to_string(), "23:M:1".to_string());
        c2m_data.insert("2:T:2".to_string(), "23:M:2".to_string());
        c2m_data.insert("3:T:1".to_string(), "24:M:1".to_string());

        let mut c2c_data = HashMap::new();
        c2c_data.insert("1001:T:1".to_string(), "1002:T:1".to_string());
        c2c_data.insert("1001:T:2".to_string(), "1002:T:2".to_string());
        c2c_data.insert("2001:S:1".to_string(), "2002:S:1".to_string());

        let cache = RoutingCache::from_maps(c2m_data, HashMap::new(), c2c_data);

        let routes = cache.get_c2m_by_prefix("2:T:");
        assert_eq!(routes.len(), 2);

        let routes = cache.get_c2m_by_prefix("3:");
        assert_eq!(routes.len(), 1);

        // Test C2C prefix filtering
        let routes = cache.get_c2c_by_prefix("1001:T:");
        assert_eq!(routes.len(), 2);

        let routes = cache.get_c2c_by_prefix("2001:");
        assert_eq!(routes.len(), 1);
    }

    #[test]
    fn test_c2c_operations() {
        let cache = RoutingCache::new();

        // Test insert and lookup
        cache.insert_c2c("1001:T:1".to_string(), "1002:T:5".to_string());
        assert_eq!(cache.lookup_c2c("1001:T:1"), Some(Arc::from("1002:T:5")));

        // Test multiple inserts
        cache.insert_c2c("1001:S:2".to_string(), "1003:S:1".to_string());
        cache.insert_c2c("2001:A:1".to_string(), "2002:C:3".to_string());

        // Verify lookups
        assert_eq!(cache.lookup_c2c("1001:S:2"), Some(Arc::from("1003:S:1")));
        assert_eq!(cache.lookup_c2c("2001:A:1"), Some(Arc::from("2002:C:3")));
        assert_eq!(cache.lookup_c2c("nonexistent"), None);

        // Test remove
        let removed = cache.remove_c2c("1001:T:1");
        assert_eq!(
            removed,
            Some(("1001:T:1".to_string(), Arc::from("1002:T:5")))
        );
        assert_eq!(cache.lookup_c2c("1001:T:1"), None);

        // Test prefix filtering
        cache.insert_c2c("3001:T:1".to_string(), "3002:T:1".to_string());
        cache.insert_c2c("3001:T:2".to_string(), "3002:T:2".to_string());

        let routes = cache.get_c2c_by_prefix("3001:T:");
        assert_eq!(routes.len(), 2);

        let routes = cache.get_c2c_by_prefix("1001:");
        assert_eq!(routes.len(), 1); // Only 1001:S:2 remains
    }
}
