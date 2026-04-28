//! IEC 61850 MMS protocol adapter.
//!
//! Provides polling-mode data collection from IEC 61850 IED servers via the
//! MMS (Manufacturing Message Specification) application protocol.
//!
//! # Protocol Stack
//!
//! ```text
//! TCP (port 102) → TPKT → COTP → ISO Session → ISO Presentation → ACSE → MMS
//! ```
//!
//! # YAML Configuration Example
//!
//! ```yaml
//! id: 10
//! name: IED1
//! protocol: iec61850
//! parameters:
//!   address: "192.168.1.10:102"
//!   connect_timeout_ms: 10000
//!   request_timeout_ms: 5000
//! points:
//!   - id: 1001
//!     point_type: Telemetry
//!     name: "AnIn1 magnitude"
//!     address: "simpleIOGenericIO/GGIO1$MX$AnIn1$mag$f"
//! ```

pub mod mms;
pub mod transport;

use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{error, info, warn};
use voltage_model::PointType;

use crate::protocols::core::data::{DataBatch, DataPoint, Value};
use crate::protocols::core::error::{GatewayError, Result};
use crate::protocols::core::point::{PointConfig, TransformConfig};
use crate::protocols::core::quality::Quality;
use crate::protocols::core::traits::{
    ConnectionState, DataEventReceiver, Diagnostics, PointFailure, PollResult,
};
use crate::protocols::gateway::ChannelRuntime;

use self::mms::{MmsValue, build_read_request, parse_read_response};
use self::transport::Framer;

// ── Timeout defaults ──────────────────────────────────────────────────────────

fn default_connect_timeout_ms() -> u64 {
    10_000
}
fn default_request_timeout_ms() -> u64 {
    5_000
}

// ── Parameters config (parsed from YAML/JSON `parameters` block) ──────────────

/// IEC 61850 channel parameters (parsed from the `parameters:` block).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Iec61850ParamsConfig {
    /// Server address, e.g. `"192.168.1.10:102"`. Default port is 102.
    pub address: String,

    /// TCP connect timeout in milliseconds.
    #[serde(default = "default_connect_timeout_ms")]
    pub connect_timeout_ms: u64,

    /// Per-request timeout in milliseconds.
    #[serde(default = "default_request_timeout_ms")]
    pub request_timeout_ms: u64,
}

// ── Channel ───────────────────────────────────────────────────────────────────

/// Stores the parsed IEC 61850 address along with the PointConfig metadata.
struct PointEntry {
    domain: String,
    item: String,
    id: u32,
    point_type: PointType,
    transform: TransformConfig,
}

/// IEC 61850 MMS polling channel.
pub struct Iec61850Channel {
    id: u32,
    name: String,

    address: String,
    connect_timeout: Duration,
    request_timeout: Duration,

    /// Active TCP + framing layer. `None` when disconnected.
    framer: Option<Framer>,

    state: ConnectionState,

    /// Monotonic invoke-ID counter (1–255).
    invoke_id: u8,

    /// Points to poll in order.
    points: Vec<PointEntry>,
}

impl Iec61850Channel {
    pub fn new(
        id: u32,
        name: impl Into<String>,
        params: &Iec61850ParamsConfig,
        points: Vec<PointConfig>,
    ) -> Self {
        let entries = points
            .into_iter()
            .filter_map(|p| {
                if !p.enabled {
                    return None;
                }
                #[cfg(feature = "iec61850")]
                if let crate::protocols::core::point::ProtocolAddress::Iec61850(ref addr) =
                    p.address
                {
                    return Some(PointEntry {
                        domain: addr.domain.clone(),
                        item: addr.item.clone(),
                        id: p.id,
                        point_type: p.point_type,
                        transform: p.transform.clone(),
                    });
                }
                None
            })
            .collect();

        Self {
            id,
            name: name.into(),
            address: params.address.clone(),
            connect_timeout: Duration::from_millis(params.connect_timeout_ms),
            request_timeout: Duration::from_millis(params.request_timeout_ms),
            framer: None,
            state: ConnectionState::Disconnected,
            invoke_id: 1,
            points: entries,
        }
    }

    fn next_invoke_id(&mut self) -> u8 {
        let id = self.invoke_id;
        self.invoke_id = if self.invoke_id == 255 {
            1
        } else {
            self.invoke_id + 1
        };
        id
    }

    async fn try_connect(&mut self) -> Result<()> {
        self.state = ConnectionState::Connecting;

        let connect_timeout_ms = self.connect_timeout.as_millis() as u64;
        let stream = timeout(self.connect_timeout, TcpStream::connect(&self.address))
            .await
            .map_err(|_| GatewayError::ConnectionTimeout(connect_timeout_ms))?
            .map_err(GatewayError::Io)?;

        stream.set_nodelay(true).ok();
        let mut framer = Framer::new(stream);

        timeout(self.connect_timeout, framer.handshake_cotp())
            .await
            .map_err(|_| GatewayError::ConnectionTimeout(connect_timeout_ms))?
            .map_err(|e| GatewayError::Protocol(format!("IEC 61850: COTP handshake: {}", e)))?;

        timeout(self.connect_timeout, framer.handshake_mms())
            .await
            .map_err(|_| GatewayError::ConnectionTimeout(connect_timeout_ms))?
            .map_err(|e| GatewayError::Protocol(format!("IEC 61850: MMS initiate: {}", e)))?;

        self.framer = Some(framer);
        self.state = ConnectionState::Connected;
        info!("IEC 61850 [{}] connected to {}", self.name, self.address);
        Ok(())
    }

