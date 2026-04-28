use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use bytes::Bytes;
use chrono::Utc;
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use voltage_rtdb::Rtdb;

// ── Subscription State ────────────────────────────────────────────────────────

/// Cached metadata for a homepage calculated point.
#[derive(Debug, Clone)]
pub struct HomepagePoint {
    pub id: i64,
    pub name: String,
    pub unit: String,
    pub imgurl: String,
    /// Redis lookup: "hash_key:field_id" → HGET hash_key field_id
    pub formula: String,
}

#[derive(Debug, Clone, Default)]
pub struct Subscription {
    pub source: String,
    pub channels: Vec<i64>,
    pub data_types: Vec<String>,
    pub interval_ms: u64,
    /// Populated once on homepage subscribe; reused for every push tick.
    pub homepage_points: Vec<HomepagePoint>,
}

#[derive(Debug)]
struct ClientHandle {
    tx: mpsc::UnboundedSender<String>,
    sub: RwLock<Subscription>,
    data_type_ws: String,
    connected_at: i64,
    last_activity: Arc<AtomicI64>,
}

// ── WebSocket Hub ─────────────────────────────────────────────────────────────

pub struct WsHub {
    clients: DashMap<String, Arc<ClientHandle>>,
    rtdb: Arc<voltage_rtdb::RedisRtdb>,
    db: SqlitePool,
}

impl WsHub {
    pub fn new(rtdb: Arc<voltage_rtdb::RedisRtdb>, db: SqlitePool) -> Arc<Self> {
        Arc::new(Self {
            clients: DashMap::new(),
            rtdb,
            db,
        })
    }

    pub fn register(
        &self,
        client_id: String,
        data_type_ws: String,
    ) -> mpsc::UnboundedReceiver<String> {
        let (tx, rx) = mpsc::unbounded_channel();
        let now = Utc::now().timestamp();

        let handle = Arc::new(ClientHandle {
            tx,
            sub: RwLock::new(Subscription {
                source: "inst".to_string(),
                channels: Vec::new(),
                data_types: Vec::new(),
                interval_ms: 1000,
                homepage_points: Vec::new(),
            }),
            data_type_ws,
            connected_at: now,
            last_activity: Arc::new(AtomicI64::new(now)),
        });

        self.clients.insert(client_id, handle);
        rx
    }

    pub fn deregister(&self, client_id: &str) {
        self.clients.remove(client_id);
        info!("WS client disconnected: {}", client_id);
    }

    pub fn update_subscription(
        &self,
        client_id: &str,
        source: String,
        channels: Vec<i64>,
        data_types: Vec<String>,
        interval_ms: u64,
        homepage_points: Vec<HomepagePoint>,
    ) {
        if let Some(handle) = self.clients.get(client_id)
            && let Ok(mut sub) = handle.sub.write()
        {
            sub.source = source;
            sub.channels = channels;
            sub.data_types = data_types;
            sub.interval_ms = interval_ms;
            sub.homepage_points = homepage_points;
        }
    }

    pub fn update_activity(&self, client_id: &str) {
        if let Some(handle) = self.clients.get(client_id) {
            handle
                .last_activity
                .store(Utc::now().timestamp(), Ordering::Relaxed);
        }
    }

    pub fn send_to(&self, client_id: &str, msg: String) -> bool {
        match self.clients.get(client_id) {
            Some(handle) => handle.tx.send(msg).is_ok(),
            _ => false,
        }
    }

    pub fn broadcast(&self, msg: &str) -> (usize, Vec<String>) {
        let mut count = 0;
        let mut ids = Vec::new();
        for entry in self.clients.iter() {
            if entry.tx.send(msg.to_string()).is_ok() {
                count += 1;
                ids.push(entry.key().clone());
            }
        }
        (count, ids)
    }

    #[allow(dead_code)]
    pub fn connection_count(&self) -> usize {
        self.clients.len()
    }

