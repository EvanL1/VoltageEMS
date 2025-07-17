pub mod hub;
pub mod handlers;

use axum::{
    extract::{ws::WebSocketUpgrade, State},
    response::IntoResponse,
};
use serde::Deserialize;

use crate::AppState;

#[derive(Debug, Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

/// WebSocket connection handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // For now, we'll allow anonymous connections
    // TODO: Add token extraction from query parameters or headers
    let user_info: Option<(String, String)> = None;

    ws.on_upgrade(move |socket| handle_socket(socket, state, user_info))
}

async fn handle_socket(
    socket: axum::extract::ws::WebSocket,
    state: AppState,
    user_info: Option<(String, String)>,
) {
    use futures_util::{sink::SinkExt, stream::StreamExt};
    use tokio::sync::mpsc;

    let session_id = uuid::Uuid::new_v4().to_string();
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Register session with hub
    {
        let mut hub = state.ws_hub.write().await;
        hub.register_session(session_id.clone(), tx);
    }

    let (mut ws_sender, mut ws_receiver) = socket.split();

    log::info!(
        "WebSocket session {} connected{}",
        session_id,
        if let Some((_, ref username)) = user_info {
            format!(" (user: {})", username)
        } else {
            String::new()
        }
    );

    // Task to receive messages from hub and send to WebSocket
    let session_id_tx = session_id.clone();
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                hub::HubMessage::Text(text) => {
                    if ws_sender
                        .send(axum::extract::ws::Message::Text(text.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                hub::HubMessage::Binary(data) => {
                    if ws_sender
                        .send(axum::extract::ws::Message::Binary(data.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                hub::HubMessage::Close => {
                    let _ = ws_sender.send(axum::extract::ws::Message::Close(None)).await;
                    break;
                }
            }
        }
        log::debug!("WebSocket send task ended for session {}", session_id_tx);
    });

    // Task to receive messages from WebSocket and handle them
    let session_id_rx = session_id.clone();
    let state_rx = state.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                axum::extract::ws::Message::Text(text) => {
                    if let Err(e) = handle_websocket_message(&session_id_rx, &text, &state_rx).await {
                        log::error!("Error handling WebSocket message: {}", e);
                    }
                }
                axum::extract::ws::Message::Binary(_) => {
                    log::debug!("Binary WebSocket messages not supported");
                }
                axum::extract::ws::Message::Close(_) => {
                    break;
                }
                _ => {}
            }
        }
        log::debug!("WebSocket receive task ended for session {}", session_id_rx);
    });

    // Wait for either task to complete
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    // Cleanup: unregister session from hub
    {
        let mut hub = state.ws_hub.write().await;
        hub.unregister_session(&session_id);
    }

    log::info!("WebSocket session {} disconnected", session_id);
}

async fn handle_websocket_message(
    session_id: &str,
    message: &str,
    state: &AppState,
) -> Result<(), Box<dyn std::error::Error>> {
    // Parse message as JSON
    let msg: serde_json::Value = serde_json::from_str(message)?;

    // Handle different message types
    match msg.get("type").and_then(|t| t.as_str()) {
        Some("subscribe") => {
            if let Ok(filter) = serde_json::from_value::<hub::SubscriptionFilter>(
                msg.get("data").cloned().unwrap_or_default(),
            ) {
                let mut hub = state.ws_hub.write().await;
                hub.subscribe(session_id.to_string(), filter);
                
                // Send acknowledgment
                let ack = serde_json::json!({
                    "type": "subscribed",
                    "data": {
                        "session_id": session_id,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    }
                });
                hub.send_to_session(&session_id.to_string(), hub::HubMessage::Text(ack.to_string()));
            }
        }
        Some("unsubscribe") => {
            if let Ok(filter) = serde_json::from_value::<hub::SubscriptionFilter>(
                msg.get("data").cloned().unwrap_or_default(),
            ) {
                let mut hub = state.ws_hub.write().await;
                hub.unsubscribe(session_id.to_string(), filter);
                
                // Send acknowledgment
                let ack = serde_json::json!({
                    "type": "unsubscribed",
                    "data": {
                        "session_id": session_id,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    }
                });
                hub.send_to_session(&session_id.to_string(), hub::HubMessage::Text(ack.to_string()));
            }
        }
        Some("ping") => {
            let hub = state.ws_hub.read().await;
            let pong = serde_json::json!({
                "type": "pong",
                "data": {
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }
            });
            hub.send_to_session(&session_id.to_string(), hub::HubMessage::Text(pong.to_string()));
        }
        _ => {
            log::debug!("Unknown WebSocket message type: {:?}", msg);
        }
    }

    Ok(())
}