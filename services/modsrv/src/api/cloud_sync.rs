//! Cloud Sync API Handlers
//!
//! Endpoints for cloud-edge synchronization:
//! - POST /api/products/sync - Receive product library from cloud
//! - GET /api/instances/export - Export instance topology to cloud

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions
#![allow(clippy::type_complexity)]

use axum::{extract::State, response::Json};
use common::SuccessResponse;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn};

use crate::app_state::AppState;
use crate::config::{Product, SqlInsertableProduct};
use crate::error::ModSrvError;

/// Product library from cloud (cloud → edge sync)
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "swagger-ui", derive(utoipa::ToSchema))]
pub struct ProductLibrary {
    pub version: String,
    pub products: Vec<Product>,
}

/// Instance export item (edge → cloud sync)
#[derive(Debug, Serialize)]
pub struct InstanceExport {
    pub id: u32,
    pub name: String,
    pub product: String,
    pub parent_id: Option<u32>,
    pub properties: serde_json::Value,
}

/// Instance topology export response
#[derive(Debug, Serialize)]
pub struct InstanceTopology {
    pub version: String,
    pub instances: Vec<InstanceExport>,
}

/// Sync product library from cloud
///
/// Receives complete product library and performs full replacement within a transaction.
/// This is a destructive operation - all existing products and their points are deleted first.
///
/// @route POST /api/products/sync
/// @input Json(library): ProductLibrary - Complete product library from cloud
/// @output Json<SuccessResponse<serde_json::Value>> - Sync result
/// @status 200 - Success with product count
/// @status 400 - Invalid product data
/// @status 500 - Database error
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    post,
    path = "/api/products/sync",
    tag = "products",
    request_body = ProductLibrary,
    responses(
        (status = 200, description = "Products synced successfully",
            body = inline(Object),
            example = json!({
                "success": true,
                "data": {
                    "version": "1.0.0",
                    "products_synced": 5
                }
            })
        ),
        (status = 400, description = "Invalid product data"),
        (status = 500, description = "Database error")
    )
))]
pub async fn sync_products(
    State(state): State<Arc<AppState>>,
    Json(library): Json<ProductLibrary>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    let pool = match &state.sqlite_client {
        Some(client) => client.pool(),
        None => {
            return Err(ModSrvError::InternalError(
                "Database connection not available".to_string(),
            ))
        },
    };

    // Use transaction for atomic full replacement
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to start transaction: {}", e)))?;

    // Delete existing data (cascade will handle points)
    sqlx::query("DELETE FROM products")
        .execute(&mut *tx)
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to clear products: {}", e)))?;

    // Insert new products
    let mut product_count = 0;
    for product in &library.products {
        if product.product_name.is_empty() {
            warn!("Skipping product with empty name");
            continue;
        }

        // Insert product record
        sqlx::query("INSERT INTO products (product_name, parent_name) VALUES (?, ?)")
            .bind(&product.product_name)
            .bind(&product.parent_name)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                ModSrvError::InternalError(format!(
                    "Failed to insert product {}: {}",
                    product.product_name, e
                ))
            })?;

        // Insert measurement points
        for point in &product.measurements {
            point
                .insert_with(&mut *tx, &product.product_name)
                .await
                .map_err(|e| {
                    ModSrvError::InternalError(format!("Failed to insert measurement: {}", e))
                })?;
        }

        // Insert action points
        for point in &product.actions {
            point
                .insert_with(&mut *tx, &product.product_name)
                .await
                .map_err(|e| {
                    ModSrvError::InternalError(format!("Failed to insert action: {}", e))
                })?;
        }

        // Insert property templates
        for prop in &product.properties {
            prop.insert_with(&mut *tx, &product.product_name)
                .await
                .map_err(|e| {
                    ModSrvError::InternalError(format!("Failed to insert property: {}", e))
                })?;
        }

        product_count += 1;
    }

    // Update version metadata
    sqlx::query("DELETE FROM product_library_meta")
        .execute(&mut *tx)
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to clear meta: {}", e)))?;

    sqlx::query("INSERT INTO product_library_meta (version) VALUES (?)")
        .bind(&library.version)
        .execute(&mut *tx)
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to insert meta: {}", e)))?;

    // Commit transaction
    tx.commit()
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to commit: {}", e)))?;

    // Reload product cache
    if let Err(e) = state.product_loader.reload().await {
        warn!("Failed to reload product cache: {}", e);
    }

    info!(
        "Products synced: {} products, version {}",
        product_count, library.version
    );

    Ok(Json(SuccessResponse::new(json!({
        "version": library.version,
        "products_synced": product_count
    }))))
}

/// Export instance topology for cloud sync
///
/// Returns all instances with their topology (parent_id) and properties.
/// Used for edge → cloud synchronization.
///
/// @route GET /api/instances/export
/// @output `Json<SuccessResponse<InstanceTopology>>` - Instance topology
/// @status 200 - Success with instances
/// @status 500 - Database error
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    get,
    path = "/api/instances/export",
    tag = "products",
    responses(
        (status = 200, description = "Instance topology exported",
            body = inline(Object),
            example = json!({
                "success": true,
                "data": {
                    "version": "1.0.0",
                    "instances": [
                        {"id": 1, "name": "pv_001", "product": "pv_inverter", "parent_id": null, "properties": {}}
                    ]
                }
            })
        ),
        (status = 500, description = "Database error")
    )
))]
pub async fn export_instances(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse<InstanceTopology>>, ModSrvError> {
    let pool = match &state.sqlite_client {
        Some(client) => client.pool(),
        None => {
            return Err(ModSrvError::InternalError(
                "Database connection not available".to_string(),
            ))
        },
    };

    // Query all instances with parent_id
    let rows: Vec<(u32, String, String, Option<u32>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT instance_id, instance_name, product_name, parent_id, properties
        FROM instances
        ORDER BY instance_id
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ModSrvError::InternalError(format!("Failed to query instances: {}", e)))?;

    let instances: Vec<InstanceExport> = rows
        .into_iter()
        .map(|(id, name, product, parent_id, props_json)| {
            let properties = props_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(json!({}));

            InstanceExport {
                id,
                name,
                product,
                parent_id,
                properties,
            }
        })
        .collect();

    // Get current product library version
    let version: String = sqlx::query_scalar("SELECT version FROM product_library_meta LIMIT 1")
        .fetch_optional(pool)
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to get version: {}", e)))?
        .unwrap_or_else(|| "1.0.0".to_string());

    Ok(Json(SuccessResponse::new(InstanceTopology {
        version,
        instances,
    })))
}
