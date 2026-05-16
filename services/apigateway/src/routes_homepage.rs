use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;
use tracing::error;

use crate::{
    db,
    models::{CalculatedPoint, CalculatedPointUpdate},
    state::AppState,
};

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct PointsQuery {
    page: Option<i64>,
    limit: Option<i64>,
    name: Option<String>,
}

// ── GET /api/v1/homepage ──────────────────────────────────────────────────────

/// 列出主页面板配置的点位（分页）。
///
/// "主页点位"是运维在前端主页上拖拽显示的若干关键数值（如总功率、总
/// SOC、并网频率等），通常是基于 instance measurement 计算出来的。每个
/// 点位定义保存在 SQLite，可按 `name` 关键词过滤。
#[utoipa::path(get, path = "/api/v1/homepage", tag = "Homepage",
    security(("bearer_auth" = [])),
    params(PointsQuery),
    responses((status = 200, description = "计算点位列表")))]
pub async fn list_points(
    State(state): State<Arc<AppState>>,
    Query(q): Query<PointsQuery>,
) -> impl IntoResponse {
    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;

    match db::get_all_calculated_points(&state.db, offset, limit, q.name.as_deref()).await {
        Ok((items, total)) => {
            let pages = (total + limit - 1) / limit;
            Json(json!({
                "success": true,
                "message": "Calculated points retrieved",
                "data": {
                    "items": items,
                    "total": total,
                    "page": page,
                    "limit": limit,
                    "pages": pages,
                }
            }))
            .into_response()
        },
        Err(e) => {
            error!("List points error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response()
        },
    }
}

// ── GET /api/v1/homepage/:id ──────────────────────────────────────────────────

/// 查看单个主页点位的完整定义。
///
/// 包含显示名、计算公式 / 取值来源、单位、阈值告警配置等，用于"编辑点
/// 位"对话框预填。404 表示点位 id 不存在。
#[utoipa::path(get, path = "/api/v1/homepage/{id}", tag = "Homepage",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "点位 ID")),
    responses((status = 200, description = "点位详情", body = CalculatedPoint), (status = 404, description = "不存在")))]
pub async fn get_point(
    State(state): State<Arc<AppState>>,
    Path(point_id): Path<i64>,
) -> impl IntoResponse {
    match db::get_calculated_point_by_id(&state.db, point_id).await {
        Ok(Some(point)) => Json(json!({
            "success": true,
            "message": "Calculated point retrieved",
            "data": point,
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": format!("Calculated point ID {} not found", point_id)})),
        )
            .into_response(),
        Err(e) => {
            error!("Get point error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response()
        },
    }
}

// ── PUT /api/v1/homepage/:id ──────────────────────────────────────────────────

/// 修改单个主页点位（部分更新）。
///
/// 所有字段可选，未传字段保留原值。前端拖拽布局、改公式、改阈值都走这
/// 个端点。改动立即生效 —— 下一次前端拉取主页或下一次 WebSocket 推送
/// 就会用新定义。
#[utoipa::path(put, path = "/api/v1/homepage/{id}", tag = "Homepage",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "点位 ID")),
    request_body = CalculatedPointUpdate,
    responses((status = 200, description = "更新成功", body = CalculatedPoint), (status = 404, description = "不存在")))]
pub async fn update_point(
    State(state): State<Arc<AppState>>,
    Path(point_id): Path<i64>,
    Json(body): Json<CalculatedPointUpdate>,
) -> impl IntoResponse {
    // Check existence
    match db::get_calculated_point_by_id(&state.db, point_id).await {
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success": false, "message": format!("Calculated point ID {} not found", point_id)})),
            )
                .into_response();
        },
        Err(e) => {
            error!("DB error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response();
        },
        _ => {},
    }

    match db::update_calculated_point(
        &state.db,
        point_id,
        body.name.as_deref(),
        body.formula.as_deref(),
        body.unit.as_deref(),
        body.imgurl.as_deref(),
        body.description.as_deref(),
    )
    .await
    {
        Ok(_) => match db::get_calculated_point_by_id(&state.db, point_id).await {
            Ok(Some(updated)) => Json(json!({
                "success": true,
                "message": "Calculated point updated",
                "data": updated,
            }))
            .into_response(),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response(),
        },
        Err(e) => {
            error!("Update point error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response()
        },
    }
}

// ── POST /api/v1/homepage/reset ───────────────────────────────────────────────

/// 恢复主页点位到出厂默认配置。
///
/// 清空当前所有点位定义，重新插入项目自带的默认点位（电站总功率、
/// SOC、温度、并网状态等）。运维场景：调乱了想"重置"，或者升级后想拉
/// 取新版默认点位。**破坏性操作**：用户自定义的点位定义会被覆盖，不可
/// 撤销。
#[utoipa::path(post, path = "/api/v1/homepage/reset", tag = "Homepage",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "已恢复默认点位")))]
pub async fn reset_points(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::reset_calculated_points(&state.db).await {
        Ok(count) => Json(json!({
            "success": true,
            "message": "Default settings restored",
            "data": {
                "imported_count": count,
                "note": "所有自定义点位已被删除，已导入默认点位数据",
            }
        }))
        .into_response(),
        Err(e) => {
            error!("Reset points error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response()
        },
    }
}
