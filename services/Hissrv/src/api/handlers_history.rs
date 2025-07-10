use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::api::{models::ErrorResponse, models_history::*, AppState};

#[derive(Deserialize, ToSchema, IntoParams)]
pub struct SourceQuery {
    /// 数据源ID模式匹配
    pattern: Option<String>,
    /// 是否包含详细信息
    include_details: Option<bool>,
}

#[derive(Deserialize, ToSchema, IntoParams)]
pub struct StatisticsQuery {
    /// 开始时间
    start_time: chrono::DateTime<Utc>,
    /// 结束时间  
    end_time: chrono::DateTime<Utc>,
    /// 统计粒度 (hour, day, week, month)
    granularity: Option<String>,
    /// 数据源过滤
    sources: Option<Vec<String>>,
}

/// 查询历史数据
#[utoipa::path(
    get,
    path = "/history/query",
    tag = "history",
    params(HistoryQueryFilter),
    responses(
        (status = 200, description = "查询成功", body = HistoryApiResponse<HistoryQueryResult>),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    )
)]
pub async fn query_history(
    State(state): State<AppState>,
    Query(filter): Query<HistoryQueryFilter>,
) -> Result<Json<HistoryApiResponse<HistoryQueryResult>>, (StatusCode, Json<ErrorResponse>)> {
    let request_id = Uuid::new_v4().to_string();
    let start_time = std::time::Instant::now();

    // 验证查询参数
    if filter.start_time >= filter.end_time {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "开始时间必须早于结束时间".to_string(),
                code: "INVALID_TIME_RANGE".to_string(),
                timestamp: Utc::now(),
            }),
        ));
    }

    // 限制查询时间范围（防止查询过大的数据集）
    let duration = filter.end_time - filter.start_time;
    if duration.num_days() > 365 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "查询时间范围不能超过365天".to_string(),
                code: "TIME_RANGE_TOO_LARGE".to_string(),
                timestamp: Utc::now(),
            }),
        ));
    }

    let _storage_manager = state.storage_manager.read().await;

    // 这里应该实现实际的历史数据查询逻辑
    // 由于当前存储接口还需要调整，先返回示例数据
    let mock_result = create_mock_history_result(&filter, &request_id);

    let _execution_time = start_time.elapsed().as_millis() as u64;

    let mut response = HistoryApiResponse::success(mock_result);
    response.request_id = Some(request_id);

    Ok(Json(response))
}

/// 获取数据源列表
#[utoipa::path(
    get,
    path = "/history/sources",
    tag = "history",
    params(SourceQuery),
    responses(
        (status = 200, description = "获取成功", body = HistoryApiResponse<Vec<DataSourceInfo>>),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    )
)]
pub async fn get_data_sources(
    State(state): State<AppState>,
    Query(query): Query<SourceQuery>,
) -> Result<Json<HistoryApiResponse<Vec<DataSourceInfo>>>, (StatusCode, Json<ErrorResponse>)> {
    let _storage_manager = state.storage_manager.read().await;

    // 这里应该从存储后端获取实际的数据源信息
    let mock_sources = create_mock_data_sources(&query);

    Ok(Json(HistoryApiResponse::success(mock_sources)))
}

/// 获取特定数据源的详细信息
#[utoipa::path(
    get,
    path = "/history/sources/{source_id}",
    tag = "history",
    params(
        ("source_id" = String, Path, description = "数据源ID")
    ),
    responses(
        (status = 200, description = "获取成功", body = HistoryApiResponse<DataSourceInfo>),
        (status = 404, description = "数据源不存在", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    )
)]
pub async fn get_data_source_detail(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<Json<HistoryApiResponse<DataSourceInfo>>, (StatusCode, Json<ErrorResponse>)> {
    let _storage_manager = state.storage_manager.read().await;

    // 检查数据源是否存在
    let mock_source = create_mock_data_source(&source_id);

    Ok(Json(HistoryApiResponse::success(mock_source)))
}

/// 获取时间序列统计信息
#[utoipa::path(
    get,
    path = "/history/statistics",
    tag = "analytics",
    params(StatisticsQuery),
    responses(
        (status = 200, description = "统计信息获取成功", body = HistoryApiResponse<TimeSeriesStatistics>),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    )
)]
pub async fn get_statistics(
    State(state): State<AppState>,
    Query(query): Query<StatisticsQuery>,
) -> Result<Json<HistoryApiResponse<TimeSeriesStatistics>>, (StatusCode, Json<ErrorResponse>)> {
    let _storage_manager = state.storage_manager.read().await;

    let mock_stats = create_mock_statistics(&query);

    Ok(Json(HistoryApiResponse::success(mock_stats)))
}

