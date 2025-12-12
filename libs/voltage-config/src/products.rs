//! Product type definitions for VoltageEMS
//!
//! This module re-exports product definitions from `voltage-model`.
//! **For new code, prefer importing directly from `voltage_model::products`.**
//!
//! # Exported Types
//!
//! - `ProductType`: All supported product types (Station, Gateway, PCS, etc.)
//! - `PointDef`: Point definition (id, name, description, unit)
//! - `ProductDef`: Product definition (type, properties, measurements, actions)
//!
//! # Product Definition Constants
//!
//! - `STATION_DEF`, `GATEWAY_DEF`, `PV_INVERTER_DEF`, etc.

// Re-export everything from voltage-model::products
pub use voltage_model::products::*;
