use axum::{
    extract::{Extension, Json, State},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::auth::{jwt::JwtManager, Claims, UserInfo};
use crate::error::{ApiError, ApiResult};
use crate::redis_client::RedisClientExt;
use crate::response::success_response;
use crate::AppState;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u32,
    pub token_type: String,
    pub user: UserInfo,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub expires_in: u32,
    pub token_type: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> ApiResult<impl IntoResponse> {
    // SECURITY WARNING: Hardcoded credentials - must implement proper authentication before production
    // Options: 1) User database 2) LDAP/AD integration 3) OAuth2/OIDC
    let user_info = match req.username.as_str() {
        "admin" => {
            if req.password != "admin123" {
                return Err(ApiError::BadRequest("Invalid credentials".to_string()));
            }
            UserInfo {
                id: "1".to_string(),
                username: "admin".to_string(),
                roles: vec!["admin".to_string()],
            }
        },
        "operator" => {
            if req.password != "operator123" {
                return Err(ApiError::BadRequest("Invalid credentials".to_string()));
            }
            UserInfo {
                id: "2".to_string(),
                username: "operator".to_string(),
                roles: vec!["operator".to_string()],
            }
        },
        "engineer" => {
            if req.password != "engineer123" {
                return Err(ApiError::BadRequest("Invalid credentials".to_string()));
            }
            UserInfo {
                id: "3".to_string(),
                username: "engineer".to_string(),
                roles: vec!["engineer".to_string()],
            }
        },
        "viewer" => {
            if req.password != "viewer123" {
                return Err(ApiError::BadRequest("Invalid credentials".to_string()));
            }
            UserInfo {
                id: "4".to_string(),
                username: "viewer".to_string(),
                roles: vec!["viewer".to_string()],
            }
        },
        _ => return Err(ApiError::BadRequest("Invalid credentials".to_string())),
    };

    // Generate tokens
    let access_token = JwtManager::generate_access_token(&user_info)?;
    let refresh_token = JwtManager::generate_refresh_token(&user_info)?;

    // Store refresh token in Redis (optional, for token revocation)
    let key = format!("refresh_token:{}", user_info.id);
    state
        .redis_client
        .set_ex_api(&key, &refresh_token, 30 * 24 * 3600)
        .await?;

    let response = LoginResponse {
        access_token,
        refresh_token,
        expires_in: 3600, // 1 hour
        token_type: "Bearer".to_string(),
        user: user_info,
    };

    Ok(success_response(response))
}

pub async fn refresh_token(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> ApiResult<impl IntoResponse> {
    // Verify refresh token
    let claims = JwtManager::verify_token(&req.refresh_token)?;

    // Check if refresh token exists in Redis (optional)
    let key = format!("refresh_token:{}", claims.sub);
    let stored_token: Option<String> = state.redis_client.get_api(&key).await?;

    match stored_token {
        Some(token) if token == req.refresh_token => {
            // Token is valid, continue
        },
        _ => {
            return Err(ApiError::InvalidToken("Invalid refresh token".to_string()));
        },
    }

    // Generate new access token
    let user_info = UserInfo {
        id: claims.sub,
        username: claims.username,
        roles: claims.roles,
    };

    let access_token = JwtManager::generate_access_token(&user_info)?;

    let response = RefreshResponse {
        access_token,
        expires_in: 3600, // 1 hour
        token_type: "Bearer".to_string(),
    };

    Ok(success_response(response))
}

pub async fn logout(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> ApiResult<impl IntoResponse> {
    // Remove refresh token from Redis
    let key = format!("refresh_token:{}", claims.sub);
    state.redis_client.del_api(&key).await?;

    Ok(success_response(serde_json::json!({
        "message": "Logged out successfully"
    })))
}

pub async fn current_user(Extension(claims): Extension<Claims>) -> ApiResult<impl IntoResponse> {
    let user_info = UserInfo {
        id: claims.sub.clone(),
        username: claims.username.clone(),
        roles: claims.roles.clone(),
    };

    Ok(success_response(user_info))
}
