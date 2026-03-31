use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde_json::{json, Value};
use tracing::error;

use crate::{
    auth::{
        create_token_pair, hash_password, verify_access_token, verify_password,
        verify_refresh_token,
    },
    db,
    models::{
        PasswordChange, RefreshTokenRequest, TokenResponse, UserCreate, UserLogin, UserUpdate,
        UserWithRole,
    },
    state::AppState,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn extract_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
        .map(String::from)
}

/// Validate the Authorization header and return the claims.
fn require_auth(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<crate::auth::Claims, (StatusCode, Json<Value>)> {
    let token = extract_token(headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"success": false, "message": "缺少认证令牌"})),
        )
    })?;

    verify_access_token(&token, &state.config.jwt_secret).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"success": false, "message": "令牌无效或已过期"})),
        )
    })
}

fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<crate::auth::Claims, (StatusCode, Json<Value>)> {
    let claims = require_auth(state, headers)?;
    let role = claims.role.as_deref().unwrap_or("");
    if role != "Admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "message": "需要管理员权限"})),
        ));
    }
    Ok(claims)
}

// ── POST /api/v1/auth/register ────────────────────────────────────────────────

#[utoipa::path(post, path = "/api/v1/auth/register", tag = "Auth",
    request_body = UserCreate,
    responses((status = 200, description = "注册成功"), (status = 400, description = "参数错误")))]
pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UserCreate>,
) -> impl IntoResponse {
    if body.username.len() < 3 || body.username.len() > 50 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "用户名长度须为3-50字符"})),
        )
            .into_response();
    }

    // Check duplicate
    match db::get_user_by_username(&state.db, &body.username).await {
        Ok(Some(_)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "用户名已存在"})),
            )
                .into_response();
        },
        Err(e) => {
            error!("DB error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response();
        },
        _ => {},
    }

    let role_id = body.role_id.unwrap_or(3);
    let hash = match hash_password(&body.password) {
        Ok(h) => h,
        Err(e) => {
            error!("bcrypt hash error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response();
        },
    };

    match db::create_user(&state.db, &body.username, &hash, role_id).await {
        Ok(id) => Json(json!({
            "success": true,
            "message": "用户注册成功",
            "data": { "id": id, "username": body.username, "role_id": role_id }
        }))
        .into_response(),
        Err(e) => {
            error!("Create user error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response()
        },
    }
}

// ── POST /api/v1/auth/login ───────────────────────────────────────────────────

#[utoipa::path(post, path = "/api/v1/auth/login", tag = "Auth",
    request_body = UserLogin,
    responses((status = 200, description = "登录成功", body = TokenResponse), (status = 401, description = "认证失败")))]
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UserLogin>,
) -> impl IntoResponse {
    let user = match db::get_user_with_role_by_username(&state.db, &body.username).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return Json(json!({"success": false, "message": "用户名或密码错误"})).into_response();
        },
        Err(e) => {
            error!("DB login error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response();
        },
    };

    if !user.is_active {
        return Json(json!({"success": false, "message": "账户已被禁用"})).into_response();
    }

    let row = match db::get_user_by_username(&state.db, &body.username).await {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response()
        },
    };

    if !verify_password(&body.password, &row.password_hash) {
        return Json(json!({"success": false, "message": "用户名或密码错误"})).into_response();
    }

    let cfg = &state.config;
    match create_token_pair(
        &user,
        &cfg.jwt_secret,
        cfg.access_token_expire_minutes,
        cfg.refresh_token_expire_days,
    ) {
        Ok((tokens, token_id, token_info)) => {
            state.refresh_tokens.insert(token_id, token_info);
            let _ = db::update_user_last_login(&state.db, user.id).await;

            Json(json!({
                "success": true,
                "message": "登录成功",
                "data": {
                    "access_token": tokens.access_token,
                    "refresh_token": tokens.refresh_token,
                    "token_type": tokens.token_type,
                    "expires_in": tokens.expires_in,
                }
            }))
            .into_response()
        },
        Err(e) => {
            error!("Token creation error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response()
        },
    }
}

