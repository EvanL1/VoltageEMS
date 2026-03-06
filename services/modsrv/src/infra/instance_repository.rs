//! Instance Repository
//!
//! Trait and SQLite implementation for instance persistence.
//! Extracts SQL operations from InstanceManager for testability.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::config::{InstanceCore, TopologyNode};
use crate::product_loader::Instance;

/// Row type returned by SQLite instance queries
type InstanceRow = (u32, String, String, Option<u32>, Option<String>, String);

/// Parse properties JSON string into HashMap
fn parse_properties_json(
    json: Option<String>,
    instance_id: u32,
) -> Result<HashMap<String, serde_json::Value>> {
    match json {
        Some(s) => serde_json::from_str(&s).map_err(|e| {
            anyhow!(
                "Invalid properties JSON for instance {}: {}",
                instance_id,
                e
            )
        }),
        None => Ok(HashMap::new()),
    }
}

/// Build Instance from database row tuple
fn build_instance_from_row(row: InstanceRow) -> Result<Instance> {
    let (instance_id, instance_name, product_name, parent_id, properties_json, _created_at) = row;
    let properties = parse_properties_json(properties_json, instance_id)?;
    Ok(Instance {
        core: InstanceCore {
            instance_id,
            instance_name,
            product_name,
            parent_id,
            properties,
        },
        measurement_mappings: None,
        action_mappings: None,
        created_at: None,
    })
}

/// Escape SQL LIKE metacharacters
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Instance persistence operations
#[async_trait]
pub trait InstanceRepository: Send + Sync {
    /// Insert a new instance, returns the assigned instance_id
    async fn insert(
        &self,
        instance_id: Option<u32>,
        instance_name: &str,
        product_name: &str,
        parent_id: Option<u32>,
        properties_json: &str,
    ) -> Result<u32>;

    /// Delete an instance by ID, returns true if found
    async fn delete(&self, instance_id: u32) -> Result<bool>;

    /// Get instance by ID
    async fn get(&self, instance_id: u32) -> Result<Option<Instance>>;

    /// Get instance_name by instance_id
    async fn get_name(&self, instance_id: u32) -> Result<Option<String>>;

    /// Get instance_id by instance_name
    async fn get_id_by_name(&self, instance_name: &str) -> Result<Option<u16>>;

    /// Check if instance name exists (excluding a specific instance_id)
    async fn name_exists(&self, instance_name: &str, exclude_id: u32) -> Result<bool>;

    /// Rename an instance (updates instances + routing tables in a transaction)
    async fn rename(&self, instance_id: u32, new_name: &str) -> Result<()>;

