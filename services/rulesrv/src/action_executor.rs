//! Action Executor Module
//!
//! This module provides action execution functionality for the rule engine.
//! It supports various action types including setting values, sending controls,
//! triggering alarms, and invoking external services.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use crate::rule_engine::{Action, ActionResult, ExecutionContext, ModsrvCommandData};
use anyhow::{anyhow, Context, Result};
use reqwest::Client as HttpClient;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info};
use voltage_config::modsrv::RedisKeys;

/// Action executor for processing rule actions
pub struct ActionExecutor {
    /// RTDB for all Redis operations (data + messaging)
    rtdb: Arc<dyn voltage_rtdb::Rtdb>,

    /// Routing cache for M2C (Model to Channel) routing
    routing_cache: Arc<voltage_config::RoutingCache>,

    /// HTTP client for external requests
    http_client: HttpClient,

    /// Service endpoints for internal communication
    service_endpoints: HashMap<String, String>,
}

impl ActionExecutor {
    /// Create a new action executor from Redis URL
    pub async fn new(redis_url: &str) -> Result<Self> {
        let rtdb = voltage_rtdb::RedisRtdb::new(redis_url)
            .await
            .context("Failed to create RedisRtdb")?;
        let rtdb_arc: Arc<dyn voltage_rtdb::Rtdb> = Arc::new(rtdb);

        // Load routing cache from Redis
        let routing_cache = Self::load_routing_cache(&rtdb_arc).await?;

        Self::with_rtdb(rtdb_arc, routing_cache)
    }

    /// Load routing cache from Redis
    async fn load_routing_cache(
        rtdb: &Arc<dyn voltage_rtdb::Rtdb>,
    ) -> Result<Arc<voltage_config::RoutingCache>> {
        use std::collections::HashMap;
        use voltage_config::common::RedisRoutingKeys;

        // Load C2M routing (Channel to Model)
        let c2m_bytes = rtdb
            .hash_get_all(RedisRoutingKeys::CHANNEL_TO_MODEL)
            .await
            .unwrap_or_default();
        let c2m_data: HashMap<String, String> = c2m_bytes
            .into_iter()
            .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
            .collect();

        // Load M2C routing (Model to Channel)
        let m2c_bytes = rtdb
            .hash_get_all(RedisRoutingKeys::MODEL_TO_CHANNEL)
            .await
            .unwrap_or_default();
        let m2c_data: HashMap<String, String> = m2c_bytes
            .into_iter()
            .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
            .collect();

        Ok(Arc::new(voltage_config::RoutingCache::from_maps(
            c2m_data,
            m2c_data,
            std::collections::HashMap::new(), // C2C routing (not used in rulesrv)
        )))
    }

    /// Create action executor with custom RTDB (for testing)
    pub fn with_rtdb(
        rtdb: Arc<dyn voltage_rtdb::Rtdb>,
        routing_cache: Arc<voltage_config::RoutingCache>,
    ) -> Result<Self> {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        let mut service_endpoints = HashMap::new();
        // Configure service endpoints from environment variables with defaults from voltage-config
        use voltage_config::{
            DEFAULT_COMSRV_URL, DEFAULT_MODSRV_URL, ENV_COMSRV_URL, ENV_MODSRV_URL,
        };

        let comsrv_url =
            std::env::var(ENV_COMSRV_URL).unwrap_or_else(|_| DEFAULT_COMSRV_URL.to_string());
        let modsrv_url =
            std::env::var(ENV_MODSRV_URL).unwrap_or_else(|_| DEFAULT_MODSRV_URL.to_string());

        service_endpoints.insert("comsrv".to_string(), comsrv_url);
        service_endpoints.insert("modsrv".to_string(), modsrv_url);
        // Note: alarmsrv, hissrv, and apigateway have been removed from the project
        // These services are now handled by Python versions or Redis directly

        Ok(Self {
            rtdb,
            routing_cache,
            http_client,
            service_endpoints,
        })
    }

    /// Execute a list of actions
    pub async fn execute_actions(
        &mut self,
        actions: &[Action],
        context: &ExecutionContext,
    ) -> Vec<ActionResult> {
        let mut results = Vec::new();

        for action in actions {
            let start = std::time::Instant::now();
            let result = match self.execute_action(action, context).await {
                Ok(value) => {
                    info!("Action executed successfully: {:?}", action);
                    ActionResult {
                        action_type: self.get_action_type_name(action),
                        success: true,
                        result: Some(value),
                        error: None,
                    }
                },
                Err(e) => {
                    error!("Action execution failed: {:?}, error: {}", action, e);
                    ActionResult {
                        action_type: self.get_action_type_name(action),
                        success: false,
                        result: None,
                        error: Some(e.to_string()),
                    }
                },
            };

            debug!(
                "Action '{}' took {}ms",
                self.get_action_type_name(action),
                start.elapsed().as_millis()
            );

            results.push(result);
        }

        results
    }