    pub fn get_status(&self) -> Value {
        let mut connections = serde_json::Map::new();
        let mut subscriptions_map = serde_json::Map::new();

        for entry in self.clients.iter() {
            let id = entry.key();
            let Ok(sub) = entry.sub.read() else {
                continue;
            };

            connections.insert(
                id.clone(),
                json!({
                    "data_type": entry.data_type_ws,
                    "connected_at": entry.connected_at,
                    "last_activity": entry.last_activity.load(Ordering::Relaxed),
                }),
            );

            subscriptions_map.insert(
                id.clone(),
                json!({
                    "source": &sub.source,
                    "channels": &sub.channels,
                    "data_types": &sub.data_types,
                    "interval": sub.interval_ms,
                }),
            );
        }

        json!({
            "running": true,
            "connection_count": self.clients.len(),
            "connections_info": connections,
            "subscriptions": subscriptions_map,
        })
    }

    /// Cleanup clients that have been inactive for > 5 minutes.
    pub fn cleanup_inactive(&self) {
        let now = Utc::now().timestamp();
        let inactive: Vec<String> = self
            .clients
            .iter()
            .filter(|e| now - e.last_activity.load(Ordering::Relaxed) > 300)
            .map(|e| e.key().clone())
            .collect();

        for id in inactive {
            warn!("Removing inactive WS client: {}", id);
            self.clients.remove(&id);
        }
    }
}

// ── Background Tasks ──────────────────────────────────────────────────────────

/// Sentinel string sent via the text channel to trigger a WebSocket Ping frame.
/// The send_task converts this to `Message::Ping` so the browser WebSocket
/// library handles keepalive natively without surfacing an "unknown message
/// type" warning in application-level code.
const WS_PING_SENTINEL: &str = "\x00__ping__\x00";

/// Periodic heartbeat: sends a native WebSocket Ping frame to every client.
/// The browser responds automatically with a Pong; no application-level
/// handler is needed on the frontend.
pub async fn run_heartbeat(hub: Arc<WsHub>, shutdown: CancellationToken) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = interval.tick() => {
                hub.broadcast(WS_PING_SENTINEL);
                hub.cleanup_inactive();
            }
        }
    }
}

/// Periodic data push to subscribed clients.
pub async fn run_data_push(hub: Arc<WsHub>, shutdown: CancellationToken, interval_secs: u64) {
    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs.max(1)));
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = interval.tick() => {
                push_subscribed_data(&hub).await;
            }
        }
    }
}

async fn push_subscribed_data(hub: &Arc<WsHub>) {
    let client_ids: Vec<String> = hub.clients.iter().map(|e| e.key().clone()).collect();

    for client_id in client_ids {
        let (source, channels, data_types) = {
            let Some(handle) = hub.clients.get(&client_id) else {
                continue;
            };
            let Ok(sub) = handle.sub.read() else {
                continue;
            };
            if sub.channels.is_empty() && sub.source != "homepage" {
                continue;
            }
            (
                sub.source.clone(),
                sub.channels.clone(),
                sub.data_types.clone(),
            )
        };

        if source == "rule" {
            if let Some(rule_id) = channels.first() {
                push_rule_data(hub, &client_id, *rule_id).await;
            }
            continue;
        }

        if source == "homepage" {
            push_homepage_data(hub, &client_id).await;
            continue;
        }

        // Standard source:channel_id:data_type subscriptions
        let mut all_updates = Vec::new();
        for channel_id in &channels {
            for dt in &data_types {
                let val_key = format!("{}:{}:{}", source, channel_id, dt);
                let ts_key = format!("{}:{}:{}:ts", source, channel_id, dt);

                let values_raw = match hub.rtdb.hash_get_all(&val_key).await {
                    Ok(v) if !v.is_empty() => v,
                    Err(e) => {
                        debug!("HGETALL {} error: {}", val_key, e);
                        continue;
                    },
                    _ => continue,
                };

                let ts_raw = hub.rtdb.hash_get_all(&ts_key).await.unwrap_or_default();

                let values_obj: serde_json::Map<String, Value> = values_raw
                    .iter()
                    .map(|(k, v)| {
                        let num = String::from_utf8_lossy(v)
                            .parse::<f64>()
                            .ok()
                            .and_then(serde_json::Number::from_f64)
                            .map(Value::Number)
                            .unwrap_or(Value::Null);
                        (k.clone(), num)
                    })
                    .collect();

                let ts_obj: serde_json::Map<String, Value> = ts_raw
                    .iter()
                    .map(|(k, v)| {
                        let ts = String::from_utf8_lossy(v)
                            .parse::<i64>()
                            .ok()
                            .map(serde_json::Number::from)
                            .map(Value::Number)
                            .unwrap_or(Value::Null);
                        (k.clone(), ts)
                    })
                    .collect();

                all_updates.push(json!({
                    "source": source,
                    "channel_id": channel_id,
                    "data_type": dt,
                    "values": values_obj,
                    "ts": ts_obj,
                }));
            }
        }

        if !all_updates.is_empty() {
            let now = Utc::now().timestamp();
            let msg = json!({
                "type": "data_batch",
                "id": format!("batch_{}", now),
                "timestamp": now,
                "data": { "updates": all_updates },
            })
            .to_string();
            hub.send_to(&client_id, msg);
        }
    }
}

