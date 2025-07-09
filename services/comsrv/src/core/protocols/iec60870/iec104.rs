use async_trait::async_trait;
use chrono::Utc;
/// IEC60870-5-104 Protocol Implementation
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;
use tracing::info;

use crate::core::config::ChannelConfig;
use crate::core::protocols::common::traits::ComBase;
use crate::core::protocols::common::{ChannelStatus, PointData};
use crate::core::protocols::iec60870::asdu::{CommonAddrSize, TypeId, ASDU};
use crate::core::protocols::iec60870::common::{IecError, IecResult};
use crate::utils::{ComSrvError, Result};

/// Control field codes for APCI
const START_DT_ACT: u8 = 0x07; // Start data transfer activation
const START_DT_CON: u8 = 0x0B; // Start data transfer confirmation
const STOP_DT_ACT: u8 = 0x13; // Stop data transfer activation
const STOP_DT_CON: u8 = 0x23; // Stop data transfer confirmation
const TEST_FR_ACT: u8 = 0x43; // Test frame activation
const TEST_FR_CON: u8 = 0x83; // Test frame confirmation

/// APCI structure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApciType {
    /// I-format (information transfer format)
    IFrame { send_seq: u16, recv_seq: u16 },
    /// S-format (supervisory format)
    SFrame { recv_seq: u16 },
    /// U-format (unnumbered control format)
    UFrame(u8),
}

/// APDU (Application Protocol Data Unit) structure
#[derive(Debug, Clone)]
pub struct Apdu {
    /// APCI (Application Protocol Control Information)
    pub apci: ApciType,
    /// ASDU (Application Service Data Unit) - only for I-frames
    pub asdu: Option<ASDU>,
}

impl Apdu {
    /// Create a new I-format APDU
    pub fn new_i_frame(send_seq: u16, recv_seq: u16, asdu: ASDU) -> Self {
        Self {
            apci: ApciType::IFrame { send_seq, recv_seq },
            asdu: Some(asdu),
        }
    }

    /// Create a new S-format APDU
    pub fn new_s_frame(recv_seq: u16) -> Self {
        Self {
            apci: ApciType::SFrame { recv_seq },
            asdu: None,
        }
    }

    /// Create a new U-format APDU
    pub fn new_u_frame(code: u8) -> Self {
        Self {
            apci: ApciType::UFrame(code),
            asdu: None,
        }
    }

    /// Encode APDU to bytes
    pub fn encode(&self, common_addr_size: CommonAddrSize) -> IecResult<Vec<u8>> {
        let mut buffer = Vec::new();

        // Start with APCI
        buffer.push(0x68); // Start character

        // Reserve space for length
        buffer.push(0);

        match self.apci {
            ApciType::IFrame { send_seq, recv_seq } => {
                // Control fields for I-frame
                buffer.push(((send_seq << 1) & 0xFE) as u8);
                buffer.push((send_seq >> 7) as u8);
                buffer.push(((recv_seq << 1) & 0xFE) as u8);
                buffer.push((recv_seq >> 7) as u8);

                // Add ASDU if present
                if let Some(asdu) = &self.asdu {
                    let asdu_bytes = asdu.encode(common_addr_size)?;
                    buffer.extend_from_slice(&asdu_bytes);
                }
            }
            ApciType::SFrame { recv_seq } => {
                // Control fields for S-frame
                buffer.push(0x01);
                buffer.push(0x00);
                buffer.push(((recv_seq << 1) & 0xFE) as u8);
                buffer.push((recv_seq >> 7) as u8);
            }
            ApciType::UFrame(code) => {
                // Control fields for U-frame
                buffer.push(code);
                buffer.push(0x00);
                buffer.push(0x00);
                buffer.push(0x00);
            }
        }

        // Update length (excluding start character and length byte)
        let length = buffer.len() - 2;
        buffer[1] = length as u8;

        Ok(buffer)
    }

