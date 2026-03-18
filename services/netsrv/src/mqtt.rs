/// MQTT connection lifecycle and incoming-message dispatch.
///
/// Architecture:
/// - `run_mqtt_loop` runs forever, reconnecting whenever the connection drops
///   or `state.reconnect_signal` fires (triggered by config-change API).
/// - Incoming `Publish` events are dispatched to the appropriate handler
///   based on topic.
/// - A shared `Arc<Mutex<Option<AsyncClient>>>` in `AppState` is updated
///   every time a new connection is established, so other tasks can publish.
use std::sync::atomic::Ordering;
use std::sync::Arc;

use bytes::Bytes;
use chrono::Utc;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS, Transport, TlsConfiguration};
use tokio::time::{self, Duration};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use voltage_rtdb::Rtdb;

use crate::models::{CommandReply, ReadRequest, StatusPayload, WriteRequest};
use crate::state::AppState;

// ── Public entry point ────────────────────────────────────────────────────────

pub async fn run_mqtt_loop(state: Arc<AppState>, shutdown: CancellationToken) {
    loop {
        let cfg = state.config.read().await.clone();
        let delay = Duration::from_secs(cfg.reconnect_delay_secs);

        match connect_and_run(Arc::clone(&state), shutdown.clone()).await {
            Ok(_) => {
                if shutdown.is_cancelled() {
                    info!("MQTT task shut down cleanly");
                    return;
                }
                info!("MQTT event loop exited, will reconnect");
            }
            Err(e) => {
                warn!("MQTT connection error: {}", e);
            }
        }

        if shutdown.is_cancelled() {
            return;
        }

        // Wait before reconnecting; yield early on config-change or shutdown.
        tokio::select! {
            _ = time::sleep(delay) => {}
            _ = state.reconnect_signal.notified() => {
                info!("Reconnect signal received");
            }
            _ = shutdown.cancelled() => return,
        }
    }
}

// ── Inner connect + run ───────────────────────────────────────────────────────

async fn connect_and_run(
    state: Arc<AppState>,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let cfg = state.config.read().await.clone();

    // Resolve client ID
    let client_id = if cfg.client_id == "auto" {
        state.device.device_sn.clone()
    } else {
        cfg.client_id.clone()
    };

    let mut options = MqttOptions::new(&client_id, &cfg.broker_host, cfg.broker_port);
    options.set_keep_alive(Duration::from_secs(cfg.broker_keepalive_secs));
    options.set_clean_session(true);

    // TLS – cert_dir is fixed at startup from EnvConfig (not API-editable).
    if cfg.ssl_enabled {
        match build_tls(&state.env.cert_dir) {
            Ok(tls) => {
                options.set_transport(Transport::tls_with_config(tls));
            }
            Err(e) => {
                error!("TLS setup failed: {}. Falling back to plain TCP.", e);
            }
        }
    }

    let (client, mut event_loop) = AsyncClient::new(options, 64);
    *state.mqtt_client.lock().await = Some(client.clone());

    info!("MQTT connecting to {}:{}", cfg.broker_host, cfg.broker_port);

    loop {
        tokio::select! {
            event_result = event_loop.poll() => {
                match event_result {
                    Ok(Event::Incoming(Incoming::ConnAck(ack))) => {
                        info!("MQTT connected (return_code={:?})", ack.code);
                        state.mqtt_connected.store(true, Ordering::Relaxed);
                        on_connected(&state, &client).await;
                    }
                    Ok(Event::Incoming(Incoming::Publish(p))) => {
                        let topic = p.topic.clone();
                        let payload = p.payload.clone();
                        let s = Arc::clone(&state);
                        tokio::spawn(async move {
                            dispatch_message(s, &topic, payload).await;
                        });
                    }
                    Ok(Event::Incoming(Incoming::Disconnect)) => {
                        state.mqtt_connected.store(false, Ordering::Relaxed);
                        info!("MQTT disconnected by broker");
                        return Ok(());
                    }
                    Ok(_) => {}
                    Err(e) => {
                        state.mqtt_connected.store(false, Ordering::Relaxed);
                        return Err(anyhow::anyhow!("MQTT poll error: {}", e));
                    }
                }
            }

            _ = state.reconnect_signal.notified() => {
                info!("Config changed, reconnecting MQTT");
                // Send graceful offline before disconnecting
                let _ = publish_status(&client, &state, "offline", Some("config_reload")).await;
                state.mqtt_connected.store(false, Ordering::Relaxed);
                return Ok(());
            }

            _ = shutdown.cancelled() => {
                let _ = publish_status(&client, &state, "offline", Some("graceful_shutdown")).await;
                let _ = client.disconnect().await;
                state.mqtt_connected.store(false, Ordering::Relaxed);
                info!("MQTT disconnected (graceful shutdown)");
                return Ok(());
            }
        }
    }
}

