//! HTTP route handlers for the alarm service

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use chrono::TimeZone;
use serde_json::{json, Value};
use tracing::error;
use utoipa::OpenApi;
use utoipa_swagger_ui::{Config, SwaggerUi};

use crate::db;
use crate::models::{
    AlertEvent, AlertQueryParams, AlertRule, ApiResponse, CreateRuleRequest, EventQueryParams,
    MonitorStatus, PagedData, RuleQueryParams, UpdateRuleRequest,
};
use crate::monitor;
use crate::state::AppState;

// ============================================================================
// Router
// ============================================================================

pub fn create_routes(state: Arc<AppState>) -> Router {
    let api = Router::new()
        // Service meta
        .route("/", get(service_info))
        .route("/health", get(health))
        // Rules
        .route("/alarmApi/rules", get(list_rules).post(create_rule))
        .route("/alarmApi/rules/channel/{channel_id}", get(rules_by_channel))
        .route(
            "/alarmApi/rules/{id}",
            get(get_rule)
                .put(update_rule)
                .delete(delete_rule),
        )
        .route("/alarmApi/rules/{id}/enable", patch(enable_rule))
        .route("/alarmApi/rules/{id}/disable", patch(disable_rule))
        // Alerts
        .route("/alarmApi/alerts", get(list_alerts))
        .route("/alarmApi/alerts/{id}", get(get_alert))
        .route("/alarmApi/alerts/{id}/resolve", patch(resolve_alert))
        // Alert events
        .route("/alarmApi/alert-events", get(list_events))
        .route("/alarmApi/alert-events/export", get(export_events_csv))
        // Statistics & monitor
        .route("/alarmApi/alert-statistics", get(alert_statistics))
        .route("/alarmApi/monitor/status", get(monitor_status))
        .route("/alarmApi/monitor/check-rule/{id}", post(manual_check_rule))
        .route("/alarmApi/call-data", post(call_data))
        .with_state(state);

    api.merge(
        SwaggerUi::new("/docs")
            .url("/openapi.json", ApiDoc::openapi())
            .config(
                Config::default()
                    .default_model_rendering("model")
                    .default_models_expand_depth(1),
            ),
    )
}

// ============================================================================
// OpenAPI document
// ============================================================================

#[derive(OpenApi)]
#[openapi(
    paths(
        service_info,
        health,
        list_rules,
        create_rule,
        get_rule,
        update_rule,
        delete_rule,
        enable_rule,
        disable_rule,
        rules_by_channel,
        list_alerts,
        get_alert,
        resolve_alert,
        list_events,
        export_events_csv,
        alert_statistics,
        monitor_status,
        manual_check_rule,
        call_data,
    ),
    components(schemas(
        AlertRule,
        crate::models::Alert,
        AlertEvent,
        CreateRuleRequest,
        UpdateRuleRequest,
        MonitorStatus,
    )),
    tags(
        (name = "Rules",   description = "告警规则 CRUD"),
        (name = "Alerts",  description = "活跃告警查询与解除"),
        (name = "Events",  description = "告警事件历史与导出"),
        (name = "Monitor", description = "监控状态与手动触发"),
        (name = "Meta",    description = "服务信息"),
    ),
    info(title = "VoltageEMS Alarm Service", version = "1.0.0",
         description = "告警规则管理、活跃告警监控、事件历史查询")
)]
pub struct ApiDoc;

// ============================================================================
// Service meta
// ============================================================================

#[utoipa::path(get, path = "/", tag = "Meta",
    responses((status = 200, description = "服务基本信息")))]
async fn service_info() -> Json<Value> {
    Json(json!({
        "name": "alarmsrv",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "VoltageEMS alarm service (Rust)",
    }))
}

#[utoipa::path(get, path = "/health", tag = "Meta",
    responses((status = 200, description = "健康检查")))]
async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

// ============================================================================
// Alert rules
// ============================================================================

#[utoipa::path(get, path = "/alarmApi/rules", tag = "Rules",
    params(RuleQueryParams),
    responses(
        (status = 200, description = "规则列表"),
    ))]
