//! Data Transfer Objects for Model Service API
//!
//! This module contains all request and response structures used by the API endpoints.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;
use voltage_config::FourRemote;

// Import Core types for zero-duplication architecture
use voltage_config::modsrv::{Instance, InstanceCore};

// === Query Parameters ===

/// Query parameter for filtering by data type
#[derive(Deserialize, ToSchema)]
pub struct DataTypeQuery {
    #[serde(rename = "type")]
    pub data_type: Option<String>, // 'measurement', 'action', or null for both
}

// === Parameter Management ===

/// Request to update instance parameter routing
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct RoutingUpdate {
    pub routings: HashMap<String, String>,
    #[serde(default)]
    pub routing_type: RoutingType,
}

/// Type of routing being updated
#[derive(Debug, Clone, Deserialize, Serialize, Default, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum RoutingType {
    #[default]
    Measurement,
    Action,
}

// === Association Management ===

/// Model association between two instances
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ModelAssociation {
    pub source_id: String,
    pub target_id: String,
    pub association_type: String,
    pub metadata: Option<serde_json::Value>,
}

/// Request to add a new association
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct AddAssociationRequest {
    pub target_id: String,
    pub association_type: String,
    pub metadata: Option<serde_json::Value>,
}

// === Routing Management ===

/// Request to create or update a channel-to-instance point routing
///
/// `channel_id`, `four_remote`, and `channel_point_id` form a unit to identify a channel point.
/// All three must be present for a valid routing, or all null to unbind the routing.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct RoutingRequest {
    #[schema(example = 1)]
    pub channel_id: Option<i32>,
    #[schema(value_type = Option<String>, example = "T")]
    pub four_remote: Option<FourRemote>,
    #[schema(example = 101)]
    pub channel_point_id: Option<u32>,
    #[schema(example = 101)]
    pub point_id: u32, // Either measurement_id or action_id based on channel_type
}

/// Request to create or update routing for a single point
///
/// `channel_id`, `four_remote`, and `channel_point_id` can all be null to unbind the routing
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct SinglePointRoutingRequest {
    #[schema(example = 1)]
    pub channel_id: Option<i32>,
    #[schema(value_type = Option<String>, example = "T")]
    pub four_remote: Option<FourRemote>,
    #[schema(example = 101)]
    pub channel_point_id: Option<u32>,
    #[serde(default = "default_enabled")]
    #[schema(example = true)]
    pub enabled: bool,
}

/// Request to toggle routing enabled state for a single point
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ToggleRoutingRequest {
    #[schema(example = true)]
    pub enabled: bool,
}

/// Default value for enabled field (true)
fn default_enabled() -> bool {
    true
}

// === Instance Management ===

/// Request to create a new instance from a product template
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateInstanceDto {
    #[schema(example = 1)]
    pub instance_id: Option<u16>, // Optional - auto-generated if not provided
    #[schema(example = "pv_inverter_01")]
    pub instance_name: String, // Instance name for Redis keys
    #[schema(example = "pv_inverter")]
    pub product_name: String,
    #[schema(value_type = Object, example = json!({"rated_power": 5000.0, "manufacturer": "Huawei"}))]
    pub properties: Option<HashMap<String, serde_json::Value>>,
}

/// Request to update an existing instance
///
/// Supports updating instance_name and/or properties.
/// At least one field must be provided.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct UpdateInstanceDto {
    /// New instance name (optional, must be unique if provided)
    #[schema(example = "pv_inverter_renamed")]
    pub instance_name: Option<String>,

    /// Updated properties (optional)
    #[schema(value_type = Option<Object>, example = json!({"rated_power": 5000.0, "manufacturer": "Huawei", "model": "SUN2000-5KTL-L1"}))]
    pub properties: Option<HashMap<String, serde_json::Value>>,
}

/// Request to execute an action on an instance
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ActionRequest {
    /// Point ID - can be numeric (e.g., "1") or semantic (e.g., "power_setpoint")
    /// Also accepts "id" and "action_id" for backward compatibility
    #[serde(alias = "id", alias = "action_id")]
    #[schema(example = "1")]
    pub point_id: String,
    #[schema(example = 4500.0)]
    pub value: f64,
}

// === Calculation Requests ===

/// Request to execute multiple calculations in batch
#[derive(Deserialize, ToSchema)]
pub struct BatchExecuteRequest {
    pub calculation_ids: Vec<String>,
}

/// Request to evaluate a mathematical expression
#[derive(Deserialize, ToSchema)]
pub struct ExpressionRequest {
    pub formula: String,
    pub variables: HashMap<String, f64>,
}

