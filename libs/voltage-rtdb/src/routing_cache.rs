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
//!
//! ## Single Index Design
//!
//! All routing tables use structured tuple keys for zero-allocation lookups:
//! - C2M/C2C: `(channel_id, point_type, point_id)`
//! - M2C: `(instance_id, point_type, point_id)`
//!
//! String-based lookups (`lookup_c2c("1001:T:1")`) parse the key first, then query the tuple index.
//! Prefix queries (`get_c2c_by_prefix("1001:")`) iterate and filter the tuple index.

use arc_swap::ArcSwap;
use rustc_hash::FxHashMap;
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
        // Note: "M" in C2M targets means Measurement (instance point), not a PointType
        _ => None,
    }
}

/// Structured route key type for C2M and C2C (zero-allocation lookups)
/// Format: (channel_id, point_type, point_id)
pub type StructuredRouteKey = (u32, PointType, u32);

/// Structured route key type for M2C (zero-allocation lookups)
/// Format: (instance_id, point_type, point_id)
pub type StructuredM2CKey = (u32, PointType, u32);

/// Parse route key string "id:type:point_id" into structured key
#[inline]
fn parse_route_key(s: &str) -> Option<StructuredRouteKey> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let id = parts[0].parse().ok()?;
    let point_type = parse_point_type(parts[1])?;
    let point_id = parts[2].parse().ok()?;
    Some((id, point_type, point_id))
}

/// Parse prefix string "id:" or "id:type:" into filter components
/// Returns (id, Option<point_type>)
#[inline]
fn parse_prefix(prefix: &str) -> Option<(u32, Option<PointType>)> {
    let trimmed = prefix.trim_end_matches(':');
    let parts: Vec<&str> = trimmed.split(':').collect();
    match parts.len() {
        1 => Some((parts[0].parse().ok()?, None)),
        2 => Some((parts[0].parse().ok()?, Some(parse_point_type(parts[1])?))),
        _ => None,
    }
}

/// Format structured key back to string
#[inline]
fn format_route_key(key: &StructuredRouteKey) -> String {
    format!("{}:{}:{}", key.0, key.1.as_str(), key.2)
}

// ============================================================================
// RoutingTables (internal snapshot)
// ============================================================================

/// Internal routing tables snapshot (immutable after creation)
///
/// Replaces DashMap with FxHashMap for lock-free reads.
/// Wrapped in Arc for atomic replacement via ArcSwap.
#[derive(Debug, Default)]
struct RoutingTables {
    /// C2M routing: (channel_id, point_type, point_id) -> instance target
    c2m: FxHashMap<StructuredRouteKey, C2MTarget>,
    /// C2C routing: (channel_id, point_type, point_id) -> channel target
    c2c: FxHashMap<StructuredRouteKey, C2CTarget>,
    /// M2C routing: (instance_id, point_type, point_id) -> channel target
    m2c: FxHashMap<StructuredM2CKey, M2CTarget>,
}

// ============================================================================
// RoutingCache
// ============================================================================

/// Application-layer routing cache for C2M, C2C and M2C routing
///
/// Uses ArcSwap + FxHashMap for lock-free reads (~25ns vs ~50ns for DashMap).
/// Routing tables are atomically replaced during hot-reload.
///
/// ## Performance
/// - Read: `ArcSwap::load()` (~5ns) + `FxHashMap::get()` (~20ns) = ~25ns total
/// - Write: Build new tables + `ArcSwap::store()` (atomic pointer swap)
///
/// ## Hot Path Usage
/// For hot paths like `write_channel_batch`, use `lookup_*_by_parts()` methods
/// which take structured keys directly, avoiding temporary String allocation.
#[derive(Debug)]
pub struct RoutingCache {
    /// Atomic-swappable routing tables snapshot
    tables: ArcSwap<RoutingTables>,
}

