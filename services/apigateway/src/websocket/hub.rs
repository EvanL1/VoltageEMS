use actix::{Actor, Addr, Context, Handler, Message as ActixMessage};
use log::{debug, info, warn};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::redis_client::RedisClient;
use crate::websocket::protocol::{DataType, InternalMessage, SubscriptionFilter, WsMessage};
use crate::websocket::protocol::codec::WsMessageWrapper;
use crate::websocket::session::WsSession;

/// WebSocket连接管理中心
pub struct WsHub {
    /// 所有活跃的会话 (session_id -> session_addr)
    sessions: HashMap<String, Addr<WsSession>>,
    
    /// 通道订阅者 (channel_id -> Set<session_id>)
    channel_subscribers: HashMap<u32, HashSet<String>>,
    
    /// 数据类型订阅者 (data_type -> Set<session_id>)
    type_subscribers: HashMap<DataType, HashSet<String>>,
    
    /// 会话订阅信息 (session_id -> SubscriptionInfo)
    session_subscriptions: HashMap<String, SubscriptionInfo>,
    
    /// Redis客户端
    redis: Arc<RedisClient>,
}

/// 会话订阅信息
#[derive(Debug, Default)]
struct SubscriptionInfo {
    channels: HashSet<u32>,
    types: HashSet<DataType>,
}

impl WsHub {
    pub fn new(redis: Arc<RedisClient>) -> Self {
        Self {
            sessions: HashMap::new(),
            channel_subscribers: HashMap::new(),
            type_subscribers: HashMap::new(),
            session_subscriptions: HashMap::new(),
            redis,
        }
    }

    /// 添加会话
    fn add_session(&mut self, session_id: String, addr: Addr<WsSession>) {
        info!("Adding session to hub: {}", session_id);
        self.sessions.insert(session_id.clone(), addr);
        self.session_subscriptions.insert(session_id.clone(), SubscriptionInfo::default());
        info!("Total sessions: {}, session_subscriptions: {}", self.sessions.len(), self.session_subscriptions.len());
    }

    /// 移除会话
    fn remove_session(&mut self, session_id: &str) {
        info!("Removing session from hub: {}", session_id);
        
        // 获取会话的订阅信息
        if let Some(sub_info) = self.session_subscriptions.remove(session_id) {
            // 清理通道订阅
            for channel in &sub_info.channels {
                if let Some(subscribers) = self.channel_subscribers.get_mut(channel) {
                    subscribers.remove(session_id);
                    if subscribers.is_empty() {
                        self.channel_subscribers.remove(channel);
                    }
                }
            }
            
            // 清理类型订阅
            for data_type in &sub_info.types {
                if let Some(subscribers) = self.type_subscribers.get_mut(data_type) {
                    subscribers.remove(session_id);
                    if subscribers.is_empty() {
                        self.type_subscribers.remove(data_type);
                    }
                }
            }
        }
        
        // 移除会话
        self.sessions.remove(session_id);
    }

    /// 处理订阅
    fn handle_subscribe(&mut self, session_id: String, filter: SubscriptionFilter) {
        info!("Handling subscribe for session {}: {:?}", session_id, filter);
        
        if let Some(sub_info) = self.session_subscriptions.get_mut(&session_id) {
            // 更新通道订阅
            for channel in &filter.channels {
                sub_info.channels.insert(*channel);
                self.channel_subscribers
                    .entry(*channel)
                    .or_insert_with(HashSet::new)
                    .insert(session_id.clone());
                info!("Session {} subscribed to channel {}", session_id, channel);
            }
            
            // 更新类型订阅
            for data_type in &filter.types {
                sub_info.types.insert(*data_type);
                self.type_subscribers
                    .entry(*data_type)
                    .or_insert_with(HashSet::new)
                    .insert(session_id.clone());
                info!("Session {} subscribed to type {:?}", session_id, data_type);
            }
            info!("Subscription complete. Channel subscribers: {:?}", self.channel_subscribers.get(&filter.channels[0]));
        } else {
            warn!("Session {} not found in session_subscriptions", session_id);
        }
    }

    /// 处理取消订阅
    fn handle_unsubscribe(&mut self, session_id: String, filter: SubscriptionFilter) {
        debug!("Handling unsubscribe for session {}: {:?}", session_id, filter);
        
        if let Some(sub_info) = self.session_subscriptions.get_mut(&session_id) {
            // 只移除指定的数据类型订阅，保留通道订阅
            for data_type in &filter.types {
                sub_info.types.remove(data_type);
                if let Some(subscribers) = self.type_subscribers.get_mut(data_type) {
                    subscribers.remove(&session_id);
                    if subscribers.is_empty() {
                        self.type_subscribers.remove(data_type);
                    }
                }
            }
            
            // 只有当没有任何数据类型订阅时，才移除通道订阅
            if sub_info.types.is_empty() {
                for channel in &filter.channels {
                    sub_info.channels.remove(channel);
                    if let Some(subscribers) = self.channel_subscribers.get_mut(channel) {
                        subscribers.remove(&session_id);
                        if subscribers.is_empty() {
                            self.channel_subscribers.remove(channel);
                        }
                    }
                }
            }
        }
    }

