//! Data models for the alarm service

use serde::{Deserialize, Serialize, Serializer};
use utoipa::{IntoParams, ToSchema};

/// Serialize a stored JSON string as a parsed JSON value.
/// If the string is not valid JSON, it falls back to the raw string.
fn serialize_json_str<S>(s: &String, ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match serde_json::from_str::<serde_json::Value>(s) {
        Ok(v) => v.serialize(ser),
        Err(_) => s.serialize(ser),
    }
}

// ============================================================================
// Core domain models (map 1:1 to database tables)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct AlertRule {
    pub id: i64,
    pub service_type: String,
    pub channel_id: i64,
    pub data_type: String,
    pub point_id: i64,
    pub rule_name: String,
    /// Warning level: 1=low, 2=medium, 3=high
    pub warning_level: i64,
    /// Operator: >, <, >=, <=, ==, !=
    pub operator: String,
    pub value: f64,
    pub enabled: bool,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl AlertRule {
    pub fn evaluate(&self, current_value: f64) -> bool {
        match self.operator.as_str() {
            ">" => current_value > self.value,
            "<" => current_value < self.value,
            ">=" => current_value >= self.value,
            "<=" => current_value <= self.value,
            "==" => (current_value - self.value).abs() < 1e-6,
            "!=" => (current_value - self.value).abs() >= 1e-6,
            _ => false,
        }
    }

    /// Redis HGET key: `{service_type}:{channel_id}:{data_type}`
    ///
    /// This deliberately mirrors the format produced by `KeySpaceConfig::channel_key`
    /// (e.g. `comsrv:1001:T`) using the rule's `service_type` field as the prefix.
    /// The caller is responsible for storing the correct prefix in `service_type`
    /// (e.g. "comsrv" for channel data, "inst" for instance data).
    pub fn redis_key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.service_type, self.channel_id, self.data_type
        )
    }

    /// Redis HGET field: `{point_id}`
    pub fn redis_field(&self) -> String {
        self.point_id.to_string()
    }

    /// Serialise rule metadata as a JSON snapshot for storage in alert/event tables
    pub fn snapshot(&self) -> String {
        serde_json::json!({
            "rule_name": self.rule_name,
            "warning_level": self.warning_level,
            "operator": self.operator,
            "value": self.value,
            "description": self.description,
        })
        .to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Alert {
    pub id: i64,
    pub rule_id: i64,
    #[serde(serialize_with = "serialize_json_str")]
    pub rule_snapshot: String,
    pub service_type: String,
    pub channel_id: i64,
    pub data_type: String,
    pub point_id: i64,
    pub rule_name: String,
    pub warning_level: i64,
    pub operator: String,
    pub threshold_value: f64,
    pub current_value: f64,
    /// Always "active" — resolved alerts are deleted and moved to alert_event
    pub status: String,
    pub triggered_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct AlertEvent {
    pub id: i64,
    pub rule_id: i64,
    #[serde(serialize_with = "serialize_json_str")]
    pub rule_snapshot: String,
    pub service_type: String,
    pub channel_id: i64,
    pub data_type: String,
    pub point_id: i64,
    pub rule_name: String,
    pub warning_level: i64,
    pub operator: String,
    pub threshold_value: f64,
    pub trigger_value: Option<f64>,
    pub recovery_value: Option<f64>,
    /// "trigger" | "recovery"
    pub event_type: String,
    pub triggered_at: Option<i64>,
    pub recovered_at: Option<i64>,
    /// Duration in seconds
    pub duration: Option<i64>,
}

// ============================================================================
// Request DTOs
// ============================================================================

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({
    "service_type": "comsrv",
    "channel_id": 1001,
    "data_type": "M",
    "point_id": 1,
    "rule_name": "Overvoltage Alarm",
    "warning_level": 2,
    "operator": ">",
    "value": 260.0,
    "enabled": true,
    "description": "Trigger alarm when voltage exceeds 260V"
}))]
pub struct CreateRuleRequest {
    pub service_type: String,
    pub channel_id: i64,
    pub data_type: String,
    pub point_id: i64,
    pub rule_name: String,
    /// Warning level (default: 2)
    #[serde(default = "default_warning_level")]
    pub warning_level: i64,
    /// Operator: >, <, >=, <=, ==, !=
    pub operator: String,
    pub value: f64,
    /// Whether enabled (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRuleRequest {
    pub service_type: Option<String>,
    pub channel_id: Option<i64>,
    pub data_type: Option<String>,
    pub point_id: Option<i64>,
    pub rule_name: Option<String>,
    pub warning_level: Option<i64>,
    pub operator: Option<String>,
    pub value: Option<f64>,
    pub enabled: Option<bool>,
    pub description: Option<String>,
}