impl RoutingCache {
    /// Create an empty routing cache
    pub fn new() -> Self {
        Self {
            tables: ArcSwap::from_pointee(RoutingTables::default()),
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
        let mut tables = RoutingTables::default();

        for (k, v) in c2m_data {
            if let (Some(key), Some(target)) = (parse_route_key(&k), parse_c2m_target(&v)) {
                tables.c2m.insert(key, target);
            }
        }

        for (k, v) in m2c_data {
            if let (Some(key), Some(target)) = (parse_route_key(&k), parse_m2c_target(&v)) {
                tables.m2c.insert(key, target);
            }
        }

        for (k, v) in c2c_data {
            if let (Some(key), Some(target)) = (parse_route_key(&k), parse_c2c_target(&v)) {
                tables.c2c.insert(key, target);
            }
        }

        Self {
            tables: ArcSwap::from_pointee(tables),
        }
    }

    /// Update routing cache with new data (atomic replacement)
    ///
    /// Builds a new routing tables snapshot and atomically replaces the old one.
    /// Used during hot-reload. Readers see either old or new snapshot, never partial.
    pub fn update(
        &self,
        c2m_data: HashMap<String, String>,
        m2c_data: HashMap<String, String>,
        c2c_data: HashMap<String, String>,
    ) {
        let mut new_tables = RoutingTables::default();

        for (k, v) in c2m_data {
            if let (Some(key), Some(target)) = (parse_route_key(&k), parse_c2m_target(&v)) {
                new_tables.c2m.insert(key, target);
            }
        }

        for (k, v) in m2c_data {
            if let (Some(key), Some(target)) = (parse_route_key(&k), parse_m2c_target(&v)) {
                new_tables.m2c.insert(key, target);
            }
        }

        for (k, v) in c2c_data {
            if let (Some(key), Some(target)) = (parse_route_key(&k), parse_c2c_target(&v)) {
                new_tables.c2c.insert(key, target);
            }
        }

        // Atomic replacement - readers see either old or new, never partial
        self.tables.store(Arc::new(new_tables));
    }

    /// Lookup C2M routing by string key (parses key first)
    ///
    /// For hot paths, prefer `lookup_c2m_by_parts()` to avoid string parsing.
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
        let structured_key = parse_route_key(key)?;
        self.tables.load().c2m.get(&structured_key).copied()
    }

    /// Lookup C2M routing by structured key (zero-allocation)
    ///
    /// Use this method in hot paths to avoid string parsing overhead.
    ///
    /// ## Example
    /// ```rust
    /// use voltage_rtdb::RoutingCache;
    /// use voltage_model::PointType;
    /// use std::collections::HashMap;
    ///
    /// let mut c2m = HashMap::new();
    /// c2m.insert("2:T:1".to_string(), "23:M:1".to_string());
    /// let cache = RoutingCache::from_maps(c2m, HashMap::new(), HashMap::new());
    ///
    /// // Zero-allocation lookup
    /// if let Some(target) = cache.lookup_c2m_by_parts(2, PointType::Telemetry, 1) {
    ///     assert_eq!(target.instance_id, 23);
    ///     assert_eq!(target.point_id, 1);
    /// }
    /// ```
    #[inline]
    pub fn lookup_c2m_by_parts(
        &self,
        channel_id: u32,
        point_type: PointType,
        point_id: u32,
    ) -> Option<C2MTarget> {
        self.tables
            .load()
            .c2m
            .get(&(channel_id, point_type, point_id))
            .copied()
    }

    /// Lookup M2C routing by string key (parses key first)
    ///
    /// ## Example
    /// ```rust
    /// use voltage_rtdb::RoutingCache;
    /// use voltage_model::PointType;
    /// use std::collections::HashMap;
    ///
    /// let mut m2c = HashMap::new();
    /// m2c.insert("23:A:4".to_string(), "2:A:1".to_string());
    /// let cache = RoutingCache::from_maps(HashMap::new(), m2c, HashMap::new());
    ///
    /// if let Some(target) = cache.lookup_m2c("23:A:4") {
    ///     assert_eq!(target.channel_id, 2);
    ///     assert_eq!(target.point_type, PointType::Adjustment);
    ///     assert_eq!(target.point_id, 1);
    /// }
    /// ```
    pub fn lookup_m2c(&self, key: &str) -> Option<M2CTarget> {
        let structured_key = parse_route_key(key)?;
        self.tables.load().m2c.get(&structured_key).copied()
    }

    /// Lookup M2C routing by structured key (zero-allocation)
    #[inline]
    pub fn lookup_m2c_by_parts(
        &self,
        instance_id: u32,
        point_type: PointType,
        point_id: u32,
    ) -> Option<M2CTarget> {
        self.tables
            .load()
            .m2c
            .get(&(instance_id, point_type, point_id))
            .copied()
    }