    /// Execute a single action
    async fn execute_action(
        &mut self,
        action: &Action,
        context: &ExecutionContext,
    ) -> Result<Value> {
        match action {
            Action::SetMode { params } => self.execute_set_mode(params, context).await,
            Action::SendControl { params } => self.execute_send_control(params, context).await,
            Action::TriggerAlarm { params } => self.execute_trigger_alarm(params, context).await,
            Action::Publish { params } => self.execute_publish(params, context).await,
            Action::SetValue { target, value } => self.execute_set_value(target, value).await,
            Action::InvokeModel { model_id, params } => {
                self.execute_invoke_model(model_id, params, context).await
            },
            Action::ExecuteScript { script, args } => {
                self.execute_script(script, args, context).await
            },
            Action::HttpRequest {
                url,
                method,
                headers,
                body,
            } => {
                self.execute_http_request(url, method, headers, body.as_ref())
                    .await
            },
            Action::ModsrvCommand {
                instance_name,
                point_type,
                point_id,
                value,
            } => {
                self.execute_modsrv_command(instance_name, point_type, *point_id, value)
                    .await
            },
            Action::ModsrvBatch { commands } => self.execute_modsrv_batch(commands).await,
        }
    }

    /// Get action type name for logging
    fn get_action_type_name(&self, action: &Action) -> String {
        match action {
            Action::SetMode { .. } => "set_mode".to_string(),
            Action::SendControl { .. } => "send_control".to_string(),
            Action::TriggerAlarm { .. } => "trigger_alarm".to_string(),
            Action::Publish { .. } => "publish".to_string(),
            Action::SetValue { .. } => "set_value".to_string(),
            Action::InvokeModel { .. } => "invoke_model".to_string(),
            Action::ExecuteScript { .. } => "execute_script".to_string(),
            Action::HttpRequest { .. } => "http_request".to_string(),
            Action::ModsrvCommand { .. } => "modsrv_command".to_string(),
            Action::ModsrvBatch { .. } => "modsrv_batch".to_string(),
        }
    }

    /// Execute set mode action
    #[allow(clippy::disallowed_methods)] // json! macro internally uses unwrap (safe for known valid JSON)
    async fn execute_set_mode(
        &mut self,
        params: &HashMap<String, Value>,
        _context: &ExecutionContext,
    ) -> Result<Value> {
        let mode = params
            .get("mode")
            .ok_or_else(|| anyhow!("Mode parameter is required"))?;

        let default_target = Value::String("energy:system_mode".to_string());
        let target = params.get("target").unwrap_or(&default_target);

        debug!("Setting mode: {} to {}", target, mode);

        let target_str = match target {
            Value::String(s) => s.clone(),
            _ => target.to_string(),
        };

        let mode_str = match mode {
            Value::String(s) => s.clone(),
            _ => mode.to_string(),
        };

        // Set mode value using RTDB
        self.rtdb
            .set(&target_str, bytes::Bytes::from(mode_str.clone()))
            .await
            .context("Failed to set mode in Redis")?;

        // Publish mode change event using RTDB
        self.rtdb
            .publish(
                "mode_change",
                &serde_json::json!({
                    "target": target_str,
                    "mode": mode_str,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                })
                .to_string(),
            )
            .await
            .context("Failed to publish mode change")?;

        Ok(serde_json::json!({
            "status": "success",
            "mode": mode_str,
            "target": target_str,
        }))
    }

