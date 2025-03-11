use crate::error::{NetSrvError, Result};
use crate::formatter::DataFormatter;
use serde_json::Value;

pub struct JsonFormatter;

impl JsonFormatter {
    pub fn new() -> Self {
        JsonFormatter
    }
}

impl DataFormatter for JsonFormatter {
    fn format(&self, data: &Value) -> Result<String> {
        serde_json::to_string(data).map_err(|e| NetSrvError::FormatError(format!("JSON formatting error: {}", e)))
    }
} 