/// 创建数据导出任务
#[utoipa::path(
    post,
    path = "/history/export",
    tag = "export",
    request_body = ExportRequest,
    responses(
        (status = 202, description = "导出任务已创建", body = HistoryApiResponse<ExportJob>),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    )
)]
pub async fn create_export_job(
    State(_state): State<AppState>,
    Json(request): Json<ExportRequest>,
) -> Result<Json<HistoryApiResponse<ExportJob>>, (StatusCode, Json<ErrorResponse>)> {
    let job_id = Uuid::new_v4().to_string();

    // 验证导出请求
    if !["csv", "json", "parquet"].contains(&request.format.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "不支持的导出格式".to_string(),
                code: "UNSUPPORTED_FORMAT".to_string(),
                timestamp: Utc::now(),
            }),
        ));
    }

    let export_job = ExportJob {
        job_id: job_id.clone(),
        status: "pending".to_string(),
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        file_info: None,
        progress: Some(0.0),
        error_message: None,
    };

    // TODO: 实际的导出任务应该异步执行

    Ok(Json(HistoryApiResponse::success(export_job)))
}

/// 获取导出任务状态
#[utoipa::path(
    get,
    path = "/history/export/{job_id}",
    tag = "export",
    params(
        ("job_id" = String, Path, description = "导出任务ID")
    ),
    responses(
        (status = 200, description = "任务状态获取成功", body = HistoryApiResponse<ExportJob>),
        (status = 404, description = "任务不存在", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    )
)]
pub async fn get_export_job_status(
    State(_state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<Json<HistoryApiResponse<ExportJob>>, (StatusCode, Json<ErrorResponse>)> {
    // TODO: 从任务队列或数据库中获取真实的任务状态
    let mock_job = ExportJob {
        job_id: job_id.clone(),
        status: "completed".to_string(),
        created_at: Utc::now() - chrono::Duration::minutes(5),
        started_at: Some(Utc::now() - chrono::Duration::minutes(4)),
        completed_at: Some(Utc::now() - chrono::Duration::minutes(1)),
        file_info: Some(ExportFileInfo {
            filename: format!("export_{}.csv", job_id),
            file_size: 1024000,
            download_url: format!("/api/v1/history/export/{}/download", job_id),
            expires_at: Utc::now() + chrono::Duration::hours(24),
        }),
        progress: Some(100.0),
        error_message: None,
    };

    Ok(Json(HistoryApiResponse::success(mock_job)))
}

// Mock 数据创建函数（在实际实现中应该替换为真实的数据库查询）
fn create_mock_history_result(
    filter: &HistoryQueryFilter,
    _request_id: &str,
) -> HistoryQueryResult {
    let time_range = TimeRange {
        start_time: filter.start_time,
        end_time: filter.end_time,
        duration_seconds: (filter.end_time - filter.start_time).num_seconds() as u64,
    };

    let query_summary = QuerySummary {
        time_range,
        source_count: 1,
        point_count: 100,
        execution_time_ms: 50,
    };

    let data_points = vec![HistoryDataPoint {
        timestamp: filter.start_time,
        source_id: "device_001".to_string(),
        point_name: "temperature".to_string(),
        value: HistoryValue::Numeric(25.5),
        quality: Some("good".to_string()),
        tags: HashMap::from([
            ("location".to_string(), "room_a".to_string()),
            ("unit".to_string(), "celsius".to_string()),
        ]),
    }];

    HistoryQueryResult {
        query_summary,
        data_points,
        aggregated_data: None,
        pagination: PaginationInfo {
            total_count: 100,
            current_count: 1,
            offset: 0,
            has_more: true,
        },
    }
}

fn create_mock_data_sources(_query: &SourceQuery) -> Vec<DataSourceInfo> {
    vec![DataSourceInfo {
        source_id: "device_001".to_string(),
        source_name: Some("温度传感器001".to_string()),
        points: vec![DataPointInfo {
            point_name: "temperature".to_string(),
            data_type: "float".to_string(),
            latest_value: Some(HistoryValue::Numeric(25.5)),
            latest_timestamp: Some(Utc::now()),
            count: 1000,
        }],
        first_data_time: Some(Utc::now() - chrono::Duration::days(30)),
        last_data_time: Some(Utc::now()),
        total_points: 1000,
    }]
}

fn create_mock_data_source(source_id: &str) -> DataSourceInfo {
    DataSourceInfo {
        source_id: source_id.to_string(),
        source_name: Some(format!("数据源 {}", source_id)),
        points: vec![],
        first_data_time: Some(Utc::now() - chrono::Duration::days(30)),
        last_data_time: Some(Utc::now()),
        total_points: 1000,
    }
}

fn create_mock_statistics(query: &StatisticsQuery) -> TimeSeriesStatistics {
    let time_range = TimeRange {
        start_time: query.start_time,
        end_time: query.end_time,
        duration_seconds: (query.end_time - query.start_time).num_seconds() as u64,
    };

    TimeSeriesStatistics {
        time_range,
        sources: vec![SourceStatistics {
            source_id: "device_001".to_string(),
            point_count: 1000,
            avg_sample_rate: 60.0,
            data_completeness: 98.5,
        }],
        overall: OverallStatistics {
            total_sources: 1,
            total_points: 1000,
            avg_data_density: 60.0,
            storage_usage: StorageUsage {
                total_size_bytes: 1024000,
                by_backend: HashMap::from([
                    ("influxdb".to_string(), 800000),
                    ("redis".to_string(), 224000),
                ]),
                compression_ratio: Some(0.75),
            },
        },
    }
}
