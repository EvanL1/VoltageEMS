//! 共享 Serde 反序列化器
//!
//! 用于处理 API 请求中的可选字段，支持多种输入格式：
//! - `null` → None
//! - `""` (空字符串) → None
//! - 字符串数字 `"123"` → Some(123)
//! - 原生数字 `123` → Some(123)

use serde::{Deserialize, Deserializer};

/// 反序列化可选 i32
///
/// 支持以下输入格式：
/// - `null` → `None`
/// - `""` → `None`
/// - `123` 或 `"123"` → `Some(123)`
///
/// # Example
/// ```ignore
/// #[derive(Deserialize)]
/// struct Request {
///     #[serde(default, deserialize_with = "deserialize_optional_i32")]
///     channel_id: Option<i32>,
/// }
/// ```
pub fn deserialize_optional_i32<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrInt {
        String(String),
        Int(i32),
        Null,
    }

    match Option::<StringOrInt>::deserialize(deserializer)? {
        None => Ok(None),
        Some(StringOrInt::Null) => Ok(None),
        Some(StringOrInt::String(s)) if s.is_empty() => Ok(None),
        Some(StringOrInt::String(s)) => s
            .parse::<i32>()
            .map(Some)
            .map_err(|_| D::Error::custom(format!("invalid integer: {}", s))),
        Some(StringOrInt::Int(i)) => Ok(Some(i)),
    }
}

/// 反序列化可选 u32
///
/// 支持以下输入格式：
/// - `null` → `None`
/// - `""` → `None`
/// - `123` 或 `"123"` → `Some(123)`
///
/// # Example
/// ```ignore
/// #[derive(Deserialize)]
/// struct Request {
///     #[serde(default, deserialize_with = "deserialize_optional_u32")]
///     point_id: Option<u32>,
/// }
/// ```
pub fn deserialize_optional_u32<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrInt {
        String(String),
        Int(u32),
        Null,
    }

    match Option::<StringOrInt>::deserialize(deserializer)? {
        None => Ok(None),
        Some(StringOrInt::Null) => Ok(None),
        Some(StringOrInt::String(s)) if s.is_empty() => Ok(None),
        Some(StringOrInt::String(s)) => s
            .parse::<u32>()
            .map(Some)
            .map_err(|_| D::Error::custom(format!("invalid integer: {}", s))),
        Some(StringOrInt::Int(i)) => Ok(Some(i)),
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // unwrap is acceptable in tests
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct TestI32 {
        #[serde(default, deserialize_with = "deserialize_optional_i32")]
        value: Option<i32>,
    }

    #[derive(Deserialize)]
    struct TestU32 {
        #[serde(default, deserialize_with = "deserialize_optional_u32")]
        value: Option<u32>,
    }

    #[test]
    fn test_optional_i32_null() {
        let json = r#"{"value": null}"#;
        let result: TestI32 = serde_json::from_str(json).unwrap();
        assert_eq!(result.value, None);
    }

    #[test]
    fn test_optional_i32_empty_string() {
        let json = r#"{"value": ""}"#;
        let result: TestI32 = serde_json::from_str(json).unwrap();
        assert_eq!(result.value, None);
    }

    #[test]
    fn test_optional_i32_string_number() {
        let json = r#"{"value": "123"}"#;
        let result: TestI32 = serde_json::from_str(json).unwrap();
        assert_eq!(result.value, Some(123));
    }

    #[test]
    fn test_optional_i32_native_number() {
        let json = r#"{"value": 456}"#;
        let result: TestI32 = serde_json::from_str(json).unwrap();
        assert_eq!(result.value, Some(456));
    }

    #[test]
    fn test_optional_i32_negative() {
        let json = r#"{"value": "-789"}"#;
        let result: TestI32 = serde_json::from_str(json).unwrap();
        assert_eq!(result.value, Some(-789));
    }

    #[test]
    fn test_optional_u32_null() {
        let json = r#"{"value": null}"#;
        let result: TestU32 = serde_json::from_str(json).unwrap();
        assert_eq!(result.value, None);
    }

    #[test]
    fn test_optional_u32_string_number() {
        let json = r#"{"value": "999"}"#;
        let result: TestU32 = serde_json::from_str(json).unwrap();
        assert_eq!(result.value, Some(999));
    }

    #[test]
    fn test_optional_i32_invalid_string() {
        let json = r#"{"value": "not_a_number"}"#;
        let result: Result<TestI32, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
