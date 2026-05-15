//! Instance Data Loading and Query Operations
//!
//! This module provides data loading, querying, and synchronization operations.
//! Extracted from instance_manager.rs for better code organization.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use crate::error::ModSrvError;
use crate::infra::shm_dispatch::DispatchOutcome;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use tracing::{debug, error, warn};

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

    /// Load instance points with routing configuration (runtime merge)
    ///
    /// Two-step query + in-memory merge:
    /// 1. Product point definitions from built-in products (compile-time constants)
    /// 2. Routing data from measurement_routing/action_routing tables (real tables)
    /// 3. Merge in application layer using HashMap
    pub async fn load_instance_points(
        &self,
        instance_id: u32,
    ) -> Result<(
        Vec<crate::dto::InstanceMeasurementPoint>,
        Vec<crate::dto::InstanceActionPoint>,
    )> {
        use crate::dto::{InstanceActionPoint, InstanceMeasurementPoint, PointRouting};

        // 1. Get product_name and product definition
        let product_name = sqlx::query_scalar::<_, String>(
            "SELECT product_name FROM instances WHERE instance_id = ?",
        )
        .bind(instance_id as i64)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        let product = self
            .product_loader
            .get_product(&product_name)
            .map_err(|e| anyhow!("Product '{}' not found: {}", product_name, e))?;

        // 2. Query routing data from real tables (parallel)
        let m_routing_query = sqlx::query_as::<
            _,
            (
                u32,        // measurement_id
                Option<i32>,    // channel_id
                Option<String>, // channel_type
                Option<u32>,    // channel_point_id
                Option<bool>,   // enabled
                Option<String>, // channel_name
                Option<String>, // channel_point_name
            ),
        >(
            r#"SELECT mr.measurement_id, mr.channel_id, mr.channel_type, mr.channel_point_id, mr.enabled,
                    c.name AS channel_name,
                    COALESCE(tp.signal_name, sp.signal_name) AS channel_point_name
               FROM measurement_routing mr
               LEFT JOIN channels c ON c.channel_id = mr.channel_id
               LEFT JOIN telemetry_points tp ON tp.channel_id = mr.channel_id AND tp.point_id = mr.channel_point_id AND mr.channel_type = 'T'
               LEFT JOIN signal_points sp ON sp.channel_id = mr.channel_id AND sp.point_id = mr.channel_point_id AND mr.channel_type = 'S'
               WHERE mr.instance_id = ?"#,
        )
        .bind(instance_id as i64)
        .fetch_all(&self.pool);

        let a_routing_query = sqlx::query_as::<
            _,
            (
                u32,        // action_id
                Option<i32>,    // channel_id
                Option<String>, // channel_type
                Option<u32>,    // channel_point_id
                Option<bool>,   // enabled
                Option<String>, // channel_name
                Option<String>, // channel_point_name
            ),
        >(
            r#"SELECT ar.action_id, ar.channel_id, ar.channel_type, ar.channel_point_id, ar.enabled,
                    c.name AS channel_name,
                    COALESCE(cp.signal_name, ajp.signal_name) AS channel_point_name
               FROM action_routing ar
               LEFT JOIN channels c ON c.channel_id = ar.channel_id
               LEFT JOIN control_points cp ON cp.channel_id = ar.channel_id AND cp.point_id = ar.channel_point_id AND ar.channel_type = 'C'
               LEFT JOIN adjustment_points ajp ON ajp.channel_id = ar.channel_id AND ajp.point_id = ar.channel_point_id AND ar.channel_type = 'A'
               WHERE ar.instance_id = ?"#,
        )
        .bind(instance_id as i64)
        .fetch_all(&self.pool);

        let (m_routing_rows, a_routing_rows) = tokio::try_join!(m_routing_query, a_routing_query)?;

        // 3. Merge: product point definitions + routing data
        let mut m_routing_map: HashMap<u32, _> =
            m_routing_rows.into_iter().map(|r| (r.0, r)).collect();

        let measurements = product
            .measurements
            .iter()
            .map(|mp| {
                let routing = m_routing_map.remove(&mp.measurement_id).and_then(
                    |(_, cid, ctype, cpid, enabled, cname, cpname)| match (ctype, enabled) {
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
                );
                InstanceMeasurementPoint {
                    measurement_id: mp.measurement_id,
                    name: mp.name.clone(),
                    unit: mp.unit.clone(),
                    description: mp.description.clone(),
                    routing,
                }
            })
            .collect();

        let mut a_routing_map: HashMap<u32, _> =
            a_routing_rows.into_iter().map(|r| (r.0, r)).collect();

        let actions = product
            .actions
            .iter()
            .map(|ap| {
                let routing = a_routing_map.remove(&ap.action_id).and_then(
                    |(_, cid, ctype, cpid, enabled, cname, cpname)| match (ctype, enabled) {
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
                );
                InstanceActionPoint {
                    action_id: ap.action_id,
                    name: ap.name.clone(),
                    unit: ap.unit.clone(),
                    description: ap.description.clone(),
                    routing,
                }
            })
            .collect();

        Ok((measurements, actions))
    }

    /// Get instance points (built-in product definitions = single source of truth)
    pub async fn get_instance_points(
        &self,
        instance_id: u32,
        data_type: Option<&str>,
    ) -> Result<serde_json::Value> {
        // Get instance metadata (product_name, properties)
        let instance_row: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT product_name, properties FROM instances WHERE instance_id = ?")
                .bind(instance_id as i64)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| anyhow!("Failed to load instance {} metadata: {}", instance_id, e))?;

        let Some((product_name, properties_json)) = instance_row else {
            return Err(anyhow!("Instance {} not found", instance_id));
        };

        let properties_json = properties_json.unwrap_or_else(|| "{}".to_string());

        // Get product from built-in definitions (compile-time constants)
        let product = self
            .product_loader
            .get_product(&product_name)
            .map_err(|e| anyhow!("Product '{}' not found: {}", product_name, e))?;

        match data_type {
            Some("measurement") => {
                let mut result = serde_json::Map::new();
                for m in &product.measurements {
                    let point = serde_json::json!({
                        "measurement_id": m.measurement_id,
                        "name": &m.name,
                        "unit": &m.unit,
                        "description": &m.description
                    });
                    result.insert(m.name.clone(), point);
                }
                Ok(serde_json::Value::Object(result))
            },
            Some("action") => {
                let mut result = serde_json::Map::new();
                for a in &product.actions {
                    let point = serde_json::json!({
                        "action_id": a.action_id,
                        "name": &a.name,
                        "unit": &a.unit,
                        "description": &a.description
                    });
                    result.insert(a.name.clone(), point);
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
                let mut m_map = serde_json::Map::new();
                for m in &product.measurements {
                    let point = serde_json::json!({
                        "measurement_id": m.measurement_id,
                        "name": &m.name,
                        "unit": &m.unit,
                        "description": &m.description
                    });
                    m_map.insert(m.name.clone(), point);
                }

                let mut a_map = serde_json::Map::new();
                for a in &product.actions {
                    let point = serde_json::json!({
                        "action_id": a.action_id,
                        "name": &a.name,
                        "unit": &a.unit,
                        "description": &a.description
                    });
                    a_map.insert(a.name.clone(), point);
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
    ) -> crate::error::Result<()> {
        // Route via application-layer cache, dispatch via SHM+UDS to comsrv

        // M2C control gate: reject writes to offline channels before they hit
        // the routing/SHM path. Without this the command is silently dropped
        // by the device while modsrv thinks the write succeeded. Only enforced
        // when the action has an M2C route (no-route writes go to the instance
        // hash only and don't touch any channel).
        if let Ok(point_id_u32) = action_id.parse::<u32>()
            && let Some(target) = self.routing_cache.lookup_m2c_by_parts(
                instance_id,
                voltage_model::PointType::Adjustment,
                point_id_u32,
            )
            && !self.health_cache.is_online(target.channel_id)
        {
            warn!(
                "Action rejected: channel {} offline (instance {} action {})",
                target.channel_id, instance_id, action_id
            );
            return Err(ModSrvError::ChannelUnreachable {
                channel_id: target.channel_id,
            });
        }

        let outcome = voltage_routing::set_action_point(
            self.rtdb.as_ref(),
            &self.routing_cache,
            instance_id,
            action_id,
            value,
        )
        .await
        .map_err(|e| ModSrvError::InternalError(e.to_string()))?;

        // Dispatch action to comsrv via SHM+UDS (production) or noop (tests)
        // SCADA safety: dispatch failure must propagate to the operator via API error
        if let Some(ctx) = &outcome.route_context {
            match self.dispatch.dispatch(ctx, value).await {
                DispatchOutcome::Delivered | DispatchOutcome::Noop => {},
                DispatchOutcome::ShmOnly { reason } => {
                    warn!(
                        "Action dispatch degraded: SHM written but UDS failed ({}) for instance {} action {}",
                        reason, instance_id, action_id
                    );
                    return Err(ModSrvError::DispatchDegraded(format!(
                        "UDS notification failed ({}): command may not reach device",
                        reason
                    )));
                },
                DispatchOutcome::NoWriter => {
                    error!(
                        "Action dispatch failed: no SHM writer for instance {} action {}",
                        instance_id, action_id
                    );
                    return Err(ModSrvError::DispatchDegraded(
                        "SHM writer unavailable: comsrv may have restarted".to_string(),
                    ));
                },
            }
        }

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

        // 1. Get product_name and product definition
        let product_name = sqlx::query_scalar::<_, String>(
            "SELECT product_name FROM instances WHERE instance_id = ?",
        )
        .bind(instance_id as i64)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        let product = self
            .product_loader
            .get_product(&product_name)
            .map_err(|e| anyhow!("Product '{}' not found: {}", product_name, e))?;

        // 2. Find the measurement point in built-in product
        let mp = product
            .measurements
            .iter()
            .find(|m| m.measurement_id == point_id)
            .ok_or_else(|| {
                anyhow!(
                    "Measurement point {} not found in product '{}'",
                    point_id,
                    product_name
                )
            })?;

        // 3. Query routing for this specific point
        let routing_row = sqlx::query_as::<
            _,
            (
                Option<i32>,    // channel_id
                Option<String>, // channel_type
                Option<u32>,    // channel_point_id
                Option<bool>,   // enabled
                Option<String>, // channel_name
                Option<String>, // channel_point_name
            ),
        >(
            r#"SELECT mr.channel_id, mr.channel_type, mr.channel_point_id, mr.enabled,
                    c.name AS channel_name,
                    COALESCE(tp.signal_name, sp.signal_name) AS channel_point_name
               FROM measurement_routing mr
               LEFT JOIN channels c ON c.channel_id = mr.channel_id
               LEFT JOIN telemetry_points tp ON tp.channel_id = mr.channel_id AND tp.point_id = mr.channel_point_id AND mr.channel_type = 'T'
               LEFT JOIN signal_points sp ON sp.channel_id = mr.channel_id AND sp.point_id = mr.channel_point_id AND mr.channel_type = 'S'
               WHERE mr.instance_id = ? AND mr.measurement_id = ?"#,
        )
        .bind(instance_id as i64)
        .bind(point_id)
        .fetch_optional(&self.pool)
        .await?;

        let routing = routing_row.and_then(|(cid, ctype, cpid, enabled, cname, cpname)| {
            match (ctype, enabled) {
                (Some(t), Some(e)) => Some(PointRouting {
                    channel_id: cid,
                    channel_type: Some(t),
                    channel_point_id: cpid,
                    enabled: e,
                    channel_name: cname,
                    channel_point_name: cpname,
                }),
                _ => None,
            }
        });

        Ok(InstanceMeasurementPoint {
            measurement_id: mp.measurement_id,
            name: mp.name.clone(),
            unit: mp.unit.clone(),
            description: mp.description.clone(),
            routing,
        })
    }

    /// Load a single action point with routing configuration
    pub async fn load_single_action_point(
        &self,
        instance_id: u32,
        point_id: u32,
    ) -> Result<crate::dto::InstanceActionPoint> {
        use crate::dto::{InstanceActionPoint, PointRouting};

        // 1. Get product_name and product definition
        let product_name = sqlx::query_scalar::<_, String>(
            "SELECT product_name FROM instances WHERE instance_id = ?",
        )
        .bind(instance_id as i64)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        let product = self
            .product_loader
            .get_product(&product_name)
            .map_err(|e| anyhow!("Product '{}' not found: {}", product_name, e))?;

        // 2. Find the action point in built-in product
        let ap = product
            .actions
            .iter()
            .find(|a| a.action_id == point_id)
            .ok_or_else(|| {
                anyhow!(
                    "Action point {} not found in product '{}'",
                    point_id,
                    product_name
                )
            })?;

        // 3. Query routing for this specific point
        let routing_row = sqlx::query_as::<
            _,
            (
                Option<i32>,    // channel_id
                Option<String>, // channel_type
                Option<u32>,    // channel_point_id
                Option<bool>,   // enabled
                Option<String>, // channel_name
                Option<String>, // channel_point_name
            ),
        >(
            r#"SELECT ar.channel_id, ar.channel_type, ar.channel_point_id, ar.enabled,
                    c.name AS channel_name,
                    COALESCE(cp.signal_name, ajp.signal_name) AS channel_point_name
               FROM action_routing ar
               LEFT JOIN channels c ON c.channel_id = ar.channel_id
               LEFT JOIN control_points cp ON cp.channel_id = ar.channel_id AND cp.point_id = ar.channel_point_id AND ar.channel_type = 'C'
               LEFT JOIN adjustment_points ajp ON ajp.channel_id = ar.channel_id AND ajp.point_id = ar.channel_point_id AND ar.channel_type = 'A'
               WHERE ar.instance_id = ? AND ar.action_id = ?"#,
        )
        .bind(instance_id as i64)
        .bind(point_id)
        .fetch_optional(&self.pool)
        .await?;

        let routing = routing_row.and_then(|(cid, ctype, cpid, enabled, cname, cpname)| {
            match (ctype, enabled) {
                (Some(t), Some(e)) => Some(PointRouting {
                    channel_id: cid,
                    channel_type: Some(t),
                    channel_point_id: cpid,
                    enabled: e,
                    channel_name: cname,
                    channel_point_name: cpname,
                }),
                _ => None,
            }
        });

        Ok(InstanceActionPoint {
            action_id: ap.action_id,
            name: ap.name.clone(),
            unit: ap.unit.clone(),
            description: ap.description.clone(),
            routing,
        })
    }
}