async fn list_rules(
    State(state): State<Arc<AppState>>,
    Query(params): Query<RuleQueryParams>,
) -> impl IntoResponse {
    match db::list_rules(&state.db, &params).await {
        Ok((total, list)) => {
            let msg = format!("查询成功，共找到 {} 条规则", total);
            Json(ApiResponse::ok(msg, PagedData { total, list })).into_response()
        },
        Err(e) => {
            error!("list_rules: {}", e);
            server_error("查询规则失败")
        },
    }
}

#[utoipa::path(post, path = "/alarmApi/rules", tag = "Rules",
    request_body = CreateRuleRequest,
    responses(
        (status = 201, description = "规则创建成功", body = AlertRule),
        (status = 400, description = "无效的运算符"),
    ))]
async fn create_rule(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRuleRequest>,
) -> impl IntoResponse {
    if !is_valid_operator(&req.operator) {
        return bad_request("无效的运算符，支持: >, <, >=, <=, ==, !=");
    }

    match db::insert_rule(
        &state.db,
        &req.service_type,
        req.channel_id,
        &req.data_type,
        req.point_id,
        &req.rule_name,
        req.warning_level,
        &req.operator,
        req.value,
        req.enabled,
        req.description.as_deref(),
    )
    .await
    {
        Ok(id) => {
            let rule = db::get_rule_by_id(&state.db, id).await.ok().flatten();
            (
                StatusCode::CREATED,
                Json(ApiResponse::ok(
                    "创建规则成功",
                    json!({ "id": id, "rule": rule }),
                )),
            )
                .into_response()
        },
        Err(e) => {
            error!("create_rule: {}", e);
            server_error("创建规则失败")
        },
    }
}

#[utoipa::path(get, path = "/alarmApi/rules/{id}", tag = "Rules",
    params(("id" = i64, Path, description = "规则 ID")),
    responses(
        (status = 200, description = "规则详情", body = AlertRule),
        (status = 404, description = "规则不存在"),
    ))]
async fn get_rule(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    match db::get_rule_by_id(&state.db, id).await {
        Ok(Some(rule)) => {
            Json(ApiResponse::ok("获取规则成功", json!({ "rule": rule }))).into_response()
        },
        Ok(None) => not_found("规则不存在"),
        Err(e) => {
            error!("get_rule: {}", e);
            server_error("获取规则失败")
        },
    }
}

#[utoipa::path(put, path = "/alarmApi/rules/{id}", tag = "Rules",
    params(("id" = i64, Path, description = "规则 ID")),
    request_body = UpdateRuleRequest,
    responses(
        (status = 200, description = "规则更新成功", body = AlertRule),
        (status = 400, description = "无效运算符"),
        (status = 404, description = "规则不存在"),
    ))]
async fn update_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateRuleRequest>,
) -> impl IntoResponse {
    if let Some(ref op) = req.operator {
        if !is_valid_operator(op) {
            return bad_request("无效的运算符");
        }
    }

    match db::update_rule(
        &state.db,
        id,
        req.service_type.as_deref(),
        req.channel_id,
        req.data_type.as_deref(),
        req.point_id,
        req.rule_name.as_deref(),
        req.warning_level,
        req.operator.as_deref(),
        req.value,
        req.enabled,
        req.description.as_deref().map(Some),
    )
    .await
    {
        Ok(true) => {
            monitor::on_rule_updated(&state, id).await;
            let rule = db::get_rule_by_id(&state.db, id).await.ok().flatten();
            Json(ApiResponse::ok("更新规则成功", json!({ "rule": rule }))).into_response()
        },
        Ok(false) => not_found("规则不存在"),
        Err(e) => {
            error!("update_rule: {}", e);
            server_error("更新规则失败")
        },
    }
}

#[utoipa::path(delete, path = "/alarmApi/rules/{id}", tag = "Rules",
    params(("id" = i64, Path, description = "规则 ID")),
    responses(
        (status = 200, description = "规则删除成功"),
        (status = 404, description = "规则不存在"),
    ))]
