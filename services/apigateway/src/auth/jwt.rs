use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use once_cell::sync::Lazy;
use uuid::Uuid;

use super::{Claims, UserInfo};
use crate::error::{ApiError, ApiResult};

static JWT_SECRET: Lazy<String> = Lazy::new(|| {
    std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "voltage-ems-secret-key-change-this-in-production".to_string())
});

static JWT_ALGORITHM: Algorithm = Algorithm::HS256;

pub struct JwtManager;

impl JwtManager {
    pub fn generate_token(user_info: &UserInfo, duration_hours: i64) -> ApiResult<String> {
        let now = Utc::now();
        let exp = now + Duration::hours(duration_hours);

        let claims = Claims {
            sub: user_info.id.clone(),
            username: user_info.username.clone(),
            roles: user_info.roles.clone(),
            exp: exp.timestamp() as usize,
            iat: now.timestamp() as usize,
            jti: Uuid::new_v4(),
        };

        let header = Header::new(JWT_ALGORITHM);
        let encoding_key = EncodingKey::from_secret(JWT_SECRET.as_bytes());

        encode(&header, &claims, &encoding_key)
            .map_err(|e| ApiError::InternalError(format!("Failed to generate token: {}", e)))
    }

    pub fn verify_token(token: &str) -> ApiResult<Claims> {
        let decoding_key = DecodingKey::from_secret(JWT_SECRET.as_bytes());
        let validation = Validation::new(JWT_ALGORITHM);

        decode::<Claims>(token, &decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| ApiError::BadRequest(format!("Invalid token: {}", e)))
    }

    pub fn generate_refresh_token(user_info: &UserInfo) -> ApiResult<String> {
        // Generate a longer-lived refresh token (30 days)
        Self::generate_token(user_info, 24 * 30)
    }

    pub fn generate_access_token(user_info: &UserInfo) -> ApiResult<String> {
        // Generate a short-lived access token (1 hour)
        Self::generate_token(user_info, 1)
    }
}