// ── Connection established ────────────────────────────────────────────────────

async fn on_connected(state: &AppState, client: &AsyncClient) {
    // Re-subscribe to all command topics
    for (topic, qos) in state.topics.subscriptions() {
        if let Err(e) = client.subscribe(topic, qos).await {
            error!("Subscribe '{}' failed: {}", topic, e);
        }
    }

    // Send online status
    let _ = publish_status(client, state, "online", None).await;
}

// ── Status message helper ─────────────────────────────────────────────────────

pub async fn publish_status(
    client: &AsyncClient,
    state: &AppState,
    msg_type: &str,
    reason: Option<&str>,
) -> anyhow::Result<()> {
    let payload = StatusPayload {
        msg_type: msg_type.to_string(),
        gateway: String::new(),
        timestamp: Utc::now().timestamp(),
        reason: reason.map(|s| s.to_string()),
    };
    let json = serde_json::to_string(&payload)?;
    client
        .publish(&state.topics.status, QoS::AtLeastOnce, true, json)
        .await?;
    Ok(())
}

/// Publish any JSON value to a topic.
pub async fn publish_json(
    state: &AppState,
    topic: &str,
    value: &impl serde::Serialize,
) -> anyhow::Result<()> {
    let guard = state.mqtt_client.lock().await;
    let client = guard.as_ref().ok_or_else(|| anyhow::anyhow!("MQTT not connected"))?;
    let json = serde_json::to_string(value)?;
    client.publish(topic, QoS::AtLeastOnce, false, json).await?;
    Ok(())
}

// ── TLS configuration ─────────────────────────────────────────────────────────

fn build_tls(cert_dir: &str) -> anyhow::Result<TlsConfiguration> {
    let ca = std::fs::read(format!("{}/AmazonRootCA1.pem", cert_dir))
        .or_else(|_| std::fs::read(format!("{}/ca.pem", cert_dir)))
        .map_err(|e| anyhow::anyhow!("CA cert not found in {}: {}", cert_dir, e))?;

    let client_cert = std::fs::read(format!("{}/certificate.pem.crt", cert_dir))
        .or_else(|_| std::fs::read(format!("{}/client.crt", cert_dir)))
        .map_err(|e| anyhow::anyhow!("Client cert not found: {}", e))?;

    let client_key = std::fs::read(format!("{}/private.pem.key", cert_dir))
        .or_else(|_| std::fs::read(format!("{}/client.key", cert_dir)))
        .map_err(|e| anyhow::anyhow!("Client key not found: {}", e))?;

    // rumqttc 0.24 Simple: client_auth is (cert_pem_bytes, key_pem_bytes)
    Ok(TlsConfiguration::Simple {
        ca,
        alpn: None,
        client_auth: Some((client_cert, client_key)),
    })
}

// ── Incoming message dispatch ─────────────────────────────────────────────────

async fn dispatch_message(state: Arc<AppState>, topic: &str, payload: Bytes) {
    let t = &state.topics;

    if topic == t.read {
        handle_read(state, payload).await;
    } else if topic == t.write {
        handle_write(state, payload).await;
    } else if topic == t.call_data {
        handle_call_data(state, payload).await;
    } else if topic == t.call_alarm {
        handle_call_alarm(state, payload).await;
    }
}

// ── Command handlers ──────────────────────────────────────────────────────────

