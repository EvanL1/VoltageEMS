//! CSV loader for product definitions

use anyhow::Result;
use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

/// Load product definition from CSV files in products/{name}/ directory
#[allow(clippy::disallowed_methods)] // json! macro internally uses unwrap (safe for known valid JSON)
pub fn load_product_from_csv(product_name: &str) -> Result<Value> {
    let base_path = Path::new("products").join(product_name);

    if !base_path.exists() {
        return Err(anyhow::anyhow!(
            "Product directory not found: {}",
            base_path.display()
        ));
    }

    // Load measurements
    let measurements = load_csv_file(&base_path.join("measurements.csv"), "measurements")?;

    // Load actions
    let actions = load_csv_file(&base_path.join("actions.csv"), "actions")?;

    // Load properties
    let properties = load_csv_file(&base_path.join("properties.csv"), "properties")?;

    // Build product JSON
    let product = json!({
        "name": product_name,
        "measurements": measurements,
        "actions": actions,
        "properties": properties
    });

    Ok(product)
}

#[allow(clippy::disallowed_methods)] // json! macro internally uses unwrap (safe for known valid JSON)
fn load_csv_file(path: &Path, field_type: &str) -> Result<Vec<Value>> {
    if !path.exists() {
        // Return empty array if file doesn't exist
        return Ok(vec![]);
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut csv_reader = csv::Reader::from_reader(reader);

    let mut items = Vec::new();

    for result in csv_reader.records() {
        let record = result?;

        let item = match field_type {
            "measurements" | "actions" => {
                // Format: id,name,unit (for measurements) or id,name,description (for actions)
                if record.len() < 2 {
                    continue;
                }

                let mut obj = json!({
                    "id": record.get(0).unwrap_or("").parse::<u32>().unwrap_or(0),
                    "name": record.get(1).unwrap_or("")
                });

                if field_type == "measurements" && record.len() > 2 {
                    obj["unit"] = json!(record.get(2).unwrap_or(""));
                } else if field_type == "actions" && record.len() > 2 {
                    obj["description"] = json!(record.get(2).unwrap_or(""));
                }

                obj
            },
            "properties" => {
                // Format: id,name,unit,description
                if record.len() < 2 {
                    continue;
                }

                json!({
                    "id": record.get(0).unwrap_or("").parse::<u32>().unwrap_or(0),
                    "name": record.get(1).unwrap_or(""),
                    "unit": record.get(2).unwrap_or(""),
                    "description": record.get(3).unwrap_or("")
                })
            },
            _ => continue,
        };

        items.push(item);
    }

    Ok(items)
}

/// List available products in the products/ directory
pub fn list_available_products() -> Result<()> {
    let products_dir = Path::new("products");

    if !products_dir.exists() {
        println!("No products directory found");
        return Ok(());
    }

    println!("Available products:");

    for entry in fs::read_dir(products_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            if let Some(name) = path.file_name() {
                if let Some(name_str) = name.to_str() {
                    // Check if it has at least one CSV file
                    let has_csv = ["measurements.csv", "actions.csv", "properties.csv"]
                        .iter()
                        .any(|f| path.join(f).exists());

                    if has_csv {
                        println!("  - {}", name_str);
                    }
                }
            }
        }
    }

    Ok(())
}
