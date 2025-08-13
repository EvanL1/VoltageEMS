mod ascii_formatter;
mod json_formatter;

use crate::error::Result;

/// Data formatter trait that can be sent across threads
/// Used to convert data from Redis into different string formats
pub trait DataFormatter: Send + Sync {
    /// Format data string into desired format
    fn format(&self, data: &str) -> Result<String>;
}

pub use ascii_formatter::AsciiFormatter;
pub use json_formatter::JsonFormatter;

// FormatType and factory functions removed as they're not used
// Formatters are created directly where needed
