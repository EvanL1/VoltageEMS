use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::{IntoParams, ToSchema};

// 历史数据查询相关模型
#[derive(Debug, Serialize, Deserialize, ToSchema, IntoParams)]
pub struct HistoryQueryFilter {
    /// 数据源/设备ID
    pub source_id: Option<String>,
    /// 数据点名称或模式
    pub point_name: Option<String>,
    /// 数据类型过滤
    pub data_type: Option<String>,
    /// 开始时间
    pub start_time: DateTime<Utc>,
    /// 结束时间
    pub end_time: DateTime<Utc>,
    /// 聚合类型 (raw, avg, min, max, count)
    pub aggregation: Option<String>,
    /// 聚合时间间隔 (例如: "1m", "5m", "1h")
    pub interval: Option<String>,
    /// 限制返回数量
    pub limit: Option<u32>,
    /// 偏移量
    pub offset: Option<u32>,
    /// 排序方式 (asc, desc)
    pub order: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HistoryDataPoint {
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 数据源/设备ID
    pub source_id: String,
    /// 数据点名称
    pub point_name: String,
    /// 数据值
    pub value: HistoryValue,
    /// 数据质量标识
    pub quality: Option<String>,
    /// 标签信息
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum HistoryValue {
    /// 数值型数据
    Numeric(f64),
    /// 字符串数据
    Text(String),
    /// 布尔值
    Boolean(bool),
    /// JSON对象
    Object(serde_json::Value),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HistoryQueryResult {
    /// 查询条件摘要
    pub query_summary: QuerySummary,
    /// 历史数据点列表
    pub data_points: Vec<HistoryDataPoint>,
    /// 聚合结果 (如果有聚合查询)
    pub aggregated_data: Option<Vec<AggregatedDataPoint>>,
    /// 分页信息
    pub pagination: PaginationInfo,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct QuerySummary {
    /// 查询的时间范围
    pub time_range: TimeRange,
    /// 匹配的数据源数量
    pub source_count: u32,
    /// 返回的数据点总数
    pub point_count: u64,
    /// 查询执行时间(毫秒)
    pub execution_time_ms: u64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TimeRange {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AggregatedDataPoint {
    /// 时间窗口开始时间
    pub window_start: DateTime<Utc>,
    /// 时间窗口结束时间
    pub window_end: DateTime<Utc>,
    /// 聚合值
    pub value: f64,
    /// 聚合类型
    pub aggregation_type: String,
    /// 该窗口内的原始数据点数量
    pub sample_count: u32,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaginationInfo {
    /// 总记录数
    pub total_count: u64,
    /// 当前页返回数量
    pub current_count: u32,
    /// 偏移量
    pub offset: u32,
    /// 是否还有更多数据
    pub has_more: bool,
}

// 统计和分析相关模型
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DataSourceInfo {
    /// 数据源ID
    pub source_id: String,
    /// 数据源名称
    pub source_name: Option<String>,
    /// 数据点列表
    pub points: Vec<DataPointInfo>,
    /// 第一条数据时间
    pub first_data_time: Option<DateTime<Utc>>,
    /// 最后一条数据时间
    pub last_data_time: Option<DateTime<Utc>>,
    /// 数据点总数
    pub total_points: u64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DataPointInfo {
    /// 数据点名称
    pub point_name: String,
    /// 数据类型
    pub data_type: String,
    /// 最新值
    pub latest_value: Option<HistoryValue>,
    /// 最新数据时间
    pub latest_timestamp: Option<DateTime<Utc>>,
    /// 该点的数据条数
    pub count: u64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TimeSeriesStatistics {
    /// 统计时间范围
    pub time_range: TimeRange,
    /// 数据源统计
    pub sources: Vec<SourceStatistics>,
    /// 整体统计
    pub overall: OverallStatistics,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SourceStatistics {
    /// 数据源ID
    pub source_id: String,
    /// 数据点数量
    pub point_count: u64,
    /// 平均采样率 (points/hour)
    pub avg_sample_rate: f64,
    /// 数据完整性百分比
    pub data_completeness: f64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OverallStatistics {
    /// 总数据源数量
    pub total_sources: u32,
    /// 总数据点数量
    pub total_points: u64,
    /// 平均数据密度
    pub avg_data_density: f64,
    /// 存储空间使用情况
    pub storage_usage: StorageUsage,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StorageUsage {
    /// 总存储大小(字节)
    pub total_size_bytes: u64,
    /// 按存储后端分组的使用情况
    pub by_backend: HashMap<String, u64>,
    /// 数据压缩率
    pub compression_ratio: Option<f64>,
}

// 数据导出相关模型
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExportRequest {
    /// 查询条件
    pub query: HistoryQueryFilter,
    /// 导出格式 (csv, json, parquet)
    pub format: String,
    /// 导出选项
    pub options: ExportOptions,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExportOptions {
    /// 是否包含表头
    pub include_header: Option<bool>,
    /// 时间格式
    pub time_format: Option<String>,
    /// 压缩方式
    pub compression: Option<String>,
    /// 文件名前缀
    pub filename_prefix: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExportJob {
    /// 导出任务ID
    pub job_id: String,
    /// 任务状态 (pending, running, completed, failed)
    pub status: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 开始时间
    pub started_at: Option<DateTime<Utc>>,
    /// 完成时间
    pub completed_at: Option<DateTime<Utc>>,
    /// 导出文件信息
    pub file_info: Option<ExportFileInfo>,
    /// 进度百分比
    pub progress: Option<f32>,
    /// 错误信息
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExportFileInfo {
    /// 文件名
    pub filename: String,
    /// 文件大小(字节)
    pub file_size: u64,
    /// 下载链接
    pub download_url: String,
    /// 文件过期时间
    pub expires_at: DateTime<Utc>,
}

// 通用响应模型
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HistoryApiResponse<T> {
    /// 操作是否成功
    pub success: bool,
    /// 响应消息
    pub message: String,
    /// 响应数据
    pub data: Option<T>,
    /// 响应时间戳
    pub timestamp: DateTime<Utc>,
    /// 请求ID (用于追踪)
    pub request_id: Option<String>,
}

impl<T> HistoryApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            message: "Success".to_string(),
            data: Some(data),
            timestamp: Utc::now(),
            request_id: None,
        }
    }

    pub fn success_with_message(data: T, message: String) -> Self {
        Self {
            success: true,
            message,
            data: Some(data),
            timestamp: Utc::now(),
            request_id: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            message,
            data: None,
            timestamp: Utc::now(),
            request_id: None,
        }
    }
}