/// Load calculated_points from SQLite once at subscribe time.
async fn load_homepage_points(db: &SqlitePool) -> Vec<HomepagePoint> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        name: String,
        formula: Option<String>,
        unit: Option<String>,
        imgurl: Option<String>,
    }

    match sqlx::query_as::<_, Row>(
        "SELECT id, name, formula, unit, imgurl FROM calculated_points ORDER BY id",
    )
    .fetch_all(db)
    .await
    {
        Ok(rows) => rows
            .into_iter()
            .map(|r| HomepagePoint {
                id: r.id,
                name: r.name,
                unit: r.unit.unwrap_or_default(),
                imgurl: r.imgurl.unwrap_or_default(),
                formula: r.formula.unwrap_or_default(),
            })
            .collect(),
        Err(e) => {
            error!("Failed to load calculated_points: {}", e);
            Vec::new()
        },
    }
}

/// Push homepage_batch to a subscribed client.
/// Uses the point list cached at subscribe time; only reads Redis for current values.
///
/// Formula format: `"hash_key:field_id"` → HGET `hash_key` field `field_id`
/// Empty formula → value is null.
async fn push_homepage_data(hub: &Arc<WsHub>, client_id: &str) {
    let points = {
        let Some(handle) = hub.clients.get(client_id) else {
            return;
        };
        let Ok(sub) = handle.sub.read() else { return };
        sub.homepage_points.clone()
    };

    if points.is_empty() {
        return;
    }

    let mut updates = Vec::with_capacity(points.len());
    for pt in &points {
        let value = if !pt.formula.is_empty() {
            let parts: Vec<&str> = pt.formula.rsplitn(2, ':').collect();
            if parts.len() == 2 {
                let field_id = parts[0];
                let hash_key = parts[1];
                match hub.rtdb.hash_get(hash_key, field_id).await {
                    Ok(Some(bytes)) => String::from_utf8_lossy(&bytes)
                        .parse::<f64>()
                        .ok()
                        .and_then(serde_json::Number::from_f64)
                        .map(Value::Number)
                        .unwrap_or(Value::Null),
                    _ => Value::Null,
                }
            } else {
                Value::Null
            }
        } else {
            Value::Null
        };

        updates.push(json!({
            "id": pt.id,
            "name": pt.name,
            "values": value,
            "unit": pt.unit,
            "imgurl": pt.imgurl,
        }));
    }

    let now = Utc::now().timestamp();
    let msg = json!({
        "type": "homepage_batch",
        "id": format!("homepage_batch_{}", now),
        "timestamp": now,
        "data": { "updates": updates }
    })
    .to_string();
    hub.send_to(client_id, msg);
}