    /// Lookup C2C routing by string key (parses key first)
    ///
    /// For hot paths, prefer `lookup_c2c_by_parts()` to avoid string parsing.
    ///
    /// ## Example
    /// ```rust
    /// use voltage_rtdb::RoutingCache;
    /// use voltage_model::PointType;
    /// use std::collections::HashMap;
    ///
    /// let mut c2c = HashMap::new();
    /// c2c.insert("1001:T:1".to_string(), "1002:T:5".to_string());
    /// let cache = RoutingCache::from_maps(HashMap::new(), HashMap::new(), c2c);
    ///
    /// if let Some(target) = cache.lookup_c2c("1001:T:1") {
    ///     assert_eq!(target.channel_id, 1002);
    ///     assert_eq!(target.point_type, PointType::Telemetry);
    ///     assert_eq!(target.point_id, 5);
    /// }
    /// ```
    pub fn lookup_c2c(&self, key: &str) -> Option<C2CTarget> {
        let structured_key = parse_route_key(key)?;
        self.tables.load().c2c.get(&structured_key).copied()
    }

    /// Lookup C2C routing by structured key (zero-allocation)
    ///
    /// Use this method in hot paths to avoid string parsing overhead.
    ///
    /// ## Example
    /// ```rust
    /// use voltage_rtdb::RoutingCache;
    /// use voltage_model::PointType;
    /// use std::collections::HashMap;
    ///
    /// let mut c2c = HashMap::new();
    /// c2c.insert("1001:T:1".to_string(), "1002:T:5".to_string());
    /// let cache = RoutingCache::from_maps(HashMap::new(), HashMap::new(), c2c);
    ///
    /// // Zero-allocation lookup
    /// if let Some(target) = cache.lookup_c2c_by_parts(1001, PointType::Telemetry, 1) {
    ///     assert_eq!(target.channel_id, 1002);
    ///     assert_eq!(target.point_id, 5);
    /// }
    /// ```
    #[inline]
    pub fn lookup_c2c_by_parts(
        &self,
        channel_id: u32,
        point_type: PointType,
        point_id: u32,
    ) -> Option<C2CTarget> {
        self.tables
            .load()
            .c2c
            .get(&(channel_id, point_type, point_id))
            .copied()
    }

    /// Insert C2C routing entry from string keys (copy-on-write)
    ///
    /// Note: This is a cold-path operation. For bulk updates, use `update()`.
    pub fn insert_c2c(&self, source_key: impl AsRef<str>, target_key: &str) {
        let source_key = source_key.as_ref();
        if let (Some(key), Some(target)) =
            (parse_route_key(source_key), parse_c2c_target(target_key))
        {
            self.insert_c2c_by_parts(key.0, key.1, key.2, target);
        }
    }

    /// Insert C2C routing entry from structured key (copy-on-write)
    ///
    /// Note: This is a cold-path operation. For bulk updates, use `update()`.
    pub fn insert_c2c_by_parts(
        &self,
        channel_id: u32,
        point_type: PointType,
        point_id: u32,
        target: C2CTarget,
    ) {
        // Copy-on-write: clone current tables, modify, atomic replace
        let old = self.tables.load();
        let mut new_tables = RoutingTables {
            c2m: old.c2m.clone(),
            c2c: old.c2c.clone(),
            m2c: old.m2c.clone(),
        };
        new_tables
            .c2c
            .insert((channel_id, point_type, point_id), target);
        self.tables.store(Arc::new(new_tables));
    }

    /// Remove C2C routing entry by string key (copy-on-write)
    ///
    /// Returns the removed entry as (formatted_key, target) for compatibility.
    /// Note: This is a cold-path operation.
    pub fn remove_c2c(&self, source_key: &str) -> Option<(Arc<str>, C2CTarget)> {
        let structured_key = parse_route_key(source_key)?;
        let result =
            self.remove_c2c_by_parts(structured_key.0, structured_key.1, structured_key.2)?;
        Some((Arc::from(format_route_key(&structured_key)), result))
    }