async fn delete_rule(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let rule = match db::get_rule_by_id(&state.db, id).await {
        Ok(Some(r)) => r,
        Ok(None) => return not_found("规则不存在"),
        Err(e) => {
            error!("delete_rule fetch: {}", e);
            return server_error("删除规则失败");
        },
    };

    monitor::on_rule_deleted(&state, &rule).await;

    match db::delete_rule(&state.db, id).await {
        Ok(true) => Json(ApiResponse::ok("删除规则成功", json!({ "id": id }))).into_response(),
        Ok(false) => not_found("规则不存在"),
        Err(e) => {
            error!("delete_rule: {}", e);
            server_error("删除规则失败")
        },
    }
}

#[utoipa::path(patch, path = "/alarmApi/rules/{id}/enable", tag = "Rules",
    params(("id" = i64, Path, description = "规则 ID")),
    responses(
        (status = 200, description = "规则已启用"),
        (status = 404, description = "规则不存在"),
    ))]
async fn enable_rule(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    match db::set_rule_enabled(&state.db, id, true).await {
        Ok(true) => Json(ApiResponse::ok(
            "规则已启用",
            json!({ "id": id, "enabled": true }),
        ))
        .into_response(),
        Ok(false) => not_found("规则不存在"),
        Err(e) => {
            error!("enable_rule: {}", e);
            server_error("启用规则失败")
        },
    }
}

#[utoipa::path(patch, path = "/alarmApi/rules/{id}/disable", tag = "Rules",
    params(("id" = i64, Path, description = "规则 ID")),
    responses(
        (status = 200, description = "规则已禁用"),
        (status = 404, description = "规则不存在"),
    ))]
async fn disable_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match db::set_rule_enabled(&state.db, id, false).await {
        Ok(true) => {
            monitor::on_rule_updated(&state, id).await;
            Json(ApiResponse::ok(
                "规则已禁用",
                json!({ "id": id, "enabled": false }),
            ))
            .into_response()
        },
        Ok(false) => not_found("规则不存在"),
        Err(e) => {
            error!("disable_rule: {}", e);
            server_error("禁用规则失败")
        },
    }
}

#[utoipa::path(get, path = "/alarmApi/rules/channel/{channel_id}", tag = "Rules",
    params(("channel_id" = i64, Path, description = "通道 ID")),
    responses((status = 200, description = "该通道下的规则列表")))]
async fn rules_by_channel(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<i64>,
) -> impl IntoResponse {
    match db::get_rules_by_channel(&state.db, channel_id).await {
        Ok(list) => {
            let total = list.len() as i64;
            Json(ApiResponse::ok(
                format!("查询成功，共找到 {} 条规则", total),
                PagedData { total, list },
            ))
            .into_response()
        },
        Err(e) => {
            error!("rules_by_channel: {}", e);
            server_error("查询规则失败")
        },
    }
}

// ============================================================================
// Alerts
// ============================================================================

#[utoipa::path(get, path = "/alarmApi/alerts", tag = "Alerts",
    params(AlertQueryParams),
    responses((status = 200, description = "活跃告警列表")))]
async fn list_alerts(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AlertQueryParams>,
) -> impl IntoResponse {
    match db::list_alerts(&state.db, &params).await {
        Ok((total, list)) => Json(ApiResponse::ok(
            format!("查询成功，共找到 {} 条活跃告警", total),
            PagedData { total, list },
        ))
        .into_response(),
        Err(e) => {
            error!("list_alerts: {}", e);
            server_error("查询告警失败")
        },
    }
}

#[utoipa::path(get, path = "/alarmApi/alerts/{id}", tag = "Alerts",
    params(("id" = i64, Path, description = "告警 ID")),
    responses(
        (status = 200, description = "告警详情", body = crate::models::Alert),
        (status = 404, description = "告警不存在"),
    ))]
async fn get_alert(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    match db::get_alert_by_id(&state.db, id).await {
        Ok(Some(alert)) => {
            Json(ApiResponse::ok("获取告警成功", json!({ "alert": alert }))).into_response()
        },
        Ok(None) => not_found("告警不存在"),
        Err(e) => {
            error!("get_alert: {}", e);
            server_error("获取告警失败")
        },
    }
}

