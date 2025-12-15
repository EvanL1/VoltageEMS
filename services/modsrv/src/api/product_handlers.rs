//! Product Management API Handlers
//!
//! Provides endpoints for querying and creating product templates and definitions.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use crate::config::{Product, SqlInsertableProduct};
use axum::{
    extract::{Path, State},
    response::Json,
};
use common::SuccessResponse;
use serde_json::json;
use std::sync::Arc;

use crate::app_state::AppState;
use crate::error::ModSrvError;

/// List all available product templates (lightweight)
///
/// Returns a lightweight list containing only product names and parent relationships.
/// This endpoint is optimized for frontend dropdown lists and product selection interfaces.
/// For detailed product information including measurements/actions/properties, use GET /api/products/{product_name}/points.
///
/// @route GET /api/products
/// @output Json<SuccessResponse<serde_json::Value>> - Lightweight list with {count, products}
/// @status 200 - Success with array of {product_name, parent_name}
/// @status 500 - Database error
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    get,
    path = "/api/products",
    tag = "products",
    responses(
        (status = 200, description = "Lightweight product list retrieved successfully",
            body = inline(Object),
            example = json!({
                "success": true,
                "data": {
                    "count": 3,
                    "products": [
                        {"product_name": "pv_inverter", "parent_name": null},
                        {"product_name": "battery_pack", "parent_name": "pv_inverter"},
                        {"product_name": "dc_converter", "parent_name": null}
                    ]
                }
            })
        ),
        (status = 500, description = "Database error")
    )
))]
pub async fn list_products(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match state.product_loader.get_all_product_names().await {
        Ok(product_names) => {
            let products: Vec<serde_json::Value> = product_names
                .into_iter()
                .map(|(product_name, parent_name)| {
                    json!({
                        "product_name": product_name,
                        "parent_name": parent_name
                    })
                })
                .collect();

            Ok(Json(SuccessResponse::new(json!({
                "count": products.len(),
                "products": products
            }))))
        },
        Err(e) => Err(ModSrvError::InternalError(format!(
            "Failed to get products: {}",
            e
        ))),
    }
}

/// Get product definition with nested structure
///
/// Returns detailed product information including all measurement,
/// action, and property points.
///
/// @route GET /api/products/{product_name}/points
/// @input Path(product_name): String - Product identifier
/// @output Json<SuccessResponse<serde_json::Value>> - Product with measurement/action/property points
/// @status 200 - Success with {product}
/// @status 404 - Product not found
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    get,
    path = "/api/products/{product_name}/points",
    tag = "products",
    params(
        ("product_name" = String, Path, description = "Product identifier")
    ),
    responses(
        (status = 200, description = "Product details with all points retrieved successfully",
            body = inline(Object),
            example = json!({
                "success": true,
                "data": {
                    "product": {
                        "product_name": "pv_inverter",
                        "parent_name": null,
                        "measurements": [
                            {"measurement_id": 1, "name": "DC_Voltage", "unit": "V", "description": "DC bus voltage"}
                        ],
                        "actions": [
                            {"action_id": 1, "name": "Start", "unit": null, "description": "Start inverter"}
                        ],
                        "properties": []
                    }
                }
            })
        ),
        (status = 404, description = "Product not found"),
        (status = 500, description = "Database error")
    )
))]
pub async fn get_product_points(
    State(state): State<Arc<AppState>>,
    Path(product_name): Path<String>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match state.product_loader.get_product(&product_name).await {
        Ok(product) => Ok(Json(SuccessResponse::new(json!({
            "product": product
        })))),
        Err(e) => {
            // Check if it's a "not found" error
            if e.to_string().contains("not found") || e.to_string().contains("does not exist") {
                Err(ModSrvError::InternalError(format!(
                    "Not found: Product '{}' not found",
                    product_name
                )))
            } else {
                Err(ModSrvError::InternalError(format!(
                    "Failed to get product {}: {}",
                    product_name, e
                )))
            }
        },
    }
}

/// Create a new product with measurements, actions, and properties
///
/// Creates a product template in the database with all associated point definitions.
/// This operation is atomic - either all inserts succeed or none do.
///
/// @route POST /api/products
/// @input Json(product): Product - Complete product definition
/// @output Json<SuccessResponse<serde_json::Value>> - Created product confirmation
/// @status 200 - Success with {product_name}
/// @status 400 - Invalid product data
/// @status 500 - Database error
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    post,
    path = "/api/products",
    tag = "products",
    request_body = Product,
    responses(
        (status = 200, description = "Product created successfully",
            body = inline(Object),
            example = json!({
                "success": true,
                "data": {
                    "product_name": "new_product",
                    "measurements_count": 5,
                    "actions_count": 3,
                    "properties_count": 2
                }
            })
        ),
        (status = 400, description = "Invalid product data or product already exists"),
        (status = 500, description = "Database error")
    )
))]
pub async fn create_product(
    State(state): State<Arc<AppState>>,
    Json(product): Json<Product>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Validate product name is not empty
    if product.product_name.is_empty() {
        return Err(ModSrvError::InvalidData(
            "Product name cannot be empty".to_string(),
        ));
    }

    // Get database pool from state
    let pool = match &state.sqlite_client {
        Some(client) => client.pool(),
        None => {
            return Err(ModSrvError::InternalError(
                "Database connection not available".to_string(),
            ))
        },
    };

    // Use transaction to ensure atomicity
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to start transaction: {}", e)))?;

    // Insert product
    sqlx::query("INSERT INTO products (product_name, parent_name) VALUES (?, ?)")
        .bind(&product.product_name)
        .bind(&product.parent_name)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            if e.to_string().contains("UNIQUE constraint failed") {
                ModSrvError::InvalidData(format!(
                    "Product '{}' already exists",
                    product.product_name
                ))
            } else {
                ModSrvError::InternalError(format!("Failed to insert product: {}", e))
            }
        })?;

    // Insert measurement points
    for point in &product.measurements {
        point
            .insert_with(&mut *tx, &product.product_name)
            .await
            .map_err(|e| {
                ModSrvError::InternalError(format!("Failed to insert measurement point: {}", e))
            })?;
    }

    // Insert action points
    for point in &product.actions {
        point
            .insert_with(&mut *tx, &product.product_name)
            .await
            .map_err(|e| {
                ModSrvError::InternalError(format!("Failed to insert action point: {}", e))
            })?;
    }

    // Insert property templates
    for property in &product.properties {
        property
            .insert_with(&mut *tx, &product.product_name)
            .await
            .map_err(|e| {
                ModSrvError::InternalError(format!("Failed to insert property template: {}", e))
            })?;
    }

    // Commit transaction
    tx.commit()
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to commit transaction: {}", e)))?;

    Ok(Json(SuccessResponse::new(json!({
        "product_name": product.product_name,
        "measurements_count": product.measurements.len(),
        "actions_count": product.actions.len(),
        "properties_count": product.properties.len()
    }))))
}
