use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

use crate::api::models_history::{HistoryValue, TimeRange, PaginationInfo};

// 查询模式
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum QueryMode {
    /// 快速模式 - 优先使用Redis缓存
    Fast,
    /// 平衡模式 - 自动选择最优数据源
    Balanced,
    /// 精确模式 - 优先使用InfluxDB
    Accurate,
}

// 高级历史查询请求
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AdvancedHistoryQuery {
    /// 时间范围
    pub time_range: TimeRange,
    /// 过滤条件列表
    pub filters: Vec<QueryFilter>,
    /// 数据源模式匹配（支持通配符）
    pub source_patterns: Option<Vec<String>>,
    /// 聚合配置
    pub aggregations: Option<Vec<AggregationConfig>>,
    /// 分组字段
    pub group_by: Option<Vec<String>>,
    /// 排序配置
    pub order_by: Option<Vec<OrderByConfig>>,
    /// 分页配置
    pub pagination: Option<PaginationConfig>,
    /// 输出格式配置
    pub output_config: Option<OutputConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QueryFilter {
    /// 字段名称
    pub field: String,
    /// 操作符
    pub operator: FilterOperator,
    /// 值
    pub value: serde_json::Value,
    /// 是否大小写敏感（仅对字符串有效）
    pub case_sensitive: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum FilterOperator {
    Eq,         // 等于
    Ne,         // 不等于
    Gt,         // 大于
    Gte,        // 大于等于
    Lt,         // 小于
    Lte,        // 小于等于
    In,         // 在列表中
    NotIn,      // 不在列表中
    Like,       // 模糊匹配
    NotLike,    // 不匹配
    Regex,      // 正则表达式
    Between,    // 区间
    IsNull,     // 为空
    IsNotNull,  // 不为空
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AggregationConfig {
    /// 聚合函数
    pub function: AggregationFunction,
    /// 字段名称
    pub field: String,
    /// 输出别名
    pub alias: Option<String>,
    /// 时间窗口（如果是时间聚合）
    pub window: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AggregationFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    Median,
    Stddev,
    Variance,
    First,
    Last,
    Percentile(u8),
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderByConfig {
    /// 排序字段
    pub field: String,
    /// 排序方向
    pub direction: SortDirection,
    /// 空值处理
    pub nulls: Option<NullsOrder>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum NullsOrder {
    First,
    Last,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginationConfig {
    /// 页码（从1开始）
    pub page: u32,
    /// 每页大小
    pub page_size: u32,
    /// 是否返回总数
    pub include_total: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OutputConfig {
    /// 包含的字段
    pub include_fields: Option<Vec<String>>,
    /// 排除的字段
    pub exclude_fields: Option<Vec<String>>,
    /// 时间格式
    pub time_format: Option<String>,
    /// 数值精度
    pub numeric_precision: Option<u8>,
}

// 增强查询响应
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EnhancedQueryResponse {
    /// 请求ID
    pub request_id: String,
    /// 查询结果
    pub query_result: EnhancedQueryResult,
    /// 执行时间（毫秒）
    pub execution_time_ms: u64,
    /// 查询计划（调试用）
    pub query_plan: Option<crate::query_optimizer::QueryPlan>,
    /// 是否命中缓存
    pub cache_hit: bool,
    /// 响应时间戳
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EnhancedQueryResult {
    /// 数据点列表
    pub data_points: Vec<EnhancedDataPoint>,
    /// 聚合结果
    pub aggregations: Option<HashMap<String, serde_json::Value>>,
    /// 查询元数据
    pub metadata: QueryMetadata,
    /// 分页信息
    pub pagination: PaginationInfo,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EnhancedDataPoint {
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 数据源ID
    pub source_id: String,
    /// 数据点名称
    pub point_name: String,
    /// 数据值
    pub value: HistoryValue,
    /// 数据质量
    pub quality: DataQuality,
    /// 标签
    pub tags: HashMap<String, String>,
    /// 附加元数据
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DataQuality {
    /// 质量代码
    pub code: QualityCode,
    /// 质量描述
    pub description: String,
    /// 置信度（0-1）
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QualityCode {
    Good,
    Uncertain,
    Bad,
    NoData,
    ConfigError,
    NotApplicable,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct QueryMetadata {
    /// 总数据点数
    pub total_points: u64,
    /// 过滤后的数据点数
    pub filtered_points: u64,
    /// 涉及的数据源
    pub data_sources: Vec<String>,
    /// 实际查询的时间范围
    pub actual_time_range: Option<TimeRange>,
    /// 数据质量信息
    pub quality_info: Option<QualityInfo>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct QualityInfo {
    /// 良好数据比例
    pub good_ratio: f64,
    /// 缺失数据比例
    pub missing_ratio: f64,
    /// 异常数据比例
    pub bad_ratio: f64,
}

// 批量查询
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BatchHistoryQuery {
    /// 批量查询列表
    pub queries: Vec<AdvancedHistoryQuery>,
    /// 是否并行执行
    pub parallel: Option<bool>,
    /// 失败策略
    pub failure_strategy: Option<FailureStrategy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum FailureStrategy {
    /// 快速失败 - 任何查询失败则整体失败
    FailFast,
    /// 继续执行 - 忽略失败的查询
    Continue,
    /// 部分成功 - 返回成功的结果和失败信息
    Partial,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchQueryResponse {
    /// 批次ID
    pub batch_id: String,
    /// 批次状态
    pub status: BatchStatus,
    /// 查询总数
    pub total_queries: usize,
    /// 完成的查询数
    pub completed_queries: usize,
    /// 查询结果列表
    pub results: Vec<BatchQueryResult>,
    /// 错误列表
    pub errors: Vec<BatchQueryError>,
    /// 总执行时间
    pub execution_time_ms: u64,
    /// 响应时间戳
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum BatchStatus {
    Accepted,
    Running,
    Completed,
    Failed,
    PartialSuccess,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchQueryResult {
    /// 查询ID
    pub query_id: String,
    /// 原始查询
    pub query: AdvancedHistoryQuery,
    /// 查询结果
    pub result: Option<EnhancedQueryResult>,
    /// 错误信息（如果失败）
    pub error: Option<String>,
    /// 执行时间
    pub execution_time_ms: u64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchQueryError {
    /// 查询ID
    pub query_id: String,
    /// 原始查询
    pub query: AdvancedHistoryQuery,
    /// 错误信息
    pub error: String,
    /// 错误代码
    pub error_code: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchQueryAccepted {
    /// 批次ID
    pub batch_id: String,
    /// 状态查询URL
    pub status_url: String,
    /// 预计完成时间
    pub estimated_completion: Option<DateTime<Utc>>,
}

// 流式查询
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StreamHistoryQuery {
    /// 基础查询
    pub query: AdvancedHistoryQuery,
    /// 每个数据块的大小
    pub chunk_size: Option<u32>,
    /// 数据块之间的延迟（毫秒）
    pub chunk_delay_ms: Option<u64>,
    /// 是否包含中间聚合
    pub include_partial_aggregations: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StreamChunk {
    /// 块ID
    pub chunk_id: String,
    /// 块序号
    pub sequence: u64,
    /// 数据点
    pub data_points: Vec<EnhancedDataPoint>,
    /// 是否还有更多数据
    pub has_more: bool,
    /// 块时间戳
    pub timestamp: DateTime<Utc>,
}

// 数据分析
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TrendAnalysisRequest {
    /// 数据源ID
    pub source_id: String,
    /// 数据点名称
    pub point_name: String,
    /// 时间范围
    pub time_range: TimeRange,
    /// 分析参数
    pub parameters: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TrendAnalysisResponse {
    /// 请求ID
    pub request_id: String,
    /// 数据源ID
    pub source_id: String,
    /// 数据点名称
    pub point_name: String,
    /// 分析时间范围
    pub time_range: TimeRange,
    /// 趋势信息
    pub trend_info: TrendInfo,
    /// 统计信息
    pub statistics: TrendStatistics,
    /// 异常点
    pub anomalies: Vec<AnomalyPoint>,
    /// 预测结果（如果请求）
    pub forecast: Option<ForecastResult>,
    /// 执行时间
    pub execution_time_ms: u64,
    /// 响应时间戳
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TrendInfo {
    /// 趋势方向
    pub direction: TrendDirection,
    /// 斜率
    pub slope: f64,
    /// 相关系数
    pub correlation: f64,
    /// 置信度
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Stable,
    Volatile,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TrendStatistics {
    /// 平均值
    pub mean: f64,
    /// 中位数
    pub median: f64,
    /// 标准差
    pub std_dev: f64,
    /// 最小值
    pub min: f64,
    /// 最大值
    pub max: f64,
    /// 百分位数
    pub percentiles: HashMap<u8, f64>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AnomalyPoint {
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 实际值
    pub actual_value: f64,
    /// 期望值
    pub expected_value: f64,
    /// 异常分数
    pub anomaly_score: f64,
    /// 异常类型
    pub anomaly_type: AnomalyType,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AnomalyType {
    Spike,
    Dip,
    LevelShift,
    Variance,
    Missing,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ForecastResult {
    /// 预测点
    pub forecast_points: Vec<ForecastPoint>,
    /// 置信区间
    pub confidence_intervals: Vec<ConfidenceInterval>,
    /// 使用的算法
    pub algorithm_used: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ForecastPoint {
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 预测值
    pub value: f64,
    /// 置信度
    pub confidence: f64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConfidenceInterval {
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 下界
    pub lower_bound: f64,
    /// 上界
    pub upper_bound: f64,
    /// 置信水平
    pub confidence_level: f64,
}

// 聚合分析
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AggregateAnalysisRequest {
    /// 查询过滤条件
    pub filter: AdvancedHistoryQuery,
    /// 聚合函数列表
    pub aggregations: Vec<AggregationFunction>,
    /// 分组字段
    pub group_by: Option<Vec<String>>,
    /// 是否包含子聚合
    pub include_sub_aggregations: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AggregateAnalysisResponse {
    /// 请求ID
    pub request_id: String,
    /// 查询过滤条件
    pub query_filter: AdvancedHistoryQuery,
    /// 聚合结果
    pub results: Vec<AggregateResult>,
    /// 分组结果（如果有分组）
    pub group_by_results: Option<Vec<GroupByResult>>,
    /// 执行时间
    pub execution_time_ms: u64,
    /// 响应时间戳
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AggregateResult {
    /// 聚合函数
    pub aggregation: AggregationFunction,
    /// 聚合值
    pub value: f64,
    /// 样本数量
    pub sample_count: u64,
    /// 附加元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GroupByResult {
    /// 分组键值
    pub group_key: HashMap<String, String>,
    /// 该组的聚合结果
    pub aggregates: Vec<AggregateResult>,
    /// 组内记录数
    pub count: u64,
}

// 数据质量报告
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DataQualityReport {
    /// 报告ID
    pub report_id: String,
    /// 分析时间范围
    pub time_range: TimeRange,
    /// 数据源质量信息
    pub sources: Vec<SourceQualityInfo>,
    /// 整体质量指标
    pub overall_quality: QualityMetrics,
    /// 发现的问题
    pub issues: Vec<QualityIssue>,
    /// 改进建议
    pub recommendations: Vec<String>,
    /// 生成时间
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SourceQualityInfo {
    /// 数据源ID
    pub source_id: String,
    /// 质量指标
    pub metrics: QualityMetrics,
    /// 数据点质量详情
    pub point_quality: Vec<PointQualityInfo>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct QualityMetrics {
    /// 完整性（0-100）
    pub completeness: f64,
    /// 一致性（0-100）
    pub consistency: f64,
    /// 及时性（0-100）
    pub timeliness: f64,
    /// 有效性（0-100）
    pub validity: f64,
    /// 唯一性（0-100）
    pub uniqueness: f64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PointQualityInfo {
    /// 数据点名称
    pub point_name: String,
    /// 质量指标
    pub metrics: QualityMetrics,
    /// 样本数量
    pub sample_count: u64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct QualityIssue {
    /// 问题类型
    pub issue_type: QualityIssueType,
    /// 严重程度
    pub severity: IssueSeverity,
    /// 受影响的数据源
    pub affected_sources: Vec<String>,
    /// 问题描述
    pub description: String,
    /// 发生时间范围
    pub time_range: Option<TimeRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum QualityIssueType {
    MissingData,
    DuplicateData,
    OutOfRange,
    InconsistentFrequency,
    LateData,
    CorruptedData,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum IssueSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

// 合并策略
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum MergeStrategy {
    /// 合并所有结果
    Union,
    /// 交集
    Intersection,
    /// 按时间戳去重
    DeduplicateByTime,
    /// 保留最新
    KeepLatest,
}

// 趋势算法
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrendAlgorithm {
    LinearRegression,
    MovingAverage,
    ExponentialSmoothing,
    PolynomialRegression,
    SeasonalDecomposition,
    ArimaModel,
}