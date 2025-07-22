use crate::error::ApiGatewayError;
use chrono::{DateTime, Utc};
use influxdb::{Client, ReadQuery};
use serde_json::json;
use std::collections::HashMap;
use tracing::{debug, error, warn};

/// InfluxDB客户端包装器
pub struct InfluxDbClient {
    client: Client,
    database: String,
}

impl InfluxDbClient {
    /// 创建新的InfluxDB客户端
    pub fn new(url: &str, database: &str) -> Self {
        let client = Client::new(url, database);
        Self {
            client,
            database: database.to_string(),
        }
    }

    /// 查询历史数据
    pub async fn query_historical_data(
        &self,
        channel_id: u32,
        point_id: Option<u32>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        limit: Option<u32>,
    ) -> Result<serde_json::Value, ApiGatewayError> {
        let mut query_str = format!(
            "SELECT * FROM measurement WHERE channel_id = '{}'",
            channel_id
        );

        // 添加点位ID过滤
        if let Some(pid) = point_id {
            query_str.push_str(&format!(" AND point_id = '{}'", pid));
        }

        // 添加时间范围过滤
        if let Some(start) = start_time {
            query_str.push_str(&format!(" AND time >= '{}'", start.to_rfc3339()));
        }
        if let Some(end) = end_time {
            query_str.push_str(&format!(" AND time <= '{}'", end.to_rfc3339()));
        }

        // 按时间排序
        query_str.push_str(" ORDER BY time DESC");

        // 添加限制
        if let Some(l) = limit {
            query_str.push_str(&format!(" LIMIT {}", l));
        }

        debug!("InfluxDB query: {}", query_str);

        let query = ReadQuery::new(query_str);
        
        match self.client.query(query).await {
            Ok(result) => {
                // 转换InfluxDB结果为JSON格式
                let json_result = self.convert_influxdb_result(result)?;
                Ok(json_result)
            }
            Err(e) => {
                error!("InfluxDB query failed: {}", e);
                Err(ApiGatewayError::InfluxDb(e.to_string()))
            }
        }
    }

    /// 查询聚合数据
    pub async fn query_aggregated_data(
        &self,
        channel_id: u32,
        point_id: Option<u32>,
        aggregation: &str, // "mean", "sum", "count", "min", "max"
        interval: &str,    // "1m", "5m", "1h", "1d"
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
    ) -> Result<serde_json::Value, ApiGatewayError> {
        let mut query_str = format!(
            "SELECT {}(value) FROM measurement WHERE channel_id = '{}'",
            aggregation, channel_id
        );

        // 添加点位ID过滤
        if let Some(pid) = point_id {
            query_str.push_str(&format!(" AND point_id = '{}'", pid));
        }

        // 添加时间范围过滤
        if let Some(start) = start_time {
            query_str.push_str(&format!(" AND time >= '{}'", start.to_rfc3339()));
        }
        if let Some(end) = end_time {
            query_str.push_str(&format!(" AND time <= '{}'", end.to_rfc3339()));
        }

        // 添加分组间隔
        query_str.push_str(&format!(" GROUP BY time({})", interval));

        debug!("InfluxDB aggregation query: {}", query_str);

        let query = ReadQuery::new(query_str);
        
        match self.client.query(query).await {
            Ok(result) => {
                let json_result = self.convert_influxdb_result(result)?;
                Ok(json_result)
            }
            Err(e) => {
                error!("InfluxDB aggregation query failed: {}", e);
                Err(ApiGatewayError::InfluxDb(e.to_string()))
            }
        }
    }

    /// 查询数据统计信息
    pub async fn query_statistics(
        &self,
        channel_id: u32,
        date: &str, // YYYYMMDD format
    ) -> Result<serde_json::Value, ApiGatewayError> {
        let start_time = format!("{}T00:00:00Z", date);
        let end_time = format!("{}T23:59:59Z", date);

        let query_str = format!(
            "SELECT count(value) as count, mean(value) as mean, min(value) as min, max(value) as max \
             FROM measurement WHERE channel_id = '{}' AND time >= '{}' AND time <= '{}'",
            channel_id, start_time, end_time
        );

        debug!("InfluxDB statistics query: {}", query_str);

        let query = ReadQuery::new(query_str);
        
        match self.client.query(query).await {
            Ok(result) => {
                let json_result = self.convert_influxdb_result(result)?;
                Ok(json_result)
            }
            Err(e) => {
                error!("InfluxDB statistics query failed: {}", e);
                Err(ApiGatewayError::InfluxDb(e.to_string()))
            }
        }
    }

    /// 转换InfluxDB查询结果为JSON格式
    fn convert_influxdb_result(
        &self,
        result: String,
    ) -> Result<serde_json::Value, ApiGatewayError> {
        // 简化实现：直接返回查询结果的JSON解析
        // 在真实实现中，应该解析InfluxDB的JSON格式结果
        match serde_json::from_str::<serde_json::Value>(&result) {
            Ok(parsed) => Ok(parsed),
            Err(e) => {
                error!("Failed to parse InfluxDB result: {}", e);
                // 返回原始字符串作为fallback
                Ok(json!({
                    "raw_result": result,
                    "parse_error": e.to_string(),
                    "data": []
                }))
            }
        }
    }

    /// 健康检查
    pub async fn health_check(&self) -> Result<bool, ApiGatewayError> {
        let query = ReadQuery::new("SHOW DATABASES");
        
        match self.client.query(query).await {
            Ok(_) => {
                debug!("InfluxDB health check passed");
                Ok(true)
            }
            Err(e) => {
                warn!("InfluxDB health check failed: {}", e);
                Ok(false)
            }
        }
    }

    /// 获取数据库信息
    pub async fn get_database_info(&self) -> Result<serde_json::Value, ApiGatewayError> {
        let query = ReadQuery::new("SHOW DATABASES");
        
        match self.client.query(query).await {
            Ok(result) => {
                let databases = self.convert_influxdb_result(result)?;
                Ok(json!({
                    "current_database": self.database,
                    "available_databases": databases
                }))
            }
            Err(e) => {
                error!("Failed to get InfluxDB database info: {}", e);
                Err(ApiGatewayError::InfluxDb(e.to_string()))
            }
        }
    }
}

/// 解析历史数据键
pub fn parse_historical_key(key: &str) -> Option<HistoricalQuery> {
    if let Some(rest) = key.strip_prefix("his:") {
        let parts: Vec<&str> = rest.split(':').collect();
        
        match parts.as_slice() {
            ["index", channel_id, date] => {
                if let Ok(cid) = channel_id.parse::<u32>() {
                    return Some(HistoricalQuery::Index {
                        channel_id: cid,
                        date: date.to_string(),
                    });
                }
            }
            ["query", query_id] => {
                return Some(HistoricalQuery::CachedQuery {
                    query_id: query_id.to_string(),
                });
            }
            ["stats", channel_id, date] => {
                if let Ok(cid) = channel_id.parse::<u32>() {
                    return Some(HistoricalQuery::Statistics {
                        channel_id: cid,
                        date: date.to_string(),
                    });
                }
            }
            _ => {}
        }
    }
    None
}

/// 历史数据查询类型
#[derive(Debug, Clone)]
pub enum HistoricalQuery {
    Index {
        channel_id: u32,
        date: String,
    },
    CachedQuery {
        query_id: String,
    },
    Statistics {
        channel_id: u32,
        date: String,
    },
}