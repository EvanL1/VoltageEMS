//! J1939 Protocol Client Implementation
//!
//! Implements the igw Protocol traits for J1939/CAN communication.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use socketcan::{CanSocket, EmbeddedFrame, Frame, Socket};
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use voltage_j1939::{build_spn_database, database_stats, decode_frame, extract_source_address};

use crate::protocols::core::data::{DataBatch, DataPoint, Value};
use crate::protocols::core::error::{GatewayError, Result};
use crate::protocols::core::quality::Quality;
use crate::protocols::core::slot::SlotStore;
use crate::protocols::core::traits::{
    AdjustmentCommand, CommunicationMode, ConnectionState, ControlCommand, DataEvent,
    DataEventHandler, DataEventReceiver, DataEventSender, Diagnostics, EventDrivenProtocol,
    PollResult, Protocol, ProtocolCapabilities, ProtocolClient, WriteResult,
};

// ============================================================================
// Configuration
// ============================================================================

/// J1939 client configuration.
#[derive(Debug, Clone)]
pub struct J1939Config {
    /// CAN interface name (e.g., "can0").
    pub can_interface: String,

    /// Source address of the target device (ECU address).
    pub source_address: u8,

    /// Our address for sending request PGNs.
    pub our_address: u8,

    /// Request interval for on-demand PGNs in milliseconds.
    pub request_interval_ms: u64,
}

impl Default for J1939Config {
    fn default() -> Self {
        Self {
            can_interface: "can0".to_string(),
            source_address: 0x00,
            our_address: 0xFE,
            request_interval_ms: 1000,
        }
    }
}

// ============================================================================
// J1939Client
// ============================================================================

/// J1939 protocol client.
///
/// Implements event-driven communication over CAN bus using the SAE J1939 protocol.
/// Uses `voltage_j1939` crate for protocol parsing and SPN database.
pub struct J1939Client {
    config: J1939Config,

    // Connection state
    connection_state: Arc<RwLock<ConnectionState>>,
    is_connected: Arc<AtomicBool>,

    // Statistics
    read_count: Arc<AtomicU64>,
    error_count: Arc<AtomicU64>,
    last_error: Arc<RwLock<Option<String>>>,

    // Tasks
    receive_handle: Option<JoinHandle<()>>,

    // Event channel (broadcast for multiple subscribers)
    event_tx: DataEventSender,
    event_handler: Option<Arc<dyn DataEventHandler>>,

    // Slot store for cached data - pre-built from J1939 SPN database
    slot_store: Arc<SlotStore>,
}

impl J1939Client {
    /// Create a new J1939 client with the given configuration.
    pub fn new(config: J1939Config) -> Self {
        // Use broadcast channel for multiple subscribers
        let (event_tx, _) = broadcast::channel(1024);

        // Build SlotStore from J1939 SPN database - all known SPNs pre-indexed
        let spn_db = build_spn_database();
        let spn_ids: Vec<u32> = spn_db.keys().copied().collect();
        let slot_store = Arc::new(SlotStore::from_points(&spn_ids));

        Self {
            config,
            connection_state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            is_connected: Arc::new(AtomicBool::new(false)),
            read_count: Arc::new(AtomicU64::new(0)),
            error_count: Arc::new(AtomicU64::new(0)),
            last_error: Arc::new(RwLock::new(None)),
            receive_handle: None,
            event_tx,
            event_handler: None,
            slot_store,
        }
    }

