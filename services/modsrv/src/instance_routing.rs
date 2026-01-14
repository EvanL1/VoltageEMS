//! Instance Routing Management
//!
//! This module provides routing CRUD operations for measurement and action points.
//! Extracted from instance_manager.rs for better code organization.

use anyhow::{anyhow, Result};
use common::{ValidationLevel, ValidationResult};
use tracing::{debug, warn};
use voltage_model::PointType;

use crate::routing_loader::{
    ActionRouting, ActionRoutingRow, MeasurementRouting, MeasurementRoutingRow,
};

use super::instance_manager::InstanceManager;
use voltage_rtdb::{Rtdb, SharedSlotRef};

impl<R: Rtdb + 'static> InstanceManager<R> {
    /// Create or update routing for a single measurement point (UPSERT)
    pub async fn upsert_measurement_routing(
        &self,
        instance_id: u32,
        point_id: u32,
        request: crate::dto::SinglePointRoutingRequest,
    ) -> Result<()> {
        // Validate channel_type (must be T or S for measurement, skip if None - unbound)
        if let Some(ref fr) = request.four_remote {
            if !fr.is_input() {
                return Err(anyhow!(
                    "Invalid channel_type '{}' for measurement routing (must be T or S)",
                    fr
                ));
            }
        }

        // Get instance_name for routing table denormalization
        let instance_name = sqlx::query_scalar::<_, String>(
            "SELECT instance_name FROM instances WHERE instance_id = ?",
        )
        .bind(instance_id as i32)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        // UPSERT into measurement_routing
        sqlx::query(
            r#"
            INSERT INTO measurement_routing
            (instance_id, instance_name, channel_id, channel_type, channel_point_id,
             measurement_id, enabled)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(instance_id, measurement_id)
            DO UPDATE SET
                channel_id = excluded.channel_id,
                channel_type = excluded.channel_type,
                channel_point_id = excluded.channel_point_id,
                enabled = excluded.enabled,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(instance_id as i32)
        .bind(instance_name)
        .bind(request.channel_id)
        .bind(request.four_remote.map(|fr| fr.as_str()))
        .bind(request.channel_point_id)
        .bind(point_id)
        .bind(request.enabled)
        .execute(&self.pool)
        .await?;

        // Dynamic Slot Allocation: Add shared_slot reference for C2M routing
        // Only for enabled bound routing (has channel info)
        if request.enabled {
            if let (Some(index), Some(channel_id), Some(channel_type), Some(channel_point_id)) = (
                self.dynamic_instance_index(),
                request.channel_id,
                request.four_remote,
                request.channel_point_id,
            ) {
                let channel_id = channel_id as u32;
                let slot_ref = SharedSlotRef {
                    // slot_id = 0: Delayed computation - resolve when accessing shared memory
                    slot_id: 0,
                    // point_type = source channel type (T/S for measurement routing)
                    point_type: channel_type,
                    point_id,
                    source_channel_id: channel_id,
                    source_point_type: channel_type,
                    source_point_id: channel_point_id,
                };

                match index.add_shared_slot(instance_id, slot_ref) {
                    Ok(()) => {
                        debug!(
                            "Inst{}:M:{} shared_slot added (ch:{}:{}:{})",
                            instance_id, point_id, channel_id, channel_type, channel_point_id
                        );
                    },
                    Err(e) => {
                        warn!(
                            "Inst{}:M:{} shared_slot add failed: {}",
                            instance_id, point_id, e
                        );
                    },
                }
            }
        }

        Ok(())
    }

    /// Create or update routing for a single action point (UPSERT)
    pub async fn upsert_action_routing(
        &self,
        instance_id: u32,
        point_id: u32,
        request: crate::dto::SinglePointRoutingRequest,
    ) -> Result<()> {
        // Validate channel_type (must be C or A for action, skip if None - unbound)
        if let Some(ref fr) = request.four_remote {
            if !fr.is_output() {
                return Err(anyhow!(
                    "Invalid channel_type '{}' for action routing (must be C or A)",
                    fr
                ));
            }
        }

        // Get instance_name for routing table denormalization
        let instance_name = sqlx::query_scalar::<_, String>(
            "SELECT instance_name FROM instances WHERE instance_id = ?",
        )
        .bind(instance_id as i32)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        // UPSERT into action_routing
        sqlx::query(
            r#"
            INSERT INTO action_routing
            (instance_id, instance_name, action_id, channel_id, channel_type,
             channel_point_id, enabled)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(instance_id, action_id)
            DO UPDATE SET
                channel_id = excluded.channel_id,
                channel_type = excluded.channel_type,
                channel_point_id = excluded.channel_point_id,
                enabled = excluded.enabled,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(instance_id as i32)
        .bind(instance_name)
        .bind(point_id)
        .bind(request.channel_id)
        .bind(request.four_remote.map(|fr| fr.as_str()))
        .bind(request.channel_point_id)
        .bind(request.enabled)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete routing for a single measurement point
    pub async fn delete_measurement_routing(&self, instance_id: u32, point_id: u32) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM measurement_routing WHERE instance_id = ? AND measurement_id = ?",
        )
        .bind(instance_id as i32)
        .bind(point_id)
        .execute(&self.pool)
        .await?;

        // Dynamic Slot Allocation: Remove shared_slot reference
        // Try both Telemetry and Signal types since measurement routing uses T/S
        if result.rows_affected() > 0 {
            if let Some(index) = self.dynamic_instance_index() {
                // Try Telemetry first, then Signal
                let removed = index
                    .remove_shared_slot(instance_id, PointType::Telemetry, point_id)
                    .ok()
                    .flatten()
                    .or_else(|| {
                        index
                            .remove_shared_slot(instance_id, PointType::Signal, point_id)
                            .ok()
                            .flatten()
                    });

                match removed {
                    Some(slot_ref) => {
                        debug!(
                            "Inst{}:M:{} shared_slot removed (ch:{}:{}:{})",
                            instance_id,
                            point_id,
                            slot_ref.source_channel_id,
                            slot_ref.source_point_type,
                            slot_ref.source_point_id
                        );
                    },
                    None => {
                        // No shared_slot found - might be unbound routing
                        debug!(
                            "Inst{}:M:{} no shared_slot to remove",
                            instance_id, point_id
                        );
                    },
                }
            }
        }

        Ok(result.rows_affected())
    }

    /// Delete routing for a single action point
    pub async fn delete_action_routing(&self, instance_id: u32, point_id: u32) -> Result<u64> {
        let result =
            sqlx::query("DELETE FROM action_routing WHERE instance_id = ? AND action_id = ?")
                .bind(instance_id as i32)
                .bind(point_id)
                .execute(&self.pool)
                .await?;

        Ok(result.rows_affected())
    }

    /// Toggle enabled state for a single measurement point routing
    pub async fn toggle_measurement_routing(
        &self,
        instance_id: u32,
        point_id: u32,
        enabled: bool,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE measurement_routing
            SET enabled = ?, updated_at = CURRENT_TIMESTAMP
            WHERE instance_id = ? AND measurement_id = ?
            "#,
        )
        .bind(enabled)
        .bind(instance_id as i32)
        .bind(point_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Toggle enabled state for a single action point routing
    pub async fn toggle_action_routing(
        &self,
        instance_id: u32,
        point_id: u32,
        enabled: bool,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE action_routing
            SET enabled = ?, updated_at = CURRENT_TIMESTAMP
            WHERE instance_id = ? AND action_id = ?
            "#,
        )
        .bind(enabled)
        .bind(instance_id as i32)
        .bind(point_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get all measurement routing for an instance
    ///
    /// Retrieves all enabled measurement routing entries for the specified instance.
    pub async fn get_measurement_routing(
        &self,
        instance_id: u32,
    ) -> Result<Vec<MeasurementRouting>> {
        let routing = sqlx::query_as::<_, MeasurementRouting>(
            r#"
            SELECT * FROM measurement_routing
            WHERE instance_id = ? AND enabled = TRUE
            ORDER BY channel_id, channel_type, channel_point_id
            "#,
        )
        .bind(instance_id as i32)
        .fetch_all(&self.pool)
        .await?;

        Ok(routing)
    }

    /// Get all action routing for an instance
    ///
    /// Retrieves all enabled action routing entries for the specified instance.
    pub async fn get_action_routing(&self, instance_id: u32) -> Result<Vec<ActionRouting>> {
        let routing = sqlx::query_as::<_, ActionRouting>(
            r#"
            SELECT * FROM action_routing
            WHERE instance_id = ? AND enabled = TRUE
            ORDER BY action_id
            "#,
        )
        .bind(instance_id as i32)
        .fetch_all(&self.pool)
        .await?;

        Ok(routing)
    }

    /// Validate a measurement routing entry
    ///
    /// Checks if a measurement routing configuration is valid by verifying:
    /// - Instance exists
    /// - Channel type is input (T or S)
    /// - Measurement point exists for the instance's product
    pub async fn validate_measurement_routing(
        &self,
        routing: &MeasurementRoutingRow,
        instance_name: &str,
    ) -> Result<ValidationResult> {
        let mut errors = Vec::new();

        // Validate instance exists
        let instance_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM instances WHERE instance_name = ?)",
        )
        .bind(instance_name)
        .fetch_one(&self.pool)
        .await?;

        if !instance_exists {
            errors.push(format!("Instance {} does not exist", instance_name));
        }

        // Validate channel_type (skip if None - unbound routing is valid)
        if let Some(ref ct) = routing.channel_type {
            if !ct.is_input() {
                errors.push(format!(
                    "Invalid channel_type for measurement: {}. Must be T or S",
                    ct
                ));
            }
        }

        // Validate measurement point exists
        let point_exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM measurement_points mp
                JOIN instances i ON i.product_name = mp.product_name
                WHERE i.instance_name = ? AND mp.measurement_id = ?
            )
            "#,
        )
        .bind(instance_name)
        .bind(routing.measurement_id)
        .fetch_one(&self.pool)
        .await?;

        if !point_exists {
            errors.push(format!(
                "Measurement point {} not found for instance {}",
                routing.measurement_id, instance_name
            ));
        }

        let mut result = ValidationResult::new(ValidationLevel::Business);
        for error in errors {
            result.add_error(error);
        }
        Ok(result)
    }

    /// Validate an action routing entry
    ///
    /// Checks if an action routing configuration is valid by verifying:
    /// - Instance exists
    /// - Channel type is output (C or A)
    /// - Action point exists for the instance's product
    pub async fn validate_action_routing(
        &self,
        routing: &ActionRoutingRow,
        instance_name: &str,
    ) -> Result<ValidationResult> {
        let mut errors = Vec::new();

        // Validate instance exists
        let instance_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM instances WHERE instance_name = ?)",
        )
        .bind(instance_name)
        .fetch_one(&self.pool)
        .await?;

        if !instance_exists {
            errors.push(format!("Instance {} does not exist", instance_name));
        }

        // Validate channel_type (skip if None - unbound routing is valid)
        if let Some(ref ct) = routing.channel_type {
            if !ct.is_output() {
                errors.push(format!(
                    "Invalid channel_type for action: {}. Must be C or A",
                    ct
                ));
            }
        }

        // Validate action point exists
        let point_exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM action_points ap
                JOIN instances i ON i.product_name = ap.product_name
                WHERE i.instance_name = ? AND ap.action_id = ?
            )
            "#,
        )
        .bind(instance_name)
        .bind(routing.action_id)
        .fetch_one(&self.pool)
        .await?;

        if !point_exists {
            errors.push(format!(
                "Action point {} not found for instance {}",
                routing.action_id, instance_name
            ));
        }

        let mut result = ValidationResult::new(ValidationLevel::Business);
        for error in errors {
            result.add_error(error);
        }
        Ok(result)
    }

    /// Delete all routing for an instance
    ///
    /// Removes all measurement and action routing entries for the specified instance.
    /// Also clears all shared_slots from InstanceIndex.
    pub async fn delete_all_routing(&self, instance_id: u32) -> Result<(u64, u64)> {
        let measurement_result =
            sqlx::query("DELETE FROM measurement_routing WHERE instance_id = ?")
                .bind(instance_id as i32)
                .execute(&self.pool)
                .await?;

        let action_result = sqlx::query("DELETE FROM action_routing WHERE instance_id = ?")
            .bind(instance_id as i32)
            .execute(&self.pool)
            .await?;

        // Dynamic Slot Allocation: Clear all shared_slots for this instance
        // Note: We don't remove the instance from InstanceIndex, just clear its shared_slots
        if measurement_result.rows_affected() > 0 {
            if let Some(index) = self.dynamic_instance_index() {
                match index.clear_shared_slots(instance_id) {
                    Ok(cleared_count) if cleared_count > 0 => {
                        debug!(
                            "Inst{} cleared {} shared_slots (routing deleted)",
                            instance_id, cleared_count
                        );
                    },
                    Ok(_) => {},
                    Err(e) => {
                        // Instance not found in InstanceIndex - might not be using dynamic allocation
                        debug!("Inst{} clear_shared_slots skipped: {}", instance_id, e);
                    },
                }
            }
        }

        Ok((
            measurement_result.rows_affected(),
            action_result.rows_affected(),
        ))
    }
}
