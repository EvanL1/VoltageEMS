//! Channel point counts for SHM slot allocation.
//!
//! Provides a routing-independent data source for SHM layout.
//! Slot allocation is based on channel point definitions (from SQLite point tables),
//! not on routing mappings. This decouples SHM layout from routing changes.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

/// Per-channel point counts: [T, S, C, A].
///
/// Each element is `max(point_id) + 1` for that type, representing
/// the number of slots needed. BTreeMap ensures deterministic iteration order.
#[derive(Debug, Clone, Default)]
pub struct ChannelPointCounts(pub BTreeMap<u32, [u32; 4]>);

impl ChannelPointCounts {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Load channel point counts from SQLite point tables.
    ///
    /// Queries telemetry_points, signal_points, control_points, adjustment_points
    /// to determine max(point_id)+1 per channel per type.
    pub async fn load_from_db(pool: &sqlx::SqlitePool) -> anyhow::Result<Self> {
        let mut counts: BTreeMap<u32, [u32; 4]> = BTreeMap::new();

        // Query each point table for max point_id per channel
        let tables = [
            ("telemetry_points", 0usize), // T
            ("signal_points", 1),         // S
            ("control_points", 2),        // C
            ("adjustment_points", 3),     // A
        ];

        for (table, type_idx) in tables {
            // T/S slots: exclude virtual channels (they have no physical device,
            // and allocating T/S slots would shadow Redis values written by rules)
            let query = if type_idx <= 1 {
                format!(
                    "SELECT p.channel_id, MAX(p.point_id) + 1 AS cnt \
                     FROM {} p JOIN channels c ON c.channel_id = p.channel_id \
                     WHERE c.protocol != 'virtual' \
                     GROUP BY p.channel_id",
                    table
                )
            } else {
                format!(
                    "SELECT channel_id, MAX(point_id) + 1 AS cnt FROM {} GROUP BY channel_id",
                    table
                )
            };
            let rows: Vec<(i64, i64)> = sqlx::query_as(&query).fetch_all(pool).await?;

            for (channel_id, cnt) in rows {
                let entry = counts.entry(channel_id as u32).or_insert([0; 4]);
                entry[type_idx] = cnt as u32;
            }
        }

        Ok(Self(counts))
    }

    /// Compute a deterministic hash of the channel point layout.
    ///
    /// Same channel points → same hash → same SHM slot allocation.
    /// Used for cross-process SHM header validation.
    pub fn layout_hash(&self) -> u64 {
        let mut hasher = rustc_hash::FxHasher::default();
        // BTreeMap iterates in sorted key order — deterministic
        for (channel_id, counts) in &self.0 {
            channel_id.hash(&mut hasher);
            counts.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Build from a raw BTreeMap (for tests and non-async contexts).
    pub fn from_map(map: BTreeMap<u32, [u32; 4]>) -> Self {
        Self(map)
    }

    /// Get the inner map.
    pub fn inner(&self) -> &BTreeMap<u32, [u32; 4]> {
        &self.0
    }
}
