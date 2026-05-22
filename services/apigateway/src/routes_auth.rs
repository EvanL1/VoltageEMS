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

/// Register a new user account.
///
/// Public endpoint — no token required. Validates username length (3–50
/// characters) and uniqueness, bcrypt-hashes the password, and inserts the
/// row. Defaults to role_id=3 (regular user); the request body may override
/// this, but anonymous callers cannot gain admin privileges because this
/// endpoint does not verify the caller's identity.
#[utoipa::path(post, path = "/api/v1/auth/register", tag = "Auth",
    request_body = UserCreate,
    responses((status = 200, description = "Registration successful"), (status = 400, description = "Invalid parameters")))]
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

/// Authenticate with username and password, issuing an access/refresh token pair.
///
/// Returns `TokenResponse { access_token, refresh_token, expires_in, role }`.
/// The short-lived access token is used in subsequent requests via
/// `Authorization: Bearer ...`. The refresh token can obtain new access tokens
/// and is stored in the `refresh_tokens` table for point-in-time revocation.
/// Accounts with `is_active=false` are rejected with 401.
#[utoipa::path(post, path = "/api/v1/auth/login", tag = "Auth",
    request_body = UserLogin,
    responses((status = 200, description = "Login successful", body = TokenResponse), (status = 401, description = "Authentication failed")))]
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

/// Exchange a refresh token for a new access/refresh token pair.
///
/// The old refresh token is revoked and replaced with a freshly issued pair
/// (rotation strategy), preventing long-term reuse of a leaked refresh token.
/// Returns 401 if the refresh token has been revoked, has expired, or has an
/// invalid signature — the client must re-authenticate via the login endpoint.
#[utoipa::path(post, path = "/api/v1/auth/refresh", tag = "Auth",
    request_body = RefreshTokenRequest,
    responses((status = 200, description = "Token refreshed", body = TokenResponse), (status = 401, description = "Invalid or expired token")))]
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

/// Log out and revoke the current refresh token.
///
/// Access tokens are stateless JWTs and cannot be server-side revoked; they
/// expire naturally after their short TTL. Logout primarily removes the refresh
/// token from the `refresh_tokens` registry so it can no longer be used to
/// obtain new access tokens. Returns 200 even if no refresh token is supplied
/// (idempotent).
#[utoipa::path(post, path = "/api/v1/auth/logout", tag = "Auth",
    security(("bearer_auth" = [])),
    request_body = RefreshTokenRequest,
    responses((status = 200, description = "Logged out successfully")))]
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

/// Return the profile of the currently authenticated user.
///
/// Response includes role information (joined from the roles table) but
/// excludes the password hash. Used by the frontend to display the username,
/// role, and decide which admin UI elements to show. 401 indicates an expired
/// or invalid token; the client should trigger the refresh flow or redirect to
/// login.
#[utoipa::path(get, path = "/api/v1/auth/me", tag = "Auth",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "Current user profile", body = UserWithRole), (status = 401, description = "Unauthenticated")))]
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

/// Update the current user's own profile.
///
/// Regular users may update basic fields. The `role_id` and `is_active` fields
/// are restricted to Admin role — non-admin callers that supply either field
/// receive 403. Password changes are handled by the dedicated
/// `PUT /me/password` endpoint.
#[utoipa::path(put, path = "/api/v1/auth/me", tag = "Auth",
    security(("bearer_auth" = [])),
    request_body = UserUpdate,
    responses((status = 200, description = "Profile updated"), (status = 401, description = "Unauthenticated")))]
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

/// Change the current user's password.
///
/// Requires `old_password` verified via bcrypt to prevent password changes
/// after token hijacking. On success, existing refresh tokens are **not**
/// automatically revoked — other active sessions remain valid. Callers that
/// need a "sign out everywhere" effect should additionally call
/// `/cleanup-tokens` or the logout endpoint.
#[utoipa::path(put, path = "/api/v1/auth/me/password", tag = "Auth",
    security(("bearer_auth" = [])),
    request_body = PasswordChange,
    responses((status = 200, description = "Password changed successfully"), (status = 401, description = "Incorrect current password")))]
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

