//! WebSocket management module
//!
//! Provides WebSocket connection management and real-time data push functionality

use axum::{
    extract::{ws::WebSocket, Path, State, WebSocketUpgrade},
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};

use crate::api::ApiState;
use crate::model::ModelManager;

/// WebSocket message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    /// Data update
    Update { point: String, value: f64 },
    /// Heartbeat
    Heartbeat,
    /// Error
    Error { message: String },
}

/// WebSocket connection information
struct WsConnection {
    model_id: String,
    tx: mpsc::UnboundedSender<WsMessage>,
}

/// WebSocket connection manager
pub struct WsConnectionManager {
    connections: Arc<RwLock<HashMap<String, Vec<WsConnection>>>>,
    model_manager: Arc<ModelManager>,
}

impl WsConnectionManager {
    /// Create new connection manager
    pub fn new(model_manager: Arc<ModelManager>) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            model_manager,
        }
    }

    /// Start Redis subscription (listen for data updates)
    pub async fn start_redis_subscription(&self) -> Result<(), Box<dyn std::error::Error>> {
        let connections = self.connections.clone();

        // Create new Redis connection for subscription
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
        let client = redis::Client::open(redis_url)?;
        let mut pubsub = client.get_async_pubsub().await?;

        // Subscribe to all model update channels
        pubsub.psubscribe("modsrv:*:update").await?;

        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut stream = pubsub.on_message();

            while let Some(msg) = stream.next().await {
                let channel: String = msg.get_channel().unwrap();
                let payload: String = msg.get_payload().unwrap();

                // Parse channel to get model_id
                if let Some(model_id) = parse_model_id_from_channel(&channel) {
                    // Parse payload to get point and value
                    if let Some((point, value)) = parse_update_payload(&payload) {
                        // Broadcast to all connections subscribed to this model
                        let conns = connections.read().await;
                        if let Some(model_conns) = conns.get(&model_id) {
                            let update_msg = WsMessage::Update { point, value };
                            for conn in model_conns {
                                if let Err(e) = conn.tx.send(update_msg.clone()) {
                                    warn!("Failed to send WebSocket message: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Add connection
    async fn add_connection(
        &self,
        model_id: String,
        tx: mpsc::UnboundedSender<WsMessage>,
    ) -> String {
        let conn_id = uuid::Uuid::new_v4().to_string();
        let conn = WsConnection {
            model_id: model_id.clone(),
            tx,
        };

        let mut connections = self.connections.write().await;
        connections
            .entry(model_id)
            .or_insert_with(Vec::new)
            .push(conn);

        conn_id
    }

    /// Remove connection
    async fn remove_connection(&self, model_id: &str, _conn_id: &str) {
        let mut connections = self.connections.write().await;
        if let Some(model_conns) = connections.get_mut(model_id) {
            // Since we simplified the implementation, here we just clean up disconnected connections
            model_conns.retain(|conn| {
                // Test if the connection is still alive
                !conn.tx.is_closed()
            });

            if model_conns.is_empty() {
                connections.remove(model_id);
            }
        }
    }

    /// Start heartbeat task
    pub async fn start_heartbeat(&self) {
        let connections = self.connections.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                let mut conns = connections.write().await;
                for (model_id, model_conns) in conns.iter_mut() {
                    model_conns.retain(|conn| {
                        if conn.tx.send(WsMessage::Heartbeat).is_err() {
                            debug!("Removed disconnected WebSocket connection: {}", model_id);
                            false
                        } else {
                            true
                        }
                    });
                }

                // Clean up empty model entries
                conns.retain(|_, v| !v.is_empty());
            }
        });
    }
}

/// WebSocket handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(model_id): Path<String>,
    State(state): State<ApiState>,
) -> Response {
    // Check if model exists
    if state.model_manager.get_model(&model_id).await.is_none() {
        return ws.on_upgrade(move |socket| async move {
            handle_error_socket(socket, "Model not found").await;
        });
    }

    ws.on_upgrade(move |socket| handle_socket(socket, model_id, state))
}

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket, model_id: String, state: ApiState) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();

    // Add connection
    let conn_id = state.ws_manager.add_connection(model_id.clone(), tx).await;
    info!("WebSocket connection established: model_id={}", model_id);

    // Send initial data
    if let Ok(values) = state.model_manager.get_model_values(&model_id).await {
        for (point, value) in values {
            let msg = WsMessage::Update { point, value };
            if sender
                .send(axum::extract::ws::Message::Text(
                    serde_json::to_string(&msg).unwrap().into(),
                ))
                .await
                .is_err()
            {
                break;
            }
        }
    }

    // Start send task
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let text = serde_json::to_string(&msg).unwrap();
            if sender
                .send(axum::extract::ws::Message::Text(text.into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Start receive task (handle client messages)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                axum::extract::ws::Message::Text(text) => {
                    debug!("Received WebSocket message: {}", text);
                }
                axum::extract::ws::Message::Close(_) => {
                    debug!("WebSocket connection closed");
                    break;
                }
                _ => {}
            }
        }
    });

    // Wait for any task to end
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    // Clean up connection
    state
        .ws_manager
        .remove_connection(&model_id, &conn_id)
        .await;
    info!("WebSocket connection disconnected: model_id={}", model_id);
}

/// Handle erroneous WebSocket connection
async fn handle_error_socket(mut socket: WebSocket, error: &str) {
    let error_msg = WsMessage::Error {
        message: error.to_string(),
    };

    if let Ok(text) = serde_json::to_string(&error_msg) {
        let _ = socket
            .send(axum::extract::ws::Message::Text(text.into()))
            .await;
    }

    let _ = socket.close().await;
}

/// Parse channel to get model_id
fn parse_model_id_from_channel(channel: &str) -> Option<String> {
    // channel format: "modsrv:{model_id}:update"
    let parts: Vec<&str> = channel.split(':').collect();
    if parts.len() == 3 && parts[0] == "modsrv" && parts[2] == "update" {
        Some(parts[1].to_string())
    } else {
        None
    }
}

/// Parse update payload
fn parse_update_payload(payload: &str) -> Option<(String, f64)> {
    // payload format: "{point}:{value}"
    let parts: Vec<&str> = payload.split(':').collect();
    if parts.len() == 2 {
        if let Ok(value) = parts[1].parse::<f64>() {
            return Some((parts[0].to_string(), value));
        }
    }
    None
}