// ── POST /api/v1/auth/refresh ─────────────────────────────────────────────────

#[utoipa::path(post, path = "/api/v1/auth/refresh", tag = "Auth",
    request_body = RefreshTokenRequest,
    responses((status = 200, description = "刷新成功", body = TokenResponse), (status = 401, description = "Token 无效")))]
pub async fn refresh_token(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RefreshTokenRequest>,
) -> impl IntoResponse {
    let claims = match verify_refresh_token(&body.refresh_token, &state.config.jwt_secret) {
        Some(c) => c,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "刷新令牌无效或已过期"})),
            )
                .into_response();
        },
    };

    let token_id = match &claims.token_id {
        Some(id) => id.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "令牌格式错误"})),
            )
                .into_response();
        },
    };

    // Check token_id is in our store
    let now = Utc::now().timestamp();
    {
        let stored = state.refresh_tokens.get(&token_id);
        match stored {
            None => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"success": false, "message": "刷新令牌已撤销"})),
                )
                    .into_response();
            },
            Some(info) if info.expires_at < now => {
                drop(info);
                state.refresh_tokens.remove(&token_id);
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"success": false, "message": "刷新令牌已过期"})),
                )
                    .into_response();
            },
            _ => {},
        }
    }

    // Revoke old refresh token
    state.refresh_tokens.remove(&token_id);

    // Issue new token pair
    let user = match db::get_user_with_role(&state.db, claims.user_id).await {
        Ok(Some(u)) => u,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response()
        },
    };

    let cfg = &state.config;
    match create_token_pair(
        &user,
        &cfg.jwt_secret,
        cfg.access_token_expire_minutes,
        cfg.refresh_token_expire_days,
    ) {
        Ok((tokens, new_token_id, token_info)) => {
            state.refresh_tokens.insert(new_token_id, token_info);
            Json(json!({
                "success": true,
                "message": "令牌刷新成功",
                "data": {
                    "access_token": tokens.access_token,
                    "refresh_token": tokens.refresh_token,
                    "token_type": tokens.token_type,
                    "expires_in": tokens.expires_in,
                }
            }))
            .into_response()
        },
        Err(e) => {
            error!("Token refresh error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response()
        },
    }
}

// ── POST /api/v1/auth/logout ──────────────────────────────────────────────────

#[utoipa::path(post, path = "/api/v1/auth/logout", tag = "Auth",
    security(("bearer_auth" = [])),
    request_body = RefreshTokenRequest,
    responses((status = 200, description = "退出成功")))]
pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<RefreshTokenRequest>,
) -> impl IntoResponse {
    let _ = require_auth(&state, &headers);

    // Revoke refresh token if valid
    if let Some(claims) = verify_refresh_token(&body.refresh_token, &state.config.jwt_secret) {
        if let Some(token_id) = claims.token_id {
            state.refresh_tokens.remove(&token_id);
        }
    }

    Json(json!({"success": true, "message": "退出登录成功"}))
}

// ── GET /api/v1/auth/me ───────────────────────────────────────────────────────

#[utoipa::path(get, path = "/api/v1/auth/me", tag = "Auth",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "当前用户信息", body = UserWithRole), (status = 401, description = "未认证")))]
pub async fn get_me(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    let claims = match require_auth(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };

    match db::get_user_with_role(&state.db, claims.user_id).await {
        Ok(Some(user)) => Json(json!({
            "success": true,
            "message": "获取用户信息成功",
            "data": user,
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "用户不存在"})),
        )
            .into_response(),
        Err(e) => {
            error!("DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response()
        },
    }
}

// ── PUT /api/v1/auth/me ───────────────────────────────────────────────────────

#[utoipa::path(put, path = "/api/v1/auth/me", tag = "Auth",
    security(("bearer_auth" = [])),
    request_body = UserUpdate,
    responses((status = 200, description = "更新成功"), (status = 401, description = "未认证")))]
