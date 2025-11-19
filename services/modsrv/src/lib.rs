//! ModSrv library exports for testing

pub mod api;
pub mod app_state;
pub mod bootstrap;
pub mod calculation_engine;
pub mod cleanup_provider;
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