    /// Execute send control action
    #[allow(clippy::disallowed_methods)] // json! macro internally uses unwrap (safe for known valid JSON)
    async fn execute_send_control(
        &mut self,
        params: &HashMap<String, Value>,
        _context: &ExecutionContext,
    ) -> Result<Value> {
        let target = params
            .get("target")
            .ok_or_else(|| anyhow!("Target parameter is required"))?;

        let command = params
            .get("command")
            .ok_or_else(|| anyhow!("Command parameter is required"))?;

        let value = params.get("value");

        debug!(
            "Sending control to {}: command={}, value={:?}",
            target, command, value
        );

        // Parse target format: "comsrv:channel_id" or direct channel
        let target_str = match target {
            Value::String(s) => s.clone(),
            _ => target.to_string(),
        };

        if target_str.starts_with("comsrv:") {
            // Send to comsrv
            let channel_id = match target_str.strip_prefix("comsrv:") {
                Some(id) => id,
                None => {
                    tracing::error!(
                        "Failed to strip 'comsrv:' prefix from target: {}",
                        target_str
                    );
                    return Err(anyhow!("Invalid comsrv target format"));
                },
            };
            let endpoint = self
                .service_endpoints
                .get("comsrv")
                .ok_or_else(|| anyhow!("Comsrv endpoint not configured"))?;

            let url = format!("{}/api/channels/{}/control", endpoint, channel_id);

            let body = serde_json::json!({
                "command": command,
                "value": value,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            });

            let response = self
                .http_client
                .post(&url)
                .json(&body)
                .send()
                .await
                .context("Failed to send control to comsrv")?;

            if response.status().is_success() {
                let result = response
                    .json::<Value>()
                    .await
                    .unwrap_or_else(|_| serde_json::json!({"status": "success"}));
                Ok(result)
            } else {
                Err(anyhow!("Control request failed: {}", response.status()))
            }
        } else {
            // Direct Redis control
            let control_data = serde_json::json!({
                "command": command,
                "value": value,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            });

            self.rtdb
                .hash_set(
                    &target_str,
                    "control",
                    bytes::Bytes::from(control_data.to_string()),
                )
                .await
                .context("Failed to set control in Redis")?;

            Ok(serde_json::json!({
                "status": "success",
                "target": target_str,
                "command": command,
            }))
        }
    }

    /// Execute trigger alarm action
    async fn execute_trigger_alarm(
        &mut self,
        params: &HashMap<String, Value>,
        _context: &ExecutionContext,
    ) -> Result<Value> {
        let default_level = Value::String("warning".to_string());
        let level = params.get("level").unwrap_or(&default_level);

        let message = params
            .get("message")
            .ok_or_else(|| anyhow!("Message parameter is required"))?;

        let _code = params.get("code");

        debug!("Triggering alarm: level={}, message={}", level, message);

        // Direct Redis implementation (alarmsrv service has been removed)
        // Alarms are now handled directly through Redis or by Python services

        let alarm_id = format!("alarm:{}", uuid::Uuid::new_v4());

        let fields = vec![
            ("level".to_string(), bytes::Bytes::from(level.to_string())),
            (
                "message".to_string(),
                bytes::Bytes::from(message.to_string()),
            ),
            (
                "timestamp".to_string(),
                bytes::Bytes::from(chrono::Utc::now().to_rfc3339()),
            ),
        ];

        self.rtdb
            .hash_mset(&alarm_id, fields)
            .await
            .context("Failed to create alarm in Redis")?;

        Ok(serde_json::json!({
            "status": "success",
            "alarm_id": alarm_id,
        }))
    }

    /// Execute publish action
    async fn execute_publish(
        &mut self,
        params: &HashMap<String, Value>,
        _context: &ExecutionContext,
    ) -> Result<Value> {
        let channel = params
            .get("channel")
            .ok_or_else(|| anyhow!("Channel parameter is required"))?;

        let default_message = Value::Object(params.clone().into_iter().collect());
        let message = params.get("message").unwrap_or(&default_message);

        debug!("Publishing to channel {}: {:?}", channel, message);

        let channel_str = match channel {
            Value::String(s) => s.clone(),
            _ => channel.to_string(),
        };

        self.rtdb
            .publish(&channel_str, &message.to_string())
            .await
            .context("Failed to publish message")?;

        Ok(serde_json::json!({
            "status": "success",
            "channel": channel_str,
        }))
    }

    /// Execute set value action
    async fn execute_set_value(&mut self, target: &str, value: &Value) -> Result<Value> {
        debug!("Setting value: {} = {:?}", target, value);

        // Parse target format
        let parts: Vec<&str> = target.split(':').collect();

        if parts.len() >= 3 {
            // Hash field update
            let key = parts[0..parts.len() - 1].join(":");
            let field = parts[parts.len() - 1];

            self.rtdb
                .hash_set(&key, field, bytes::Bytes::from(value.to_string()))
                .await
                .context(format!("Failed to set hash field {}:{}", key, field))?;
        } else {
            // Simple key update
            self.rtdb
                .set(target, bytes::Bytes::from(value.to_string()))
                .await
                .context(format!("Failed to set key {}", target))?;
        }

        Ok(serde_json::json!({
            "status": "success",
            "target": target,
            "value": value,
        }))
    }

