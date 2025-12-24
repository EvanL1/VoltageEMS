//! Channels API - Library Mode
//!
//! Direct library calls to comsrv for channel management

use crate::context::ComsrvContext;
use crate::lib_api::{LibApiError, Result};
use serde::{Deserialize, Serialize};
use voltage_rtdb::Rtdb; // Trait must be in scope for method calls

// Use chrono from sqlx::types for timestamp generation
use sqlx::types::chrono::Utc;

/// Channel summary for list operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelSummary {
    pub id: u32,
    pub name: String,
    pub protocol: String,
    pub enabled: bool,
    pub connected: bool,
}

/// Channel detailed status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelStatus {
    pub id: u32,
    pub name: String,
    pub protocol: String,
    pub enabled: bool,
    pub connected: bool,
    pub connection_info: Option<String>,
    pub last_error: Option<String>,
}

/// Channels service - provides channel management operations
pub struct ChannelsService<'a> {
    ctx: &'a ComsrvContext,
}

impl<'a> ChannelsService<'a> {
    /// Create a new channels service from context
    pub fn new(ctx: &'a ComsrvContext) -> Self {
        Self { ctx }
    }

    /// List all channels
    ///
    /// Returns a list of channel summaries including connection status.
    /// This method queries the database for configuration and the channel
    /// manager for runtime status.
    pub async fn list(&self) -> Result<Vec<ChannelSummary>> {
        // Query database for channel configurations
        let db_channels: Vec<(u32, String, String, bool)> = sqlx::query_as(
            "SELECT channel_id, name, protocol, enabled FROM channels ORDER BY channel_id",
        )
        .fetch_all(&self.ctx.sqlite_pool)
        .await?;

        // Get runtime status from channel manager
        let manager = self.ctx.channel_manager.read().await;
        let mut summaries = Vec::new();

        for (channel_id, name, protocol, enabled) in db_channels {
            // Check if channel is connected at runtime
            let connected = manager
                .get_channel(channel_id)
                .map(|_ch| {
                    // Access the channel's runtime status
                    // Note: This assumes channels have a is_connected method
                    // If not available, default to false
                    false // TODO: Implement connection status check
                })
                .unwrap_or(false);

            summaries.push(ChannelSummary {
                id: channel_id,
                name,
                protocol,
                enabled,
                connected,
            });
        }

        Ok(summaries)
    }

    /// Get channel status by ID
    ///
    /// Returns detailed status information for a specific channel including
    /// configuration, connection status, and any error information.
    pub async fn get_status(&self, channel_id: u32) -> Result<ChannelStatus> {
        // Query database for channel configuration
        let db_channel: Option<(u32, String, String, bool)> = sqlx::query_as(
            "SELECT channel_id, name, protocol, enabled FROM channels WHERE channel_id = ?",
        )
        .bind(channel_id)
        .fetch_optional(&self.ctx.sqlite_pool)
        .await?;

        let (_, name, protocol, enabled) = db_channel
            .ok_or_else(|| LibApiError::not_found(format!("Channel {} not found", channel_id)))?;

        // Get runtime status from channel manager
        let manager = self.ctx.channel_manager.read().await;
        let (connected, connection_info, last_error) = manager
            .get_channel(channel_id)
            .map(|_ch| {
                // TODO: Extract actual connection info and error from channel
                (false, None, None)
            })
            .unwrap_or((false, None, None));

        Ok(ChannelStatus {
            id: channel_id,
            name,
            protocol,
            enabled,
            connected,
            connection_info,
            last_error,
        })
    }

    /// Send control command (C)
    ///
    /// Sends a control command to a specific point on a channel.
    /// Value should be 0 or 1 for digital control.
    #[allow(clippy::disallowed_methods)] // serde_json::json! macro uses unwrap internally
    pub async fn send_control(&self, channel_id: u32, point_id: u32, value: u8) -> Result<()> {
        if value > 1 {
            return Err(
                LibApiError::invalid_input("Control value must be 0 or 1".to_string()).into(),
            );
        }

        // Build the key for the control point
        let key = format!("comsrv:{}:C", channel_id);
        let todo_key = format!("comsrv:{}:C:TODO", channel_id);

        // Write to RTDB (this will trigger the channel to send the command)
        self.ctx
            .rtdb
            .hash_set(&key, &point_id.to_string(), value.to_string().into())
            .await?;

        // Add to TODO queue for processing
        let command = serde_json::json!({
            "point_id": point_id,
            "value": value,
            "timestamp": Utc::now().timestamp_millis(),
        });
        self.ctx
            .rtdb
            .list_rpush(&todo_key, serde_json::to_string(&command)?.into())
            .await?;

        Ok(())
    }

    /// Send adjustment command (A)
    ///
    /// Sends an analog adjustment value to a specific point on a channel.
    #[allow(clippy::disallowed_methods)] // serde_json::json! macro uses unwrap internally
    pub async fn send_adjustment(&self, channel_id: u32, point_id: u32, value: f64) -> Result<()> {
        // Write to Redis TODO queue via channel manager
        let key = format!("comsrv:{}:A", channel_id);
        let todo_key = format!("comsrv:{}:A:TODO", channel_id);

        // Write to RTDB
        self.ctx
            .rtdb
            .hash_set(&key, &point_id.to_string(), value.to_string().into())
            .await?;

        // Add to TODO queue for processing
        let command = serde_json::json!({
            "point_id": point_id,
            "value": value,
            "timestamp": Utc::now().timestamp_millis(),
        });
        self.ctx
            .rtdb
            .list_rpush(&todo_key, serde_json::to_string(&command)?.into())
            .await?;

        Ok(())
    }

    /// Reload channel configurations
    ///
    /// Triggers a hot reload of all channel configurations from the database.
    /// This is equivalent to calling the /api/channels/reload HTTP endpoint.
    pub async fn reload(&self) -> Result<String> {
        // TODO: Call comsrv's reload functionality
        // For now, return a simple message
        // In full implementation, this would call the ReloadableService trait
        Ok("Channel reload not yet implemented in lib mode".to_string())
    }

    /// Get telemetry points (T)
    ///
    /// Retrieves all telemetry point values for a channel.
    /// Public API for data access.
    #[allow(dead_code)]
    pub async fn get_telemetry_points(&self, channel_id: u16) -> Result<Vec<(String, String)>> {
        let key = format!("comsrv:{}:T", channel_id);
        let points = self.ctx.rtdb.hash_get_all(&key).await?;

        let result: Vec<(String, String)> = points
            .into_iter()
            .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
            .collect();

        Ok(result)
    }

    /// Get signal points (S)
    ///
    /// Retrieves all signal point values for a channel.
    /// Public API for data access.
    #[allow(dead_code)]
    pub async fn get_signal_points(&self, channel_id: u16) -> Result<Vec<(String, String)>> {
        let key = format!("comsrv:{}:S", channel_id);
        let points = self.ctx.rtdb.hash_get_all(&key).await?;

        let result: Vec<(String, String)> = points
            .into_iter()
            .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
            .collect();

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    // Note: Tests would require a full service context setup
    // For now, we'll skip unit tests and rely on integration tests
}
