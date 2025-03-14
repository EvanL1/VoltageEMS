mod json_formatter;
mod ascii_formatter;

use crate::config::network_config::FormatType;
use crate::error::Result;
use serde_json::Value;

pub use json_formatter::JsonFormatter;
pub use ascii_formatter::AsciiFormatter;

pub trait DataFormatter {
    fn format(&self, data: &Value) -> Result<String>;
}

pub fn create_formatter(format_type: &FormatType) -> Box<dyn DataFormatter> {
    match format_type {
        FormatType::Json => Box::new(JsonFormatter::new()),
        FormatType::Ascii => Box::new(AsciiFormatter::new()),
    }
} 