pub async fn update_me(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<UserUpdate>,
) -> impl IntoResponse {
    let claims = match require_auth(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };

    let is_admin = claims.role.as_deref() == Some("Admin");
    if !is_admin && (body.role_id.is_some() || body.is_active.is_some()) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "message": "只有管理员可以修改角色和激活状态"})),
        )
            .into_response();
    }

    apply_user_update(&state, claims.user_id, &body).await
}

// ── PUT /api/v1/auth/me/password ──────────────────────────────────────────────

#[utoipa::path(put, path = "/api/v1/auth/me/password", tag = "Auth",
    security(("bearer_auth" = [])),
    request_body = PasswordChange,
    responses((status = 200, description = "密码修改成功"), (status = 401, description = "旧密码错误")))]
pub async fn change_password(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<PasswordChange>,
) -> impl IntoResponse {
    let claims = match require_auth(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };

    apply_password_change(&state, claims.user_id, &body).await
}

// ── GET /api/v1/auth/roles ────────────────────────────────────────────────────

#[utoipa::path(get, path = "/api/v1/auth/roles", tag = "Auth",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "角色列表")))]
pub async fn get_roles(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::get_all_roles(&state.db).await {
        Ok(roles) => Json(json!({
            "success": true,
            "message": "获取角色列表成功",
            "data": roles,
            "total": roles.len(),
        }))
        .into_response(),
        Err(e) => {
            error!("Get roles error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        },
    }
}

// ── GET /api/v1/auth/users ────────────────────────────────────────────────────

#[utoipa::path(get, path = "/api/v1/auth/users", tag = "Auth",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "用户列表（仅管理员）")))]
pub async fn get_all_users(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::get_all_users_with_roles(&state.db).await {
        Ok(users) => {
            // Strip password hashes
            let list: Vec<Value> = users
                .iter()
                .map(|u| {
                    json!({
                        "id": u.id,
                        "username": u.username,
                        "is_active": u.is_active,
                        "last_login": u.last_login,
                        "created_at": u.created_at,
                        "updated_at": u.updated_at,
                        "role": u.role,
                    })
                })
                .collect();

            Json(json!({
                "success": true,
                "message": "获取用户列表成功",
                "data": { "total": list.len(), "list": list }
            }))
            .into_response()
        },
        Err(e) => {
            error!("Get users error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        },
    }
}

// ── GET /api/v1/auth/users/:id (admin) ───────────────────────────────────────

#[utoipa::path(get, path = "/api/v1/auth/users/{id}", tag = "Auth",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "用户 ID")),
    responses((status = 200, description = "用户详情", body = UserWithRole), (status = 404, description = "用户不存在")))]
pub async fn admin_get_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(user_id): Path<i64>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&state, &headers) {
        return e.into_response();
    }

    match db::get_user_with_role(&state.db, user_id).await {
        Ok(Some(user)) => Json(json!({
            "success": true,
            "message": "获取用户信息成功",
            "data": user,
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "用户不存在"})),
        )
            .into_response(),
        Err(e) => {
            error!("DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response()
        },
    }
}

// ── PUT /api/v1/auth/users/:id (admin) ───────────────────────────────────────

#[utoipa::path(put, path = "/api/v1/auth/users/{id}", tag = "Auth",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "用户 ID")),
    request_body = UserUpdate,
    responses((status = 200, description = "更新成功"), (status = 403, description = "权限不足")))]
pub async fn admin_update_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(user_id): Path<i64>,
    Json(body): Json<UserUpdate>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&state, &headers) {
        return e.into_response();
    }

    apply_user_update(&state, user_id, &body).await
}

// ── DELETE /api/v1/auth/users/:id (admin) ────────────────────────────────────

#[utoipa::path(delete, path = "/api/v1/auth/users/{id}", tag = "Auth",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "用户 ID")),
    responses((status = 200, description = "删除成功"), (status = 400, description = "不能删除默认管理员")))]
