use crate::config::InfluxDBConfig;
use crate::error::{HisSrvError, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// 数据点值类型
#[derive(Debug, Clone)]
pub enum DataValue {
    Float(f64),
    Integer(i64),
    String(String),
    Boolean(bool),
}

impl DataValue {
    /// 转换为 InfluxDB Line Protocol 格式的值
    pub fn to_line_protocol(&self) -> String {
        match self {
            DataValue::Float(f) => f.to_string(),
            DataValue::Integer(i) => format!("{}i", i),
            DataValue::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
            DataValue::Boolean(b) => b.to_string(),
        }
    }
}

/// 从 voltage-common 的 PointData 转换
impl From<voltage_common::types::PointData> for DataValue {
    fn from(point_data: voltage_common::types::PointData) -> Self {
        match point_data.value {
            voltage_common::types::PointValue::Float(f) => DataValue::Float(f),
            voltage_common::types::PointValue::Int(i) => DataValue::Integer(i),
            voltage_common::types::PointValue::String(s) => DataValue::String(s),
            voltage_common::types::PointValue::Bool(b) => DataValue::Boolean(b),
            voltage_common::types::PointValue::Binary(_) => DataValue::String("binary_data".to_string()),
            voltage_common::types::PointValue::Null => DataValue::String("null".to_string()),
        }
    }
}

/// 数据点结构
#[derive(Debug, Clone)]
pub struct DataPoint {
    /// 测量名称
    pub measurement: String,
    /// 标签
    pub tags: HashMap<String, String>,
    /// 字段
    pub fields: HashMap<String, DataValue>,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
}

impl DataPoint {
    /// 创建新的数据点
    pub fn new(
        measurement: String,
        tags: HashMap<String, String>,
        fields: HashMap<String, DataValue>,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            measurement,
            tags,
            fields,
            timestamp,
        }
    }

    /// 从通道信息创建数据点
    pub fn from_channel_data(
        channel_id: u32,
        point_id: u32,
        point_type: &str,
        value: DataValue,
        timestamp: DateTime<Utc>,
    ) -> Self {
        let measurement = match point_type {
            "m" => "telemetry".to_string(),
            "s" => "signal".to_string(),
            "c" => "control".to_string(),
            "a" => "adjustment".to_string(),
            _ => "unknown".to_string(),
        };

        let mut tags = HashMap::new();
        tags.insert("channel_id".to_string(), channel_id.to_string());
        tags.insert("point_id".to_string(), point_id.to_string());
        tags.insert("point_type".to_string(), point_type.to_string());

        let mut fields = HashMap::new();
        fields.insert("value".to_string(), value);

        Self::new(measurement, tags, fields, timestamp)
    }

    /// 转换为 InfluxDB Line Protocol 格式
    pub fn to_line_protocol(&self) -> String {
        let mut line = self.measurement.clone();

        // 添加标签
        if !self.tags.is_empty() {
            line.push(',');
            let tag_parts: Vec<String> = self
                .tags
                .iter()
                .map(|(k, v)| format!("{}={}", escape_tag_key(k), escape_tag_value(v)))
                .collect();
            line.push_str(&tag_parts.join(","));
        }

        // 添加字段
        line.push(' ');
        let field_parts: Vec<String> = self
            .fields
            .iter()
            .map(|(k, v)| format!("{}={}", escape_field_key(k), v.to_line_protocol()))
            .collect();
        line.push_str(&field_parts.join(","));

        // 添加时间戳 (纳秒)
        line.push(' ');
        line.push_str(&self.timestamp.timestamp_nanos_opt().unwrap_or(0).to_string());

        line
    }
}

/// InfluxDB 3.2 客户端
#[derive(Clone)]
pub struct InfluxDBClient {
    config: InfluxDBConfig,
    client: Client,
    batch_buffer: Arc<Mutex<Vec<DataPoint>>>,
}

impl InfluxDBClient {
    /// 创建新的 InfluxDB 客户端
    pub fn new(config: InfluxDBConfig) -> Self {
        let client = Client::new();
        let batch_buffer = Arc::new(Mutex::new(Vec::with_capacity(config.batch_size)));

        Self {
            config,
            client,
            batch_buffer,
        }
    }

    /// 测试连接
    pub async fn ping(&self) -> Result<()> {
        let url = format!("{}/health", self.config.url);
        
        let mut request = self.client.get(&url);
        
        // 添加认证头（如果有 token）
        if let Some(ref token) = self.config.token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await.map_err(|e| {
            HisSrvError::influxdb(format!("连接 InfluxDB 失败: {}", e))
        })?;

        if response.status().is_success() {
            info!("InfluxDB 连接成功");
            Ok(())
        } else {
            Err(HisSrvError::influxdb(format!(
                "InfluxDB 健康检查失败: {}",
                response.status()
            )))
        }
    }

    /// 写入单个数据点
    pub async fn write_point(&self, point: DataPoint) -> Result<()> {
        let mut buffer = self.batch_buffer.lock().await;
        buffer.push(point);

        // 如果达到批量大小，触发写入
        if buffer.len() >= self.config.batch_size {
            let points = buffer.drain(..).collect();
            drop(buffer); // 释放锁
            self.flush_points(points).await?;
        }

        Ok(())
    }

    /// 写入多个数据点
    pub async fn write_points(&self, points: Vec<DataPoint>) -> Result<()> {
        for point in points {
            self.write_point(point).await?;
        }
        Ok(())
    }

