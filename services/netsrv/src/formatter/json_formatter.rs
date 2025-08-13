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
    fn format(&self, data: &str) -> Result<String> {
        // For JSON formatter, we can either pass through the string directly
        // or parse and re-format it to ensure valid JSON
        match serde_json::from_str::<Value>(data) {
            Ok(parsed) => serde_json::to_string(&parsed)
                .map_err(|e| NetSrvError::Format(format!("JSON formatting error: {}", e))),
            Err(_) => {
                // If it's not valid JSON, wrap it as a string value
                serde_json::to_string(data)
                    .map_err(|e| NetSrvError::Format(format!("JSON formatting error: {}", e)))
            },
        }
    }
}
