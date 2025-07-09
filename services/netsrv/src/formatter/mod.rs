mod ascii_formatter;
mod json_formatter;

use crate::error::Result;
use serde_json::Value;

/// Data formatter trait that can be sent across threads
/// Used to convert JSON data from Redis into different string formats
pub trait DataFormatter: Send + Sync {
    /// Format a JSON value to string
    fn format(&self, data: &Value) -> Result<String>;
}

pub use ascii_formatter::AsciiFormatter;
pub use json_formatter::JsonFormatter;

/// Format types supported by the system
#[derive(Debug, Clone, PartialEq)]
pub enum FormatType {
    Json,
    Ascii,
}

/// Factory function to create formatters
pub fn create_formatter(format_type: &FormatType) -> Box<dyn DataFormatter> {
    match format_type {
        FormatType::Json => Box::new(JsonFormatter::new()),
        FormatType::Ascii => Box::new(AsciiFormatter::new()),
    }
}

/// Create default JSON formatter
pub fn default_formatter() -> Box<dyn DataFormatter> {
    Box::new(JsonFormatter::new())
}
