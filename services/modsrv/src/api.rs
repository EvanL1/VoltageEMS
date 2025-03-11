//! API Module Aggregation
//!
//! This module organizes API handlers by functional domain.
//! All handlers are grouped under the api/ directory for clear separation.

// Handler modules organized by functional domain
pub mod calculation_management_handlers;
pub mod computation_handlers;
pub mod global_routing_handlers; // New unified database routing APIs
pub mod health_handlers;
pub mod instance_action_handlers;
pub mod instance_management_handlers;
pub mod instance_query_handlers;
pub mod product_handlers;

// Routing handlers (refactored to work with unified database)
pub mod routing_management_handlers;
pub mod routing_query_handlers;
pub mod single_point_handlers;

// Re-export from root modules
pub use crate::dto;
pub use crate::routes;
