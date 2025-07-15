use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::ToSchema;

use crate::api::models_enhanced::{
    AdvancedHistoryQuery, AggregationConfig, EnhancedDataPoint, EnhancedQueryResult,
    QueryMetadata, QueryMode, DataQuality, QualityCode, QualityInfo,
};
use crate::api::models_history::{PaginationInfo, TimeRange};
use crate::error::{HisSrvError, Result};
use crate::storage::StorageManager;

/// 查询优化器
pub struct QueryOptimizer {
    storage_manager: Arc<RwLock<StorageManager>>,
    cache: Arc<RwLock<QueryCache>>,
    stats: Arc<RwLock<QueryStatistics>>,
}

/// 查询计划
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QueryPlan {
    /// 计划ID
    pub plan_id: String,
    /// 查询步骤
    pub steps: Vec<QueryStep>,
    /// 预估成本
    pub estimated_cost: QueryCost,
    /// 优化建议
    pub optimization_hints: Vec<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

/// 查询步骤
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QueryStep {
    /// 步骤ID
    pub step_id: String,
    /// 步骤类型
    pub step_type: StepType,
    /// 数据源
    pub source: QuerySource,
    /// 依赖的步骤
    pub dependencies: Vec<String>,
    /// 预估行数
    pub estimated_rows: u64,
    /// 是否可并行
    pub parallelizable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum StepType {
    Scan,           // 数据扫描
    Filter,         // 过滤
    Aggregate,      // 聚合
    Sort,           // 排序
    Join,           // 连接
    Cache,          // 缓存查询
    Transform,      // 数据转换
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Hash, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum QuerySource {
    Redis,
    InfluxDB,
    Cache,
    Mixed,
}

/// 查询成本
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QueryCost {
    /// CPU成本（相对值）
    pub cpu_cost: f64,
    /// IO成本（相对值）
    pub io_cost: f64,
    /// 网络成本（相对值）
    pub network_cost: f64,
    /// 内存使用（字节）
    pub memory_bytes: u64,
    /// 预估执行时间（毫秒）
    pub estimated_time_ms: u64,
}

/// 查询缓存
struct QueryCache {
    entries: HashMap<String, CacheEntry>,
    max_size: usize,
    ttl_seconds: u64,
}

struct CacheEntry {
    key: String,
    result: EnhancedQueryResult,
    created_at: DateTime<Utc>,
    hit_count: u64,
    size_bytes: usize,
}

/// 查询统计
struct QueryStatistics {
    total_queries: u64,
    cache_hits: u64,
    cache_misses: u64,
    avg_execution_time_ms: f64,
    source_usage: HashMap<QuerySource, u64>,
}

impl QueryOptimizer {
    pub fn new(storage_manager: Arc<RwLock<StorageManager>>) -> Self {
        Self {
            storage_manager,
            cache: Arc::new(RwLock::new(QueryCache {
                entries: HashMap::new(),
                max_size: 1000,
                ttl_seconds: 300,
            })),
            stats: Arc::new(RwLock::new(QueryStatistics {
                total_queries: 0,
                cache_hits: 0,
                cache_misses: 0,
                avg_execution_time_ms: 0.0,
                source_usage: HashMap::new(),
            })),
        }
    }

    /// 创建查询计划
    pub async fn create_plan(
        &self,
        query: &AdvancedHistoryQuery,
        mode: QueryMode,
    ) -> Result<QueryPlan> {
        let plan_id = uuid::Uuid::new_v4().to_string();
        let mut steps = Vec::new();
        let mut optimization_hints = Vec::new();

        // 分析时间范围
        let time_analysis = self.analyze_time_range(&query.time_range);
        
        // 选择数据源
        let source = self.select_source(&time_analysis, &mode);
        
        // 生成扫描步骤
        let scan_step = QueryStep {
            step_id: format!("{}_scan", plan_id),
            step_type: StepType::Scan,
            source: source.clone(),
            dependencies: vec![],
            estimated_rows: self.estimate_scan_rows(&query.time_range, &source).await?,
            parallelizable: true,
        };
        steps.push(scan_step.clone());

        // 生成过滤步骤
        if !query.filters.is_empty() {
            let filter_step = QueryStep {
                step_id: format!("{}_filter", plan_id),
                step_type: StepType::Filter,
                source: source.clone(),
                dependencies: vec![scan_step.step_id.clone()],
                estimated_rows: (scan_step.estimated_rows as f64 * 0.3) as u64, // 估计30%的选择率
                parallelizable: true,
            };
            steps.push(filter_step);
        }

        // 生成聚合步骤
        if let Some(aggregations) = &query.aggregations {
            if !aggregations.is_empty() {
                let agg_step = QueryStep {
                    step_id: format!("{}_aggregate", plan_id),
                    step_type: StepType::Aggregate,
                    source: source.clone(),
                    dependencies: steps.last().map(|s| vec![s.step_id.clone()]).unwrap_or_default(),
                    estimated_rows: self.estimate_aggregation_rows(aggregations, query.group_by.as_ref()),
                    parallelizable: false,
                };
                steps.push(agg_step);
            }
        }

        // 生成排序步骤
        if let Some(order_by) = &query.order_by {
            if !order_by.is_empty() {
                let sort_step = QueryStep {
                    step_id: format!("{}_sort", plan_id),
                    step_type: StepType::Sort,
                    source: source.clone(),
                    dependencies: steps.last().map(|s| vec![s.step_id.clone()]).unwrap_or_default(),
                    estimated_rows: steps.last().map(|s| s.estimated_rows).unwrap_or(0),
                    parallelizable: false,
                };
                steps.push(sort_step);
            }
        }

        // 计算成本
        let estimated_cost = self.calculate_cost(&steps, &source);

        // 生成优化建议
        optimization_hints.extend(self.generate_optimization_hints(&query, &time_analysis));

        Ok(QueryPlan {
            plan_id,
            steps,
            estimated_cost,
            optimization_hints,
            created_at: Utc::now(),
        })
    }

    /// 执行查询计划
    pub async fn execute_plan(
        &self,
        plan: &QueryPlan,
        max_parallelism: u8,
    ) -> Result<EnhancedQueryResult> {
        // 检查缓存
        if let Some(cached_result) = self.check_cache(&plan.plan_id).await {
            self.update_stats(true, 0, &plan.steps[0].source).await;
            return Ok(cached_result);
        }

        let start_time = std::time::Instant::now();
        
        // 执行查询步骤
        let result = self.execute_steps(&plan.steps, max_parallelism).await?;
        
        let execution_time = start_time.elapsed().as_millis() as u64;
        
        // 更新统计信息
        self.update_stats(false, execution_time, &plan.steps[0].source).await;
        
        // 缓存结果
        self.cache_result(&plan.plan_id, &result).await;
        
        Ok(result)
    }

    /// 分析时间范围
    fn analyze_time_range(&self, time_range: &TimeRange) -> TimeRangeAnalysis {
        let duration = time_range.end_time - time_range.start_time;
        let now = Utc::now();
        let age = now - time_range.end_time;
        
        TimeRangeAnalysis {
            duration_hours: duration.num_hours() as u64,
            is_recent: age < Duration::hours(1),
            is_realtime: age < Duration::minutes(5),
            spans_multiple_days: duration.num_days() > 1,
        }
    }

    /// 选择数据源
    fn select_source(&self, analysis: &TimeRangeAnalysis, mode: &QueryMode) -> QuerySource {
        match mode {
            QueryMode::Fast => {
                if analysis.is_realtime {
                    QuerySource::Redis
                } else {
                    QuerySource::Cache
                }
            }
            QueryMode::Accurate => QuerySource::InfluxDB,
            QueryMode::Balanced => {
                if analysis.is_realtime && analysis.duration_hours <= 1 {
                    QuerySource::Redis
                } else if analysis.duration_hours > 24 * 7 {
                    QuerySource::InfluxDB
                } else {
                    QuerySource::Mixed
                }
            }
        }
    }

    /// 估算扫描行数
    async fn estimate_scan_rows(&self, time_range: &TimeRange, source: &QuerySource) -> Result<u64> {
        let duration_seconds = (time_range.end_time - time_range.start_time).num_seconds() as u64;
        
        // 基于数据源和时间范围估算
        let estimated_rows = match source {
            QuerySource::Redis => {
                // Redis通常存储最近的数据，假设每秒10个点
                duration_seconds * 10
            }
            QuerySource::InfluxDB => {
                // InfluxDB存储历史数据，假设每分钟1个点
                duration_seconds / 60
            }
            QuerySource::Cache => {
                // 缓存数据量取决于之前的查询
                1000
            }
            QuerySource::Mixed => {
                // 混合源取平均
                (duration_seconds * 10 + duration_seconds / 60) / 2
            }
        };
        
        Ok(estimated_rows.min(1_000_000)) // 最多100万行
    }

    /// 估算聚合行数
    fn estimate_aggregation_rows(
        &self,
        aggregations: &[AggregationConfig],
        group_by: Option<&Vec<String>>,
    ) -> u64 {
        if let Some(groups) = group_by {
            // 假设每个分组字段有10个唯一值
            10u64.pow(groups.len() as u32).min(10000)
        } else {
            // 没有分组，只返回聚合结果
            aggregations.len() as u64
        }
    }

    /// 计算查询成本
    fn calculate_cost(&self, steps: &[QueryStep], source: &QuerySource) -> QueryCost {
        let mut cpu_cost = 0.0;
        let mut io_cost = 0.0;
        let mut network_cost = 0.0;
        let mut memory_bytes = 0u64;
        
        for step in steps {
            match step.step_type {
                StepType::Scan => {
                    io_cost += step.estimated_rows as f64 * 0.001;
                    match source {
                        QuerySource::Redis => network_cost += step.estimated_rows as f64 * 0.0001,
                        QuerySource::InfluxDB => network_cost += step.estimated_rows as f64 * 0.0002,
                        _ => {}
                    }
                }
                StepType::Filter => {
                    cpu_cost += step.estimated_rows as f64 * 0.0001;
                }
                StepType::Aggregate => {
                    cpu_cost += step.estimated_rows as f64 * 0.001;
                    memory_bytes += step.estimated_rows * 100; // 假设每行100字节
                }
                StepType::Sort => {
                    cpu_cost += step.estimated_rows as f64 * (step.estimated_rows as f64).log2() * 0.0001;
                    memory_bytes += step.estimated_rows * 8; // 排序索引
                }
                _ => {}
            }
        }
        
        let estimated_time_ms = (cpu_cost + io_cost + network_cost) as u64;
        
        QueryCost {
            cpu_cost,
            io_cost,
            network_cost,
            memory_bytes,
            estimated_time_ms,
        }
    }

    /// 生成优化建议
    fn generate_optimization_hints(
        &self,
        query: &AdvancedHistoryQuery,
        analysis: &TimeRangeAnalysis,
    ) -> Vec<String> {
        let mut hints = Vec::new();
        
        // 时间范围建议
        if analysis.duration_hours > 24 * 30 {
            hints.push("考虑缩小查询时间范围以提高性能".to_string());
        }
        
        // 聚合建议
        if let Some(aggregations) = &query.aggregations {
            if aggregations.len() > 5 {
                hints.push("过多的聚合函数可能影响性能，建议减少聚合数量".to_string());
            }
        }
        
        // 分页建议
        if query.pagination.is_none() {
            hints.push("建议添加分页参数以限制返回数据量".to_string());
        }
        
        // 索引建议
        if !query.filters.is_empty() {
            hints.push("确保过滤字段已建立索引".to_string());
        }
        
        hints
    }

    /// 执行查询步骤
    async fn execute_steps(
        &self,
        steps: &[QueryStep],
        max_parallelism: u8,
    ) -> Result<EnhancedQueryResult> {
        // TODO: 实现实际的步骤执行逻辑
        // 这里返回模拟数据
        Ok(EnhancedQueryResult {
            data_points: vec![],
            aggregations: None,
            metadata: QueryMetadata {
                total_points: 0,
                filtered_points: 0,
                data_sources: vec![],
                actual_time_range: None,
                quality_info: Some(QualityInfo {
                    good_ratio: 95.0,
                    missing_ratio: 3.0,
                    bad_ratio: 2.0,
                }),
            },
            pagination: PaginationInfo {
                total_count: 0,
                current_count: 0,
                offset: 0,
                has_more: false,
            },
        })
    }

    /// 检查缓存
    async fn check_cache(&self, key: &str) -> Option<EnhancedQueryResult> {
        let cache = self.cache.read().await;
        if let Some(entry) = cache.entries.get(key) {
            let age = Utc::now() - entry.created_at;
            if age.num_seconds() < cache.ttl_seconds as i64 {
                return Some(entry.result.clone());
            }
        }
        None
    }

    /// 缓存结果
    async fn cache_result(&self, key: &str, result: &EnhancedQueryResult) {
        let mut cache = self.cache.write().await;
        
        // 简单的LRU实现
        if cache.entries.len() >= cache.max_size {
            // 移除最少使用的条目
            if let Some(lru_key) = cache.entries.iter()
                .min_by_key(|(_, entry)| entry.hit_count)
                .map(|(k, _)| k.clone()) {
                cache.entries.remove(&lru_key);
            }
        }
        
        cache.entries.insert(key.to_string(), CacheEntry {
            key: key.to_string(),
            result: result.clone(),
            created_at: Utc::now(),
            hit_count: 0,
            size_bytes: 1000, // TODO: 计算实际大小
        });
    }

    /// 更新统计信息
    async fn update_stats(&self, cache_hit: bool, execution_time: u64, source: &QuerySource) {
        let mut stats = self.stats.write().await;
        stats.total_queries += 1;
        
        if cache_hit {
            stats.cache_hits += 1;
        } else {
            stats.cache_misses += 1;
        }
        
        // 更新平均执行时间
        let n = stats.total_queries as f64;
        stats.avg_execution_time_ms = 
            (stats.avg_execution_time_ms * (n - 1.0) + execution_time as f64) / n;
        
        // 更新数据源使用统计
        *stats.source_usage.entry(source.clone()).or_insert(0) += 1;
    }
}

/// 时间范围分析结果
struct TimeRangeAnalysis {
    duration_hours: u64,
    is_recent: bool,
    is_realtime: bool,
    spans_multiple_days: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_query_optimizer_creation() {
        // TODO: 添加测试
    }
}