async fn push_rule_data(hub: &Arc<WsHub>, client_id: &str, rule_id: i64) {
    let key = format!("rule:{}:exec", rule_id);
    match hub.rtdb.hash_get_all(&key).await {
        Ok(data) if !data.is_empty() => {
            let to_str = |b: &Bytes| String::from_utf8_lossy(b).to_string();

            let rule_name = data.get("rule_name").map(to_str).unwrap_or_default();

            let exec_timestamp: i64 = data
                .get("timestamp")
                .and_then(|v| String::from_utf8_lossy(v).parse().ok())
                .unwrap_or(0);

            let success = data
                .get("success")
                .map(|v| String::from_utf8_lossy(v).eq_ignore_ascii_case("true"))
                .unwrap_or(false);

            let error_val = data.get("error").map(to_str).filter(|s| !s.is_empty());

            let execution_path: Value = data
                .get("execution_path")
                .and_then(|v| serde_json::from_str(&String::from_utf8_lossy(v)).ok())
                .unwrap_or(Value::Array(vec![]));

            let variable_values: Value = data
                .get("variable_values")
                .and_then(|v| serde_json::from_str(&String::from_utf8_lossy(v)).ok())
                .unwrap_or(json!({}));

            let node_details: Value = data
                .get("node_details")
                .and_then(|v| serde_json::from_str(&String::from_utf8_lossy(v)).ok())
                .unwrap_or(json!({}));

            let msg = json!({
                "type": "data_batch",
                "timestamp": Utc::now().timestamp(),
                "data": {
                    "rule_id": rule_id,
                    "rule_name": rule_name,
                    "variables": {},
                    "last_execution": {
                        "success": success,
                        "timestamp": exec_timestamp,
                        "error": error_val,
                        "execution_path": execution_path,
                        "variable_values": variable_values,
                        "node_details": node_details,
                    }
                }
            })
            .to_string();

            hub.send_to(client_id, msg);
        },
        Err(e) => debug!("Rule data HGETALL error rule={}: {}", rule_id, e),
        _ => {},
    }
}

// ── WebSocket Connection Handler ──────────────────────────────────────────────

pub async fn handle_socket(
    socket: WebSocket,
    client_id: String,
    data_type_ws: String,
    hub: Arc<WsHub>,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let mut rx = hub.register(client_id.clone(), data_type_ws);

    // Send welcome message
    let welcome = json!({
        "type": "connection_established",
        "id": format!("welcome_{}", client_id),
        "timestamp": Utc::now().timestamp(),
        "data": {
            "client_id": client_id,
            "message": "Connected. Subscribe to a data channel to receive real-time data"
        }
    })
    .to_string();

    if ws_sender.send(Message::Text(welcome.into())).await.is_err() {
        hub.deregister(&client_id);
        return;
    }

    info!("WS client connected: {}", client_id);

    // Forward from channel to WebSocket.
    // WS_PING_SENTINEL is converted to a protocol-level Ping frame so the
    // browser handles it transparently without triggering application logic.
    let client_id_send = client_id.clone();
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let frame = if msg == WS_PING_SENTINEL {
                Message::Ping(bytes::Bytes::new())
            } else {
                Message::Text(msg.into())
            };
            if ws_sender.send(frame).await.is_err() {
                break;
            }
        }
        let _ = ws_sender.close().await;
        client_id_send
    });

    // Handle incoming messages
    let hub_recv = hub.clone();
    let client_id_recv = client_id.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Text(text) => {
                    hub_recv.update_activity(&client_id_recv);
                    handle_client_message(&hub_recv, &client_id_recv, &text).await;
                },
                Message::Ping(data) => {
                    // Axum handles pong automatically, just update activity
                    hub_recv.update_activity(&client_id_recv);
                    let _ = data;
                },
                Message::Close(_) => break,
                _ => {},
            }
        }
        client_id_recv
    });

    tokio::select! {
        _ = &mut send_task => { recv_task.abort(); }
        _ = &mut recv_task => { send_task.abort(); }
    }

    hub.deregister(&client_id);
}