    /// Decode APDU from bytes
    pub fn decode(data: &[u8], common_addr_size: CommonAddrSize) -> IecResult<Self> {
        if data.len() < 6 {
            return Err(IecError::ProtocolError("APDU data too short".to_string()));
        }

        // Check start character
        if data[0] != 0x68 {
            return Err(IecError::ProtocolError(format!(
                "Invalid start character: {:02X}",
                data[0]
            )));
        }

        // Check length
        let length = data[1] as usize;
        if data.len() < length + 2 {
            return Err(IecError::ProtocolError(format!(
                "APDU data too short. Expected {} bytes, got {}",
                length + 2,
                data.len()
            )));
        }

        // Determine APCI type
        let control1 = data[2];

        if (control1 & 0x01) == 0 {
            // I-format
            let send_seq = (((data[3] as u16) << 7) | ((control1 as u16) >> 1)) & 0x7FFF;
            let recv_seq = (((data[5] as u16) << 7) | ((data[4] as u16) >> 1)) & 0x7FFF;

            // Decode ASDU if present
            let asdu = if data.len() > 6 {
                Some(ASDU::decode(&data[6..], common_addr_size)?)
            } else {
                None
            };

            Ok(Self {
                apci: ApciType::IFrame { send_seq, recv_seq },
                asdu,
            })
        } else if (control1 & 0x03) == 0x01 {
            // S-format
            let recv_seq = (((data[5] as u16) << 7) | ((data[4] as u16) >> 1)) & 0x7FFF;

            Ok(Self {
                apci: ApciType::SFrame { recv_seq },
                asdu: None,
            })
        } else if (control1 & 0x03) == 0x03 {
            // U-format
            Ok(Self {
                apci: ApciType::UFrame(control1),
                asdu: None,
            })
        } else {
            Err(IecError::ProtocolError(format!(
                "Invalid control field: {control1:02X}"
            )))
        }
    }
}

/// IEC-104 client implementation
#[derive(Debug)]
pub struct Iec104Client {
    /// Service name
    name: String,
    /// Channel ID
    channel_id: u16,
    /// Channel configuration
    config: ChannelConfig,
    /// TCP host address
    host: String,
    /// TCP port
    port: u16,
    /// Connection timeout in seconds
    timeout: u64,
    /// Maximum retries on connection failure
    max_retries: u32,
    /// Polling rate in milliseconds
    poll_rate: u64,
    /// TCP connection
    connection: Arc<Mutex<Option<TcpStream>>>,
    /// Channel status
    status: Arc<RwLock<ChannelStatus>>,
    /// Running state
    running: Arc<RwLock<bool>>,
    /// Send sequence number counter
    send_seq: Arc<Mutex<u16>>,
    /// Receive sequence number counter
    recv_seq: Arc<Mutex<u16>>,
    /// Common address size
    common_addr_size: CommonAddrSize,
    /// Saved real-time point data
    point_data: Arc<RwLock<Vec<PointData>>>,
}