// ============================================================================
// Query parameter structs
// ============================================================================

#[derive(Debug, Deserialize, Default, IntoParams)]
pub struct RuleQueryParams {
    pub service_type: Option<String>,
    pub channel_id: Option<i64>,
    pub data_type: Option<String>,
    pub enabled: Option<bool>,
    pub warning_level: Option<i64>,
    /// Page number (1-based; takes priority over skip when set)
    pub page: Option<i64>,
    /// Page size (used with page; takes priority over limit when set)
    pub page_size: Option<i64>,
    /// Offset rows (legacy; ignored when page is present)
    #[serde(default)]
    pub skip: i64,
    /// Max rows to return (legacy; ignored when page_size is present)
    #[serde(default = "default_limit")]
    pub limit: i64,
}

#[derive(Debug, Deserialize, Default, IntoParams)]
pub struct AlertQueryParams {
    pub warning_level: Option<i64>,
    pub service_type: Option<String>,
    pub channel_id: Option<i64>,
    pub keyword: Option<String>,
    /// Page number (1-based; takes priority over skip when set)
    pub page: Option<i64>,
    /// Page size (used with page; takes priority over limit when set)
    pub page_size: Option<i64>,
    /// Offset rows (legacy)
    #[serde(default)]
    pub skip: i64,
    /// Max rows to return (legacy)
    #[serde(default = "default_limit")]
    pub limit: i64,
}

#[derive(Debug, Deserialize, Default, IntoParams)]
pub struct EventQueryParams {
    pub rule_id: Option<i64>,
    /// "trigger" or "recovery"
    pub event_type: Option<String>,
    pub service_type: Option<String>,
    pub warning_level: Option<i64>,
    /// Unix timestamp (seconds)
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    /// Page number (1-based; takes priority over skip when set)
    pub page: Option<i64>,
    /// Page size (used with page; takes priority over limit when set)
    pub page_size: Option<i64>,
    /// Offset rows (legacy)
    #[serde(default)]
    pub skip: i64,
    /// Max rows to return (legacy)
    #[serde(default = "default_limit")]
    pub limit: i64,
}

/// 统一计算分页参数：返回 `(effective_limit, offset, resolved_page, resolved_page_size)`。
///
/// 优先使用 `page`/`page_size`；若未提供则回退到 `skip`/`limit`。
pub fn resolve_pagination(
    page: Option<i64>,
    page_size: Option<i64>,
    skip: i64,
    limit: i64,
) -> (i64, i64, i64, i64) {
    const MAX_PAGE_SIZE: i64 = 200;
    match page {
        Some(p) => {
            let p = p.max(1);
            let ps = page_size.unwrap_or(limit).clamp(1, MAX_PAGE_SIZE);
            let offset = (p - 1) * ps;
            (ps, offset, p, ps)
        },
        None => {
            let ps = page_size.unwrap_or(limit).clamp(1, MAX_PAGE_SIZE);
            let offset = skip.max(0);
            // Convert skip/limit back to a logical page number (best-effort)
            let p = if ps > 0 { offset / ps + 1 } else { 1 };
            (ps, offset, p, ps)
        },
    }
}

// ============================================================================
// Response wrappers
// ============================================================================

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub message: String,
    pub data: T,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(message: impl Into<String>, data: T) -> Self {
        Self {
            success: true,
            message: message.into(),
            data,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PagedData<T: Serialize> {
    pub total: i64,
    pub list: Vec<T>,
    /// Current page number (1-based)
    pub page: i64,
    /// Page size used for this query
    pub page_size: i64,
}

// ============================================================================
// Monitor state
// ============================================================================

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MonitorStatus {
    pub running: bool,
    pub last_check_time: Option<i64>,
    pub check_interval: u64,
    pub redis_url: String,
}

// ============================================================================
// Helpers
// ============================================================================

fn default_warning_level() -> i64 {
    2
}

fn default_true() -> bool {
    true
}

fn default_limit() -> i64 {
    20
}