    /// Remove C2C routing entry by structured key (copy-on-write)
    ///
    /// Note: This is a cold-path operation. For bulk updates, use `update()`.
    pub fn remove_c2c_by_parts(
        &self,
        channel_id: u32,
        point_type: PointType,
        point_id: u32,
    ) -> Option<C2CTarget> {
        // Copy-on-write: clone current tables, modify, atomic replace
        let old = self.tables.load();
        let target = old.c2c.get(&(channel_id, point_type, point_id)).copied()?;

        let mut new_tables = RoutingTables {
            c2m: old.c2m.clone(),
            c2c: old.c2c.clone(),
            m2c: old.m2c.clone(),
        };
        new_tables.c2c.remove(&(channel_id, point_type, point_id));
        self.tables.store(Arc::new(new_tables));
        Some(target)
    }

    /// Get all C2C routing entries matching a prefix
    ///
    /// Iterates and filters the tuple index, formatting keys for output.
    /// This is a cold path operation (CLI tools).
    pub fn get_c2c_by_prefix(&self, prefix: &str) -> Vec<(Arc<str>, C2CTarget)> {
        let Some((id, point_type_filter)) = parse_prefix(prefix) else {
            return vec![];
        };
        let tables = self.tables.load();
        tables
            .c2c
            .iter()
            .filter(|(k, _)| k.0 == id && point_type_filter.is_none_or(|pt| k.1 == pt))
            .map(|(k, v)| (Arc::from(format_route_key(k)), *v))
            .collect()
    }

    /// Get all C2M routing entries matching a prefix
    ///
    /// Iterates and filters the tuple index, formatting keys for output.
    /// This is a cold path operation (CLI tools).
    pub fn get_c2m_by_prefix(&self, prefix: &str) -> Vec<(Arc<str>, C2MTarget)> {
        let Some((id, point_type_filter)) = parse_prefix(prefix) else {
            return vec![];
        };
        let tables = self.tables.load();
        tables
            .c2m
            .iter()
            .filter(|(k, _)| k.0 == id && point_type_filter.is_none_or(|pt| k.1 == pt))
            .map(|(k, v)| (Arc::from(format_route_key(k)), *v))
            .collect()
    }

    /// Get all M2C routing entries matching a prefix
    ///
    /// Iterates and filters the tuple index, formatting keys for output.
    /// This is a cold path operation (CLI tools).
    pub fn get_m2c_by_prefix(&self, prefix: &str) -> Vec<(Arc<str>, M2CTarget)> {
        let Some((id, point_type_filter)) = parse_prefix(prefix) else {
            return vec![];
        };
        let tables = self.tables.load();
        tables
            .m2c
            .iter()
            .filter(|(k, _)| k.0 == id && point_type_filter.is_none_or(|pt| k.1 == pt))
            .map(|(k, v)| (Arc::from(format_route_key(k)), *v))
            .collect()
    }

    /// Get cache statistics
    pub fn stats(&self) -> RoutingCacheStats {
        let tables = self.tables.load();
        RoutingCacheStats {
            c2m_count: tables.c2m.len(),
            m2c_count: tables.m2c.len(),
            c2c_count: tables.c2c.len(),
        }
    }

    /// Iterate over all C2M routes (for building ChannelToSlotIndex)
    ///
    /// Returns a Vec of all C2M routes for building direct channel-to-slot mappings.
    /// The returned Vec is a snapshot of the current routing state.
    ///
    /// # Example
    /// ```ignore
    /// for (key, target) in routing_cache.c2m_iter() {
    ///     // key = (channel_id, point_type, point_id)
    ///     // target = C2MTarget { instance_id, point_id }
    /// }
    /// ```
    #[inline]
    pub fn c2m_iter(&self) -> Vec<(StructuredRouteKey, C2MTarget)> {
        let tables = self.tables.load();
        tables.c2m.iter().map(|(&k, &v)| (k, v)).collect()
    }