    /// List instances with pagination
    async fn list_paginated(
        &self,
        product_name: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<(u32, Vec<Instance>)>;

    /// Search instances by name keyword
    async fn search(
        &self,
        keyword: &str,
        product_name: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<(u32, Vec<Instance>)>;

    /// Get direct children of an instance
    async fn get_children(&self, instance_id: u32) -> Result<Vec<Instance>>;

    /// Get all child instance_ids (BFS, leaf-first order)
    async fn collect_descendants(&self, instance_id: u32) -> Result<Vec<u32>>;

    /// Get full topology tree
    async fn get_topology_tree(&self) -> Result<Vec<TopologyNode>>;

    /// Get product_name for an instance
    async fn get_product_name(&self, instance_id: u32) -> Result<String>;

    /// Count all instances
    async fn count(&self) -> Result<i64>;

    /// List all instances (for startup sync)
    async fn list_all(&self) -> Result<Vec<Instance>>;

    /// List all instance (name, id) pairs (for name cache warmup)
    async fn list_name_id_pairs(&self) -> Result<Vec<(String, u16)>>;

    /// Get parent's product_name (for hierarchy validation)
    async fn get_parent_product_name(&self, parent_id: u32) -> Result<Option<String>>;
}

/// SQLite implementation of InstanceRepository
pub struct SqliteInstanceRepository {
    pool: SqlitePool,
}

impl SqliteInstanceRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[async_trait]
impl InstanceRepository for SqliteInstanceRepository {
    async fn insert(
        &self,
        instance_id: Option<u32>,
        instance_name: &str,
        product_name: &str,
        parent_id: Option<u32>,
        properties_json: &str,
    ) -> Result<u32> {
        let result = sqlx::query(
            r#"INSERT INTO instances (instance_id, instance_name, product_name, parent_id, properties)
               VALUES (?, ?, ?, ?, ?)"#,
        )
        .bind(instance_id.map(|id| id as i64))
        .bind(instance_name)
        .bind(product_name)
        .bind(parent_id.map(|id| id as i64))
        .bind(properties_json)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid() as u32)
    }

    async fn delete(&self, instance_id: u32) -> Result<bool> {
        let result = sqlx::query("DELETE FROM instances WHERE instance_id = ?")
            .bind(instance_id as i64)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn get(&self, instance_id: u32) -> Result<Option<Instance>> {
        let row = sqlx::query_as::<_, (String, String, Option<u32>, Option<String>, String)>(
            r#"SELECT instance_name, product_name, parent_id, properties, created_at
               FROM instances WHERE instance_id = ?"#,
        )
        .bind(instance_id as i64)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some((instance_name, product_name, parent_id, properties_json, _created_at)) => {
                let properties = parse_properties_json(properties_json, instance_id)?;
                Ok(Some(Instance {
                    core: InstanceCore {
                        instance_id,
                        instance_name,
                        product_name,
                        parent_id,
                        properties,
                    },
                    measurement_mappings: None,
                    action_mappings: None,
                    created_at: None,
                }))
            },
            None => Ok(None),
        }
    }

    async fn get_name(&self, instance_id: u32) -> Result<Option<String>> {
        let name = sqlx::query_scalar::<_, String>(
            "SELECT instance_name FROM instances WHERE instance_id = ?",
        )
        .bind(instance_id as i64)
        .fetch_optional(&self.pool)
        .await?;
        Ok(name)
    }

    async fn get_id_by_name(&self, instance_name: &str) -> Result<Option<u16>> {
        let id = sqlx::query_scalar::<_, u16>(
            "SELECT instance_id FROM instances WHERE instance_name = ?",
        )
        .bind(instance_name)
        .fetch_optional(&self.pool)
        .await?;
        Ok(id)
    }

    async fn name_exists(&self, instance_name: &str, exclude_id: u32) -> Result<bool> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM instances WHERE instance_name = ? AND instance_id != ?",
        )
        .bind(instance_name)
        .bind(exclude_id as i64)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    async fn rename(&self, instance_id: u32, new_name: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "UPDATE instances SET instance_name = ?, updated_at = CURRENT_TIMESTAMP WHERE instance_id = ?",
        )
        .bind(new_name)
        .bind(instance_id as i64)
        .execute(&mut *tx)
        .await?;

        // Update denormalized instance_name in routing tables
        sqlx::query("UPDATE measurement_routing SET instance_name = ? WHERE instance_id = ?")
            .bind(new_name)
            .bind(instance_id as i64)
            .execute(&mut *tx)
            .await?;