    async fn read_variable(&mut self, invoke_id: u8, domain: &str, item: &str) -> Result<MmsValue> {
        let framer = self
            .framer
            .as_mut()
            .ok_or_else(|| GatewayError::Protocol("IEC 61850: not connected".into()))?;

        let req = build_read_request(invoke_id, domain, item);

        timeout(self.request_timeout, framer.send_mms(&req))
            .await
            .map_err(|_| GatewayError::WriteTimeout)??;

        let resp = timeout(self.request_timeout, framer.recv_mms())
            .await
            .map_err(|_| GatewayError::ReadTimeout)??;

        let (_, value) = parse_read_response(&resp)
            .map_err(|e| GatewayError::Protocol(format!("IEC 61850: parse response: {}", e)))?;

        Ok(value)
    }

    fn go_disconnected(&mut self) {
        self.framer = None;
        self.state = ConnectionState::Disconnected;
    }
}

// ── ChannelRuntime ────────────────────────────────────────────────────────────

#[async_trait]
impl ChannelRuntime for Iec61850Channel {
    fn id(&self) -> u32 {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn protocol(&self) -> &str {
        "iec61850"
    }

    fn is_event_driven(&self) -> bool {
        false
    }

    async fn connect(&mut self) -> Result<()> {
        self.try_connect().await.map_err(|e| {
            self.state = ConnectionState::Error;
            error!("IEC 61850 [{}] connect failed: {}", self.name, e);
            e
        })
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.go_disconnected();
        info!("IEC 61850 [{}] disconnected", self.name);
        Ok(())
    }

    async fn poll_once(&mut self) -> PollResult {
        if self.state != ConnectionState::Connected {
            return PollResult::default();
        }

        let mut batch = DataBatch::with_capacity(self.points.len());
        let mut failures: Vec<PointFailure> = Vec::new();
        let point_count = self.points.len();

        for i in 0..point_count {
            let (domain, item, point_id, point_type, transform) = {
                let p = &self.points[i];
                (
                    p.domain.clone(),
                    p.item.clone(),
                    p.id,
                    p.point_type,
                    p.transform.clone(),
                )
            };

            let invoke_id = self.next_invoke_id();

            match self.read_variable(invoke_id, &domain, &item).await {
                Ok(mms_val) if mms_val.is_ok() => {
                    let raw_value = mms_to_value(&mms_val);
                    let value = apply_transform(&raw_value, &transform);
                    batch.add(DataPoint {
                        id: point_id,
                        point_type,
                        value,
                        quality: Quality::Good,
                        timestamp: Utc::now(),
                        source_timestamp: None,
                    });
                },
                Ok(MmsValue::Failure(code)) => {
                    // MMS application-level error (variable not found, access denied, etc.)
                    // Don't disconnect — the TCP session is still valid.
                    failures.push(PointFailure::with_error(
                        point_id,
                        format!("MMS data-access error {}", code),
                    ));
                },
                Err(GatewayError::ReadTimeout) | Err(GatewayError::WriteTimeout) => {
                    // Per-request timeout: server didn't respond for this variable.
                    // Could be unsupported attribute or transient congestion.
                    // Keep the session alive and try remaining points.
                    warn!(
                        "IEC 61850 [{}] read {}/{} timeout (skipping)",
                        self.name, domain, item
                    );
                    failures.push(PointFailure::with_error(
                        point_id,
                        "read timeout".to_string(),
                    ));
                    // After a timeout the TCP stream may have stale data; reconnect.
                    self.go_disconnected();
                    break;
                },
                Err(e) => {
                    // IO-level error (TCP closed, connection reset, etc.) — must reconnect.
                    warn!(
                        "IEC 61850 [{}] read {}/{} IO error: {}",
                        self.name, domain, item, e
                    );
                    self.go_disconnected();
                    failures.push(PointFailure::with_error(point_id, e.to_string()));
                    break;
                },
                _ => unreachable!(),
            }
        }

        if failures.is_empty() {
            PollResult::success(batch)
        } else {
            PollResult::partial(batch, failures)
        }
    }

    async fn write_control(&mut self, _commands: &[(u32, f64)]) -> Result<usize> {
        Ok(0) // MMS control not yet implemented
    }

    async fn write_adjustment(&mut self, _adjustments: &[(u32, f64)]) -> Result<usize> {
        Ok(0)
    }

    fn subscribe(&self) -> Option<DataEventReceiver> {
        None
    }

    async fn start_events(&mut self) -> Result<()> {
        Ok(())
    }

    async fn stop_events(&mut self) -> Result<()> {
        Ok(())
    }

    async fn diagnostics(&self) -> Result<Diagnostics> {
        let mut d = Diagnostics::new("iec61850");
        d.connection_state = self.state;
        Ok(d)
    }

    fn connection_state(&self) -> ConnectionState {
        self.state
    }
}

// ── Value helpers ─────────────────────────────────────────────────────────────

fn mms_to_value(mms: &MmsValue) -> Value {
    match mms {
        MmsValue::Float32(f) => Value::Float(*f as f64),
        MmsValue::Float64(f) => Value::Float(*f),
        MmsValue::Integer(i) => Value::Integer(*i),
        MmsValue::Unsigned(u) => Value::Integer(*u as i64),
        MmsValue::Boolean(b) => Value::Bool(*b),
        MmsValue::VisibleString(s) => Value::String(s.clone()),
        _ => Value::Null,
    }
}

fn apply_transform(value: &Value, transform: &TransformConfig) -> Value {
    match value {
        Value::Float(f) => Value::Float(transform.apply(*f)),
        Value::Integer(i) => Value::Float(transform.apply(*i as f64)),
        Value::Bool(b) => Value::Bool(transform.apply_bool(*b)),
        other => other.clone(),
    }
}
