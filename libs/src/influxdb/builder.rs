//! Line Protocol Builder

use std::fmt::{self, Write};

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
            FieldValue::Float(v) => write!(f, "{v}"),
            FieldValue::Integer(v) => write!(f, "{v}i"),
            FieldValue::UnsignedInteger(v) => write!(f, "{v}u"),
            FieldValue::String(v) => write!(f, "\"{}\"", v.replace('"', "\\\"")),
            FieldValue::Boolean(v) => write!(f, "{v}"),
        }
    }
}

/// `InfluxDB` Line Protocol Builder
#[derive(Debug)]
pub struct LineProtocolBuilder {
    measurement: String,
    tags: Vec<(String, String)>,
    fields: Vec<(String, FieldValue)>,
    timestamp: Option<i64>,
}

impl LineProtocolBuilder {
    /// Create new builder
    pub fn new(measurement: impl Into<String>) -> Self {
        Self {
            measurement: measurement.into(),
            tags: Vec::new(),
            fields: Vec::new(),
            timestamp: None,
        }
    }

    /// Add tag
    #[must_use]
    pub fn tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.push((key.into(), value.into()));
        self
    }

    /// Add field
    #[must_use]
    pub fn field(mut self, key: impl Into<String>, value: impl Into<FieldValue>) -> Self {
        self.fields.push((key.into(), value.into()));
        self
    }

    /// Set timestamp (nanoseconds)
    #[must_use]
    pub fn timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Build line protocol string
    pub fn build(self) -> String {
        let mut result = String::new();

        // Measurement name
        write!(&mut result, "{}", escape_measurement(&self.measurement)).unwrap();

        // Tags
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
            write!(&mut result, " {ts}").unwrap();
        }

        result
    }
}

// Escape functions
fn escape_measurement(s: &str) -> String {
    s.replace(',', "\\,").replace(' ', "\\ ")
}

fn escape_tag_key(s: &str) -> String {
    s.replace(',', "\\,")
        .replace('=', "\\=")
        .replace(' ', "\\ ")
}

fn escape_tag_value(s: &str) -> String {
    s.replace(',', "\\,")
        .replace('=', "\\=")
        .replace(' ', "\\ ")
}

fn escape_field_key(s: &str) -> String {
    s.replace(',', "\\,")
        .replace('=', "\\=")
        .replace(' ', "\\ ")
}

// 便捷converting
impl From<f64> for FieldValue {
    fn from(value: f64) -> Self {
        FieldValue::Float(value)
    }
}

impl From<i64> for FieldValue {
    fn from(value: i64) -> Self {
        FieldValue::Integer(value)
    }
}

impl From<u64> for FieldValue {
    fn from(value: u64) -> Self {
        FieldValue::UnsignedInteger(value)
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
