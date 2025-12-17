//! Application-layer routing cache
//!
//! Provides in-memory caching of routing tables for high-performance lookups.
//! This is a pure data structure without external dependencies.
//!
//! ## Structured Route Targets
//!
//! All route targets are stored as structured types, eliminating runtime string parsing:
//! - `C2MTarget`: Channel → Instance (measurement point)
//! - `C2CTarget`: Channel → Channel (data forwarding)
//! - `M2CTarget`: Instance → Channel (action/control)

use dashmap::DashMap;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use voltage_model::PointType;

// ============================================================================
// Route Target Types
// ============================================================================

/// C2M (Channel to Model) route target
///
/// Routes channel point data to an instance measurement point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct C2MTarget {
    /// Target instance ID
    pub instance_id: u32,
    /// Target measurement point ID
    pub point_id: u32,
}

impl fmt::Display for C2MTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:M:{}", self.instance_id, self.point_id)
    }
}

/// C2C (Channel to Channel) route target
///
/// Routes channel point data to another channel point (data forwarding).
/// This is a Copy type - clone is zero-cost (12 bytes stack copy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct C2CTarget {
    /// Target channel ID
    pub channel_id: u32,
    /// Target point type (T/S/C/A)
    pub point_type: PointType,
    /// Target point ID
    pub point_id: u32,
}

impl fmt::Display for C2CTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.channel_id,
            self.point_type.as_str(),
            self.point_id
        )
    }
}

/// M2C (Model to Channel) route target
///
/// Routes instance action point to a channel point for control/adjustment.
/// This is a Copy type - clone is zero-cost (12 bytes stack copy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct M2CTarget {
    /// Target channel ID
    pub channel_id: u32,
    /// Target point type (typically C or A)
    pub point_type: PointType,
    /// Target point ID
    pub point_id: u32,
}

impl fmt::Display for M2CTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.channel_id,
            self.point_type.as_str(),
            self.point_id
        )
    }
}

// ============================================================================
// Parsing helpers
// ============================================================================

/// Parse C2M target from string "instance_id:M:point_id"
fn parse_c2m_target(s: &str) -> Option<C2MTarget> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let instance_id = parts[0].parse().ok()?;
    // parts[1] should be "M" - we ignore it as it's always M for C2M
    let point_id = parts[2].parse().ok()?;
    Some(C2MTarget {
        instance_id,
        point_id,
    })
}

/// Parse C2C target from string "channel_id:type:point_id"
fn parse_c2c_target(s: &str) -> Option<C2CTarget> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let channel_id = parts[0].parse().ok()?;
    let point_type = parse_point_type(parts[1])?;
    let point_id = parts[2].parse().ok()?;
    Some(C2CTarget {
        channel_id,
        point_type,
        point_id,
    })
}

/// Parse M2C target from string "channel_id:type:point_id"
fn parse_m2c_target(s: &str) -> Option<M2CTarget> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let channel_id = parts[0].parse().ok()?;
    let point_type = parse_point_type(parts[1])?;
    let point_id = parts[2].parse().ok()?;
    Some(M2CTarget {
        channel_id,
        point_type,
        point_id,
    })
}

/// Parse point type string to PointType enum
#[inline]
fn parse_point_type(s: &str) -> Option<PointType> {
    match s {
        "T" => Some(PointType::Telemetry),
        "S" => Some(PointType::Signal),
        "C" => Some(PointType::Control),
        "A" => Some(PointType::Adjustment),
        _ => None,
    }
}

// ============================================================================
// RoutingCache
// ============================================================================

/// Application-layer routing cache for C2M, C2C and M2C routing
///
/// Stores structured route targets for zero-cost lookups (no runtime parsing).
#[derive(Debug, Clone)]
pub struct RoutingCache {
    /// C2M routing: channel_key -> instance target
    /// Key: "channel_id:type:point_id" (e.g., "2:T:1")
    c2m: Arc<DashMap<String, C2MTarget>>,
    /// C2C routing: channel_key -> channel target
    /// Key: "channel_id:type:point_id" (e.g., "2:T:1")
    c2c: Arc<DashMap<String, C2CTarget>>,
    /// M2C routing: instance_key -> channel target
    /// Key: "instance_id:A:point_id" (e.g., "23:A:4")
    m2c: Arc<DashMap<String, M2CTarget>>,
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
    /// Parses string targets into structured types at load time.
    /// Invalid targets are silently skipped (logged in production).
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
        // Initialize C2M routing (parse to structured targets)
        let c2m = Arc::new(DashMap::new());
        for (k, v) in c2m_data {
            if let Some(target) = parse_c2m_target(&v) {
                c2m.insert(k, target);
            }
        }

