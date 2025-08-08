//! Line Protocol Builder for InfluxDB
//!
//! Provides a type-safe builder for creating InfluxDB Line Protocol strings.

use std::collections::BTreeMap;
use std::fmt::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

/// Field value type
#[derive(Debug, Clone)]
pub enum FieldValue {
    Float(f64),
    Integer(i64),
    UnsignedInteger(u64),
    String(String),
    Boolean(bool),
}

impl fmt::Display for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Control float precision to avoid excessive decimal places
            FieldValue::Float(v) => {
                if v.is_finite() {
                    // Use fixed precision for normal values
                    if v.abs() < 1e-6 || v.abs() > 1e6 {
                        write!(f, "{:e}", v) // Scientific notation for very large/small
                    } else {
                        write!(f, "{:.6}", v) // 6 decimal places for normal range
                    }
                } else {
                    write!(f, "{}", v) // Handle inf, -inf, NaN
                }
            },
            FieldValue::Integer(v) => write!(f, "{}i", v),
            FieldValue::UnsignedInteger(v) => write!(f, "{}u", v),
            FieldValue::String(v) => write!(f, "\"{}\"", escape_string_value(v)),
            FieldValue::Boolean(v) => write!(f, "{}", v),
        }
    }
}

/// Timestamp precision for InfluxDB
#[derive(Debug, Clone, Copy)]
pub enum TimestampPrecision {
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
}

impl TimestampPrecision {
    /// Convert SystemTime to timestamp with specified precision
    pub fn convert_system_time(&self, time: SystemTime) -> i64 {
        let duration = time
            .duration_since(UNIX_EPOCH)
            .expect("SystemTime before UNIX_EPOCH");

        match self {
            TimestampPrecision::Nanoseconds => {
                duration.as_secs() as i64 * 1_000_000_000 + duration.subsec_nanos() as i64
            },
            TimestampPrecision::Microseconds => {
                duration.as_secs() as i64 * 1_000_000 + duration.subsec_micros() as i64
            },
            TimestampPrecision::Milliseconds => {
                duration.as_secs() as i64 * 1_000 + duration.subsec_millis() as i64
            },
            TimestampPrecision::Seconds => duration.as_secs() as i64,
        }
    }
}

/// InfluxDB Line Protocol Builder
///
/// Example:
/// ```
/// use std::time::SystemTime;
///
/// let line = LineProtocolBuilder::new("cpu")
///     .tag("host", "server01")
///     .tag("region", "us-west")
///     .field("usage", 0.64)
///     .field("active", true)
///     .timestamp_system(SystemTime::now())
///     .build()
///     .expect("Failed to build line protocol");
/// ```
#[derive(Debug)]
pub struct LineProtocolBuilder {
    measurement: String,
    tags: BTreeMap<String, String>, // Use BTreeMap for stable ordering
    fields: Vec<(String, FieldValue)>,
    timestamp: Option<i64>,
    precision: TimestampPrecision,
    sort_fields: bool,
}

impl LineProtocolBuilder {
    /// Create new builder with measurement name
    pub fn new(measurement: impl Into<String>) -> Self {
        Self {
            measurement: measurement.into(),
            tags: BTreeMap::new(),
            fields: Vec::new(),
            timestamp: None,
            precision: TimestampPrecision::Nanoseconds,
            sort_fields: false,
        }
    }

    /// Set timestamp precision (default: Nanoseconds)
    #[must_use]
    pub fn with_precision(mut self, precision: TimestampPrecision) -> Self {
        self.precision = precision;
        self
    }

    /// Enable field sorting for stable output (default: false)
    #[must_use]
    pub fn with_sorted_fields(mut self, sort: bool) -> Self {
        self.sort_fields = sort;
        self
    }

    /// Add tag (automatically sorted by key)
    #[must_use]
    pub fn tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// Add multiple tags
    #[must_use]
    pub fn tags<K, V>(mut self, tags: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        for (key, value) in tags {
            self.tags.insert(key.into(), value.into());
        }
        self
    }

    /// Add field
    #[must_use]
    pub fn field(mut self, key: impl Into<String>, value: impl Into<FieldValue>) -> Self {
        self.fields.push((key.into(), value.into()));
        self
    }

    /// Add multiple fields
    #[must_use]
    pub fn fields<K>(mut self, fields: impl IntoIterator<Item = (K, FieldValue)>) -> Self
    where
        K: Into<String>,
    {
        for (key, value) in fields {
            self.fields.push((key.into(), value));
        }
        self
    }

