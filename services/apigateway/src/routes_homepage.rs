use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
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
                "message": "获取计算点位列表成功",
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
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        },
    }
}

// ── GET /api/v1/homepage/:id ──────────────────────────────────────────────────

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
            "message": "获取计算点位详情成功",
            "data": point,
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": format!("计算点位 ID {} 不存在", point_id)})),
        )
            .into_response(),
        Err(e) => {
            error!("Get point error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        },
    }
}

// ── PUT /api/v1/homepage/:id ──────────────────────────────────────────────────

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
                Json(json!({"success": false, "message": format!("计算点位 ID {} 不存在", point_id)})),
            )
                .into_response();
        },
        Err(e) => {
            error!("DB error: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
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
                "message": "计算点位更新成功",
                "data": updated,
            }))
            .into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Err(e) => {
            error!("Update point error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        },
    }
}

// ── POST /api/v1/homepage/reset ───────────────────────────────────────────────

#[utoipa::path(post, path = "/api/v1/homepage/reset", tag = "Homepage",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "已恢复默认点位")))]
pub async fn reset_points(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::reset_calculated_points(&state.db).await {
        Ok(count) => Json(json!({
            "success": true,
            "message": "已恢复默认设置",
            "data": {
                "imported_count": count,
                "note": "所有自定义点位已被删除，已导入默认点位数据",
            }
        }))
        .into_response(),
        Err(e) => {
            error!("Reset points error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        },
    }
}
