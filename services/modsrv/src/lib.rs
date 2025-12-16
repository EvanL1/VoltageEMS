//! ModSrv library exports for testing

pub mod config;

pub mod api {
    //! API Module Aggregation
    //!
    //! Organizes API handlers by functional domain under the `api/` directory.
    //!
    //! Handler groups:
    //! - routing (management + query)
    //! - instance (management + query + action)
    //! - product
    //! - health
    //! - single point APIs
    //! - admin (log level management)
    //! - cloud sync (cloud-edge synchronization)
    pub mod admin_handlers;
    pub mod cloud_sync;
    pub mod global_routing_handlers;
    pub mod health_handlers;
    pub mod instance_management_handlers;
    pub mod instance_query_handlers;
    pub mod product_handlers;
    pub mod routing_management_handlers;
    pub mod routing_query_handlers;
    pub mod single_point_handlers;

    // Re-export dto/routes for convenience
    pub use crate::routes;
}
// Map dto module to api/dto.rs while keeping crate::dto path stable
pub mod app_state;
pub mod bootstrap;
pub mod cleanup_provider;
#[path = "api/dto.rs"]
pub mod dto;
pub mod error;
pub mod instance_manager;
// Extension impl blocks for InstanceManager (split for maintainability)
mod instance_data;
mod instance_redis_sync;
mod instance_routing;
pub mod product_loader;
pub mod redis_state;
pub mod reload;
pub mod routes;
pub mod routing_loader;

// Rule Engine - local routes module
pub mod rule_routes;

// Re-export Rule Engine types from voltage-rules library
pub use voltage_rules::{
    delete_rule, extract_rule_flow, get_rule, get_rule_for_execution, list_rules, load_all_rules,
    load_enabled_rules, set_rule_enabled, upsert_rule, ActionResult, Result as RuleResult,
    RuleError, RuleExecutionResult, RuleExecutor, RuleScheduler, SchedulerStatus, TriggerConfig,
    DEFAULT_TICK_MS,
};

// Re-export routing types from shared library
pub use voltage_routing::{set_action_point, ActionRouteOutcome, RouteContext};

// Re-export commonly used types
pub use error::{ModSrvError, Result};
pub use instance_manager::InstanceManager;
pub use product_loader::{
    ActionPoint, CreateInstanceRequest, Instance, MeasurementPoint, PointRole, Product,
    ProductHierarchy, ProductLoader, PropertyTemplate,
};
pub use routing_loader::{
    ActionRouting, ActionRoutingRow, MeasurementRouting, MeasurementRoutingRow, RoutingLoader,
};