#[utoipa::path(patch, path = "/alarmApi/alerts/{id}/resolve", tag = "Alerts",
    params(("id" = i64, Path, description = "告警 ID")),
    responses(
        (status = 200, description = "告警已手动解除"),
        (status = 404, description = "告警不存在"),
    ))]
async fn resolve_alert(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let alert = match db::get_alert_by_id(&state.db, id).await {
        Ok(Some(a)) => a,
        Ok(None) => return not_found("告警不存在"),
        Err(e) => {
            error!("resolve_alert fetch: {}", e);
            return server_error("解除告警失败");
        },
    };

    let recovery_value = alert.current_value;
    let rule_id = alert.rule_id;

    match db::resolve_alert(&state.db, &alert, recovery_value).await {
        Ok(_) => {
            if let Ok(Some(rule)) = db::get_rule_by_id(&state.db, rule_id).await {
                state
                    .broadcaster
                    .send_alarm_recovery(id, &rule, Some(recovery_value), "手动解除")
                    .await;
            }
            if let Ok(counts) = db::get_active_alarm_counts(&state.db).await {
                state.broadcaster.send_alarm_count(&counts).await;
            }
            Json(ApiResponse::ok("告警已解除", json!({ "id": id }))).into_response()
        },
        Err(e) => {
            error!("resolve_alert: {}", e);
            server_error("解除告警失败")
        },
    }
}

// ============================================================================
// Alert events
// ============================================================================

#[utoipa::path(get, path = "/alarmApi/alert-events", tag = "Events",
    params(EventQueryParams),
    responses((status = 200, description = "告警事件历史列表")))]
async fn list_events(
    State(state): State<Arc<AppState>>,
    Query(params): Query<EventQueryParams>,
) -> impl IntoResponse {
    match db::list_events(&state.db, &params).await {
        Ok((total, list)) => Json(ApiResponse::ok(
            format!("查询成功，共找到 {} 条记录", total),
            PagedData { total, list },
        ))
        .into_response(),
        Err(e) => {
            error!("list_events: {}", e);
            server_error("查询告警事件失败")
        },
    }
}

#[utoipa::path(get, path = "/alarmApi/alert-events/export", tag = "Events",
    params(EventQueryParams),
    responses(
        (status = 200, description = "返回 CSV 文件流",
         content_type = "text/csv"),
    ))]
async fn export_events_csv(
    State(state): State<Arc<AppState>>,
    Query(params): Query<EventQueryParams>,
) -> impl IntoResponse {
    let events = match db::get_all_events_for_export(&state.db, &params).await {
        Ok(e) => e,
        Err(e) => {
            error!("export_events_csv: {}", e);
            return server_error("导出失败");
        },
    };

    let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);

    // Header
    let _ = wtr.write_record([
        "Event ID",
        "Rule ID",
        "Rule Name",
        "Service Type",
        "Channel ID",
        "Data Type",
        "Point ID",
        "Warning Level",
        "Operator",
        "Threshold",
        "Trigger Value",
        "Recovery Value",
        "Event Type",
        "Triggered At",
        "Recovered At",
        "Duration (Seconds)",
    ]);

    for ev in &events {
        let triggered_str = ev.triggered_at.map(format_timestamp).unwrap_or_default();
        let recovered_str = ev.recovered_at.map(format_timestamp).unwrap_or_default();
        let duration_str = ev.duration.map(|d| d.to_string()).unwrap_or_default();

        let _ = wtr.write_record(&[
            ev.id.to_string(),
            ev.rule_id.to_string(),
            ev.rule_name.clone(),
            ev.service_type.clone(),
            ev.channel_id.to_string(),
            ev.data_type.clone(),
            ev.point_id.to_string(),
            ev.warning_level.to_string(),
            ev.operator.clone(),
            ev.threshold_value.to_string(),
            ev.trigger_value.map(|v| v.to_string()).unwrap_or_default(),
            ev.recovery_value.map(|v| v.to_string()).unwrap_or_default(),
            ev.event_type.clone(),
            triggered_str,
            recovered_str,
            duration_str,
        ]);
    }

    match wtr.into_inner() {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"alert_events.csv\"",
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(e) => {
            error!("csv flush: {}", e);
            server_error("导出失败")
        },
    }
}

