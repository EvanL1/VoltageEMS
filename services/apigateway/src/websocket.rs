use actix::{fut, Actor, ActorContext, AsyncContext, Handler, Message, StreamHandler};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use futures::future::{ready, Ready};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::redis_client::RedisClient;

/// WebSocket heartbeat interval
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// Client timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// WebSocket message types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    /// Subscribe to channels
    Subscribe {
        channels: Vec<String>,
    },
    /// Unsubscribe from channels
    Unsubscribe {
        channels: Vec<String>,
    },
    /// Data update from Redis
    DataUpdate {
        channel: String,
        data: serde_json::Value,
    },
    /// Heartbeat
    Ping,
    Pong,
    /// Error message
    Error {
        message: String,
    },
}

/// WebSocket connection actor
pub struct WsConnection {
    /// Client heartbeat
    hb: Instant,
    /// Redis client
    redis_client: Arc<RedisClient>,
    /// Subscribed channels
    subscribed_channels: Vec<String>,
    /// Redis subscription receiver
    redis_rx: Option<mpsc::Receiver<(String, String)>>,
}

impl WsConnection {
    pub fn new(redis_client: Arc<RedisClient>) -> Self {
        Self {
            hb: Instant::now(),
            redis_client,
            subscribed_channels: Vec::new(),
            redis_rx: None,
        }
    }

    /// Start heartbeat
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                info!("WebSocket client heartbeat failed, disconnecting!");
                ctx.stop();
                return;
            }

            ctx.text(serde_json::to_string(&WsMessage::Ping).unwrap());
        });
    }

    /// Subscribe to Redis channels
    fn subscribe_channels(&mut self, channels: Vec<String>) {
        // Create channel for receiving Redis messages
        let (_tx, rx) = mpsc::channel(100);
        self.redis_rx = Some(rx);

        // Subscribe to Redis channels
        for channel in &channels {
            if !self.subscribed_channels.contains(channel) {
                self.subscribed_channels.push(channel.clone());
                // TODO: Implement actual Redis subscription
                info!("Subscribed to channel: {}", channel);
            }
        }
    }

    /// Unsubscribe from Redis channels
    fn unsubscribe_channels(&mut self, channels: Vec<String>) {
        for channel in &channels {
            if let Some(pos) = self.subscribed_channels.iter().position(|x| x == channel) {
                self.subscribed_channels.remove(pos);
                info!("Unsubscribed from channel: {}", channel);
            }
        }
    }
}

impl Actor for WsConnection {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
        info!("WebSocket connection started");
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        info!("WebSocket connection stopped");
    }
}

/// Handler for WebSocket messages
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConnection {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                debug!("Received text message: {}", text);

                // Parse message
                match serde_json::from_str::<WsMessage>(&text) {
                    Ok(msg) => match msg {
                        WsMessage::Subscribe { channels } => {
                            self.subscribe_channels(channels);
                        }
                        WsMessage::Unsubscribe { channels } => {
                            self.unsubscribe_channels(channels);
                        }
                        WsMessage::Ping => {
                            ctx.text(serde_json::to_string(&WsMessage::Pong).unwrap());
                        }
                        _ => {}
                    },
                    Err(e) => {
                        error!("Failed to parse WebSocket message: {}", e);
                        let error_msg = WsMessage::Error {
                            message: format!("Invalid message format: {}", e),
                        };
                        ctx.text(serde_json::to_string(&error_msg).unwrap());
                    }
                }
            }
            Ok(ws::Message::Binary(bin)) => {
                debug!("Received binary message");
                ctx.binary(bin);
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

/// Redis message
#[derive(Message)]
#[rtype(result = "()")]
pub struct RedisMessage {
    pub channel: String,
    pub data: String,
}

/// Handler for Redis messages
impl Handler<RedisMessage> for WsConnection {
    type Result = ();

    fn handle(&mut self, msg: RedisMessage, ctx: &mut Self::Context) {
        // Parse data as JSON
        let data = match serde_json::from_str::<serde_json::Value>(&msg.data) {
            Ok(data) => data,
            Err(_) => serde_json::Value::String(msg.data),
        };

        let ws_msg = WsMessage::DataUpdate {
            channel: msg.channel,
            data,
        };

        ctx.text(serde_json::to_string(&ws_msg).unwrap());
    }
}

/// WebSocket endpoint handler
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    redis_client: web::Data<Arc<RedisClient>>,
) -> Result<HttpResponse, Error> {
    let conn = WsConnection::new(redis_client.get_ref().clone());
    ws::start(conn, &req, stream)
}
