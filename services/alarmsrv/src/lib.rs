//! Alarm Service Library
//! 
//! This module exports the public API for the alarm service.

pub mod api;
pub mod config;
pub mod domain;
pub mod redis;
pub mod services;

pub use config::AlarmConfig;
pub use domain::{Alarm, AlarmClassifier, AlarmLevel, AlarmStatus, AlarmStatistics};
pub use redis::{
    AlarmFilter, AlarmQueryService, AlarmRedisClient, AlarmStatisticsManager,
    AlarmStore,
};

/// Application state
#[derive(Clone)]
pub struct AppState {
    pub alarms: std::sync::Arc<tokio::sync::RwLock<Vec<Alarm>>>,
    pub config: std::sync::Arc<AlarmConfig>,
    pub redis_client: std::sync::Arc<AlarmRedisClient>,
    pub alarm_store: std::sync::Arc<AlarmStore>,
    pub query_service: std::sync::Arc<AlarmQueryService>,
    pub stats_manager: std::sync::Arc<AlarmStatisticsManager>,
    pub classifier: std::sync::Arc<AlarmClassifier>,
}