// ============================================================================
// Statistics & monitor
// ============================================================================

#[utoipa::path(get, path = "/alarmApi/alert-statistics", tag = "Monitor",
    responses((status = 200, description = "告警统计数据")))]
async fn alert_statistics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::get_statistics(&state.db).await {
        Ok(stats) => Json(ApiResponse::ok("统计数据获取成功", stats)).into_response(),
        Err(e) => {
            error!("alert_statistics: {}", e);
            server_error("获取统计信息失败")
        },
    }
}

#[utoipa::path(get, path = "/alarmApi/monitor/status", tag = "Monitor",
    responses((status = 200, description = "监控循环运行状态", body = MonitorStatus)))]
async fn monitor_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let ms = state.monitor_status.read().await.clone();
    Json(ApiResponse::ok(
        "监控状态获取成功",
        json!({
            "running": ms.running,
            "last_check_time": ms.last_check_time,
            "check_interval": ms.check_interval,
            "redis_url": ms.redis_url,
        }),
    ))
}

#[utoipa::path(post, path = "/alarmApi/monitor/check-rule/{id}", tag = "Monitor",
    params(("id" = i64, Path, description = "规则 ID")),
    responses(
        (status = 200, description = "手动检查结果"),
        (status = 404, description = "规则不存在"),
    ))]
async fn manual_check_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match monitor::manual_check_rule(&state, id).await {
        Ok(result) => Json(result).into_response(),
        Err(e) => {
            error!("manual_check_rule: {}", e);
            Json(json!({
                "success": false,
                "message": format!("检查失败: {}", e),
                "data": {},
            }))
            .into_response()
        },
    }
}

#[utoipa::path(post, path = "/alarmApi/call-data", tag = "Monitor",
    responses((status = 200, description = "广播当前所有活跃告警")))]
async fn call_data(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let alerts = match db::get_all_active_alerts(&state.db).await {
        Ok(a) => a,
        Err(e) => {
            error!("call_data get alerts: {}", e);
            return server_error("获取告警失败");
        },
    };

    if alerts.is_empty() {
        if let Ok(counts) = db::get_active_alarm_counts(&state.db).await {
            state.broadcaster.send_alarm_count(&counts).await;
        }
        return Json(ApiResponse::ok(
            "当前没有活动告警",
            json!({ "broadcast_count": 0, "alarm_count": 0 }),
        ))
        .into_response();
    }

    let mut rule_map: HashMap<i64, crate::models::AlertRule> = HashMap::new();
    for alert in &alerts {
        if !rule_map.contains_key(&alert.rule_id) {
            if let Ok(Some(rule)) = db::get_rule_by_id(&state.db, alert.rule_id).await {
                rule_map.insert(rule.id, rule);
            }
        }
    }

    let alarm_count = alerts.len();
    state
        .broadcaster
        .broadcast_active_alerts(&alerts, &rule_map)
        .await;

    if let Ok(counts) = db::get_active_alarm_counts(&state.db).await {
        state.broadcaster.send_alarm_count(&counts).await;
    }

    Json(ApiResponse::ok(
        format!("广播完成，共 {} 条告警", alarm_count),
        json!({
            "broadcast_count": alarm_count,
            "alarm_count": alarm_count,
        }),
    ))
    .into_response()
}

// ============================================================================
// Helpers
// ============================================================================

fn is_valid_operator(op: &str) -> bool {
    matches!(op, ">" | "<" | ">=" | "<=" | "==" | "!=")
}

fn format_timestamp(ts: i64) -> String {
    chrono::Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

fn not_found(msg: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "success": false, "message": msg, "data": null })),
    )
        .into_response()
}

fn bad_request(msg: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "success": false, "message": msg, "data": null })),
    )
        .into_response()
}

fn server_error(msg: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "message": msg, "data": null })),
    )
        .into_response()
}