    /// 广播消息给订阅者
    fn broadcast(&self, channel_id: u32, data_type: DataType, message: WsMessage) {
        debug!("Broadcasting message for channel {} with type {:?}", channel_id, data_type);
        let mut target_sessions = HashSet::new();
        
        // 收集订阅了该通道的会话
        if let Some(channel_subs) = self.channel_subscribers.get(&channel_id) {
            debug!("Found {} sessions subscribed to channel {}", channel_subs.len(), channel_id);
            target_sessions.extend(channel_subs.iter().cloned());
        } else {
            debug!("No sessions subscribed to channel {}", channel_id);
        }
        
        // 收集订阅了该数据类型的会话
        if let Some(type_subs) = self.type_subscribers.get(&data_type) {
            for session_id in type_subs {
                if let Some(sub_info) = self.session_subscriptions.get(session_id) {
                    // 只有同时订阅了通道和类型的会话才接收消息
                    if sub_info.channels.contains(&channel_id) {
                        target_sessions.insert(session_id.clone());
                    }
                }
            }
        }
        
        // 收集订阅了"All"类型的会话
        if let Some(all_subs) = self.type_subscribers.get(&DataType::All) {
            for session_id in all_subs {
                if let Some(sub_info) = self.session_subscriptions.get(session_id) {
                    if sub_info.channels.contains(&channel_id) {
                        target_sessions.insert(session_id.to_string());
                    }
                }
            }
        }
        
        // 发送消息给目标会话
        debug!("Broadcasting to {} target sessions", target_sessions.len());
        let wrapped_msg = WsMessageWrapper(message);
        for session_id in &target_sessions {
            if let Some(addr) = self.sessions.get(session_id) {
                debug!("Sending message to session {}", session_id);
                addr.do_send(wrapped_msg.clone());
            } else {
                warn!("Session {} not found in sessions map", session_id);
            }
        }
    }

    /// 广播消息给所有会话
    pub fn broadcast_all(&self, message: WsMessage) {
        let wrapped_msg = WsMessageWrapper(message);
        for (_, addr) in &self.sessions {
            addr.do_send(wrapped_msg.clone());
        }
    }

    /// 获取当前连接数
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// 获取订阅统计信息
    pub fn get_stats(&self) -> HubStats {
        HubStats {
            total_sessions: self.sessions.len(),
            channel_subscriptions: self.channel_subscribers.len(),
            type_subscriptions: self.type_subscribers.len(),
        }
    }
}

impl Actor for WsHub {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("WebSocket Hub started");
    }
}

/// 处理内部消息
impl Handler<InternalMessage> for WsHub {
    type Result = ();

    fn handle(&mut self, msg: InternalMessage, _ctx: &mut Self::Context) {
        match msg {
            InternalMessage::Connect { session_id, addr } => {
                self.add_session(session_id, addr);
            }
            InternalMessage::Disconnect { session_id } => {
                self.remove_session(&session_id);
            }
            InternalMessage::Subscribe { session_id, filter } => {
                self.handle_subscribe(session_id, filter);
            }
            InternalMessage::Unsubscribe { session_id, filter } => {
                self.handle_unsubscribe(session_id, filter);
            }
            InternalMessage::Broadcast { channel_id, data_type, message } => {
                self.broadcast(channel_id, data_type, message);
            }
        }
    }
}

/// Hub统计信息
#[derive(Debug, Clone, actix::MessageResponse)]
pub struct HubStats {
    pub total_sessions: usize,
    pub channel_subscriptions: usize,
    pub type_subscriptions: usize,
}

/// 获取Hub统计信息的消息
#[derive(ActixMessage)]
#[rtype(result = "HubStats")]
pub struct GetStats;

impl Handler<GetStats> for WsHub {
    type Result = HubStats;

    fn handle(&mut self, _msg: GetStats, _ctx: &mut Self::Context) -> Self::Result {
        self.get_stats()
    }
}

/// 广播消息给所有连接的消息
#[derive(ActixMessage)]
#[rtype(result = "()")]
pub struct BroadcastAll(pub WsMessage);

impl Handler<BroadcastAll> for WsHub {
    type Result = ();

    fn handle(&mut self, msg: BroadcastAll, _ctx: &mut Self::Context) {
        self.broadcast_all(msg.0);
    }
}