        // Initialize M2C routing (parse to structured targets)
        let m2c = Arc::new(DashMap::new());
        for (k, v) in m2c_data {
            if let Some(target) = parse_m2c_target(&v) {
                m2c.insert(k, target);
            }
        }

        // Initialize C2C routing (parse to structured targets)
        let c2c = Arc::new(DashMap::new());
        for (k, v) in c2c_data {
            if let Some(target) = parse_c2c_target(&v) {
                c2c.insert(k, target);
            }
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
            if let Some(target) = parse_c2m_target(&v) {
                self.c2m.insert(k, target);
            }
        }

        self.m2c.clear();
        for (k, v) in m2c_data {
            if let Some(target) = parse_m2c_target(&v) {
                self.m2c.insert(k, target);
            }
        }

        self.c2c.clear();
        for (k, v) in c2c_data {
            if let Some(target) = parse_c2c_target(&v) {
                self.c2c.insert(k, target);
            }
        }
    }

    /// Lookup C2M routing (Channel to Model)
    ///
    /// Returns structured target with instance_id and point_id.
    /// No runtime parsing - target was parsed at load time.
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
    ///     assert_eq!(target.instance_id, 23);
    ///     assert_eq!(target.point_id, 1);
    /// }
    /// ```
    pub fn lookup_c2m(&self, key: &str) -> Option<C2MTarget> {
        self.c2m.get(key).map(|v| *v.value())
    }

    /// Lookup M2C routing (Model to Channel)
    ///
    /// Returns structured target with channel_id, point_type, and point_id.
    /// No runtime parsing - target was parsed at load time.
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
    ///     assert_eq!(target.channel_id, 2);
    ///     assert_eq!(target.point_type, "A");
    ///     assert_eq!(target.point_id, 1);
    /// }
    /// ```
    pub fn lookup_m2c(&self, key: &str) -> Option<M2CTarget> {
        self.m2c.get(key).map(|v| *v.value())
    }

    /// Lookup C2C routing (Channel to Channel)
    ///
    /// Returns structured target with channel_id, point_type, and point_id.
    /// No runtime parsing - target was parsed at load time.
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
    ///     assert_eq!(target.channel_id, 1002);
    ///     assert_eq!(target.point_type, "T");
    ///     assert_eq!(target.point_id, 5);
    /// }
    /// ```
    pub fn lookup_c2c(&self, key: &str) -> Option<C2CTarget> {
        self.c2c.get(key).map(|v| *v.value())
    }

    /// Insert C2C routing entry from string target
    pub fn insert_c2c(&self, source_key: String, target_key: String) {
        if let Some(target) = parse_c2c_target(&target_key) {
            self.c2c.insert(source_key, target);
        }
    }

    /// Remove C2C routing entry
    pub fn remove_c2c(&self, source_key: &str) -> Option<(String, C2CTarget)> {
        self.c2c.remove(source_key)
    }

    /// Get all C2C routing entries matching a prefix
    pub fn get_c2c_by_prefix(&self, prefix: &str) -> Vec<(String, C2CTarget)> {
        self.c2c
            .iter()
            .filter(|entry| entry.key().starts_with(prefix))
            .map(|entry| (entry.key().clone(), *entry.value()))
            .collect()
    }

    /// Get all C2M routing entries matching a prefix
    pub fn get_c2m_by_prefix(&self, prefix: &str) -> Vec<(String, C2MTarget)> {
        self.c2m
            .iter()
            .filter(|entry| entry.key().starts_with(prefix))
            .map(|entry| (entry.key().clone(), *entry.value()))
            .collect()
    }

    /// Get all M2C routing entries matching a prefix
    pub fn get_m2c_by_prefix(&self, prefix: &str) -> Vec<(String, M2CTarget)> {
        self.m2c
            .iter()
            .filter(|entry| entry.key().starts_with(prefix))
            .map(|entry| (entry.key().clone(), *entry.value()))
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
#[allow(clippy::disallowed_methods)] // Tests can use unwrap for clarity
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

        // Verify C2M lookup returns structured type
        let c2m = cache.lookup_c2m("2:T:1").unwrap();
        assert_eq!(c2m.instance_id, 23);
        assert_eq!(c2m.point_id, 1);

        // Verify M2C lookup returns structured type
        let m2c = cache.lookup_m2c("23:A:4").unwrap();
        assert_eq!(m2c.channel_id, 2);
        assert_eq!(m2c.point_type, PointType::Adjustment);
        assert_eq!(m2c.point_id, 1);

        // Verify C2C lookup returns structured type
        let c2c = cache.lookup_c2c("1001:T:1").unwrap();
        assert_eq!(c2c.channel_id, 1002);
        assert_eq!(c2c.point_type, PointType::Telemetry);
        assert_eq!(c2c.point_id, 5);

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

        // Verify updated values
        let c2m = cache.lookup_c2m("2:T:1").unwrap();
        assert_eq!(c2m.instance_id, 23);
        assert_eq!(c2m.point_id, 1);

        let m2c = cache.lookup_m2c("23:A:4").unwrap();
        assert_eq!(m2c.channel_id, 2);
        assert_eq!(m2c.point_type, PointType::Adjustment);
        assert_eq!(m2c.point_id, 1);

        let c2c = cache.lookup_c2c("1001:S:2").unwrap();
        assert_eq!(c2c.channel_id, 1002);
        assert_eq!(c2c.point_type, PointType::Signal);
        assert_eq!(c2c.point_id, 3);
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
        let c2c = cache.lookup_c2c("1001:T:1").unwrap();
        assert_eq!(c2c.channel_id, 1002);
        assert_eq!(c2c.point_type, PointType::Telemetry);
        assert_eq!(c2c.point_id, 5);

        // Test multiple inserts
        cache.insert_c2c("1001:S:2".to_string(), "1003:S:1".to_string());
        cache.insert_c2c("2001:A:1".to_string(), "2002:C:3".to_string());

        // Verify lookups
        let c2c = cache.lookup_c2c("1001:S:2").unwrap();
        assert_eq!(c2c.channel_id, 1003);
        assert_eq!(c2c.point_type, PointType::Signal);
        assert_eq!(c2c.point_id, 1);

        let c2c = cache.lookup_c2c("2001:A:1").unwrap();
        assert_eq!(c2c.channel_id, 2002);
        assert_eq!(c2c.point_type, PointType::Control);
        assert_eq!(c2c.point_id, 3);

        assert!(cache.lookup_c2c("nonexistent").is_none());

        // Test remove
        let removed = cache.remove_c2c("1001:T:1");
        assert!(removed.is_some());
        let (key, target) = removed.unwrap();
        assert_eq!(key, "1001:T:1");
        assert_eq!(target.channel_id, 1002);
        assert_eq!(target.point_id, 5);
        assert!(cache.lookup_c2c("1001:T:1").is_none());

        // Test prefix filtering
        cache.insert_c2c("3001:T:1".to_string(), "3002:T:1".to_string());
        cache.insert_c2c("3001:T:2".to_string(), "3002:T:2".to_string());

        let routes = cache.get_c2c_by_prefix("3001:T:");
        assert_eq!(routes.len(), 2);

        let routes = cache.get_c2c_by_prefix("1001:");
        assert_eq!(routes.len(), 1); // Only 1001:S:2 remains
    }

    #[test]
    fn test_parse_invalid_targets() {
        // Invalid format should be skipped
        let mut c2m_data = HashMap::new();
        c2m_data.insert("valid:T:1".to_string(), "23:M:1".to_string());
        c2m_data.insert("invalid:T:2".to_string(), "not:a:valid:target".to_string());
        c2m_data.insert("also_invalid".to_string(), "short".to_string());

        let cache = RoutingCache::from_maps(c2m_data, HashMap::new(), HashMap::new());

        // Only valid entry should be present
        assert!(cache.lookup_c2m("valid:T:1").is_some());
        assert!(cache.lookup_c2m("invalid:T:2").is_none());
        assert!(cache.lookup_c2m("also_invalid").is_none());
        assert_eq!(cache.stats().c2m_count, 1);
    }
}
