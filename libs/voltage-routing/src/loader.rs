//! SQLite Routing Loader
//!
//! Provides unified routing map loading from SQLite for both comsrv and modsrv.

use anyhow::Result;
use std::collections::HashMap;
use tracing::{debug, info};
use voltage_rtdb::KeySpaceConfig;

/// Routing maps loaded from SQLite
#[derive(Debug, Default)]
pub struct RoutingMaps {
    /// Channel to Model routing (uplink)
    pub c2m: HashMap<String, String>,
    /// Model to Channel routing (downlink)
    pub m2c: HashMap<String, String>,
    /// Channel to Channel routing
    pub c2c: HashMap<String, String>,
}

impl RoutingMaps {
    /// Get total route count
    pub fn total_routes(&self) -> usize {
        self.c2m.len() + self.m2c.len() + self.c2c.len()
    }
}

/// Load routing maps from SQLite database
///
/// This function reads routing configuration from SQLite and builds
/// C2M (Channel to Model), M2C (Model to Channel), and C2C (Channel to Channel) mappings.
///
/// # Arguments
/// * `sqlite_pool` - SQLite connection pool
///
/// # Returns
/// * `Ok(RoutingMaps)` - Loaded routing maps
/// * `Err(anyhow::Error)` - Database error
///
/// # Example
/// ```ignore
/// let pool = SqlitePool::connect("sqlite:data/voltage.db").await?;
/// let maps = load_routing_maps(&pool).await?;
/// println!("Loaded {} C2M routes", maps.c2m.len());
/// ```
pub async fn load_routing_maps(sqlite_pool: &sqlx::SqlitePool) -> Result<RoutingMaps> {
    debug!("Loading routing maps from SQLite");

    let keyspace = KeySpaceConfig::production();
    let mut maps = RoutingMaps::default();

    // Load C2M routing (measurement_routing table)
    load_c2m_routes(sqlite_pool, &keyspace, &mut maps.c2m).await?;

    // Load M2C routing (action_routing table)
    load_m2c_routes(sqlite_pool, &keyspace, &mut maps.m2c).await?;

    // Load C2C routing (channel_routing table) - optional
    load_c2c_routes(sqlite_pool, &keyspace, &mut maps.c2c).await;

    info!(
        "Routes loaded: {} C2M, {} M2C, {} C2C",
        maps.c2m.len(),
        maps.m2c.len(),
        maps.c2c.len()
    );

    Ok(maps)
}

/// Load C2M (Channel to Model) routing from measurement_routing table
async fn load_c2m_routes(
    pool: &sqlx::SqlitePool,
    keyspace: &KeySpaceConfig,
    c2m_map: &mut HashMap<String, String>,
) -> Result<()> {
    let rows = sqlx::query_as::<_, (u32, String, u32, String, u32, u32)>(
        r#"
        SELECT instance_id, instance_name, channel_id, channel_type, channel_point_id,
               measurement_id
        FROM measurement_routing
        WHERE enabled = TRUE
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (instance_id, _, channel_id, channel_type, channel_point_id, measurement_id) in rows {
        let point_type = voltage_model::PointType::from_str(&channel_type)
            .ok_or_else(|| anyhow::anyhow!("Invalid channel type: {}", channel_type))?;

        // From: channel_id:type:point_id -> To: instance_id:M:point_id
        let from_key =
            keyspace.c2m_route_key(channel_id, point_type, &channel_point_id.to_string());
        let to_key = format!("{}:M:{}", instance_id, measurement_id);

        c2m_map.insert(from_key.to_string(), to_key);
    }

    Ok(())
}

/// Load M2C (Model to Channel) routing from action_routing table
async fn load_m2c_routes(
    pool: &sqlx::SqlitePool,
    keyspace: &KeySpaceConfig,
    m2c_map: &mut HashMap<String, String>,
) -> Result<()> {
    let rows = sqlx::query_as::<_, (u32, String, u32, u32, String, u32)>(
        r#"
        SELECT instance_id, instance_name, action_id, channel_id, channel_type,
               channel_point_id
        FROM action_routing
        WHERE enabled = TRUE
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (instance_id, _, action_id, channel_id, channel_type, channel_point_id) in rows {
        let point_type = voltage_model::PointType::from_str(&channel_type)
            .ok_or_else(|| anyhow::anyhow!("Invalid channel type: {}", channel_type))?;

        // From: instance_id:A:point_id -> To: channel_id:type:point_id
        let from_key = format!("{}:A:{}", instance_id, action_id);
        let to_key = keyspace.c2m_route_key(channel_id, point_type, &channel_point_id.to_string());

        m2c_map.insert(from_key, to_key.to_string());
    }

    Ok(())
}

/// Load C2C (Channel to Channel) routing from channel_routing table
///
/// Note: This is optional - the table might not exist in older databases.
async fn load_c2c_routes(
    pool: &sqlx::SqlitePool,
    keyspace: &KeySpaceConfig,
    c2c_map: &mut HashMap<String, String>,
) {
    let rows = sqlx::query_as::<_, (u32, String, u32, u32, String, u32, f64, f64)>(
        r#"
        SELECT source_channel_id, source_type, source_point_id,
               target_channel_id, target_type, target_point_id,
               scale, offset
        FROM channel_routing
        WHERE enabled = TRUE
        "#,
    )
    .fetch_all(pool)
    .await;

    let rows = match rows {
        Ok(r) => r,
        Err(e) => {
            debug!("channel_routing table not found, C2C disabled: {}", e);
            return;
        },
    };

    for (
        source_channel_id,
        source_type,
        source_point_id,
        target_channel_id,
        target_type,
        target_point_id,
        _scale,
        _offset,
    ) in rows
    {
        let source_point_type = match voltage_model::PointType::from_str(&source_type) {
            Some(t) => t,
            None => continue,
        };
        let target_point_type = match voltage_model::PointType::from_str(&target_type) {
            Some(t) => t,
            None => continue,
        };

        let from_key = keyspace.c2m_route_key(
            source_channel_id,
            source_point_type,
            &source_point_id.to_string(),
        );
        let to_key = keyspace.c2m_route_key(
            target_channel_id,
            target_point_type,
            &target_point_id.to_string(),
        );

        c2c_map.insert(from_key.to_string(), to_key.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_maps_default() {
        let maps = RoutingMaps::default();
        assert!(maps.c2m.is_empty());
        assert!(maps.m2c.is_empty());
        assert!(maps.c2c.is_empty());
        assert_eq!(maps.total_routes(), 0);
    }
}