pub async fn admin_delete_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(user_id): Path<i64>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&state, &headers) {
        return e.into_response();
    }

    match db::get_user_with_role(&state.db, user_id).await {
        Ok(Some(user)) => {
            if user.username == "admin" {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"success": false, "message": "不能删除默认管理员账户"})),
                )
                    .into_response();
            }
            match db::delete_user(&state.db, user_id).await {
                Ok(true) => Json(json!({
                    "success": true,
                    "message": "用户已删除",
                    "data": { "user_id": user_id, "username": user.username }
                }))
                .into_response(),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": "内部服务器错误"})),
                )
                    .into_response(),
            }
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "用户不存在"})),
        )
            .into_response(),
        Err(e) => {
            error!("DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response()
        },
    }
}

// ── GET /api/v1/auth/stats (admin) ───────────────────────────────────────────

#[utoipa::path(get, path = "/api/v1/auth/stats", tag = "Auth",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "认证统计信息")))]
pub async fn get_auth_stats(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&state, &headers) {
        return e.into_response();
    }

    let now = Utc::now().timestamp();
    let active = state.refresh_tokens.len();
    let expired = state
        .refresh_tokens
        .iter()
        .filter(|e| e.expires_at < now)
        .count();

    Json(json!({
        "success": true,
        "message": "获取认证统计成功",
        "data": {
            "active_refresh_tokens": active,
            "expired_tokens": expired,
            "access_token_expire_minutes": state.config.access_token_expire_minutes,
            "refresh_token_expire_days": state.config.refresh_token_expire_days,
        }
    }))
    .into_response()
}

// ── POST /api/v1/auth/cleanup-tokens (admin) ─────────────────────────────────

#[utoipa::path(post, path = "/api/v1/auth/cleanup-tokens", tag = "Auth",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "清理过期 token")))]
pub async fn cleanup_tokens(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&state, &headers) {
        return e.into_response();
    }

    let now = Utc::now().timestamp();
    let expired_ids: Vec<String> = state
        .refresh_tokens
        .iter()
        .filter(|e| e.expires_at < now)
        .map(|e| e.key().clone())
        .collect();

    let count = expired_ids.len();
    for id in expired_ids {
        state.refresh_tokens.remove(&id);
    }

    Json(json!({
        "success": true,
        "message": format!("已清理 {} 个过期令牌", count)
    }))
    .into_response()
}

// ── Shared helpers ────────────────────────────────────────────────────────────

async fn apply_user_update(
    state: &AppState,
    user_id: i64,
    body: &UserUpdate,
) -> axum::response::Response {
    if let Some(role_id) = body.role_id {
        if let Err(e) = db::update_user_role(&state.db, user_id, role_id).await {
            error!("Update role error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response();
        }
    }

    if let Some(is_active) = body.is_active {
        if let Err(e) = db::update_user_active(&state.db, user_id, is_active).await {
            error!("Update active error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response();
        }
    }

    if body.old_password.is_some() || body.new_password.is_some() {
        match (&body.old_password, &body.new_password) {
            (Some(old), Some(new)) => {
                let pw = PasswordChange {
                    old_password: old.clone(),
                    new_password: new.clone(),
                };
                return apply_password_change(state, user_id, &pw).await;
            },
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"success": false, "message": "修改密码需同时提供old_password和new_password"})),
                )
                    .into_response();
            },
        }
    }

    match db::get_user_with_role(&state.db, user_id).await {
        Ok(Some(user)) => Json(json!({
            "success": true,
            "message": "用户信息更新成功",
            "data": user,
        }))
        .into_response(),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": "内部服务器错误"})),
        )
            .into_response(),
    }
}

async fn apply_password_change(
    state: &AppState,
    user_id: i64,
    body: &PasswordChange,
) -> axum::response::Response {
    let row = match db::get_user_by_id(&state.db, user_id).await {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response()
        },
    };

    if !verify_password(&body.old_password, &row.password_hash) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "原密码错误"})),
        )
            .into_response();
    }

    let new_hash = match hash_password(&body.new_password) {
        Ok(h) => h,
        Err(e) => {
            error!("bcrypt error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response();
        },
    };

    match db::update_user_password(&state.db, user_id, &new_hash).await {
        Ok(_) => Json(json!({"success": true, "message": "密码修改成功"})).into_response(),
        Err(e) => {
            error!("Update password error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "内部服务器错误"})),
            )
                .into_response()
        },
    }
}
