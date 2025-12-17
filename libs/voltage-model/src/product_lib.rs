//! Built-in Product Library
//!
//! This module provides the built-in product templates that are embedded
//! at compile time. Products define the structure for device instances
//! including their measurements, actions, and properties.

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

/// Point definition for measurements, actions, and properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointDef {
    /// Point ID (unique within product)
    pub id: u32,
    /// Point name
    pub name: String,
    /// Unit of measurement (empty string if none)
    #[serde(default)]
    pub unit: String,
    /// Value type (number, string, etc.)
    #[serde(rename = "type", default)]
    pub value_type: String,
}

/// Built-in product definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltinProduct {
    /// Product name (unique identifier)
    pub name: String,
    /// Parent product name for hierarchy (e.g., Battery -> ESS -> Station)
    #[serde(rename = "pName")]
    pub parent_name: Option<String>,
    /// Property definitions (P)
    #[serde(rename = "P", default)]
    pub properties: Vec<PointDef>,
    /// Measurement point definitions (M)
    #[serde(rename = "M", default)]
    pub measurements: Vec<PointDef>,
    /// Action point definitions (A)
    #[serde(rename = "A", default)]
    pub actions: Vec<PointDef>,
}

// Embed all product JSON files at compile time
// Note: products/ is a git submodule (voltage-product-lib)
static BUILTIN_PRODUCTS: Lazy<Vec<BuiltinProduct>> = Lazy::new(|| {
    let jsons: &[&str] = &[
        include_str!("products/Station.json"),
        include_str!("products/ESS.json"),
        include_str!("products/Generator.json"),
        include_str!("products/Battery.json"),
        include_str!("products/PCS.json"),
        include_str!("products/Diesel.json"),
        include_str!("products/PV_DCDC.json"),
        include_str!("products/Env.json"),
        include_str!("products/Load.json"),
    ];

    jsons
        .iter()
        .filter_map(|s| serde_json::from_str(s).ok())
        .collect()
});

/// Get all built-in products
pub fn get_builtin_products() -> &'static [BuiltinProduct] {
    &BUILTIN_PRODUCTS
}

/// Get a built-in product by name
pub fn get_builtin_product(name: &str) -> Option<&'static BuiltinProduct> {
    BUILTIN_PRODUCTS.iter().find(|p| p.name == name)
}

/// Get all product names
pub fn get_product_names() -> Vec<&'static str> {
    BUILTIN_PRODUCTS.iter().map(|p| p.name.as_str()).collect()
}

/// Check if a product exists in the built-in library
pub fn product_exists(name: &str) -> bool {
    BUILTIN_PRODUCTS.iter().any(|p| p.name == name)
}

/// Get child products of a given parent
pub fn get_child_products(parent_name: &str) -> Vec<&'static BuiltinProduct> {
    BUILTIN_PRODUCTS
        .iter()
        .filter(|p| p.parent_name.as_deref() == Some(parent_name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_products_loaded() {
        let products = get_builtin_products();
        assert!(!products.is_empty(), "Should have built-in products");
        assert_eq!(products.len(), 9, "Should have 9 products");
    }

    #[test]
    fn test_get_product_by_name() {
        let battery = get_builtin_product("Battery").expect("Battery should exist");
        assert_eq!(battery.name, "Battery");
        assert_eq!(battery.parent_name.as_deref(), Some("ESS"));
        assert!(!battery.measurements.is_empty());
    }

    #[test]
    fn test_product_hierarchy() {
        // Station is root
        let station = get_builtin_product("Station").expect("Station should exist");
        assert!(station.parent_name.is_none());

        // ESS -> Station
        let ess = get_builtin_product("ESS").expect("ESS should exist");
        assert_eq!(ess.parent_name.as_deref(), Some("Station"));

        // Battery -> ESS
        let battery = get_builtin_product("Battery").expect("Battery should exist");
        assert_eq!(battery.parent_name.as_deref(), Some("ESS"));
    }

    #[test]
    fn test_get_child_products() {
        let station_children = get_child_products("Station");
        let names: Vec<_> = station_children.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"ESS"));
        assert!(names.contains(&"Generator"));
        assert!(names.contains(&"Env"));
        assert!(names.contains(&"Load"));
    }

    #[test]
    fn test_product_exists() {
        assert!(product_exists("Battery"));
        assert!(product_exists("PCS"));
        assert!(!product_exists("NonExistent"));
    }
}