    /// Iterate over all M2C routes (for building reverse mappings)
    ///
    /// Returns a Vec of all M2C routes.
    #[inline]
    pub fn m2c_iter(&self) -> Vec<(StructuredM2CKey, M2CTarget)> {
        let tables = self.tables.load();
        tables.m2c.iter().map(|(&k, &v)| (k, v)).collect()
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
    fn test_by_parts_lookup() {
        let mut c2m_data = HashMap::new();
        c2m_data.insert("2:T:1".to_string(), "23:M:1".to_string());

        let mut c2c_data = HashMap::new();
        c2c_data.insert("1001:T:1".to_string(), "1002:T:5".to_string());

        let cache = RoutingCache::from_maps(c2m_data, HashMap::new(), c2c_data);

        // Test C2M by_parts lookup
        let c2m = cache
            .lookup_c2m_by_parts(2, PointType::Telemetry, 1)
            .unwrap();
        assert_eq!(c2m.instance_id, 23);
        assert_eq!(c2m.point_id, 1);

        // Test C2C by_parts lookup
        let c2c = cache
            .lookup_c2c_by_parts(1001, PointType::Telemetry, 1)
            .unwrap();
        assert_eq!(c2c.channel_id, 1002);
        assert_eq!(c2c.point_id, 5);

        // Non-existent should return None
        assert!(cache
            .lookup_c2m_by_parts(999, PointType::Telemetry, 1)
            .is_none());
        assert!(cache
            .lookup_c2c_by_parts(999, PointType::Telemetry, 1)
            .is_none());
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
        cache.insert_c2c("1001:T:1", "1002:T:5");
        let c2c = cache.lookup_c2c("1001:T:1").unwrap();
        assert_eq!(c2c.channel_id, 1002);
        assert_eq!(c2c.point_type, PointType::Telemetry);
        assert_eq!(c2c.point_id, 5);

        // Test by_parts lookup after string insert
        let c2c = cache
            .lookup_c2c_by_parts(1001, PointType::Telemetry, 1)
            .unwrap();
        assert_eq!(c2c.channel_id, 1002);

        // Test multiple inserts
        cache.insert_c2c("1001:S:2", "1003:S:1");
        cache.insert_c2c("2001:A:1", "2002:C:3");

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
        assert_eq!(&*key, "1001:T:1");
        assert_eq!(target.channel_id, 1002);
        assert_eq!(target.point_id, 5);
        assert!(cache.lookup_c2c("1001:T:1").is_none());
        assert!(cache
            .lookup_c2c_by_parts(1001, PointType::Telemetry, 1)
            .is_none());

        // Test prefix filtering
        cache.insert_c2c("3001:T:1", "3002:T:1");
        cache.insert_c2c("3001:T:2", "3002:T:2");

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

        // Only valid entry should be present (note: "valid" parses as channel_id fails)
        // Actually "valid" won't parse as u32, so none will be present
        assert_eq!(cache.stats().c2m_count, 0);
    }

    #[test]
    fn test_parse_valid_numeric_keys() {
        let mut c2m_data = HashMap::new();
        c2m_data.insert("100:T:1".to_string(), "23:M:1".to_string());
        c2m_data.insert("100:T:2".to_string(), "23:M:2".to_string());

        let cache = RoutingCache::from_maps(c2m_data, HashMap::new(), HashMap::new());

        assert!(cache.lookup_c2m("100:T:1").is_some());
        assert!(cache.lookup_c2m("100:T:2").is_some());
        assert_eq!(cache.stats().c2m_count, 2);
    }

    #[test]
    fn test_c2m_iter() {
        let mut c2m_data = HashMap::new();
        c2m_data.insert("1001:T:1".to_string(), "5:M:10".to_string());
        c2m_data.insert("1001:T:2".to_string(), "5:M:20".to_string());
        c2m_data.insert("1002:S:1".to_string(), "6:M:30".to_string());

        let mut m2c_data = HashMap::new();
        m2c_data.insert("5:A:1".to_string(), "1001:C:1".to_string());

        let cache = RoutingCache::from_maps(c2m_data, m2c_data, HashMap::new());

        // Test c2m_iter
        let c2m_routes = cache.c2m_iter();
        assert_eq!(c2m_routes.len(), 3);

        // Verify routes contain expected data
        let has_route = c2m_routes.iter().any(|(key, target)| {
            key.0 == 1001 && key.1 == PointType::Telemetry && key.2 == 1 && target.instance_id == 5
        });
        assert!(has_route);

        // Test m2c_iter
        let m2c_routes = cache.m2c_iter();
        assert_eq!(m2c_routes.len(), 1);

        let has_m2c_route = m2c_routes.iter().any(|(key, target)| {
            key.0 == 5 && key.1 == PointType::Adjustment && key.2 == 1 && target.channel_id == 1001
        });
        assert!(has_m2c_route);
    }
}
