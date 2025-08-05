pub mod health;

// Re-export health check handlers
pub use health::{detailed_health, health_check};