    /// 强制刷新缓冲区
    pub async fn flush(&self) -> Result<()> {
        let mut buffer = self.batch_buffer.lock().await;
        if buffer.is_empty() {
            return Ok(());
        }

        let points = buffer.drain(..).collect();
        drop(buffer); // 释放锁
        self.flush_points(points).await
    }

    /// 执行实际的写入操作
    async fn flush_points(&self, points: Vec<DataPoint>) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }

        debug!("写入 {} 个数据点到 InfluxDB", points.len());

        // 构建 Line Protocol 数据
        let line_protocol: Vec<String> = points
            .iter()
            .map(|point| point.to_line_protocol())
            .collect();
        let body = line_protocol.join("\n");

        // 构建写入 URL
        let mut url = format!("{}/api/v2/write", self.config.url);
        
        // 添加查询参数
        let mut query_params = vec![
            ("bucket", self.config.database.as_str()),
            ("precision", "ns"), // 纳秒精度
        ];

        if let Some(ref org) = self.config.organization {
            query_params.push(("org", org.as_str()));
        }

        url.push('?');
        url.push_str(
            &query_params
                .iter()
                .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
                .collect::<Vec<_>>()
                .join("&"),
        );

        // 构建请求
        let mut request = self.client.post(&url)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(body);

        // 添加认证头
        if let Some(ref token) = self.config.token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        // 发送请求
        let response = request.send().await.map_err(|e| {
            HisSrvError::influxdb(format!("写入 InfluxDB 失败: {}", e))
        })?;

        if response.status().is_success() {
            debug!("成功写入 {} 个数据点", points.len());
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("InfluxDB 写入失败: {} - {}", status, body);
            Err(HisSrvError::influxdb(format!(
                "写入失败: {} - {}",
                status, body
            )))
        }
    }

    /// 查询数据（基本的 SQL 查询支持）
    pub async fn query(&self, sql: &str) -> Result<Value> {
        let url = format!("{}/api/v2/query", self.config.url);
        
        let mut request = self.client.post(&url)
            .header("Content-Type", "application/json");

        // 添加认证头
        if let Some(ref token) = self.config.token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        // 构建查询体
        let query_body = serde_json::json!({
            "query": sql,
            "type": "sql"
        });

        let response = request.json(&query_body).send().await.map_err(|e| {
            HisSrvError::influxdb(format!("查询 InfluxDB 失败: {}", e))
        })?;

        if response.status().is_success() {
            let result: Value = response.json().await.map_err(|e| {
                HisSrvError::influxdb(format!("解析查询结果失败: {}", e))
            })?;
            Ok(result)
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(HisSrvError::influxdb(format!(
                "查询失败: {} - {}",
                status, body
            )))
        }
    }

    /// 获取配置
    pub fn config(&self) -> &InfluxDBConfig {
        &self.config
    }
}

/// 转义标签键
fn escape_tag_key(key: &str) -> String {
    key.replace(' ', "\\ ")
        .replace(',', "\\,")
        .replace('=', "\\=")
}

/// 转义标签值
fn escape_tag_value(value: &str) -> String {
    value.replace(' ', "\\ ")
         .replace(',', "\\,")
         .replace('=', "\\=")
}

/// 转义字段键
fn escape_field_key(key: &str) -> String {
    key.replace(' ', "\\ ")
        .replace(',', "\\,")
        .replace('=', "\\=")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_value_to_line_protocol() {
        assert_eq!(DataValue::Float(3.14).to_line_protocol(), "3.14");
        assert_eq!(DataValue::Integer(42).to_line_protocol(), "42i");
        assert_eq!(DataValue::String("hello".to_string()).to_line_protocol(), "\"hello\"");
        assert_eq!(DataValue::Boolean(true).to_line_protocol(), "true");
    }

    #[test]
    fn test_data_point_to_line_protocol() {
        let mut tags = HashMap::new();
        tags.insert("channel_id".to_string(), "1001".to_string());
        tags.insert("point_id".to_string(), "10001".to_string());

        let mut fields = HashMap::new();
        fields.insert("value".to_string(), DataValue::Float(23.5));

        let point = DataPoint::new(
            "telemetry".to_string(),
            tags,
            fields,
            DateTime::from_timestamp(1642681200, 0).unwrap(),
        );

        let line = point.to_line_protocol();
        assert!(line.contains("telemetry"));
        assert!(line.contains("channel_id=1001"));
        assert!(line.contains("point_id=10001"));
        assert!(line.contains("value=23.5"));
        assert!(line.contains("1642681200000000000"));
    }

    #[test]
    fn test_from_channel_data() {
        let point = DataPoint::from_channel_data(
            1001,
            10001,
            "m",
            DataValue::Float(25.6),
            Utc::now(),
        );

        assert_eq!(point.measurement, "telemetry");
        assert_eq!(point.tags.get("channel_id"), Some(&"1001".to_string()));
        assert_eq!(point.tags.get("point_id"), Some(&"10001".to_string()));
        assert_eq!(point.tags.get("point_type"), Some(&"m".to_string()));
    }

    #[test]
    fn test_escape_functions() {
        assert_eq!(escape_tag_key("test key"), "test\\ key");
        assert_eq!(escape_tag_value("value,with=spaces"), "value\\,with\\=spaces");
        assert_eq!(escape_field_key("field key"), "field\\ key");
    }
}