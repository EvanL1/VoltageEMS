//! Models API - Library Mode
//!
//! Direct library calls to modsrv for instance and product management

use crate::context::ModsrvContext;
use crate::lib_api::{LibApiError, Result};
use modsrv::{CreateInstanceRequest, Instance, Product};
use serde::{Deserialize, Serialize};
use voltage_rtdb::Rtdb;

/// Instance summary for list operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceSummary {
    pub id: u32,
    pub name: String,
    pub product_name: String,
    pub enabled: bool,
}

/// Models service - provides instance and product management operations
pub struct ModelsService<'a> {
    ctx: &'a ModsrvContext,
}

impl<'a> ModelsService<'a> {
    /// Create a new models service from context
    pub fn new(ctx: &'a ModsrvContext) -> Self {
        Self { ctx }
    }

    /// List all instances
    ///
    /// Returns a list of all configured model instances.
    pub async fn list_instances(&self) -> Result<Vec<InstanceSummary>> {
        // Query database for instances
        let db_instances: Vec<(u32, String, String, bool)> = sqlx::query_as(
            "SELECT instance_id, name, product_name, enabled FROM instances ORDER BY instance_id",
        )
        .fetch_all(&self.ctx.sqlite_pool)
        .await?;

        let summaries: Vec<InstanceSummary> = db_instances
            .into_iter()
            .map(|(id, name, product_name, enabled)| InstanceSummary {
                id,
                name,
                product_name,
                enabled,
            })
            .collect();

        Ok(summaries)
    }

    /// Get instance by name
    ///
    /// Returns detailed information about a specific instance.
    pub async fn get_instance(&self, name: &str) -> Result<Instance> {
        // First, get instance ID from database
        let instance: Option<(u32,)> =
            sqlx::query_as("SELECT instance_id FROM instances WHERE instance_name = ?")
                .bind(name)
                .fetch_optional(&self.ctx.sqlite_pool)
                .await?;

        let (instance_id,) = instance
            .ok_or_else(|| LibApiError::not_found(format!("Instance '{}' not found", name)))?;

        // Use instance manager to get instance by ID
        self.ctx
            .instance_manager
            .get_instance(instance_id)
            .await
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    LibApiError::not_found(format!("Instance '{}' not found", name)).into()
                } else {
                    e
                }
            })
    }

    /// Create a new instance
    ///
    /// Creates a new model instance based on a product template.
    pub async fn create_instance(&self, request: CreateInstanceRequest) -> Result<Instance> {
        self.ctx.instance_manager.create_instance(request).await
    }

    /// Delete an instance
    ///
    /// Removes an instance and cleans up its data from Redis.
    pub async fn delete_instance(&self, name: &str) -> Result<()> {
        // First, get instance ID from database
        let instance: Option<(u32,)> =
            sqlx::query_as("SELECT instance_id FROM instances WHERE instance_name = ?")
                .bind(name)
                .fetch_optional(&self.ctx.sqlite_pool)
                .await?;

        let (instance_id,) = instance
            .ok_or_else(|| LibApiError::not_found(format!("Instance '{}' not found", name)))?;

        // Delete instance by ID
        self.ctx.instance_manager.delete_instance(instance_id).await
    }

    /// List all product templates
    ///
    /// Returns a list of all available product templates.
    /// Products are compile-time constants from voltage-model crate.
    pub async fn list_products(&self) -> Result<Vec<Product>> {
        Ok(self.ctx.product_loader.get_all_products())
    }

    /// Get product template details
    ///
    /// Returns detailed information about a specific product template.
    /// Products are compile-time constants from voltage-model crate.
    pub async fn get_product(&self, product_name: &str) -> Result<Product> {
        self.ctx
            .product_loader
            .get_product(product_name)
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    LibApiError::not_found(format!("Product '{}' not found", product_name)).into()
                } else {
                    e
                }
            })
    }

    /// Get instance measurement data
    ///
    /// Retrieves all measurement point values for an instance from Redis.
    /// Public API for RTDB access.
    #[allow(dead_code)]
    pub async fn get_instance_data(&self, name: &str) -> Result<Vec<(String, String)>> {
        // First, get instance ID from database
        let instance: Option<(i64,)> =
            sqlx::query_as("SELECT instance_id FROM instances WHERE name = ?")
                .bind(name)
                .fetch_optional(&self.ctx.sqlite_pool)
                .await?;

        let (instance_id,) = instance
            .ok_or_else(|| LibApiError::not_found(format!("Instance '{}' not found", name)))?;

        // Get data from Redis
        let key = format!("inst:{}:M", instance_id);
        let points = self.ctx.rtdb.hash_get_all(&key).await?;

        let result: Vec<(String, String)> = points
            .into_iter()
            .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
            .collect();

        Ok(result)
    }

    /// Write action point value
    ///
    /// Writes a value to an action point, which will trigger routing to the
    /// corresponding channel control/adjustment point.
    /// Public API for RTDB access.
    #[allow(dead_code)]
    pub async fn write_action_point(
        &self,
        instance_name: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        // Get instance ID
        let instance: Option<(i64,)> =
            sqlx::query_as("SELECT instance_id FROM instances WHERE name = ?")
                .bind(instance_name)
                .fetch_optional(&self.ctx.sqlite_pool)
                .await?;

        let (instance_id,) = instance.ok_or_else(|| {
            LibApiError::not_found(format!("Instance '{}' not found", instance_name))
        })?;

        // Write to instance action hash
        // Note: Actual routing is handled by application layer (voltage_routing::set_action_point)
        let key = format!("inst:{}:A", instance_id);
        self.ctx
            .rtdb
            .hash_set(&key, &point_id.to_string(), value.to_string().into())
            .await?;

        // TODO: Trigger routing via application layer
        // For now, just write to hash (routing should be handled by modsrv)

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // Note: Tests would require a full service context setup
    // For now, we'll skip unit tests and rely on integration tests
}
