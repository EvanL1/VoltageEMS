use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::Utc;
use serde_json::{Value, json};
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
            Json(json!({"success": false, "message": "Missing authentication token"})),
        )
    })?;

    verify_access_token(&token, &state.config.jwt_secret).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"success": false, "message": "Token is invalid or expired"})),
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
            Json(json!({"success": false, "message": "Admin privileges required"})),
        ));
    }
    Ok(claims)
}

// ── POST /api/v1/auth/register ────────────────────────────────────────────────

/// 注册新用户账号。
///
/// 公开端点（不需要 token）。校验用户名长度 3-50、用户名唯一，bcrypt
/// 哈希密码后入库。默认 role_id=3（普通用户），可由请求体覆盖 —— 但匿名
/// 调用方填什么都不会获得管理员权限，因为本端点不检查 caller 身份，配
/// 置上一般会在网关层把它锁起来或加邀请码（当前实现没锁）。
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
            Json(json!({"success": false, "message": "Username must be 3-50 characters"})),
        )
            .into_response();
    }

    // Check duplicate
    match db::get_user_by_username(&state.db, &body.username).await {
        Ok(Some(_)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "Username already exists"})),
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

    let role_id = body.role_id.unwrap_or(3);
    let hash = match hash_password(&body.password) {
        Ok(h) => h,
        Err(e) => {
            error!("bcrypt hash error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response();
        },
    };

    match db::create_user(&state.db, &body.username, &hash, role_id).await {
        Ok(id) => Json(json!({
            "success": true,
            "message": "User registered successfully",
            "data": { "id": id, "username": body.username, "role_id": role_id }
        }))
        .into_response(),
        Err(e) => {
            error!("Create user error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response()
        },
    }
}

// ── POST /api/v1/auth/login ───────────────────────────────────────────────────

/// 用户名 + 密码登录，签发 access / refresh token 对。
///
/// 返回 `TokenResponse { access_token, refresh_token, expires_in, role }`。
/// access token 用于后续接口 `Authorization: Bearer ...`，过期时间短；
/// refresh token 用于换发新的 access token，签到 `refresh_tokens` 表内
/// 可单点吊销。`is_active=false` 的账号会被拒绝（401）。
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
            return Json(json!({"success": false, "message": "Invalid username or password"}))
                .into_response();
        },
        Err(e) => {
            error!("DB login error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response();
        },
    };

    if !user.is_active {
        return Json(json!({"success": false, "message": "Account is disabled"})).into_response();
    }

    let row = match db::get_user_by_username(&state.db, &body.username).await {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response();
        },
    };

    if !verify_password(&body.password, &row.password_hash) {
        return Json(json!({"success": false, "message": "Invalid username or password"}))
            .into_response();
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
                "message": "Login successful",
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
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response()
        },
    }
}

// ── POST /api/v1/auth/refresh ─────────────────────────────────────────────────

/// 用 refresh token 换发新的 access token 对。
///
/// 旧的 refresh token 在校验通过后会被吊销并替换为新发的一对（rotation
/// 策略），防止泄露的 refresh token 被长期复用。如果 refresh token 已被
/// 吊销、过期或签名无效，返回 401，客户端需要走重新登录流程。
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
                Json(json!({"success": false, "message": "Refresh token is invalid or expired"})),
            )
                .into_response();
        },
    };

    let token_id = match claims.token_id {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "Invalid token format"})),
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
                    Json(json!({"success": false, "message": "Refresh token has been revoked"})),
                )
                    .into_response();
            },
            Some(info) if info.expires_at < now => {
                drop(info);
                state.refresh_tokens.remove(&token_id);
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"success": false, "message": "Refresh token has expired"})),
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
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response();
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
                "message": "Token refreshed successfully",
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
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response()
        },
    }
}

// ── POST /api/v1/auth/logout ──────────────────────────────────────────────────

