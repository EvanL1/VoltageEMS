
use async_trait::async_trait;
use serde_json::Value;

use crate::core::config::point::Point;

/// A trait for protocol adapters, providing a unified interface for different protocols.
#[async_trait]
pub trait Adapter: Send + Sync {
    /// Returns a unique identifier for the adapter (e.g., "modbus", "mqtt").
    fn id(&self) -> &'static str;

    /// Initializes the adapter, establishing any necessary connections.
    async fn init(&mut self) -> anyhow::Result<()>;

    /// Reads a value from the specified point.
    async fn read(&self, point: &Point) -> anyhow::Result<Value>;

    /// Writes a value to the specified point.
    async fn write(&self, point: &Point, value: Value) -> anyhow::Result<()>;
}