    /// Start the receive task.
    fn start_receive_task(&mut self) -> Result<()> {
        let can_interface = self.config.can_interface.clone();
        let source_address = self.config.source_address;
        let is_connected = Arc::clone(&self.is_connected);
        let slot_store = Arc::clone(&self.slot_store);
        let read_count = Arc::clone(&self.read_count);
        let error_count = Arc::clone(&self.error_count);
        let last_error = Arc::clone(&self.last_error);
        let event_tx = self.event_tx.clone();
        let event_handler = self.event_handler.clone();

        let handle = tokio::spawn(async move {
            let socket = match CanSocket::open(&can_interface) {
                Ok(s) => s,
                Err(e) => {
                    *last_error.write().await = Some(format!("Failed to open CAN socket: {}", e));
                    error_count.fetch_add(1, Ordering::Relaxed);
                    return;
                },
            };

            loop {
                if !is_connected.load(Ordering::SeqCst) {
                    break;
                }

                match socket.read_frame() {
                    Ok(frame) => {
                        if let Some(id) = frame.id().as_extended() {
                            let can_id = id.as_raw();
                            let sa = extract_source_address(can_id);

                            // Filter by source address
                            if sa != source_address {
                                continue;
                            }

                            // Use voltage_j1939 to decode the frame
                            let decoded_spns = decode_frame(can_id, frame.data());
                            if decoded_spns.is_empty() {
                                continue;
                            }

                            // Pre-allocate batch and update slot store (lock-free)
                            let mut batch = DataBatch::with_capacity(decoded_spns.len());

                            for d in decoded_spns {
                                let value = Value::Float(d.value);

                                // Update slot store (lock-free atomic operation)
                                slot_store.update(d.spn, value.clone(), Quality::Good);

                                // Add to batch for event dispatch
                                let data_point = DataPoint::new(d.spn, value);
                                batch.add(data_point);
                            }

                            if !batch.is_empty() {
                                read_count.fetch_add(1, Ordering::Relaxed);

                                // Send event (broadcast is sync)
                                // Arc wrap for O(1) broadcast clone
                                let _ =
                                    event_tx.send(DataEvent::DataUpdate(Arc::new(batch.clone())));

                                // Call handler
                                if let Some(ref handler) = event_handler {
                                    handler.on_data_update(batch).await;
                                }
                            }
                        }
                    },
                    Err(e) => {
                        *last_error.write().await = Some(format!("CAN read error: {}", e));
                        error_count.fetch_add(1, Ordering::Relaxed);
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    },
                }
            }
        });

        self.receive_handle = Some(handle);
        Ok(())
    }
}

// ============================================================================
// Trait Implementations
// ============================================================================

impl ProtocolCapabilities for J1939Client {
    fn name(&self) -> &'static str {
        "J1939"
    }

    fn supported_modes(&self) -> &[CommunicationMode] {
        &[CommunicationMode::EventDriven]
    }

    fn supports_client(&self) -> bool {
        true
    }

    fn supports_server(&self) -> bool {
        false
    }

    fn version(&self) -> &'static str {
        "SAE J1939-21"
    }
}

impl Protocol for J1939Client {
    fn connection_state(&self) -> ConnectionState {
        *futures::executor::block_on(self.connection_state.read())
    }

    async fn diagnostics(&self) -> Result<Diagnostics> {
        let (spn_count, pgn_count) = database_stats();

        Ok(Diagnostics {
            protocol: "J1939".to_string(),
            connection_state: *self.connection_state.read().await,
            read_count: self.read_count.load(Ordering::Relaxed),
            write_count: 0,
            error_count: self.error_count.load(Ordering::Relaxed),
            last_error: self.last_error.read().await.clone(),
            extra: serde_json::json!({
                "can_interface": self.config.can_interface,
                "source_address": format!("0x{:02X}", self.config.source_address),
                "spn_count": spn_count,
                "pgn_count": pgn_count,
            }),
        })
    }
}

