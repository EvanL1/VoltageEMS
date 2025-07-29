pub mod jwt;
pub mod middleware;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,        // User ID
    pub username: String,   // Username
    pub roles: Vec<String>, // User roles
    pub exp: usize,         // Expiration time (as UTC timestamp)
    pub iat: usize,         // Issued at (as UTC timestamp)
    pub jti: Uuid,          // JWT ID (unique identifier)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub roles: Vec<String>,
}
