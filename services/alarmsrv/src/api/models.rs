//! API request and response models

use serde::{Deserialize, Serialize};

use crate::domain::{Alarm, AlarmLevel, AlarmStatus};

/// Health check endpoint response
pub const HEALTH_OK: &str = "OK";

/// Alarm query parameters
#[derive(Deserialize)]
pub struct AlarmQuery {
    pub level: Option<AlarmLevel>,
    pub status: Option<AlarmStatus>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub keyword: Option<String>,
}

/// Alarm list response with pagination info
#[derive(Serialize)]
pub struct AlarmListResponse {
    pub alarms: Vec<Alarm>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

/// Create alarm request
#[derive(Deserialize)]
pub struct CreateAlarmRequest {
    pub title: String,
    pub description: String,
    pub level: AlarmLevel,
}

/// Status response
#[derive(Serialize)]
pub struct StatusResponse {
    pub service: String,
    pub status: String,
    pub total_alarms: usize,
    pub active_alarms: usize,
    pub redis_connected: bool,
    pub classifier_rules: usize,
}
