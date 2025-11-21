//! ModSrv library exports for testing

pub mod api {
    //! API Module Aggregation
    //!
    //! Organizes API handlers by functional domain under the `api/` directory.
    //!
    //! Handler groups:
    //! - calculation management / computation
    //! - routing (management + query)
    //! - instance (management + query + action)
    //! - product
    //! - health
    //! - single point APIs
    pub mod calculation_management_handlers;
    pub mod computation_handlers;
    pub mod global_routing_handlers; // unified database routing APIs
    pub mod health_handlers;
    pub mod instance_action_handlers;
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
pub mod calculation_engine;
pub mod cleanup_provider;
#[path = "api/dto.rs"]
pub mod dto;
pub mod error;
pub mod instance_logger;
pub mod instance_manager;
pub mod product_loader;
pub mod redis_state;
pub mod reload;
pub mod routes;
pub mod routing_loader;
#[cfg(feature = "virtual-points")]
pub mod time_series;
#[cfg(feature = "virtual-points")]
pub mod virtual_calc;

// Re-export routing types from shared library
pub use voltage_routing::{set_action_point, ActionRouteOutcome, RouteContext};

// Re-export commonly used types
pub use calculation_engine::CalculationEngine;
pub use error::{ModSrvError, Result};
pub use instance_manager::InstanceManager;
pub use product_loader::{
    ActionPoint, CreateInstanceRequest, Instance, MeasurementPoint, PointType, Product,
    ProductHierarchy, ProductLoader, PropertyTemplate,
};
pub use routing_loader::{
    ActionRouting, ActionRoutingRow, MeasurementRouting, MeasurementRoutingRow, RoutingLoader,
};