    /// Execute invoke model action
    async fn execute_invoke_model(
        &mut self,
        model_id: &str,
        params: &HashMap<String, Value>,
        _context: &ExecutionContext,
    ) -> Result<Value> {
        debug!("Invoking model {}: {:?}", model_id, params);

        let endpoint = self
            .service_endpoints
            .get("modsrv")
            .ok_or_else(|| anyhow!("Modsrv endpoint not configured"))?;

        let url = format!("{}/api/models/{}/action", endpoint, model_id);

        // Convert params to action request
        let default_action = Value::String("calculate".to_string());
        let action = params.get("action").unwrap_or(&default_action);

        let body = serde_json::json!({
            "action": action,
            "value": params,
        });

        let response = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to invoke model")?;

        if response.status().is_success() {
            let result = response
                .json::<Value>()
                .await
                .context("Failed to parse model response")?;
            Ok(result)
        } else {
            Err(anyhow!("Model invocation failed: {}", response.status()))
        }
    }

    /// Execute Lua script action
    async fn execute_script(
        &mut self,
        script: &str,
        args: &[Value],
        _context: &ExecutionContext,
    ) -> Result<Value> {
        debug!("Executing script {}: {:?}", script, args);

        // Convert args to strings
        let str_args: Vec<String> = args.iter().map(|v| v.to_string()).collect();

        // Execute Redis Function using RTDB
        let arg_refs: Vec<&str> = str_args.iter().map(|s| s.as_str()).collect();
        let result = self
            .rtdb
            .fcall(script, &[], &arg_refs)
            .await
            .context(format!("Failed to execute script {}", script))?;

        // Try to parse result as JSON
        match serde_json::from_str(&result) {
            Ok(json) => Ok(json),
            Err(_) => Ok(Value::String(result)),
        }
    }