impl Iec104Client {
    /// Create a new IEC-104 client
    pub fn new(config: ChannelConfig) -> Self {
        let channel_id = config.id;
        let status = ChannelStatus::new(&channel_id.to_string());

        // Extract parameters from config
        let mut host = String::from("localhost");
        let mut port = 2404; // Default IEC-104 port
        let mut timeout = 5;
        let mut max_retries = 3;
        let mut poll_rate = 1000;

        // Extract specific parameters from config
        let params = &config.parameters;
        if let Some(val) = params.get("host") {
            if let Some(s) = val.as_str() {
                host = s.to_string();
            }
        }

        if let Some(val) = params.get("port") {
            if let Some(n) = val.as_u64() {
                port = n as u16;
            }
        }

        if let Some(val) = params.get("timeout") {
            if let Some(n) = val.as_u64() {
                timeout = n;
            }
        }

        if let Some(val) = params.get("max_retries") {
            if let Some(n) = val.as_u64() {
                max_retries = n as u32;
            }
        }

        if let Some(val) = params.get("poll_rate") {
            if let Some(n) = val.as_u64() {
                poll_rate = n;
            }
        }

        Self {
            name: format!("IEC-104 Client ({})", channel_id),
            channel_id,
            config,
            host,
            port,
            timeout,
            max_retries,
            poll_rate,
            connection: Arc::new(Mutex::new(None)),
            status: Arc::new(RwLock::new(status)),
            running: Arc::new(RwLock::new(false)),
            send_seq: Arc::new(Mutex::new(0)),
            recv_seq: Arc::new(Mutex::new(0)),
            common_addr_size: CommonAddrSize::TwoOctets,
            point_data: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get the next send sequence number
    async fn next_send_seq(&self) -> u16 {
        let mut send_seq = self.send_seq.lock().await;
        let result = *send_seq;
        *send_seq = (*send_seq + 1) % 32768; // Wrap around at 2^15
        result
    }

    /// Get the current receive sequence number
    async fn current_recv_seq(&self) -> u16 {
        let recv_seq = self.recv_seq.lock().await;
        *recv_seq
    }

    /// Update the receive sequence number
    async fn update_recv_seq(&self, new_recv_seq: u16) {
        let mut recv_seq = self.recv_seq.lock().await;
        *recv_seq = new_recv_seq;
    }

    /// Connect to the IEC-104 server
    async fn connect(&self) -> IecResult<()> {
        let mut retries = 0;
        let mut last_error_str = String::new();

        while retries < self.max_retries {
            let connection_str = format!("{}:{}", self.host, self.port);
            tracing::debug!("Connecting to IEC-104 server at {connection_str}");

            match tokio::time::timeout(
                Duration::from_secs(self.timeout),
                TcpStream::connect(&connection_str),
            )
            .await
            {
                Ok(Ok(stream)) => {
                    // Connected successfully
                    tracing::info!("Connected to IEC-104 server at {connection_str}");

                    // Store the connection
                    let mut connection = self.connection.lock().await;
                    *connection = Some(stream);

                    // Reset sequence numbers
                    let mut send_seq = self.send_seq.lock().await;
                    *send_seq = 0;
                    let mut recv_seq = self.recv_seq.lock().await;
                    *recv_seq = 0;

                    // Update status
                    self.update_status(true, 0.0, None).await;

                    return Ok(());
                }
                Ok(Err(e)) => {
                    // Connection error
                    last_error_str = e.to_string();
                    tracing::warn!("Failed to connect to IEC-104 server: {last_error_str}");
                }
                Err(_) => {
                    // Timeout
                    last_error_str = "Connection timed out".to_string();
                    tracing::warn!("Connection to IEC-104 server timed out");
                }
            }

            retries += 1;
            if retries < self.max_retries {
                sleep(Duration::from_secs(1)).await;
            }
        }

        // Update status
        self.update_status(false, 0.0, Some(&last_error_str)).await;

        Err(IecError::ConnectionError(format!(
            "Failed to connect after {} retries: {}",
            self.max_retries, last_error_str
        )))
    }

    /// Send APDU to the server
    async fn send_apdu(&self, apdu: &Apdu) -> IecResult<()> {
        let connection = self.connection.lock().await;

        if let Some(stream) = &*connection {
            let data = apdu.encode(self.common_addr_size)?;
            let start = Instant::now();

            match stream.try_write(&data) {
                Ok(n) if n == data.len() => {
                    let duration = start.elapsed();
                    tracing::debug!(
                        "Sent APDU: {:?} in {:.2}ms",
                        apdu,
                        duration.as_secs_f64() * 1000.0
                    );
                    self.update_status(true, duration.as_secs_f64() * 1000.0, None)
                        .await;
                    Ok(())
                }
                Ok(n) => {
                    let err = format!("Incomplete write: {}/{} bytes", n, data.len());
                    tracing::error!("{err}");
                    self.update_status(false, 0.0, Some(&err)).await;
                    Err(IecError::IoError(err))
                }
                Err(e) => {
                    tracing::error!("Failed to send APDU: {e}");
                    self.update_status(false, 0.0, Some(&e.to_string())).await;
                    Err(IecError::IoError(e.to_string()))
                }
            }
        } else {
            tracing::error!("Cannot send APDU: not connected");
            Err(IecError::ConnectionError("Not connected".to_string()))
        }
    }

    /// Update the channel status
    async fn update_status(&self, connected: bool, response_time: f64, error: Option<&str>) {
        let mut status = self.status.write().await;
        status.connected = connected;
        status.last_response_time = response_time;
        if let Some(err) = error {
            status.last_error = err.to_string();
        } else if connected {
            status.last_error = String::new();
        }
        status.last_update_time = Utc::now();
    }

    /// Handle incoming APDU
    async fn handle_apdu(&self, apdu: Apdu) -> IecResult<()> {
        match apdu.apci {
            ApciType::IFrame { send_seq, recv_seq } => {
                // Update receive sequence
                self.update_recv_seq((send_seq + 1) % 32768).await;

                // Handle ASDU if present
                if let Some(asdu) = apdu.asdu {
                    self.handle_asdu(asdu).await?;
                }

                // Send S-format confirmation periodically (not after every I-frame)
                // This would normally be done based on W counter in production
                let s_frame = Apdu::new_s_frame(self.current_recv_seq().await);
                self.send_apdu(&s_frame).await?;
            }
            ApciType::SFrame { recv_seq } => {
                // No action needed, just log
                tracing::debug!("Received S-frame with recv_seq = {recv_seq}");
            }
            ApciType::UFrame(code) => {
                match code {
                    START_DT_ACT => {
                        // Send START_DT_CON
                        let response = Apdu::new_u_frame(START_DT_CON);
                        self.send_apdu(&response).await?;
                    }
                    STOP_DT_ACT => {
                        // Send STOP_DT_CON
                        let response = Apdu::new_u_frame(STOP_DT_CON);
                        self.send_apdu(&response).await?;
                    }
                    TEST_FR_ACT => {
                        // Send TEST_FR_CON
                        let response = Apdu::new_u_frame(TEST_FR_CON);
                        self.send_apdu(&response).await?;
                    }
                    START_DT_CON | STOP_DT_CON | TEST_FR_CON => {
                        // No action needed, just log
                        tracing::debug!("Received U-frame confirmation: {code:02X}");
                    }
                    _ => {
                        tracing::warn!("Received unknown U-frame code: {code:02X}");
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle incoming ASDU
    async fn handle_asdu(&self, asdu: ASDU) -> IecResult<()> {
        // Example of processing a measured value (type 13)
        if asdu.type_id == TypeId::MeasuredValueFloat {
            // Process measured values (implementation depends on data format)
            tracing::debug!("Received measured value: {asdu:?}");

            // Here you would extract the value from ASDU and store it
            // This is a placeholder
            let point_data = PointData {
                id: format!("{}:{}", asdu.common_addr, "some_point_id"),
                name: format!("IEC Point {}:{}", asdu.common_addr, "some_point_id"),
                value: "null".to_string(), // Replace with actual value
                timestamp: Utc::now(),
                unit: "".to_string(),
                description: format!(
                    "IEC 60870 data point {}:{}",
                    asdu.common_addr, "some_point_id"
                ),
                telemetry_type: Some(crate::core::protocols::common::TelemetryType::Telemetry),
                channel_id: Some(self.channel_id),
            };

            // Store point data
            let mut data = self.point_data.write().await;
            data.push(point_data);
        }

        Ok(())
    }
}

#[async_trait]
impl ComBase for Iec104Client {
    fn name(&self) -> &str {
        &self.name
    }

    fn channel_id(&self) -> String {
        self.channel_id.to_string()
    }

    fn protocol_type(&self) -> &str {
        "IEC60870-5-104"
    }

    fn get_parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("protocol".to_string(), "IEC60870-5-104".to_string());
        params.insert("channel_id".to_string(), self.channel_id.to_string());
        params.insert("host".to_string(), self.host.clone());
        params.insert("port".to_string(), self.port.to_string());
        params.insert("timeout".to_string(), self.timeout.to_string());
        params.insert("max_retries".to_string(), self.max_retries.to_string());
        params.insert("poll_rate".to_string(), self.poll_rate.to_string());
        params
    }

    async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    async fn start(&mut self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }

        // Set running flag
        *running = true;

        // Clone Arc references for worker task
        let connection = self.connection.clone();
        let status = self.status.clone();
        let running_flag = self.running.clone();
        let common_addr_size = self.common_addr_size;
        let poll_rate = self.poll_rate;

        // Start worker task
        tokio::spawn(async move {
            let mut buffer = [0u8; 1024];
            let mut last_activity = Instant::now();

            while *running_flag.read().await {
                // Check connection status
                let mut conn_locked = connection.lock().await;

                // If not connected, nothing to do in this iteration
                if conn_locked.is_none() {
                    drop(conn_locked); // Release lock explicitly
                    sleep(Duration::from_millis(poll_rate)).await;
                    continue;
                }

                // Check for incoming data (with timeout)
                let stream = conn_locked.as_mut().unwrap();
                match tokio::time::timeout(Duration::from_millis(100), stream.read(&mut buffer))
                    .await
                {
                    Ok(Ok(0)) => {
                        // Connection closed
                        tracing::warn!("IEC-104 server closed the connection");
                        *conn_locked = None;

                        // Update status
                        let mut status_locked = status.write().await;
                        status_locked.connected = false;
                        status_locked.last_error = "Connection closed by server".to_string();
                        status_locked.last_update_time = Utc::now();

                        drop(conn_locked); // Release lock explicitly
                    }
                    Ok(Ok(n)) => {
                        // Received data
                        tracing::debug!("Received {} bytes from IEC-104 server", n);
                        last_activity = Instant::now();

                        // Process APDU
                        match Apdu::decode(&buffer[..n], common_addr_size) {
                            Ok(apdu) => {
                                drop(conn_locked); // Release lock before async operation
                                if let Err(e) = handle_apdu(&apdu, &status).await {
                                    tracing::error!("Error handling APDU: {e}");
                                }
                            }
                            Err(e) => {
                                tracing::error!("Error decoding APDU: {e}");

                                // Update status
                                let mut status_locked = status.write().await;
                                status_locked.last_error = format!("Error decoding APDU: {e}");
                                status_locked.last_update_time = Utc::now();

                                drop(conn_locked); // Release lock explicitly
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        // IO error
                        tracing::error!("IO error: {e}");
                        *conn_locked = None;

                        // Update status
                        let mut status_locked = status.write().await;
                        status_locked.connected = false;
                        status_locked.last_error = format!("IO error: {e}");
                        status_locked.last_update_time = Utc::now();

                        drop(conn_locked); // Release lock explicitly
                    }
                    Err(_) => {
                        // Timeout (no data available)
                        drop(conn_locked); // Release lock explicitly

                        // Check if we need to send TEST_FR_ACT
                        if last_activity.elapsed() > Duration::from_secs(20) {
                            tracing::debug!("Sending TEST_FR_ACT");
                            last_activity = Instant::now();

                            // Send test frame
                            let conn_guard = connection.lock().await;
                            if conn_guard.is_some() {
                                drop(conn_guard); // Release lock before async operation
                                send_test_frame(&connection, &status).await;
                            } else {
                                drop(conn_guard);
                            }
                        }
                    }
                }

                // Sleep before next iteration
                sleep(Duration::from_millis(10)).await;
            }
        });

        // Connect to the server
        match self.connect().await {
            Ok(_) => {
                // Send STARTDT activation
                let start_dt = Apdu::new_u_frame(START_DT_ACT);
                if let Err(e) = self.send_apdu(&start_dt).await {
                    tracing::error!("Failed to send STARTDT activation: {e}");
                    return Err(ComSrvError::ProtocolError(format!(
                        "Failed to send STARTDT activation: {e}"
                    )));
                }

                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to connect: {e}");
                *running = false;
                Err(ComSrvError::ProtocolError(format!(
                    "Failed to connect: {e}"
                )))
            }
        }
    }

    async fn stop(&mut self) -> Result<()> {
        let mut running = self.running.write().await;
        if !*running {
            return Ok(());
        }

        // Set running flag to false to stop worker task
        *running = false;

        // Send STOPDT activation if connected
        let mut connection = self.connection.lock().await;
        if connection.is_some() {
            let stop_dt = Apdu::new_u_frame(STOP_DT_ACT);
            if let Err(e) = self.send_apdu(&stop_dt).await {
                tracing::warn!("Failed to send STOPDT activation: {e}");
                // Continue with closure even if STOPDT fails
            }

            // Close connection
            *connection = None;
        }

        // Update status
        let mut status = self.status.write().await;
        status.connected = false;
        status.last_update_time = Utc::now();

        Ok(())
    }

    async fn status(&self) -> ChannelStatus {
        self.status.read().await.clone()
    }

    async fn get_all_points(&self) -> Vec<PointData> {
        self.point_data.read().await.clone()
    }

    async fn update_status(&mut self, status: ChannelStatus) -> Result<()> {
        *self.status.write().await = status;
        Ok(())
    }

    async fn read_point(&self, point_id: &str) -> Result<PointData> {
        let points = self.point_data.read().await;
        points
            .iter()
            .find(|p| p.id == point_id)
            .cloned()
            .ok_or_else(|| ComSrvError::InvalidParameter(format!("Point {} not found", point_id)))
    }

    async fn write_point(&mut self, point_id: &str, value: &str) -> Result<()> {
        // TODO: Implement IEC 60870-5-104 command sending
        info!("IEC104 write point: {} = {value}", point_id);
        Ok(())
    }

    async fn get_diagnostics(&self) -> HashMap<String, String> {
        let mut diag = HashMap::new();
        let status = self.status.read().await;
        diag.insert("protocol".to_string(), "IEC60870-5-104".to_string());
        diag.insert("connected".to_string(), status.connected.to_string());
        diag.insert(
            "last_response_time".to_string(),
            status.last_response_time.to_string(),
        );
        diag.insert("last_error".to_string(), status.last_error.clone());
        diag
    }
}

/// Send a test frame to the server
async fn send_test_frame(
    connection: &Arc<Mutex<Option<TcpStream>>>,
    status: &Arc<RwLock<ChannelStatus>>,
) {
    let mut conn = connection.lock().await;

    if let Some(stream) = &mut *conn {
        let test_frame = Apdu::new_u_frame(TEST_FR_ACT);

        match test_frame.encode(CommonAddrSize::TwoOctets) {
            Ok(data) => {
                match stream.try_write(&data) {
                    Ok(_) => {
                        // Update status
                        let mut status_locked = status.write().await;
                        status_locked.last_update_time = Utc::now();
                    }
                    Err(e) => {
                        tracing::error!("Failed to send test frame: {e}");

                        // Update status
                        let mut status_locked = status.write().await;
                        status_locked.connected = false;
                        status_locked.last_error = format!("Failed to send test frame: {e}");
                        status_locked.last_update_time = Utc::now();

                        // Close connection
                        *conn = None;
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to encode test frame: {e}");
            }
        }
    }
}

/// Handle incoming APDU (worker task helper)
async fn handle_apdu(apdu: &Apdu, status: &Arc<RwLock<ChannelStatus>>) -> IecResult<()> {
    match apdu.apci {
        ApciType::IFrame {
            send_seq: _,
            recv_seq: _,
        } => {
            // Update status
            let mut status_locked = status.write().await;
            status_locked.connected = true;
            status_locked.last_update_time = Utc::now();

            // Further processing would be done in the client's handle_apdu method
        }
        ApciType::SFrame { recv_seq: _ } => {
            // Update status
            let mut status_locked = status.write().await;
            status_locked.connected = true;
            status_locked.last_update_time = Utc::now();
        }
        ApciType::UFrame(code) => {
            // Update status
            let mut status_locked = status.write().await;
            status_locked.connected = true;
            status_locked.last_update_time = Utc::now();

            if code == TEST_FR_CON {
                tracing::debug!("Received test frame confirmation");
            }
        }
    }

    Ok(())
}