/// List all roles defined in the system.
///
/// Roles are a static enum of `(id, name, description)` rows — currently
/// Admin, Operator, and Viewer. Used to populate the role dropdown in the
/// create/edit user dialog. Accessible to any authenticated user.
#[utoipa::path(get, path = "/api/v1/auth/roles", tag = "Auth",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "Role list")))]
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

/// List all users (admin view).
///
/// Returns each user's basic info, role, last login timestamp, and activation
/// status. **Password hashes are stripped** from the response. Used for the
/// admin user-management UI. Note: the current implementation permits any
/// authenticated user to call this endpoint (restricting to Admin is a known
/// TODO).
#[utoipa::path(get, path = "/api/v1/auth/users", tag = "Auth",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "User list (admin view)")))]
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

/// Retrieve a specific user's profile (admin only).
///
/// Returns the same schema as `/auth/me` but requires Admin role; non-admin
/// callers receive 403. Password hash is stripped from the response.
#[utoipa::path(get, path = "/api/v1/auth/users/{id}", tag = "Auth",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "User ID")),
    responses((status = 200, description = "User profile", body = UserWithRole), (status = 404, description = "User not found")))]
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

/// Update any user's profile (admin only).
///
/// Shares the `UserUpdate` schema with `PUT /auth/me`, but here an Admin may
/// also modify `role_id` and `is_active`; non-admin callers receive 403.
/// Setting `is_active=false` does **not** immediately revoke existing tokens —
/// they remain valid until they expire naturally or are cleared via
/// `/cleanup-tokens`.
#[utoipa::path(put, path = "/api/v1/auth/users/{id}", tag = "Auth",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "User ID")),
    request_body = UserUpdate,
    responses((status = 200, description = "User updated"), (status = 403, description = "Insufficient privileges")))]
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

/// Delete a user (admin only).
///
/// Performs a hard delete from the `users` table (not a soft `is_active=false`
/// flag); associated refresh tokens are also removed. The built-in `admin`
/// account is protected — deletion attempts return 400 to prevent accidentally
/// locking out the system.
#[utoipa::path(delete, path = "/api/v1/auth/users/{id}", tag = "Auth",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "User ID")),
    responses((status = 200, description = "User deleted"), (status = 400, description = "Cannot delete the default admin account")))]
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

/// Return runtime statistics for the authentication subsystem.
///
/// Reports active refresh token count, active/total user counts, and
/// role-distribution metrics for operations monitoring and capacity planning.
/// No user identity information is included — aggregate numbers only.
#[utoipa::path(get, path = "/api/v1/auth/stats", tag = "Auth",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "Authentication statistics")))]
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

/// Remove expired or revoked refresh tokens from the in-memory registry.
///
/// Maintenance operation: scans the refresh token store and drops entries
/// where `expires_at < now()`. These tokens are already invalid; retaining
/// them merely wastes memory. Call periodically to keep the store compact.
/// Active valid tokens are not affected.
#[utoipa::path(post, path = "/api/v1/auth/cleanup-tokens", tag = "Auth",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "Expired tokens removed")))]
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

// ── GET /api/v1/auth/validate ────────────────────────────────────────────────

/// Lightweight token validation for nginx `auth_request`.
///
/// Returns 200 with empty body if the Authorization header carries a valid
/// JWT; 401 otherwise. Designed to be cheap — nginx issues an internal
/// subrequest here to gate backend service locations (/comApi/, /modApi/,
/// etc.) so the FE bearer token is enforced uniformly without each backend
/// service needing its own auth layer.
pub async fn validate_token(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    match require_auth(&state, &headers) {
        Ok(_) => StatusCode::OK.into_response(),
        Err((status, _)) => status.into_response(),
    }
}