async fn handle_read(state: Arc<AppState>, payload: Bytes) {
    let req: ReadRequest = match serde_json::from_slice(&payload) {
        Ok(r) => r,
        Err(e) => {
            warn!("Bad read request: {}", e);
            return;
        }
    };

    let redis_key = format!("{}:{}:{}", req.source, req.device, req.data_type);

    let value = match &req.field {
        Some(field) => {
            // Single field
            match state.rtdb.hash_get(&redis_key, field).await {
                Ok(Some(v)) => parse_redis_value(&v),
                Ok(None) => serde_json::Value::Null,
                Err(e) => {
                    error!("Redis HGET '{}' '{}': {}", redis_key, field, e);
                    serde_json::Value::Null
                }
            }
        }
        None => {
            // All fields
            match state.rtdb.hash_get_all(&redis_key).await {
                Ok(map) => {
                    let obj: serde_json::Map<String, serde_json::Value> = map
                        .into_iter()
                        .filter(|(k, _)| !k.starts_with('_'))
                        .map(|(k, v)| (k, parse_redis_value(&v)))
                        .collect();
                    serde_json::Value::Object(obj)
                }
                Err(e) => {
                    error!("Redis HGETALL '{}': {}", redis_key, e);
                    serde_json::Value::Null
                }
            }
        }
    };

    let reply = crate::models::ReadReply {
        source: req.source,
        device: req.device,
        data_type: req.data_type,
        field: req.field,
        value,
        timestamp: Utc::now().timestamp(),
        request_id: req.request_id,
        success: true,
        error: None,
    };

    if let Err(e) = publish_json(&state, &state.topics.read_reply, &reply).await {
        error!("Failed to publish read-reply: {}", e);
    }
}

async fn handle_write(state: Arc<AppState>, payload: Bytes) {
    let req: WriteRequest = match serde_json::from_slice(&payload) {
        Ok(r) => r,
        Err(e) => {
            warn!("Bad write request: {}", e);
            return;
        }
    };

    let redis_key = format!("{}:{}:{}", req.source, req.device, req.data_type);
    let value_str = match &req.value {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };

    let result = state
        .rtdb
        .hash_set(&redis_key, &req.field, Bytes::from(value_str))
        .await;

    let (success, error) = match result {
        Ok(_) => (true, None),
        Err(e) => {
            error!("Redis HSET '{}': {}", redis_key, e);
            (false, Some(e.to_string()))
        }
    };

    let reply = WriteReply {
        success,
        error,
        request_id: req.request_id,
        timestamp: Utc::now().timestamp(),
    };

    if let Err(e) = publish_json(&state, &state.topics.write_reply, &reply).await {
        error!("Failed to publish write-reply: {}", e);
    }
}

use crate::models::WriteReply;

async fn handle_call_data(state: Arc<AppState>, payload: Bytes) {
    // Parse optional request_id
    let request_id: Option<String> = serde_json::from_slice::<serde_json::Value>(&payload)
        .ok()
        .and_then(|v| v.get("request_id").and_then(|r| r.as_str()).map(|s| s.to_string()));

    // First reply, then do the full data upload
    let reply = CommandReply {
        success: true,
        error: None,
        request_id: request_id.clone(),
        timestamp: Utc::now().timestamp(),
    };
    if let Err(e) = publish_json(&state, &state.topics.call_data_reply, &reply).await {
        error!("Failed to publish call-data-reply: {}", e);
    }

    // Trigger immediate data upload
    crate::forwarder::upload_once(Arc::clone(&state)).await;
}

async fn handle_call_alarm(state: Arc<AppState>, payload: Bytes) {
    let request_id: Option<String> = serde_json::from_slice::<serde_json::Value>(&payload)
        .ok()
        .and_then(|v| v.get("request_id").and_then(|r| r.as_str()).map(|s| s.to_string()));

    let alarmsrv_url = state.config.read().await.alarmsrv_url.clone();
    let url = format!("{}/alarmApi/call-data", alarmsrv_url);

    let (success, error) = match state.http_client.post(&url).send().await {
        Ok(resp) if resp.status().is_success() => (true, None),
        Ok(resp) => (false, Some(format!("alarmsrv returned {}", resp.status()))),
        Err(e) => (false, Some(e.to_string())),
    };

    if !success {
        warn!("call-alarm: {}", error.as_deref().unwrap_or("unknown"));
    }

    let reply = CommandReply {
        success,
        error,
        request_id,
        timestamp: Utc::now().timestamp(),
    };
    if let Err(e) = publish_json(&state, &state.topics.call_alarm_reply, &reply).await {
        error!("Failed to publish call-alarm-reply: {}", e);
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_redis_value(bytes: &Bytes) -> serde_json::Value {
    if let Ok(s) = std::str::from_utf8(bytes) {
        if let Ok(n) = s.parse::<f64>() {
            return serde_json::json!(n);
        }
        return serde_json::Value::String(s.to_string());
    }
    serde_json::Value::Null
}
