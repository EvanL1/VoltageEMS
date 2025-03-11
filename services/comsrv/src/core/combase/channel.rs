//! Channel management module
//!
//! Handles channel lifecycle, metadata, and operations

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

use crate::core::combase::traits::ComClient;
use crate::core::combase::trigger::CommandTrigger;
use crate::core::config::ChannelConfig;

/// Channel metadata
#[derive(Debug, Clone)]
pub struct ChannelMetadata {
    pub name: Arc<str>,
    pub protocol_type: String,
    pub created_at: Instant,
    pub last_accessed: Arc<RwLock<Instant>>,
}

/// Channel entry, combining channel and metadata
#[derive(Clone)]
pub struct ChannelEntry {
    pub channel: Arc<RwLock<Box<dyn ComClient>>>,
    pub metadata: ChannelMetadata,
    pub command_trigger: Option<Arc<RwLock<CommandTrigger>>>,
    pub channel_config: Arc<ChannelConfig>,
}

impl std::fmt::Debug for ChannelEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelEntry")
            .field("metadata", &self.metadata)
            .finish_non_exhaustive()
    }
}

/// Channel statistics
#[derive(Debug, Clone)]
pub struct ChannelStats {
    pub channel_id: u16,
    pub name: String,
    pub protocol_type: String,
    pub is_connected: bool,
    pub created_at: Instant,
    pub last_accessed: Instant,
}

impl ChannelEntry {
    /// Create new channel entry
    pub fn new(
        channel: Arc<RwLock<Box<dyn ComClient>>>,
        channel_config: Arc<ChannelConfig>,
        protocol_type: String,
        command_trigger: Option<Arc<RwLock<CommandTrigger>>>,
    ) -> Self {
        let metadata = ChannelMetadata {
            name: Arc::from(channel_config.name()),
            protocol_type,
            created_at: Instant::now(),
            last_accessed: Arc::new(RwLock::new(Instant::now())),
        };

        Self {
            channel,
            metadata,
            command_trigger,
            channel_config,
        }
    }

    /// Get channel statistics
    pub async fn get_stats(&self, channel_id: u16) -> ChannelStats {
        let channel = self.channel.read().await;
        let last_accessed = *self.metadata.last_accessed.read().await;

        ChannelStats {
            channel_id,
            name: self.metadata.name.to_string(),
            protocol_type: self.metadata.protocol_type.clone(),
            is_connected: channel.is_connected(),
            created_at: self.metadata.created_at,
            last_accessed,
        }
    }

    /// Update last accessed time
    pub async fn touch(&self) {
        let mut last_accessed = self.metadata.last_accessed.write().await;
        *last_accessed = Instant::now();
    }
}