    /// Set timestamp (raw value)
    #[must_use]
    pub fn timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Set timestamp from SystemTime (uses configured precision)
    #[must_use]
    pub fn timestamp_system(mut self, time: SystemTime) -> Self {
        self.timestamp = Some(self.precision.convert_system_time(time));
        self
    }

    /// Set timestamp to current time
    #[must_use]
    pub fn timestamp_now(self) -> Self {
        self.timestamp_system(SystemTime::now())
    }

    /// Build line protocol string
    ///
    /// Returns error if no fields are defined (InfluxDB requirement)
    pub fn build(mut self) -> Result<String, BuildError> {
        // Validate at least one field exists
        if self.fields.is_empty() {
            return Err(BuildError::NoFields);
        }

        // Sort fields if requested
        if self.sort_fields {
            self.fields.sort_by(|a, b| a.0.cmp(&b.0));
        }

        let mut result = String::with_capacity(256); // Pre-allocate for performance

        // Measurement name
        write!(&mut result, "{}", escape_measurement(&self.measurement)).unwrap();

        // Tags (already sorted by BTreeMap)
        for (key, value) in &self.tags {
            write!(
                &mut result,
                ",{}={}",
                escape_tag_key(key),
                escape_tag_value(value)
            )
            .unwrap();
        }

        // Fields
        result.push(' ');
        let mut first = true;
        for (key, value) in &self.fields {
            if !first {
                result.push(',');
            }
            write!(&mut result, "{}={}", escape_field_key(key), value).unwrap();
            first = false;
        }

        // Timestamp
        if let Some(ts) = self.timestamp {
            write!(&mut result, " {}", ts).unwrap();
        }

        Ok(result)
    }

    /// Build without validation (for backwards compatibility)
    pub fn build_unchecked(self) -> String {
        self.build().unwrap_or_else(|_| String::new())
    }
}

/// Build error
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("Line protocol must have at least one field")]
    NoFields,
}

// Escape functions according to InfluxDB Line Protocol specification
fn escape_measurement(s: &str) -> String {
    s.replace(',', r"\,").replace(' ', r"\ ")
}

fn escape_tag_key(s: &str) -> String {
    s.replace(',', r"\,")
        .replace('=', r"\=")
        .replace(' ', r"\ ")
}

fn escape_tag_value(s: &str) -> String {
    s.replace(',', r"\,")
        .replace('=', r"\=")
        .replace(' ', r"\ ")
}

fn escape_field_key(s: &str) -> String {
    s.replace(',', r"\,")
        .replace('=', r"\=")
        .replace(' ', r"\ ")
}

