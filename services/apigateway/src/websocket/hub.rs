use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::redis_client::RedisClient;

/// WebSocket session ID type
pub type SessionId = String;

/// WebSocket Hub - manages all WebSocket sessions
pub struct Hub {
    /// Map of session ID to session sender
    sessions: HashMap<SessionId, mpsc::UnboundedSender<HubMessage>>,
    /// Map of channel ID to set of session IDs
    channel_subscribers: HashMap<u32, HashSet<SessionId>>,
    /// Map of data type to set of session IDs
    type_subscribers: HashMap<String, HashSet<SessionId>>,
    /// Session subscription info
    session_subscriptions: HashMap<SessionId, SubscriptionInfo>,
    /// Redis client
    redis_client: Arc<RedisClient>,
}

/// Message types for Hub communication
#[derive(Debug, Clone)]
pub enum HubMessage {
    /// Send text message to session
    Text(String),
    /// Send binary message to session
    Binary(Vec<u8>),
    /// Close session
    Close,
}

/// Subscription information for a session
#[derive(Debug, Clone)]
struct SubscriptionInfo {
    channels: HashSet<u32>,
    types: HashSet<String>,
}

impl SubscriptionInfo {
    fn new() -> Self {
        Self {
            channels: HashSet::new(),
            types: HashSet::new(),
        }
    }
}

/// Subscription filter
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubscriptionFilter {
    pub channels: Vec<u32>,
    pub types: Vec<String>,
}

impl Hub {
    /// Create a new Hub
    pub fn new(redis_client: Arc<RedisClient>) -> Self {
        Self {
            sessions: HashMap::new(),
            channel_subscribers: HashMap::new(),
            type_subscribers: HashMap::new(),
            session_subscriptions: HashMap::new(),
            redis_client,
        }
    }

    /// Register a new session
    pub fn register_session(&mut self, session_id: SessionId, sender: mpsc::UnboundedSender<HubMessage>) {
        info!("Registering WebSocket session: {}", session_id);
        self.sessions.insert(session_id.clone(), sender);
        self.session_subscriptions.insert(session_id, SubscriptionInfo::new());
    }

    /// Unregister a session
    pub fn unregister_session(&mut self, session_id: &SessionId) {
        info!("Unregistering WebSocket session: {}", session_id);
        
        // Remove from all subscriptions
        if let Some(sub_info) = self.session_subscriptions.remove(session_id) {
            // Remove from channel subscribers
            for channel_id in &sub_info.channels {
                if let Some(sessions) = self.channel_subscribers.get_mut(channel_id) {
                    sessions.remove(session_id);
                    if sessions.is_empty() {
                        self.channel_subscribers.remove(channel_id);
                    }
                }
            }
            
            // Remove from type subscribers
            for data_type in &sub_info.types {
                if let Some(sessions) = self.type_subscribers.get_mut(data_type) {
                    sessions.remove(session_id);
                    if sessions.is_empty() {
                        self.type_subscribers.remove(data_type);
                    }
                }
            }
        }
        
        // Remove session sender
        self.sessions.remove(session_id);
    }

    /// Subscribe a session to channels and data types
    pub fn subscribe(&mut self, session_id: SessionId, filter: SubscriptionFilter) {
        debug!("Session {} subscribing to {:?}", session_id, filter);
        
        if let Some(sub_info) = self.session_subscriptions.get_mut(&session_id) {
            // Subscribe to channels
            for channel_id in filter.channels {
                sub_info.channels.insert(channel_id);
                self.channel_subscribers
                    .entry(channel_id)
                    .or_insert_with(HashSet::new)
                    .insert(session_id.clone());
            }
            
            // Subscribe to data types
            for data_type in filter.types {
                sub_info.types.insert(data_type.clone());
                self.type_subscribers
                    .entry(data_type)
                    .or_insert_with(HashSet::new)
                    .insert(session_id.clone());
            }
        }
    }

    /// Unsubscribe a session from channels and data types
    pub fn unsubscribe(&mut self, session_id: SessionId, filter: SubscriptionFilter) {
        debug!("Session {} unsubscribing from {:?}", session_id, filter);
        
        if let Some(sub_info) = self.session_subscriptions.get_mut(&session_id) {
            // Only remove specified data types
            for data_type in &filter.types {
                sub_info.types.remove(data_type);
                
                // Remove from type subscribers
                if let Some(sessions) = self.type_subscribers.get_mut(data_type) {
                    sessions.remove(&session_id);
                    if sessions.is_empty() {
                        self.type_subscribers.remove(data_type);
                    }
                }
            }
            
            // Only remove channel subscription if no data types are subscribed
            if sub_info.types.is_empty() {
                // Remove from channel subscribers
                for channel_id in &filter.channels {
                    sub_info.channels.remove(channel_id);
                    
                    if let Some(sessions) = self.channel_subscribers.get_mut(channel_id) {
                        sessions.remove(&session_id);
                        if sessions.is_empty() {
                            self.channel_subscribers.remove(channel_id);
                        }
                    }
                }
            }
        }
    }

    /// Broadcast message to all sessions subscribed to a channel and data type
    pub fn broadcast_to_channel(&self, channel_id: u32, data_type: &str, message: &str) {
        let mut recipients = HashSet::new();
        
        // Get sessions subscribed to this channel
        if let Some(channel_sessions) = self.channel_subscribers.get(&channel_id) {
            // Get sessions subscribed to this data type
            if let Some(type_sessions) = self.type_subscribers.get(data_type) {
                // Only send to sessions subscribed to both channel and type
                for session_id in channel_sessions.intersection(type_sessions) {
                    recipients.insert(session_id);
                }
            }
        }
        
        // Send message to recipients
        for session_id in recipients {
            if let Some(sender) = self.sessions.get(session_id) {
                if let Err(e) = sender.send(HubMessage::Text(message.to_string())) {
                    error!("Failed to send message to session {}: {}", session_id, e);
                }
            }
        }
    }

    /// Send message to a specific session
    pub fn send_to_session(&self, session_id: &SessionId, message: HubMessage) {
        if let Some(sender) = self.sessions.get(session_id) {
            if let Err(e) = sender.send(message) {
                error!("Failed to send message to session {}: {}", session_id, e);
            }
        }
    }

    /// Get number of active sessions
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Get session subscription info (for debugging)
    pub fn get_session_info(&self, session_id: &SessionId) -> Option<String> {
        self.session_subscriptions.get(session_id).map(|info| {
            format!(
                "Channels: {:?}, Types: {:?}",
                info.channels, info.types
            )
        })
    }
}