        sqlx::query("UPDATE action_routing SET instance_name = ? WHERE instance_id = ?")
            .bind(new_name)
            .bind(instance_id as i64)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn list_paginated(
        &self,
        product_name: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<(u32, Vec<Instance>)> {
        let offset = (page - 1) * page_size;

        let (total,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM instances WHERE (? IS NULL OR product_name = ?)")
                .bind(product_name)
                .bind(product_name)
                .fetch_one(&self.pool)
                .await?;

        let rows: Vec<InstanceRow> = sqlx::query_as(
            r#"SELECT instance_id, instance_name, product_name, parent_id, properties, created_at
               FROM instances
               WHERE (? IS NULL OR product_name = ?)
               ORDER BY instance_id ASC
               LIMIT ? OFFSET ?"#,
        )
        .bind(product_name)
        .bind(product_name)
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        let instances = rows
            .into_iter()
            .map(build_instance_from_row)
            .collect::<Result<Vec<_>>>()?;

        Ok((u32::try_from(total).unwrap_or(u32::MAX), instances))
    }

    async fn search(
        &self,
        keyword: &str,
        product_name: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<(u32, Vec<Instance>)> {
        let offset = (page - 1) * page_size;
        let like_pattern = format!("%{}%", escape_like(keyword));

        let (total,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM instances WHERE instance_name LIKE ? ESCAPE '\\' AND (? IS NULL OR product_name = ?)",
        )
        .bind(&like_pattern)
        .bind(product_name)
        .bind(product_name)
        .fetch_one(&self.pool)
        .await?;

        let rows: Vec<InstanceRow> = sqlx::query_as(
            r#"SELECT instance_id, instance_name, product_name, parent_id, properties, created_at
               FROM instances
               WHERE instance_name LIKE ? ESCAPE '\' AND (? IS NULL OR product_name = ?)
               ORDER BY instance_id ASC
               LIMIT ? OFFSET ?"#,
        )
        .bind(&like_pattern)
        .bind(product_name)
        .bind(product_name)
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        let instances = rows
            .into_iter()
            .map(build_instance_from_row)
            .collect::<Result<Vec<_>>>()?;

        Ok((u32::try_from(total).unwrap_or(u32::MAX), instances))
    }

    async fn get_children(&self, instance_id: u32) -> Result<Vec<Instance>> {
        let rows: Vec<InstanceRow> = sqlx::query_as(
            r#"SELECT instance_id, instance_name, product_name, parent_id, properties, created_at
               FROM instances WHERE parent_id = ? ORDER BY instance_id ASC"#,
        )
        .bind(instance_id as i64)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(build_instance_from_row)
            .collect::<Result<Vec<_>>>()
    }

    async fn collect_descendants(&self, instance_id: u32) -> Result<Vec<u32>> {
        let mut all = Vec::new();
        let mut queue = vec![instance_id];
        while let Some(parent) = queue.pop() {
            let children: Vec<(u32,)> =
                sqlx::query_as("SELECT instance_id FROM instances WHERE parent_id = ?")
                    .bind(parent as i64)
                    .fetch_all(&self.pool)
                    .await?;
            for (child_id,) in children {
                all.push(child_id);
                queue.push(child_id);
            }
        }
        all.reverse();
        Ok(all)
    }

    async fn get_topology_tree(&self) -> Result<Vec<TopologyNode>> {
        let rows: Vec<(u32, String, String, Option<u32>)> = sqlx::query_as(
            r#"SELECT instance_id, instance_name, product_name, parent_id
               FROM instances ORDER BY parent_id NULLS FIRST, instance_id ASC"#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(instance_id, instance_name, product_name, parent_id)| TopologyNode {
                    instance_id,
                    instance_name,
                    product_name,
                    parent_id,
                },
            )
            .collect())
    }

    async fn get_product_name(&self, instance_id: u32) -> Result<String> {
        sqlx::query_scalar::<_, String>("SELECT product_name FROM instances WHERE instance_id = ?")
            .bind(instance_id as i64)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))
    }

    async fn count(&self) -> Result<i64> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM instances")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    async fn list_all(&self) -> Result<Vec<Instance>> {
        let rows: Vec<InstanceRow> = sqlx::query_as(
            r#"SELECT instance_id, instance_name, product_name, parent_id, properties, created_at
               FROM instances ORDER BY instance_id ASC"#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(build_instance_from_row)
            .collect::<Result<Vec<_>>>()
    }

    async fn list_name_id_pairs(&self) -> Result<Vec<(String, u16)>> {
        let pairs = sqlx::query_as("SELECT instance_name, instance_id FROM instances")
            .fetch_all(&self.pool)
            .await?;
        Ok(pairs)
    }

    async fn get_parent_product_name(&self, parent_id: u32) -> Result<Option<String>> {
        let name = sqlx::query_scalar::<_, String>(
            "SELECT product_name FROM instances WHERE instance_id = ?",
        )
        .bind(parent_id as i64)
        .fetch_optional(&self.pool)
        .await?;
        Ok(name)
    }
}