fn escape_string_value(s: &str) -> String {
    s.replace('\\', r"\\")
        .replace('"', r#"\""#)
        .replace('\n', r"\n")
        .replace('\r', r"\r")
        .replace('\t', r"\t")
}

// Convenient conversions
impl From<f64> for FieldValue {
    fn from(value: f64) -> Self {
        FieldValue::Float(value)
    }
}

impl From<f32> for FieldValue {
    fn from(value: f32) -> Self {
        FieldValue::Float(value as f64)
    }
}

impl From<i64> for FieldValue {
    fn from(value: i64) -> Self {
        FieldValue::Integer(value)
    }
}

impl From<i32> for FieldValue {
    fn from(value: i32) -> Self {
        FieldValue::Integer(value as i64)
    }
}

impl From<i16> for FieldValue {
    fn from(value: i16) -> Self {
        FieldValue::Integer(value as i64)
    }
}

impl From<i8> for FieldValue {
    fn from(value: i8) -> Self {
        FieldValue::Integer(value as i64)
    }
}

impl From<u64> for FieldValue {
    fn from(value: u64) -> Self {
        FieldValue::UnsignedInteger(value)
    }
}

impl From<u32> for FieldValue {
    fn from(value: u32) -> Self {
        FieldValue::UnsignedInteger(value as u64)
    }
}

impl From<u16> for FieldValue {
    fn from(value: u16) -> Self {
        FieldValue::UnsignedInteger(value as u64)
    }
}

impl From<u8> for FieldValue {
    fn from(value: u8) -> Self {
        FieldValue::UnsignedInteger(value as u64)
    }
}

impl From<usize> for FieldValue {
    fn from(value: usize) -> Self {
        FieldValue::UnsignedInteger(value as u64)
    }
}

impl From<String> for FieldValue {
    fn from(value: String) -> Self {
        FieldValue::String(value)
    }
}

impl From<&str> for FieldValue {
    fn from(value: &str) -> Self {
        FieldValue::String(value.to_string())
    }
}

impl From<bool> for FieldValue {
    fn from(value: bool) -> Self {
        FieldValue::Boolean(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_line_protocol() {
        let line = LineProtocolBuilder::new("cpu")
            .tag("host", "server01")
            .tag("region", "us-west")
            .field("usage", 0.64)
            .field("active", true)
            .timestamp(1687000000)
            .build()
            .unwrap();

        assert!(line.starts_with("cpu,"));
        assert!(line.contains("host=server01"));
        assert!(line.contains("region=us-west"));
        assert!(line.contains("usage=0.640000"));
        assert!(line.contains("active=true"));
        assert!(line.ends_with(" 1687000000"));
    }

    #[test]
    fn test_no_fields_error() {
        let result = LineProtocolBuilder::new("cpu")
            .tag("host", "server01")
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_escape_special_characters() {
        let line = LineProtocolBuilder::new("cpu,test")
            .tag("host=name", "server 01")
            .field("field,key", "string\nvalue")
            .build()
            .unwrap();

        assert!(line.contains(r"cpu\,test"));
        assert!(line.contains(r"host\=name=server\ 01"));
        assert!(line.contains(r"field\,key="));
        assert!(line.contains(r#""string\nvalue""#));
    }

    #[test]
    fn test_tag_ordering() {
        let line1 = LineProtocolBuilder::new("cpu")
            .tag("b", "2")
            .tag("a", "1")
            .tag("c", "3")
            .field("value", 1)
            .build()
            .unwrap();

        let line2 = LineProtocolBuilder::new("cpu")
            .tag("c", "3")
            .tag("a", "1")
            .tag("b", "2")
            .field("value", 1)
            .build()
            .unwrap();

        // Tags should be in same order (alphabetical) regardless of insertion order
        assert_eq!(line1, line2);
        assert!(line1.starts_with("cpu,a=1,b=2,c=3"));
    }

    #[test]
    fn test_field_sorting() {
        let line = LineProtocolBuilder::new("cpu")
            .with_sorted_fields(true)
            .field("c", 3)
            .field("a", 1)
            .field("b", 2)
            .build()
            .unwrap();

        // Fields should be sorted alphabetically
        assert!(line.contains("a=1i,b=2i,c=3i"));
    }

    #[test]
    fn test_timestamp_precision() {
        let now = SystemTime::now();

        let ns = TimestampPrecision::Nanoseconds.convert_system_time(now);
        let us = TimestampPrecision::Microseconds.convert_system_time(now);
        let ms = TimestampPrecision::Milliseconds.convert_system_time(now);
        let s = TimestampPrecision::Seconds.convert_system_time(now);

        assert!(ns > us * 1000);
        assert!(us > ms * 1000);
        assert!(ms > s * 1000);
    }

    #[test]
    fn test_float_formatting() {
        let line = LineProtocolBuilder::new("test")
            .field("normal", 1.234567)
            .field("small", 0.000001)
            .field("large", 1000000.0)
            .field("very_small", 0.0000001)
            .field("very_large", 10000000.0)
            .build()
            .unwrap();

        assert!(line.contains("normal=1.234567"));
        assert!(line.contains("small=0.000001"));
        assert!(line.contains("large=1000000.000000"));
        assert!(line.contains("very_small=1e-7") || line.contains("very_small=1.0e-7"));
        assert!(line.contains("very_large=1e7") || line.contains("very_large=1.0e7"));
    }

    #[test]
    fn test_system_time() {
        let line = LineProtocolBuilder::new("cpu")
            .field("value", 1)
            .timestamp_now()
            .build()
            .unwrap();

        // Should have timestamp
        let parts: Vec<&str> = line.split(' ').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts[2].parse::<i64>().is_ok());
    }

    #[test]
    fn test_batch_operations() {
        let tags = vec![("host", "server01"), ("region", "us-west")];
        let fields = vec![
            ("cpu", FieldValue::Float(0.64)),
            ("memory", FieldValue::Integer(1024)),
        ];

        let line = LineProtocolBuilder::new("system")
            .tags(tags)
            .fields(fields)
            .build()
            .unwrap();

        assert!(line.contains("host=server01"));
        assert!(line.contains("region=us-west"));
        assert!(line.contains("cpu=0.640000"));
        assert!(line.contains("memory=1024i"));
    }
}