async fn handle_client_message(hub: &WsHub, client_id: &str, text: &str) {
    let data: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => {
            let err = error_msg("INVALID_JSON", "Invalid JSON format", None);
            hub.send_to(client_id, err);
            return;
        },
    };

    let msg_type = data["type"].as_str().unwrap_or("");

    match msg_type {
        "ping" => {
            let pong = json!({
                "type": "pong",
                "id": data["id"],
                "timestamp": Utc::now().timestamp(),
                "data": { "latency_ms": 0 }
            })
            .to_string();
            hub.send_to(client_id, pong);
        },

        "subscribe" => {
            let source = data["data"]["source"]
                .as_str()
                .unwrap_or("inst")
                .to_string();
            let channels: Vec<i64> = data["data"]["channels"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_i64()).collect())
                .unwrap_or_default();
            let data_types: Vec<String> = data["data"]["data_types"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_else(|| vec!["T".to_string()]);
            let interval_ms: u64 = data["data"]["interval"].as_u64().unwrap_or(1000);
            let is_homepage = source == "homepage";

            // Load homepage points once from DB when subscribing to "homepage" source.
            let homepage_points = if is_homepage {
                load_homepage_points(&hub.db).await
            } else {
                Vec::new()
            };

            let ack = if is_homepage {
                json!({
                    "type": "subscribe_ack",
                    "id": format!("{}_ack", data["id"].as_str().unwrap_or("sub")),
                    "timestamp": Utc::now().timestamp(),
                    "data": { "source": "homepage", "message": "Homepage data subscription active" }
                })
            } else {
                json!({
                    "type": "subscribe_ack",
                    "id": format!("{}_ack", data["id"].as_str().unwrap_or("sub")),
                    "timestamp": Utc::now().timestamp(),
                    "data": { "subscribed": &channels, "failed": [] }
                })
            };
            hub.update_subscription(
                client_id,
                source,
                channels,
                data_types,
                interval_ms,
                homepage_points,
            );
            hub.send_to(client_id, ack.to_string());
        },

        "unsubscribe" => {
            let source = data["data"]["source"].as_str().unwrap_or("inst");
            hub.update_subscription(
                client_id,
                source.to_string(),
                Vec::new(),
                Vec::new(),
                1000,
                Vec::new(),
            );
            let ack = json!({
                "type": "unsubscribe_ack",
                "id": format!("{}_ack", data["id"].as_str().unwrap_or("unsub")),
                "timestamp": Utc::now().timestamp(),
                "data": { "unsubscribed": [], "failed": [] }
            })
            .to_string();
            hub.send_to(client_id, ack);
        },

        "control" => {
            handle_control(hub, client_id, &data).await;
        },

        _ => {
            debug!("Unknown WS message type '{}' from {}", msg_type, client_id);
        },
    }
}

async fn handle_control(hub: &WsHub, client_id: &str, data: &Value) {
    let control = &data["data"];
    let source = control["source"].as_str().unwrap_or("inst");
    let channel_id = control["channel_id"].as_i64();
    let point_id = control["point_id"].as_i64();
    let command_type = control["command_type"].as_str();
    let value = &control["value"];

    let (Some(channel_id), Some(point_id), Some(_command_type)) =
        (channel_id, point_id, command_type)
    else {
        let err = error_msg(
            "CONTROL_ERROR",
            "Missing required control parameters",
            data["id"].as_str(),
        );
        hub.send_to(client_id, err);
        return;
    };

    if value.is_null() {
        let err = error_msg(
            "CONTROL_ERROR",
            "Missing required control parameters",
            data["id"].as_str(),
        );
        hub.send_to(client_id, err);
        return;
    }

    // Publish control command to Redis via control key
    // Key: {source}:trigger:{channel_id}:C
    let cmd_id = data["id"]
        .as_str()
        .map(String::from)
        .unwrap_or_else(|| format!("cmd_{}", Utc::now().timestamp()));

    let cmd_data = json!({
        "point_id": point_id,
        "value": value,
        "source": client_id,
        "command_id": cmd_id,
        "timestamp": Utc::now().timestamp(),
    });

    let trigger_key = format!("{}:trigger:{}:C", source, channel_id);
    let cmd_bytes = match serde_json::to_vec(&cmd_data) {
        Ok(bytes) => Bytes::from(bytes),
        Err(e) => {
            error!("Failed to encode WS control command: {}", e);
            let err = error_msg(
                "CONTROL_ERROR",
                "Failed to encode control command",
                data["id"].as_str(),
            );
            hub.send_to(client_id, err);
            return;
        },
    };

    match hub.rtdb.list_rpush(&trigger_key, cmd_bytes).await {
        Ok(_) => {
            let ack = json!({
                "type": "control_ack",
                "id": format!("{}_ack", cmd_id),
                "timestamp": Utc::now().timestamp(),
                "data": {
                    "command_id": cmd_id,
                    "status": "executed",
                    "success": true,
                    "value": value,
                }
            })
            .to_string();
            hub.send_to(client_id, ack);
        },
        Err(e) => {
            error!("Control command publish failed: {}", e);
            let err = error_msg(
                "CONTROL_ERROR",
                "Failed to publish control command",
                data["id"].as_str(),
            );
            hub.send_to(client_id, err);
        },
    }
}

fn error_msg(code: &str, message: &str, request_id: Option<&str>) -> String {
    json!({
        "type": "error",
        "timestamp": Utc::now().timestamp(),
        "data": {
            "code": code,
            "message": message,
            "request_id": request_id,
        }
    })
    .to_string()
}