impl ProtocolClient for J1939Client {
    async fn connect(&mut self) -> Result<()> {
        *self.connection_state.write().await = ConnectionState::Connecting;

        // Verify CAN interface exists
        let _socket = CanSocket::open(&self.config.can_interface).map_err(|e| {
            GatewayError::Connection(format!(
                "Failed to open CAN interface {}: {}",
                self.config.can_interface, e
            ))
        })?;

        self.is_connected.store(true, Ordering::SeqCst);
        *self.connection_state.write().await = ConnectionState::Connected;

        // Start receive task
        self.start_receive_task()?;

        // Notify connection change (broadcast is sync)
        let _ = self
            .event_tx
            .send(DataEvent::ConnectionChanged(ConnectionState::Connected));
        if let Some(ref handler) = self.event_handler {
            handler
                .on_connection_changed(ConnectionState::Connected)
                .await;
        }

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.is_connected.store(false, Ordering::SeqCst);

        if let Some(handle) = self.receive_handle.take() {
            handle.abort();
        }

        *self.connection_state.write().await = ConnectionState::Disconnected;

        // Notify connection change (broadcast is sync)
        let _ = self
            .event_tx
            .send(DataEvent::ConnectionChanged(ConnectionState::Disconnected));
        if let Some(ref handler) = self.event_handler {
            handler
                .on_connection_changed(ConnectionState::Disconnected)
                .await;
        }

        Ok(())
    }

    async fn write_control(&mut self, _commands: &[ControlCommand]) -> Result<WriteResult> {
        // J1939 control requires proprietary PGN support
        Err(GatewayError::Unsupported(
            "J1939 control commands require proprietary PGN implementation".to_string(),
        ))
    }

    async fn poll_once(&mut self) -> PollResult {
        // J1939 is event-driven, export all cached data from slot store
        let batch = self.slot_store.export_all();
        PollResult::success(batch)
    }

    async fn write_adjustment(
        &mut self,
        _adjustments: &[AdjustmentCommand],
    ) -> Result<WriteResult> {
        // J1939 adjustment requires proprietary PGN support
        Err(GatewayError::Unsupported(
            "J1939 adjustment commands require proprietary PGN implementation".to_string(),
        ))
    }
}

impl EventDrivenProtocol for J1939Client {
    fn subscribe(&self) -> DataEventReceiver {
        // Broadcast channel supports multiple subscribers
        self.event_tx.subscribe()
    }

    fn set_event_handler(&mut self, handler: Arc<dyn DataEventHandler>) {
        self.event_handler = Some(handler);
    }

    async fn start(&mut self) -> Result<()> {
        // For J1939, start is handled in connect() which starts the receive task
        // This is a no-op since the receive task is already running
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        // Stop the receive task
        if let Some(handle) = self.receive_handle.take() {
            handle.abort();
        }
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use voltage_j1939::parse_can_id;

    #[test]
    fn test_parse_can_id() {
        // EEC1 from SA=0x00: CAN ID = 0x0CF00400
        let id = parse_can_id(0x0CF00400);
        assert_eq!(id.priority, 3);
        assert_eq!(id.pgn, 61444);
        assert_eq!(id.source_address, 0x00);

        // ET1 from SA=0x00: CAN ID = 0x18FEEE00
        let id = parse_can_id(0x18FEEE00);
        assert_eq!(id.priority, 6);
        assert_eq!(id.pgn, 65262);
        assert_eq!(id.source_address, 0x00);
    }

    #[test]
    fn test_decode_frame() {
        // EEC1 frame
        let can_id = 0x0CF00400;
        let data = [0x00, 0x00, 0x00, 0x20, 0x4E, 0x00, 0x00, 0x00];
        let decoded = decode_frame(can_id, &data);
        assert!(!decoded.is_empty());

        // Find engine speed (SPN 190)
        let engine_speed = decoded.iter().find(|d| d.spn == 190);
        assert!(engine_speed.is_some());
        assert_eq!(engine_speed.unwrap().value, 2500.0);
    }

    #[test]
    fn test_config_default() {
        let config = J1939Config::default();
        assert_eq!(config.can_interface, "can0");
        assert_eq!(config.source_address, 0x00);
        assert_eq!(config.our_address, 0xFE);
    }

    #[test]
    fn test_client_creation() {
        let config = J1939Config::default();
        let client = J1939Client::new(config);
        assert_eq!(client.name(), "J1939");
        assert_eq!(client.supported_modes(), &[CommunicationMode::EventDriven]);
    }
}
