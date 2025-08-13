pub mod data;
pub mod health;

// Re-export handlers
pub use data::{get_channel_status, get_historical_data, get_realtime_data, list_channels};
pub use health::{detailed_health, health_check};
