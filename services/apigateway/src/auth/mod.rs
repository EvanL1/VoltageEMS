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

impl Claims {
    pub fn has_permission(&self, permission: &str) -> bool {
        // Map permission strings to roles
        match permission {
            "channel:read" => self.roles.iter().any(|r| matches!(r.as_str(), "admin" | "operator" | "engineer" | "viewer")),
            "channel:write" => self.roles.iter().any(|r| matches!(r.as_str(), "admin" | "operator" | "engineer")),
            "alarm:read" => self.roles.iter().any(|r| matches!(r.as_str(), "admin" | "operator" | "viewer")),
            "alarm:write" => self.roles.iter().any(|r| matches!(r.as_str(), "admin" | "operator")),
            "system:read" => self.roles.iter().any(|r| matches!(r.as_str(), "admin")),
            "system:write" => self.roles.iter().any(|r| matches!(r.as_str(), "admin")),
            _ => false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Permission {
    Read,
    Write,
    Control,
    Admin,
}

impl Permission {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "read" => Some(Permission::Read),
            "write" => Some(Permission::Write),
            "control" => Some(Permission::Control),
            "admin" => Some(Permission::Admin),
            _ => None,
        }
    }
}

pub fn check_permission(roles: &[String], required_permission: Permission) -> bool {
    for role in roles {
        if has_permission(role, &required_permission) {
            return true;
        }
    }
    false
}

fn has_permission(role: &str, permission: &Permission) -> bool {
    match role {
        "admin" => true, // Admin has all permissions
        "operator" => matches!(
            permission,
            Permission::Read | Permission::Write | Permission::Control
        ),
        "engineer" => matches!(permission, Permission::Read | Permission::Write),
        "viewer" => matches!(permission, Permission::Read),
        _ => false,
    }
}

// Simple password verification for demo purposes
// In production, use proper password hashing like bcrypt
pub fn verify_password(password: &str, _hash: &str) -> bool {
    // For demo, just check against known passwords
    matches!(password, "admin123" | "operator123" | "engineer123" | "viewer123")
}