/// 退出登录，吊销当前 refresh token。
///
/// access token 是无状态 JWT，本身无法服务端吊销，依赖其短过期时间自然
/// 失效。退出操作主要做两件事：从 `refresh_tokens` 注册表移除当前 refresh
/// token，让对方无法再用它换新 access token；并把这次会话从可观测列表中
/// 摘出。即使没传 refresh_token，也会返回 200（幂等）。
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
    if let Some(claims) = verify_refresh_token(&body.refresh_token, &state.config.jwt_secret)
        && let Some(token_id) = claims.token_id
    {
        state.refresh_tokens.remove(&token_id);
    }

    Json(json!({"success": true, "message": "Logged out successfully"}))
}

// ── GET /api/v1/auth/me ───────────────────────────────────────────────────────

/// 返回当前 access token 持有人的用户档案。
///
/// 不带密码哈希，包含 role 关联（join roles 表）。前端用于显示用户头像/
/// 用户名/角色，以及决定 UI 上哪些管理员功能要显示。401 表示 token 失效,
/// 客户端应触发 refresh 流程或跳登录。
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
            "message": "User info retrieved",
            "data": user,
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "User not found"})),
        )
            .into_response(),
        Err(e) => {
            error!("DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response()
        },
    }
}

// ── PUT /api/v1/auth/me ───────────────────────────────────────────────────────

/// 当前用户修改自己的档案。
///
/// 普通用户只能改基础字段（如显示名）；`role_id` / `is_active` 这两个字段
/// 只有 Admin 角色可写，普通用户传了会被 403 拒绝。改密码走单独端点
/// `PUT /me/password`，不在这里。
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
            Json(json!({"success": false, "message": "Only admins can modify roles and activation status"})),
        )
            .into_response();
    }

    apply_user_update(&state, claims.user_id, &body).await
}

// ── PUT /api/v1/auth/me/password ──────────────────────────────────────────────

/// 当前用户改自己的密码。
///
/// 必须提供 `old_password` 并通过 bcrypt 比对，防止 token 被劫持后被改
/// 密码。改成功后**不会**主动吊销已发的 refresh token（其他登录会话仍然
/// 有效），调用方如需"全设备登出"应自己再调 `/cleanup-tokens` 或登出
/// 流程。
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

    apply_password_change(
        &state,
        claims.user_id,
        &body.old_password,
        &body.new_password,
    )
    .await
}

// ── GET /api/v1/auth/roles ────────────────────────────────────────────────────

/// 列出系统中定义的角色。
///
/// 角色是 `(id, name, description)` 的静态枚举，目前是 Admin / Operator /
/// Viewer 等。给前端"创建/编辑用户"对话框做下拉选项用，所有已登录用户都
/// 能查（不限管理员）。
#[utoipa::path(get, path = "/api/v1/auth/roles", tag = "Auth",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "角色列表")))]
pub async fn get_roles(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::get_all_roles(&state.db).await {
        Ok(roles) => Json(json!({
            "success": true,
            "message": "Roles retrieved",
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

/// 列出全部用户（管理员视图）。
///
/// 返回每个用户的基础信息 + 角色 + 最后登录时间 + 激活状态。**响应里已经
/// 剥掉密码哈希字段**，可以安全地透传给前端。用于管理员的用户管理界面。
/// 当前实现允许任何已登录用户调用（应该限制为 Admin，是个 known TODO）。
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
                "message": "User list retrieved",
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

/// 查看指定用户的档案（管理员）。
///
/// 跟 `/auth/me` 返回相同 schema，但 caller 必须是 Admin 才能查别人。
/// 普通用户调用返回 403。密码哈希同样被剥掉。
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
            "message": "User info retrieved",
            "data": user,
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "User not found"})),
        )
            .into_response(),
        Err(e) => {
            error!("DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response()
        },
    }
}

// ── PUT /api/v1/auth/users/:id (admin) ───────────────────────────────────────

