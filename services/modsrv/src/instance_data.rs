//! Instance Data Loading and Query Operations
//!
//! This module provides data loading, querying, and synchronization operations.
//! Extracted from instance_manager.rs for better code organization.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use tracing::debug;

use crate::redis_state;

use super::instance_manager::InstanceManager;
use voltage_rtdb::Rtdb;

impl<R: Rtdb + 'static> InstanceManager<R> {
    /// Get instance real-time data from Redis
    pub async fn get_instance_data(
        &self,
        instance_id: u32,
        data_type: Option<&str>,
    ) -> Result<serde_json::Value> {
        let data =
            redis_state::get_instance_data(self.rtdb.as_ref(), instance_id, data_type).await?;
        Ok(data)
    }

    /// Get instance point definitions from Redis (metadata, not real-time values)
    /// Load instance points with routing configuration (runtime merge)
    ///
    /// This method performs a JOIN query to combine:
    /// - Product point templates (from measurement_points/action_points tables)
    /// - Instance-specific routing (from measurement_routing/action_routing tables)
    pub async fn load_instance_points(
        &self,
        instance_id: u32,
    ) -> Result<(
        Vec<crate::dto::InstanceMeasurementPoint>,
        Vec<crate::dto::InstanceActionPoint>,
    )> {
        use crate::dto::{InstanceActionPoint, InstanceMeasurementPoint, PointRouting};

        // 1. Get product_name from instance
        let product_name = sqlx::query_scalar::<_, String>(
            "SELECT product_name FROM instances WHERE instance_id = ?",
        )
        .bind(instance_id as i32)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        // 2. JOIN query for measurement points (Product template + Instance routing)
        // Also JOIN channels and point tables to get display names
        let measurements = sqlx::query_as::<
            _,
            (
                u32,
                String,
                Option<String>,
                Option<String>, // Point fields: measurement_id, name, unit, description
                Option<i32>,
                Option<String>,
                Option<u32>,
                Option<bool>, // Routing fields: channel_id, channel_type, channel_point_id, enabled
                Option<String>, // channel_name (from channels table)
                Option<String>, // channel_point_name (from telemetry_points/signal_points)
            ),
        >(
            r#"
            SELECT
                mp.measurement_id,
                mp.name,
                mp.unit,
                mp.description,
                mr.channel_id,
                mr.channel_type,
                mr.channel_point_id,
                mr.enabled,
                c.name AS channel_name,
                COALESCE(tp.signal_name, sp.signal_name) AS channel_point_name
            FROM measurement_points mp
            LEFT JOIN measurement_routing mr
                ON mr.instance_id = ? AND mr.measurement_id = mp.measurement_id
            LEFT JOIN channels c ON c.channel_id = mr.channel_id
            LEFT JOIN telemetry_points tp
                ON tp.channel_id = mr.channel_id
                AND tp.point_id = mr.channel_point_id
                AND mr.channel_type = 'T'
            LEFT JOIN signal_points sp
                ON sp.channel_id = mr.channel_id
                AND sp.point_id = mr.channel_point_id
                AND mr.channel_type = 'S'
            WHERE mp.product_name = ?
            ORDER BY mp.measurement_id
            "#,
        )
        .bind(instance_id as i32)
        .bind(&product_name)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(
            |(idx, name, unit, desc, cid, ctype, cpid, enabled, cname, cpname)| {
                InstanceMeasurementPoint {
                    measurement_id: idx,
                    name,
                    unit,
                    description: desc,
                    routing: match (ctype, enabled) {
                        (Some(t), Some(e)) => Some(PointRouting {
                            channel_id: cid,
                            channel_type: Some(t),
                            channel_point_id: cpid,
                            enabled: e,
                            channel_name: cname,
                            channel_point_name: cpname,
                        }),
                        _ => None,
                    },
                }
            },
        )
        .collect();

        // 3. JOIN query for action points (Product template + Instance routing)
        // Also JOIN channels and point tables to get display names
        let actions = sqlx::query_as::<
            _,
            (
                u32,
                String,
                Option<String>,
                Option<String>, // Point fields: action_id, name, unit, description
                Option<i32>,
                Option<String>,
                Option<u32>,
                Option<bool>, // Routing fields: channel_id, channel_type, channel_point_id, enabled
                Option<String>, // channel_name (from channels table)
                Option<String>, // channel_point_name (from control_points/adjustment_points)
            ),
        >(
            r#"
            SELECT
                ap.action_id,
                ap.name,
                ap.unit,
                ap.description,
                ar.channel_id,
                ar.channel_type,
                ar.channel_point_id,
                ar.enabled,
                c.name AS channel_name,
                COALESCE(cp.signal_name, ajp.signal_name) AS channel_point_name
            FROM action_points ap
            LEFT JOIN action_routing ar
                ON ar.instance_id = ? AND ar.action_id = ap.action_id
            LEFT JOIN channels c ON c.channel_id = ar.channel_id
            LEFT JOIN control_points cp
                ON cp.channel_id = ar.channel_id
                AND cp.point_id = ar.channel_point_id
                AND ar.channel_type = 'C'
            LEFT JOIN adjustment_points ajp
                ON ajp.channel_id = ar.channel_id
                AND ajp.point_id = ar.channel_point_id
                AND ar.channel_type = 'A'
            WHERE ap.product_name = ?
            ORDER BY ap.action_id
            "#,
        )
        .bind(instance_id as i32)
        .bind(&product_name)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(
            |(idx, name, unit, desc, cid, ctype, cpid, enabled, cname, cpname)| {
                InstanceActionPoint {
                    action_id: idx,
                    name,
                    unit,
                    description: desc,
                    routing: match (ctype, enabled) {
                        (Some(t), Some(e)) => Some(PointRouting {
                            channel_id: cid,
                            channel_type: Some(t),
                            channel_point_id: cpid,
                            enabled: e,
                            channel_name: cname,
                            channel_point_name: cpname,
                        }),
                        _ => None,
                    },
                }
            },
        )
        .collect();

        Ok((measurements, actions))
    }

    /// Get instance points (SQLite = Single source of truth)
    pub async fn get_instance_points(
        &self,
        instance_id: u32,
        data_type: Option<&str>,
    ) -> Result<serde_json::Value> {
        // ========================================================================
        // SQLite = Single source of truth (Redis = real-time data only)
        // Query point definitions directly from SQLite instead of Redis cache
        // ========================================================================

        // Get instance metadata (product_name, properties)
        let instance_row: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT product_name, properties FROM instances WHERE instance_id = ?")
                .bind(instance_id as i32)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| anyhow!("Failed to load instance {} metadata: {}", instance_id, e))?;

        let Some((product_name, properties_json)) = instance_row else {
            return Err(anyhow!("Instance {} not found", instance_id));
        };

        let properties_json = properties_json.unwrap_or_else(|| "{}".to_string());

        match data_type {
            Some("measurement") => {
                // Query measurement points from SQLite
                let measurements: Vec<(String, String, f64, f64, String)> = sqlx::query_as(
                    "SELECT signal_name, data_type, scale, offset, unit
                     FROM measurement_points WHERE product_name = ?",
                )
                .bind(&product_name)
                .fetch_all(&self.pool)
                .await?;

                let mut result = serde_json::Map::new();
                for (signal_name, data_type, scale, offset, unit) in measurements {
                    let point = serde_json::json!({
                        "signal_name": signal_name,
                        "data_type": data_type,
                        "scale": scale,
                        "offset": offset,
                        "unit": unit
                    });
                    result.insert(signal_name.clone(), point);
                }

                Ok(serde_json::Value::Object(result))
            },
            Some("action") => {
                // Query action points from SQLite
                let actions: Vec<(String, String, f64, f64, String)> = sqlx::query_as(
                    "SELECT signal_name, data_type, scale, offset, unit
                     FROM action_points WHERE product_name = ?",
                )
                .bind(&product_name)
                .fetch_all(&self.pool)
                .await?;

                let mut result = serde_json::Map::new();
                for (signal_name, data_type, scale, offset, unit) in actions {
                    let point = serde_json::json!({
                        "signal_name": signal_name,
                        "data_type": data_type,
                        "scale": scale,
                        "offset": offset,
                        "unit": unit
                    });
                    result.insert(signal_name.clone(), point);
                }

                Ok(serde_json::Value::Object(result))
            },
            Some("property") => {
                // Return instance properties (stored as JSON in instances table)
                let properties: serde_json::Value = serde_json::from_str(&properties_json)
                    .map_err(|e| {
                        anyhow!(
                            "Invalid properties JSON for instance {}: {}",
                            instance_id,
                            e
                        )
                    })?;
                Ok(properties)
            },
            None => {
                // Return all three: measurements, actions, properties
                // Query measurements and actions in parallel using tokio::try_join!
                let measurements_query = sqlx::query_as::<_, (String, String, f64, f64, String)>(
                    "SELECT signal_name, data_type, scale, offset, unit
                     FROM measurement_points WHERE product_name = ?",
                )
                .bind(&product_name)
                .fetch_all(&self.pool);

                let actions_query = sqlx::query_as::<_, (String, String, f64, f64, String)>(
                    "SELECT signal_name, data_type, scale, offset, unit
                     FROM action_points WHERE product_name = ?",
                )
                .bind(&product_name)
                .fetch_all(&self.pool);

                let (measurements, actions) = tokio::try_join!(measurements_query, actions_query)?;

                let mut m_map = serde_json::Map::new();
                for (signal_name, data_type, scale, offset, unit) in measurements {
                    let point = serde_json::json!({
                        "signal_name": signal_name,
                        "data_type": data_type,
                        "scale": scale,
                        "offset": offset,
                        "unit": unit
                    });
                    m_map.insert(signal_name.clone(), point);
                }

                let mut a_map = serde_json::Map::new();
                for (signal_name, data_type, scale, offset, unit) in actions {
                    let point = serde_json::json!({
                        "signal_name": signal_name,
                        "data_type": data_type,
                        "scale": scale,
                        "offset": offset,
                        "unit": unit
                    });
                    a_map.insert(signal_name.clone(), point);
                }

                let properties: serde_json::Value = serde_json::from_str(&properties_json)
                    .map_err(|e| {
                        anyhow!(
                            "Invalid properties JSON for instance {}: {}",
                            instance_id,
                            e
                        )
                    })?;

                Ok(serde_json::json!({
                    "measurements": m_map,
                    "actions": a_map,
                    "properties": properties
                }))
            },
            Some(other) => Err(anyhow!(
                "Unknown data type '{}'; use 'measurement', 'action', 'property', or omit for all",
                other
            )),
        }
    }

    /// Sync measurement data to instance
    pub async fn sync_measurement(
        &self,
        instance_id: u32,
        data: HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        redis_state::sync_measurement(self.rtdb.as_ref(), instance_id, data).await?;

        debug!("Synced measurement data for instance {}", instance_id);
        Ok(())
    }

    /// Execute action on instance
    pub async fn execute_action(
        &self,
        instance_id: u32,
        action_id: &str,
        value: f64,
    ) -> Result<()> {
        // Use application-layer routing with cache
        let outcome = voltage_routing::set_action_point(
            self.rtdb.as_ref(),
            &self.routing_cache,
            instance_id,
            action_id,
            value,
        )
        .await?;

        if outcome.routed {
            debug!(
                "Action {} routed to channel {} for instance {}",
                action_id,
                outcome
                    .route_result
                    .as_deref()
                    .unwrap_or("<unknown_channel>"),
                instance_id
            );
        } else {
            debug!(
                "Action {} stored but not routed for instance {} - {}",
                action_id,
                instance_id,
                outcome
                    .route_result
                    .as_deref()
                    .unwrap_or("<no_route_reason>")
            );
        }

        Ok(())
    }

    /// Load a single measurement point with routing configuration
    pub async fn load_single_measurement_point(
        &self,
        instance_id: u32,
        point_id: u32,
    ) -> Result<crate::dto::InstanceMeasurementPoint> {
        use crate::dto::{InstanceMeasurementPoint, PointRouting};

        // 1. Get product_name
        let product_name = sqlx::query_scalar::<_, String>(
            "SELECT product_name FROM instances WHERE instance_id = ?",
        )
        .bind(instance_id as i32)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        // 2. JOIN query for the specific measurement point (with channel and point names)
        let point = sqlx::query_as::<
            _,
            (
                u32,
                String,
                Option<String>,
                Option<String>, // Point fields
                Option<i32>,
                Option<String>,
                Option<u32>,
                Option<bool>,   // Routing fields
                Option<String>, // channel_name
                Option<String>, // channel_point_name
            ),
        >(
            r#"
            SELECT
                mp.measurement_id,
                mp.name,
                mp.unit,
                mp.description,
                mr.channel_id,
                mr.channel_type,
                mr.channel_point_id,
                mr.enabled,
                c.name AS channel_name,
                COALESCE(tp.signal_name, sp.signal_name) AS channel_point_name
            FROM measurement_points mp
            LEFT JOIN measurement_routing mr
                ON mr.instance_id = ? AND mr.measurement_id = mp.measurement_id
            LEFT JOIN channels c ON c.channel_id = mr.channel_id
            LEFT JOIN telemetry_points tp
                ON tp.channel_id = mr.channel_id
                AND tp.point_id = mr.channel_point_id
                AND mr.channel_type = 'T'
            LEFT JOIN signal_points sp
                ON sp.channel_id = mr.channel_id
                AND sp.point_id = mr.channel_point_id
                AND mr.channel_type = 'S'
            WHERE mp.product_name = ? AND mp.measurement_id = ?
            "#,
        )
        .bind(instance_id as i32)
        .bind(&product_name)
        .bind(point_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            anyhow!(
                "Measurement point {} not found for instance {}: {}",
                point_id,
                instance_id,
                e
            )
        })?;

        let (idx, name, unit, desc, cid, ctype, cpid, enabled, cname, cpname) = point;

        Ok(InstanceMeasurementPoint {
            measurement_id: idx,
            name,
            unit,
            description: desc,
            routing: match (ctype, enabled) {
                (Some(t), Some(e)) => Some(PointRouting {
                    channel_id: cid,
                    channel_type: Some(t),
                    channel_point_id: cpid,
                    enabled: e,
                    channel_name: cname,
                    channel_point_name: cpname,
                }),
                _ => None,
            },
        })
    }

    /// Load a single action point with routing configuration
    pub async fn load_single_action_point(
        &self,
        instance_id: u32,
        point_id: u32,
    ) -> Result<crate::dto::InstanceActionPoint> {
        use crate::dto::{InstanceActionPoint, PointRouting};

        // 1. Get product_name
        let product_name = sqlx::query_scalar::<_, String>(
            "SELECT product_name FROM instances WHERE instance_id = ?",
        )
        .bind(instance_id as i32)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        // 2. JOIN query for the specific action point (with channel and point names)
        let point = sqlx::query_as::<
            _,
            (
                u32,
                String,
                Option<String>,
                Option<String>, // Point fields
                Option<i32>,
                Option<String>,
                Option<u32>,
                Option<bool>,   // Routing fields
                Option<String>, // channel_name
                Option<String>, // channel_point_name
            ),
        >(
            r#"
            SELECT
                ap.action_id,
                ap.name,
                ap.unit,
                ap.description,
                ar.channel_id,
                ar.channel_type,
                ar.channel_point_id,
                ar.enabled,
                c.name AS channel_name,
                COALESCE(cp.signal_name, ajp.signal_name) AS channel_point_name
            FROM action_points ap
            LEFT JOIN action_routing ar
                ON ar.instance_id = ? AND ar.action_id = ap.action_id
            LEFT JOIN channels c ON c.channel_id = ar.channel_id
            LEFT JOIN control_points cp
                ON cp.channel_id = ar.channel_id
                AND cp.point_id = ar.channel_point_id
                AND ar.channel_type = 'C'
            LEFT JOIN adjustment_points ajp
                ON ajp.channel_id = ar.channel_id
                AND ajp.point_id = ar.channel_point_id
                AND ar.channel_type = 'A'
            WHERE ap.product_name = ? AND ap.action_id = ?
            "#,
        )
        .bind(instance_id as i32)
        .bind(&product_name)
        .bind(point_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            anyhow!(
                "Action point {} not found for instance {}: {}",
                point_id,
                instance_id,
                e
            )
        })?;

        let (idx, name, unit, desc, cid, ctype, cpid, enabled, cname, cpname) = point;

        Ok(InstanceActionPoint {
            action_id: idx,
            name,
            unit,
            description: desc,
            routing: match (ctype, enabled) {
                (Some(t), Some(e)) => Some(PointRouting {
                    channel_id: cid,
                    channel_type: Some(t),
                    channel_point_id: cpid,
                    enabled: e,
                    channel_name: cname,
                    channel_point_name: cpname,
                }),
                _ => None,
            },
        })
    }
}