/// Request to perform aggregation operations
#[derive(Deserialize, ToSchema)]
pub struct AggregationRequest {
    pub operation: String,
    pub values: Vec<f64>,
}

/// Request to calculate energy metrics
#[derive(Deserialize, ToSchema)]
pub struct EnergyRequest {
    pub calculation_type: String,
    pub inputs: HashMap<String, f64>,
}

/// Request for time series operations
#[derive(Deserialize, ToSchema)]
pub struct TimeSeriesRequest {
    pub operation: String,
    pub data: Vec<f64>,
    pub window_size: Option<usize>,
}

// === Instance Result Responses ===

/// Instance operation result (create/update/delete)
/// Uses InstanceCore to eliminate field duplication
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InstanceResult {
    /// Core instance fields (instance_id, instance_name, product_name, properties)
    #[serde(flatten)]
    #[schema(value_type = Object)]
    pub core: InstanceCore,

    /// Runtime status
    #[schema(example = "active")]
    pub runtime_status: String,

    /// Operation message
    #[schema(example = "Instance created successfully")]
    pub message: Option<String>,
}

impl From<(&Instance, String, Option<String>)> for InstanceResult {
    fn from((instance, runtime_status, message): (&Instance, String, Option<String>)) -> Self {
        Self {
            core: instance.core.clone(),
            runtime_status,
            message,
        }
    }
}

/// Instance detail response (complete information)
/// Uses Instance to eliminate field duplication
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InstanceDetail {
    /// Complete instance configuration
    #[serde(flatten)]
    #[schema(value_type = Object)]
    pub instance: Instance,

    /// Runtime status
    #[schema(example = "active")]
    pub runtime_status: String,

    /// Point statistics (measurement_count, action_count, etc.)
    pub point_statistics: HashMap<String, usize>,
}

// === Runtime Point Structures (Product Point + Instance Routing) ===

/// Point routing configuration (instance-specific attribute)
///
/// This structure represents the routing configuration for an instance point.
/// It defines how the point is mapped to a channel point.
/// `channel_id`, `channel_type`, and `channel_point_id` form a unit - all null means unbound.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PointRouting {
    /// Channel ID (null if routing is unbound)
    #[schema(example = 1)]
    pub channel_id: Option<i32>,

    /// Channel type (four-remote type, null if routing is unbound)
    #[schema(example = "T")]
    pub channel_type: Option<String>,

    /// Channel point ID (null if routing is unbound)
    #[schema(example = 101)]
    pub channel_point_id: Option<u32>,

    /// Whether routing is enabled
    #[schema(example = true)]
    pub enabled: bool,

    /// Channel name (for display purposes)
    #[schema(example = "PV Inverter #1")]
    pub channel_name: Option<String>,

    /// Channel point name (signal_name from the point table)
    #[schema(example = "DC_Voltage")]
    pub channel_point_name: Option<String>,
}

/// Runtime measurement point (Product template + Instance routing)
///
/// This is the runtime view of a measurement point, combining:
/// - Product template definition (measurement_id, name, unit, description)
/// - Instance-specific routing configuration (if configured)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InstanceMeasurementPoint {
    /// Measurement ID
    #[schema(example = 101)]
    pub measurement_id: u32,

    /// Point name
    #[schema(example = "DC Voltage")]
    pub name: String,

    /// Unit of measurement
    #[schema(example = "V")]
    pub unit: Option<String>,

    /// Point description
    #[schema(example = "Direct current voltage")]
    pub description: Option<String>,

    /// Routing configuration (None if not configured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing: Option<PointRouting>,
}

/// Runtime action point (Product template + Instance routing)
///
/// This is the runtime view of an action point, combining:
/// - Product template definition (action_id, name, unit, description)
/// - Instance-specific routing configuration (if configured)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InstanceActionPoint {
    /// Action ID
    #[schema(example = 201)]
    pub action_id: u32,

    /// Action name
    #[schema(example = "Power Setpoint")]
    pub name: String,

    /// Unit for adjustment actions
    #[schema(example = "W")]
    pub unit: Option<String>,

    /// Point description
    #[schema(example = "Active power setpoint")]
    pub description: Option<String>,

    /// Routing configuration (None if not configured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing: Option<PointRouting>,
}

/// Response for GET /api/instances/{name}/points
///
/// Returns all measurement and action points for an instance,
/// with their routing configurations embedded as point attributes.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InstancePointsResponse {
    /// Instance name
    #[schema(example = "pv_inverter_01")]
    pub instance_name: String,

    /// Measurement points with routing
    pub measurements: Vec<InstanceMeasurementPoint>,

    /// Action points with routing
    pub actions: Vec<InstanceActionPoint>,
}