    /// Execute modsrv command action
    async fn execute_modsrv_command(
        &mut self,
        instance_name: &str,
        point_type: &str,
        point_id: u32,
        value: &Value,
    ) -> Result<Value> {
        debug!(
            "Executing modsrv command: {}.{}.{} = {:?}",
            instance_name, point_type, point_id, value
        );

        // Validate point type
        if point_type != "M" && point_type != "A" {
            return Err(anyhow!(
                "Invalid point type: {} (must be M or A)",
                point_type
            ));
        }
        // Convert value to a numeric-friendly string (Lua expects tonumber(value))
        let to_numeric_string = |v: &Value| -> Result<String> {
            Ok(match v {
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => {
                    if *b {
                        "1".to_string()
                    } else {
                        "0".to_string()
                    }
                },
                Value::String(s) => {
                    // Allow numeric-like strings, reject others to avoid INVALID_VALUE in Lua
                    if s.trim().parse::<f64>().is_ok() {
                        s.clone()
                    } else {
                        return Err(anyhow!("Action value must be numeric or boolean"));
                    }
                },
                other => {
                    return Err(anyhow!(
                        "Unsupported value type for modsrv command: {}",
                        other
                    ))
                },
            })
        };

        if point_type == "A" {
            // Use application-layer routing with cache
            let value_num = to_numeric_string(value)?;
            let value_f64: f64 = value_num.parse().context("Failed to parse value as f64")?;

            // Call local routing executor
            let outcome = crate::routing_executor::set_action_point(
                self.rtdb.as_ref(),
                &self.routing_cache,
                instance_name,
                &point_id.to_string(),
                value_f64,
            )
            .await
            .context("Failed to execute action routing")?;

            // Convert outcome to JSON response
            let response = serde_json::json!({
                "status": outcome.status,
                "instance": outcome.instance_name,
                "point": outcome.point_id,
                "value": outcome.value,
                "routed": outcome.routed,
                "route_result": outcome.route_result,
            });

            return Ok(response);
        }

        // For measurement points (M): write to instance hash inst:{id}:M field {point_id}
        // Query instance_id by scanning inst:*:name keys
        let name_keys = self
            .rtdb
            .scan_match("inst:*:name")
            .await
            .context("Failed to scan instance name keys")?;

        let mut instance_id: Option<u16> = None;
        for key in name_keys {
            if let Some(name_bytes) = self.rtdb.get(&key).await? {
                if String::from_utf8_lossy(&name_bytes) == instance_name {
                    // Extract ID from "inst:100:name"
                    let parts: Vec<&str> = key.split(':').collect();
                    if parts.len() == 3 && parts[0] == "inst" && parts[2] == "name" {
                        instance_id = parts[1].parse().ok();
                        break;
                    }
                }
            }
        }

        let instance_id =
            instance_id.ok_or_else(|| anyhow!("Instance not found: {}", instance_name))?;

        let key = RedisKeys::measurement_hash(instance_id);
        let value_str = match value {
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => {
                if *b {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            },
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };

        self.rtdb
            .hash_set(
                &key,
                &point_id.to_string(),
                bytes::Bytes::from(value_str.clone()),
            )
            .await
            .context(format!(
                "Failed to set measurement value {}:{}",
                key, point_id
            ))?;

        Ok(serde_json::json!({
            "status": "success",
            "instance": instance_name,
            "point_type": point_type,
            "point_id": point_id,
            "value": value_str,
        }))
    }

    /// Execute batch modsrv commands
    async fn execute_modsrv_batch(&mut self, commands: &[ModsrvCommandData]) -> Result<Value> {
        debug!("Executing {} modsrv commands in batch", commands.len());

        let mut results = Vec::new();

        for cmd in commands {
            let result = self
                .execute_modsrv_command(
                    &cmd.instance_name,
                    &cmd.point_type,
                    cmd.point_id,
                    &cmd.value,
                )
                .await;

            match result {
                Ok(res) => results.push(res),
                Err(e) => {
                    error!("Failed to execute modsrv command: {}", e);
                    results.push(serde_json::json!({
                        "status": "error",
                        "error": e.to_string(),
                        "instance": cmd.instance_name,
                        "point_id": cmd.point_id,
                    }));
                },
            }
        }

        Ok(serde_json::json!({
            "status": "success",
            "executed": commands.len(),
            "results": results,
        }))
    }

    /// Execute HTTP request action
    async fn execute_http_request(
        &mut self,
        url: &str,
        method: &str,
        headers: &HashMap<String, String>,
        body: Option<&Value>,
    ) -> Result<Value> {
        debug!("HTTP request: {} {}", method, url);

        let mut request = match method.to_uppercase().as_str() {
            "GET" => self.http_client.get(url),
            "POST" => self.http_client.post(url),
            "PUT" => self.http_client.put(url),
            "DELETE" => self.http_client.delete(url),
            "PATCH" => self.http_client.patch(url),
            _ => return Err(anyhow!("Unsupported HTTP method: {}", method)),
        };

        // Add headers
        for (key, value) in headers {
            request = request.header(key, value);
        }

        // Add body if present
        if let Some(body_data) = body {
            request = request.json(body_data);
        }

        let response = request
            .send()
            .await
            .context(format!("HTTP request failed: {} {}", method, url))?;

        let status = response.status();
        let status_code = status.as_u16();

        // Try to get response body
        let body_text = response.text().await.unwrap_or_else(|_| String::new());

        // Try to parse as JSON
        let body_value = serde_json::from_str::<Value>(&body_text)
            .unwrap_or_else(|_| Value::String(body_text.clone()));

        if status.is_success() {
            Ok(serde_json::json!({
                "status": "success",
                "status_code": status_code,
                "body": body_value,
            }))
        } else {
            Err(anyhow!(
                "HTTP request failed with status {}: {}",
                status_code,
                body_value
            ))
        }
    }
}

/// Helper function to execute actions with a new executor
#[allow(dead_code)]
pub async fn execute_actions(
    actions: &[Action],
    context: &ExecutionContext,
    rtdb: Arc<dyn voltage_rtdb::Rtdb>,
) -> Vec<ActionResult> {
    // For test helper: use empty routing cache
    let routing_cache = Arc::new(voltage_config::RoutingCache::new());
    match ActionExecutor::with_rtdb(rtdb, routing_cache) {
        Ok(mut executor) => executor.execute_actions(actions, context).await,
        Err(e) => {
            vec![ActionResult {
                action_type: "executor_init".to_string(),
                success: false,
                result: None,
                error: Some(format!("Failed to create executor: {}", e)),
            }]
        },
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use crate::rule_engine::Action;

    fn create_test_context() -> ExecutionContext {
        ExecutionContext {
            timestamp: chrono::Utc::now(),
            execution_id: "test-exec-1".to_string(),
            data: HashMap::new(),
            history: vec![],
            data_history: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_set_value_action() {
        // Create MemoryRtdb for testing (no Redis required)
        let rtdb: Arc<dyn voltage_rtdb::Rtdb> = Arc::new(voltage_rtdb::MemoryRtdb::new());
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());
        let mut executor = ActionExecutor::with_rtdb(rtdb, routing_cache)
            .expect("Failed to create ActionExecutor in test");

        let action = Action::SetValue {
            target: "test:key".to_string(),
            value: Value::String("test_value".to_string()),
        };

        let context = create_test_context();
        let result = executor.execute_action(&action, &context).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_publish_action() {
        // This test would require a Redis connection
        // Marked as example for structure
    }
}
