use crate::api::models_enhanced::{
    AdvancedHistoryQuery, AggregationConfig, AggregationType, QueryMode,
};
use crate::api::models_history::{OrderBy, OrderDirection, PaginationConfig, TimeRange};
use crate::query_optimizer::{QueryOptimizer, QueryPlan, QuerySource, StepType};
use crate::storage::StorageManager;
use chrono::{Duration, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;

async fn create_test_optimizer() -> QueryOptimizer {
    let storage_manager = Arc::new(RwLock::new(StorageManager::new()));
    QueryOptimizer::new(storage_manager)
}

#[tokio::test]
async fn test_query_plan_creation_basic() {
    let optimizer = create_test_optimizer().await;

    let query = AdvancedHistoryQuery {
        keys: vec!["test.metric".to_string()],
        time_range: TimeRange {
            start_time: Utc::now() - Duration::hours(1),
            end_time: Utc::now(),
        },
        filters: vec![],
        aggregations: None,
        group_by: None,
        order_by: None,
        pagination: None,
        fill_strategy: None,
    };

    let plan = optimizer
        .create_plan(&query, QueryMode::Balanced)
        .await
        .unwrap();

    // 验证计划基本结构
    assert!(!plan.plan_id.is_empty());
    assert!(!plan.steps.is_empty());
    
    // 应该至少有一个扫描步骤
    let scan_step = plan.steps.iter().find(|s| matches!(s.step_type, StepType::Scan));
    assert!(scan_step.is_some());
}

#[tokio::test]
async fn test_query_plan_with_filters() {
    let optimizer = create_test_optimizer().await;

    let query = AdvancedHistoryQuery {
        keys: vec!["test.metric".to_string()],
        time_range: TimeRange {
            start_time: Utc::now() - Duration::hours(1),
            end_time: Utc::now(),
        },
        filters: vec![
            crate::api::models_enhanced::Filter {
                field: "value".to_string(),
                operator: crate::api::models_enhanced::FilterOperator::GreaterThan,
                value: serde_json::json!(100),
            },
        ],
        aggregations: None,
        group_by: None,
        order_by: None,
        pagination: None,
        fill_strategy: None,
    };

    let plan = optimizer
        .create_plan(&query, QueryMode::Balanced)
        .await
        .unwrap();

    // 应该有扫描和过滤步骤
    let has_scan = plan.steps.iter().any(|s| matches!(s.step_type, StepType::Scan));
    let has_filter = plan.steps.iter().any(|s| matches!(s.step_type, StepType::Filter));
    
    assert!(has_scan);
    assert!(has_filter);
}

#[tokio::test]
async fn test_query_plan_with_aggregation() {
    let optimizer = create_test_optimizer().await;

    let query = AdvancedHistoryQuery {
        keys: vec!["test.metric".to_string()],
        time_range: TimeRange {
            start_time: Utc::now() - Duration::hours(24),
            end_time: Utc::now(),
        },
        filters: vec![],
        aggregations: Some(vec![
            AggregationConfig {
                function: AggregationType::Average,
                field: "value".to_string(),
                window: Some("1h".to_string()),
                alias: Some("avg_value".to_string()),
            },
        ]),
        group_by: None,
        order_by: None,
        pagination: None,
        fill_strategy: None,
    };

    let plan = optimizer
        .create_plan(&query, QueryMode::Balanced)
        .await
        .unwrap();

    // 应该有聚合步骤
    let has_aggregate = plan.steps.iter().any(|s| matches!(s.step_type, StepType::Aggregate));
    assert!(has_aggregate);
}

#[tokio::test]
async fn test_query_plan_with_sorting() {
    let optimizer = create_test_optimizer().await;

    let query = AdvancedHistoryQuery {
        keys: vec!["test.metric".to_string()],
        time_range: TimeRange {
            start_time: Utc::now() - Duration::hours(1),
            end_time: Utc::now(),
        },
        filters: vec![],
        aggregations: None,
        group_by: None,
        order_by: Some(vec![OrderBy {
            field: "timestamp".to_string(),
            direction: OrderDirection::Desc,
        }]),
        pagination: None,
        fill_strategy: None,
    };

    let plan = optimizer
        .create_plan(&query, QueryMode::Balanced)
        .await
        .unwrap();

    // 应该有排序步骤
    let has_sort = plan.steps.iter().any(|s| matches!(s.step_type, StepType::Sort));
    assert!(has_sort);
}

#[tokio::test]
async fn test_source_selection_based_on_time_range() {
    let optimizer = create_test_optimizer().await;

    // 测试实时数据 - 应该使用 Redis
    let realtime_query = AdvancedHistoryQuery {
        keys: vec!["test.metric".to_string()],
        time_range: TimeRange {
            start_time: Utc::now() - Duration::minutes(5),
            end_time: Utc::now(),
        },
        filters: vec![],
        aggregations: None,
        group_by: None,
        order_by: None,
        pagination: None,
        fill_strategy: None,
    };

    let plan = optimizer
        .create_plan(&realtime_query, QueryMode::Fast)
        .await
        .unwrap();
    
    if let Some(scan_step) = plan.steps.iter().find(|s| matches!(s.step_type, StepType::Scan)) {
        assert!(matches!(scan_step.source, QuerySource::Redis));
    }

    // 测试历史数据 - 应该使用 InfluxDB
    let historical_query = AdvancedHistoryQuery {
        keys: vec!["test.metric".to_string()],
        time_range: TimeRange {
            start_time: Utc::now() - Duration::days(30),
            end_time: Utc::now() - Duration::days(7),
        },
        filters: vec![],
        aggregations: None,
        group_by: None,
        order_by: None,
        pagination: None,
        fill_strategy: None,
    };

    let plan = optimizer
        .create_plan(&historical_query, QueryMode::Accurate)
        .await
        .unwrap();
    
    if let Some(scan_step) = plan.steps.iter().find(|s| matches!(s.step_type, StepType::Scan)) {
        assert!(matches!(scan_step.source, QuerySource::InfluxDB));
    }
}

#[tokio::test]
async fn test_query_mode_impact_on_source() {
    let optimizer = create_test_optimizer().await;

    let query = AdvancedHistoryQuery {
        keys: vec!["test.metric".to_string()],
        time_range: TimeRange {
            start_time: Utc::now() - Duration::hours(2),
            end_time: Utc::now(),
        },
        filters: vec![],
        aggregations: None,
        group_by: None,
        order_by: None,
        pagination: None,
        fill_strategy: None,
    };

    // Fast 模式应该优先使用缓存或 Redis
    let fast_plan = optimizer.create_plan(&query, QueryMode::Fast).await.unwrap();
    if let Some(scan_step) = fast_plan.steps.iter().find(|s| matches!(s.step_type, StepType::Scan)) {
        assert!(matches!(scan_step.source, QuerySource::Redis | QuerySource::Cache));
    }

    // Accurate 模式应该使用 InfluxDB
    let accurate_plan = optimizer
        .create_plan(&query, QueryMode::Accurate)
        .await
        .unwrap();
    if let Some(scan_step) = accurate_plan.steps.iter().find(|s| matches!(s.step_type, StepType::Scan)) {
        assert!(matches!(scan_step.source, QuerySource::InfluxDB));
    }
}

#[tokio::test]
async fn test_optimization_hints_generation() {
    let optimizer = create_test_optimizer().await;

    // 长时间范围查询
    let long_range_query = AdvancedHistoryQuery {
        keys: vec!["test.metric".to_string()],
        time_range: TimeRange {
            start_time: Utc::now() - Duration::days(60),
            end_time: Utc::now(),
        },
        filters: vec![],
        aggregations: None,
        group_by: None,
        order_by: None,
        pagination: None,
        fill_strategy: None,
    };

    let plan = optimizer
        .create_plan(&long_range_query, QueryMode::Balanced)
        .await
        .unwrap();

    // 应该有关于时间范围的优化建议
    assert!(plan
        .optimization_hints
        .iter()
        .any(|hint| hint.contains("时间范围")));

    // 无分页查询
    assert!(plan
        .optimization_hints
        .iter()
        .any(|hint| hint.contains("分页")));
}

#[tokio::test]
async fn test_cost_estimation() {
    let optimizer = create_test_optimizer().await;

    let simple_query = AdvancedHistoryQuery {
        keys: vec!["test.metric".to_string()],
        time_range: TimeRange {
            start_time: Utc::now() - Duration::minutes(10),
            end_time: Utc::now(),
        },
        filters: vec![],
        aggregations: None,
        group_by: None,
        order_by: None,
        pagination: None,
        fill_strategy: None,
    };

    let complex_query = AdvancedHistoryQuery {
        keys: vec!["test.metric1".to_string(), "test.metric2".to_string()],
        time_range: TimeRange {
            start_time: Utc::now() - Duration::days(7),
            end_time: Utc::now(),
        },
        filters: vec![
            crate::api::models_enhanced::Filter {
                field: "value".to_string(),
                operator: crate::api::models_enhanced::FilterOperator::GreaterThan,
                value: serde_json::json!(100),
            },
        ],
        aggregations: Some(vec![
            AggregationConfig {
                function: AggregationType::Average,
                field: "value".to_string(),
                window: Some("1h".to_string()),
                alias: Some("avg_value".to_string()),
            },
        ]),
        group_by: Some(vec!["tag1".to_string()]),
        order_by: Some(vec![OrderBy {
            field: "timestamp".to_string(),
            direction: OrderDirection::Desc,
        }]),
        pagination: None,
        fill_strategy: None,
    };

    let simple_plan = optimizer
        .create_plan(&simple_query, QueryMode::Balanced)
        .await
        .unwrap();
    let complex_plan = optimizer
        .create_plan(&complex_query, QueryMode::Balanced)
        .await
        .unwrap();

    // 复杂查询的成本应该更高
    assert!(complex_plan.estimated_cost.cpu_cost > simple_plan.estimated_cost.cpu_cost);
    assert!(
        complex_plan.estimated_cost.estimated_time_ms > simple_plan.estimated_cost.estimated_time_ms
    );
}

#[tokio::test]
async fn test_step_dependencies() {
    let optimizer = create_test_optimizer().await;

    let query = AdvancedHistoryQuery {
        keys: vec!["test.metric".to_string()],
        time_range: TimeRange {
            start_time: Utc::now() - Duration::hours(1),
            end_time: Utc::now(),
        },
        filters: vec![
            crate::api::models_enhanced::Filter {
                field: "value".to_string(),
                operator: crate::api::models_enhanced::FilterOperator::GreaterThan,
                value: serde_json::json!(100),
            },
        ],
        aggregations: Some(vec![
            AggregationConfig {
                function: AggregationType::Sum,
                field: "value".to_string(),
                window: Some("5m".to_string()),
                alias: Some("sum_value".to_string()),
            },
        ]),
        group_by: None,
        order_by: Some(vec![OrderBy {
            field: "sum_value".to_string(),
            direction: OrderDirection::Desc,
        }]),
        pagination: None,
        fill_strategy: None,
    };

    let plan = optimizer
        .create_plan(&query, QueryMode::Balanced)
        .await
        .unwrap();

    // 验证步骤依赖关系
    for (i, step) in plan.steps.iter().enumerate() {
        if i > 0 {
            // 除了第一步，每一步都应该依赖前一步
            assert!(!step.dependencies.is_empty());
        } else {
            // 第一步（扫描）不应该有依赖
            assert!(step.dependencies.is_empty());
        }
    }
}

#[tokio::test]
async fn test_parallelizable_steps() {
    let optimizer = create_test_optimizer().await;

    let query = AdvancedHistoryQuery {
        keys: vec!["test.metric1".to_string(), "test.metric2".to_string()],
        time_range: TimeRange {
            start_time: Utc::now() - Duration::hours(1),
            end_time: Utc::now(),
        },
        filters: vec![],
        aggregations: None,
        group_by: None,
        order_by: None,
        pagination: None,
        fill_strategy: None,
    };

    let plan = optimizer
        .create_plan(&query, QueryMode::Balanced)
        .await
        .unwrap();

    // 扫描和过滤步骤应该是可并行的
    for step in &plan.steps {
        match step.step_type {
            StepType::Scan | StepType::Filter => assert!(step.parallelizable),
            StepType::Aggregate | StepType::Sort => assert!(!step.parallelizable),
            _ => {}
        }
    }
}