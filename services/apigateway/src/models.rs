use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ── Role ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Role {
    pub id: i64,
    pub name_en: String,
    pub name_zh: String,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

// ── User ──────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub role_id: i64,
    pub is_active: bool,
    pub last_login: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserWithRole {
    pub id: i64,
    pub username: String,
    pub is_active: bool,
    pub last_login: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub role: RoleInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RoleInfo {
    pub id: i64,
    pub name_en: String,
    pub name_zh: String,
    pub description: Option<String>,
}

// ── Auth DTOs ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"username": "operator1", "password": "e10adc3949ba59abbe56e057f20f883e", "role_id": 2}))]
pub struct UserCreate {
    /// Username
    pub username: String,
    /// MD5-hashed password supplied by the frontend
    pub password: String,
    /// Role ID (defaults to Viewer)
    pub role_id: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"username": "admin", "password": "e10adc3949ba59abbe56e057f20f883e"}))]
pub struct UserLogin {
    pub username: String,
    /// MD5-hashed password supplied by the frontend
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UserUpdate {
    pub role_id: Option<i64>,
    pub is_active: Option<bool>,
    pub old_password: Option<String>,
    pub new_password: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"old_password": "e10adc3949ba59abbe56e057f20f883e", "new_password": "<MD5 hash of new password>"}))]
pub struct PasswordChange {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"refresh_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."}))]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// Stored refresh token metadata (in-memory)
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RefreshTokenInfo {
    pub user_id: i64,
    pub username: String,
    pub expires_at: i64,
}

// ── Calculated Points ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct CalculatedPoint {
    pub id: i64,
    pub name: String,
    pub formula: Option<String>,
    pub unit: Option<String>,
    pub imgurl: Option<String>,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CalculatedPointUpdate {
    pub name: Option<String>,
    pub formula: Option<String>,
    pub unit: Option<String>,
    pub imgurl: Option<String>,
    pub description: Option<String>,
}

// ── Network Config ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NetworkConfig {
    pub dhcp: bool,
    pub ip: String,
    pub subnet_mask: String,
    pub gateway: String,
    pub dns1: String,
    pub dns2: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({
    "lan": 1,
    "dhcp": false,
    "ip": "192.168.1.100",
    "subnet_mask": "255.255.255.0",
    "gateway": "192.168.1.1",
    "dns1": "8.8.8.8",
    "dns2": "8.8.4.4"
}))]
pub struct NetworkUpdateRequest {
    /// LAN port number (1 or 2)
    pub lan: u8,
    pub dhcp: bool,
    pub ip: Option<String>,
    pub subnet_mask: Option<String>,
    pub gateway: Option<String>,
    pub dns1: Option<String>,
    pub dns2: Option<String>,
}