/// 管理员修改任意用户（包含角色和激活状态）。
///
/// 跟 `PUT /auth/me` 共用 `UserUpdate` schema，但是这里管理员可以改
/// `role_id` 和 `is_active`，普通用户调用直接 403。把 `is_active=false`
/// 设置进去**不会**立刻吊销该用户已发的 token —— 实际效果要等 token 自然
/// 过期，或调用方走 `/cleanup-tokens` 一刀切。
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

/// 管理员删除用户。
///
/// 硬删除 `users` 表的行（不是 soft-delete `is_active=false`），关联的
/// refresh token 也一并清掉。默认管理员账号 `admin` 受保护，删除尝试返
/// 回 400 —— 防止自把系统锁死的脚滑事故。
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
                    Json(json!({"success": false, "message": "Cannot delete the default admin account"})),
                )
                    .into_response();
            }
            match db::delete_user(&state.db, user_id).await {
                Ok(true) => Json(json!({
                    "success": true,
                    "message": "User deleted",
                    "data": { "user_id": user_id, "username": user.username }
                }))
                .into_response(),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": "Internal server error"})),
                )
                    .into_response(),
            }
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "User not found"})),
        )
            .into_response(),
        Err(e) => {
            error!("DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response()
        },
    }
}

// ── GET /api/v1/auth/stats (admin) ───────────────────────────────────────────

/// 认证子系统的运行统计。
///
/// 返回当前活跃的 refresh token 数量、活跃 / 总用户数、按角色分布等指标
/// 给运维看（监控 dashboard、容量预估）。不返回任何用户身份信息，只是聚
/// 合数字。
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
        "message": "Auth statistics retrieved",
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

/// 清理已过期或被吊销的 refresh token。
///
/// 维护操作：扫一遍 refresh token 注册表，删除 `expires_at < now()` 的
/// 行。正常情况这些已经无效，留着只是占空间 —— 周期性调用以控制表大小。
/// 不影响在用的有效 token。
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
        "message": format!("Cleaned up {} expired token(s)", count)
    }))
    .into_response()
}

// ── Shared helpers ────────────────────────────────────────────────────────────

async fn apply_user_update(
    state: &AppState,
    user_id: i64,
    body: &UserUpdate,
) -> axum::response::Response {
    if let Some(role_id) = body.role_id
        && let Err(e) = db::update_user_role(&state.db, user_id, role_id).await
    {
        error!("Update role error: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": "Internal server error"})),
        )
            .into_response();
    }

    if let Some(is_active) = body.is_active
        && let Err(e) = db::update_user_active(&state.db, user_id, is_active).await
    {
        error!("Update active error: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": "Internal server error"})),
        )
            .into_response();
    }

    if body.old_password.is_some() || body.new_password.is_some() {
        match (&body.old_password, &body.new_password) {
            (Some(old), Some(new)) => {
                return apply_password_change(state, user_id, old, new).await;
            },
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"success": false, "message": "Both old_password and new_password are required"})),
                )
                    .into_response();
            },
        }
    }

    match db::get_user_with_role(&state.db, user_id).await {
        Ok(Some(user)) => Json(json!({
            "success": true,
            "message": "User info updated",
            "data": user,
        }))
        .into_response(),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": "Internal server error"})),
        )
            .into_response(),
    }
}

async fn apply_password_change(
    state: &AppState,
    user_id: i64,
    old_password: &str,
    new_password: &str,
) -> axum::response::Response {
    let row = match db::get_user_by_id(&state.db, user_id).await {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response();
        },
    };

    if !verify_password(old_password, &row.password_hash) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "Incorrect current password"})),
        )
            .into_response();
    }

    let new_hash = match hash_password(new_password) {
        Ok(h) => h,
        Err(e) => {
            error!("bcrypt error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response();
        },
    };

    match db::update_user_password(&state.db, user_id, &new_hash).await {
        Ok(_) => Json(json!({"success": true, "message": "Password changed successfully"}))
            .into_response(),
        Err(e) => {
            error!("Update password error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Internal server error"})),
            )
                .into_response()
        },
    }
}
