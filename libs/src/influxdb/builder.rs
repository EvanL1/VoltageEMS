//! 线协议构建器

use std::fmt::{self, Write};

/// 字段值类型
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

/// `InfluxDB` 线协议构建器
#[derive(Debug)]
pub struct LineProtocolBuilder {
    measurement: String,
    tags: Vec<(String, String)>,
    fields: Vec<(String, FieldValue)>,
    timestamp: Option<i64>,
}

impl LineProtocolBuilder {
    /// 创建新的构建器
    pub fn new(measurement: impl Into<String>) -> Self {
        Self {
            measurement: measurement.into(),
            tags: Vec::new(),
            fields: Vec::new(),
            timestamp: None,
        }
    }

    /// 添加标签
    #[must_use]
    pub fn tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.push((key.into(), value.into()));
        self
    }

    /// 添加字段
    #[must_use]
    pub fn field(mut self, key: impl Into<String>, value: impl Into<FieldValue>) -> Self {
        self.fields.push((key.into(), value.into()));
        self
    }

    /// 设置时间戳（纳秒）
    #[must_use]
    pub fn timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// 构建线协议字符串
    pub fn build(self) -> String {
        let mut result = String::new();

        // 测量名称
        write!(&mut result, "{}", escape_measurement(&self.measurement)).unwrap();

        // 标签
        for (key, value) in &self.tags {
            write!(
                &mut result,
                ",{}={}",
                escape_tag_key(key),
                escape_tag_value(value)
            )
            .unwrap();
        }

        // 字段
        result.push(' ');
        let mut first = true;
        for (key, value) in &self.fields {
            if !first {
                result.push(',');
            }
            write!(&mut result, "{}={}", escape_field_key(key), value).unwrap();
            first = false;
        }

        // 时间戳
        if let Some(ts) = self.timestamp {
            write!(&mut result, " {ts}").unwrap();
        }

        result
    }
}

// 转义函数
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

// 便捷转换
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
