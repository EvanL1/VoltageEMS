//! Modbus Server implementation for self-testing
//!
//! This module provides a Modbus TCP server that can be used for testing
//! other Modbus clients. It reads data from Redis using its own channel_id
//! and supports IP whitelisting for security.

use crate::core::combase::{
    ChannelStatus, ClientInfo, ComBase, ComServer, PointDataMap, RedisValue,
};
use crate::core::config::types::TelemetryType;
use crate::core::config::ChannelConfig;
use crate::utils::error::{ComSrvError, Result};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Modbus Server implementation
pub struct ModbusServer {
    /// Server name
    name: Arc<str>,
    /// Channel ID (used to read data from Redis)
    channel_id: u16,
    /// Channel configuration
    channel_config: Option<Arc<ChannelConfig>>,

    /// Server state
    is_running: Arc<RwLock<bool>>,
    listener: Option<Arc<TcpListener>>,

    /// Connected clients
    clients: Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,

    /// IP whitelist (empty means allow all)
    ip_whitelist: Arc<RwLock<HashSet<IpAddr>>>,

    /// Server task handle
    server_handle: Arc<RwLock<Option<JoinHandle<()>>>>,

    /// Status tracking
    status: Arc<RwLock<ChannelStatus>>,

    /// Redis client for reading data
    redis_client: Option<Arc<redis::Client>>,
}

impl ModbusServer {
    /// Create new Modbus server instance
    pub fn new(channel_config: ChannelConfig) -> Result<Self> {
        let channel_id = channel_config.id;
        let name: Arc<str> = channel_config.name.clone().into();

        // Parse IP whitelist from configuration
        let mut ip_whitelist = HashSet::new();
        if let Some(whitelist_value) = channel_config.parameters.get("ip_whitelist") {
            if let Some(whitelist_array) = whitelist_value.as_sequence() {
                for ip_str in whitelist_array {
                    if let Some(ip_string) = ip_str.as_str() {
                        if let Ok(ip) = ip_string.parse::<IpAddr>() {
                            ip_whitelist.insert(ip);
                            info!("Added {} to IP whitelist for channel {}", ip, channel_id);
                        }
                    }
                }
            }
        }

        // Get Redis URL from environment or configuration
        let redis_url = channel_config
            .parameters
            .get("redis_url")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| std::env::var("REDIS_URL").ok())
            .unwrap_or_else(|| "redis://127.0.0.1:6379".to_string());

        let redis_client = redis::Client::open(redis_url)
            .map_err(|e| ComSrvError::config(format!("Failed to create Redis client: {}", e)))?;

        Ok(Self {
            name,
            channel_id,
            channel_config: None,
            is_running: Arc::new(RwLock::new(false)),
            listener: None,
            clients: Arc::new(RwLock::new(HashMap::new())),
            ip_whitelist: Arc::new(RwLock::new(ip_whitelist)),
            server_handle: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(ChannelStatus::default())),
            redis_client: Some(Arc::new(redis_client)),
        })
    }

    /// Read data from Redis for this channel
    async fn read_from_redis(&self, telemetry_type: TelemetryType) -> Result<PointDataMap> {
        let redis_client = self
            .redis_client
            .as_ref()
            .ok_or_else(|| ComSrvError::config("Redis client not initialized"))?;

        let mut conn = redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| ComSrvError::io(format!("Failed to get Redis connection: {}", e)))?;

        // Build Redis key based on telemetry type
        let redis_key = format!("comsrv:{}:{}", self.channel_id, telemetry_type.as_str());

        // Read all fields from the hash
        let data: HashMap<String, String> = redis::cmd("HGETALL")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| ComSrvError::io(format!("Failed to read from Redis: {}", e)))?;

        // Convert to PointDataMap
        let mut result = PointDataMap::new();
        let timestamp = chrono::Utc::now().timestamp();

        for (point_id_str, value_str) in data {
            if let Ok(point_id) = point_id_str.parse::<u32>() {
                // Parse value based on telemetry type
                let value = match telemetry_type {
                    TelemetryType::Signal | TelemetryType::Control => {
                        // Boolean values
                        RedisValue::Bool(value_str == "1" || value_str.to_lowercase() == "true")
                    },
                    _ => {
                        // Numeric values
                        if let Ok(f) = value_str.parse::<f64>() {
                            RedisValue::Float(f)
                        } else if let Ok(i) = value_str.parse::<i64>() {
                            RedisValue::Integer(i)
                        } else {
                            RedisValue::String(value_str.into())
                        }
                    },
                };

                result.insert(
                    point_id,
                    crate::core::combase::PointData { value, timestamp },
                );
            }
        }

        Ok(result)
    }

    /// Handle Modbus request from client
    async fn handle_modbus_request(
        &self,
        stream: &mut TcpStream,
        request: &[u8],
        client_addr: SocketAddr,
    ) -> Result<()> {
        if request.len() < 8 {
            return Err(ComSrvError::protocol("Invalid Modbus request: too short"));
        }

        // Parse MBAP header (Modbus TCP)
        let transaction_id = u16::from_be_bytes([request[0], request[1]]);
        let _protocol_id = u16::from_be_bytes([request[2], request[3]]);
        let _length = u16::from_be_bytes([request[4], request[5]]);
        let unit_id = request[6];
        let function_code = request[7];

        debug!(
            "Received Modbus request from {}: transaction={}, unit={}, function={}",
            client_addr, transaction_id, unit_id, function_code
        );

        // Handle different function codes
        let response_pdu = match function_code {
            0x01 => self.handle_read_coils(&request[8..]).await?,
            0x02 => self.handle_read_discrete_inputs(&request[8..]).await?,
            0x03 => self.handle_read_holding_registers(&request[8..]).await?,
            0x04 => self.handle_read_input_registers(&request[8..]).await?,
            0x05 => self.handle_write_single_coil(&request[8..]).await?,
            0x06 => self.handle_write_single_register(&request[8..]).await?,
            0x0F => self.handle_write_multiple_coils(&request[8..]).await?,
            0x10 => self.handle_write_multiple_registers(&request[8..]).await?,
            _ => {
                // Unsupported function code - return exception
                vec![function_code | 0x80, 0x01] // Illegal function
            },
        };

        // Build response with MBAP header
        let response_len = response_pdu.len() + 1; // +1 for unit_id
        let mut response = Vec::with_capacity(7 + response_len);

        // MBAP header
        response.extend_from_slice(&transaction_id.to_be_bytes());
        response.extend_from_slice(&[0x00, 0x00]); // Protocol ID (Modbus)
        response.extend_from_slice(&(response_len as u16).to_be_bytes());
        response.push(unit_id);

        // PDU
        response.extend_from_slice(&response_pdu);

        // Send response
        use tokio::io::AsyncWriteExt;
        stream
            .write_all(&response)
            .await
            .map_err(|e| ComSrvError::io(format!("Failed to send response: {}", e)))?;

        // Update client statistics
        let mut clients = self.clients.write().await;
        if let Some(client_info) = clients.get_mut(&client_addr) {
            client_info.last_request = chrono::Utc::now().timestamp();
            client_info.request_count += 1;
        }

        Ok(())
    }

    /// Handle FC01: Read Coils
    async fn handle_read_coils(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 4 {
            return Ok(vec![0x81, 0x03]); // Illegal data value
        }

        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);

        // Read signal data from Redis
        let signals = self.read_from_redis(TelemetryType::Signal).await?;

        // Build response
        let byte_count = (quantity + 7) / 8;
        let mut response = vec![0x01, byte_count as u8];

        let mut byte_value = 0u8;
        for i in 0..quantity {
            let point_id = start_address + i;
            let bit_value = signals
                .get(&(point_id as u32))
                .and_then(|point| match &point.value {
                    RedisValue::Bool(b) => Some(*b),
                    RedisValue::Integer(i) => Some(*i != 0),
                    RedisValue::Float(f) => Some(*f != 0.0),
                    _ => None,
                })
                .unwrap_or(false);

            if bit_value {
                byte_value |= 1 << (i % 8);
            }

            if (i + 1) % 8 == 0 || i == quantity - 1 {
                response.push(byte_value);
                byte_value = 0;
            }
        }

        Ok(response)
    }

    /// Handle FC02: Read Discrete Inputs
    async fn handle_read_discrete_inputs(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Similar to read coils, but for discrete inputs
        self.handle_read_coils(data).await.map(|mut r| {
            r[0] = 0x02;
            r
        })
    }

    /// Handle FC03: Read Holding Registers
    async fn handle_read_holding_registers(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 4 {
            return Ok(vec![0x83, 0x03]); // Illegal data value
        }

        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);

        // Read telemetry data from Redis
        let telemetry = self.read_from_redis(TelemetryType::Telemetry).await?;

        // Build response
        let byte_count = quantity * 2;
        let mut response = vec![0x03, byte_count as u8];

        for i in 0..quantity {
            let point_id = start_address + i;
            let value = telemetry
                .get(&(point_id as u32))
                .and_then(|point| match &point.value {
                    RedisValue::Float(f) => Some(*f as u16),
                    RedisValue::Integer(i) => Some(*i as u16),
                    _ => None,
                })
                .unwrap_or(0);

            response.extend_from_slice(&value.to_be_bytes());
        }

        Ok(response)
    }

    /// Handle FC04: Read Input Registers
    async fn handle_read_input_registers(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Similar to holding registers
        self.handle_read_holding_registers(data).await.map(|mut r| {
            r[0] = 0x04;
            r
        })
    }

    /// Handle FC05: Write Single Coil
    async fn handle_write_single_coil(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 4 {
            return Ok(vec![0x85, 0x03]); // Illegal data value
        }

        let address = u16::from_be_bytes([data[0], data[1]]);
        let value = u16::from_be_bytes([data[2], data[3]]);
        let bool_value = value == 0xFF00;

        // Write to Redis
        if let Some(redis_client) = &self.redis_client {
            let mut conn = redis_client
                .get_multiplexed_async_connection()
                .await
                .map_err(|e| ComSrvError::io(format!("Redis connection failed: {}", e)))?;

            let redis_key = format!("comsrv:{}:C", self.channel_id);
            let _: () = redis::cmd("HSET")
                .arg(&redis_key)
                .arg(address.to_string())
                .arg(if bool_value { "1" } else { "0" })
                .query_async(&mut conn)
                .await
                .map_err(|e| ComSrvError::io(format!("Redis write failed: {}", e)))?;
        }

        // Echo request as response
        Ok(vec![0x05, data[0], data[1], data[2], data[3]])
    }

    /// Handle FC06: Write Single Register
    async fn handle_write_single_register(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 4 {
            return Ok(vec![0x86, 0x03]); // Illegal data value
        }

        let address = u16::from_be_bytes([data[0], data[1]]);
        let value = u16::from_be_bytes([data[2], data[3]]);

        // Write to Redis
        if let Some(redis_client) = &self.redis_client {
            let mut conn = redis_client
                .get_multiplexed_async_connection()
                .await
                .map_err(|e| ComSrvError::io(format!("Redis connection failed: {}", e)))?;

            let redis_key = format!("comsrv:{}:A", self.channel_id);
            let _: () = redis::cmd("HSET")
                .arg(&redis_key)
                .arg(address.to_string())
                .arg(value.to_string())
                .query_async(&mut conn)
                .await
                .map_err(|e| ComSrvError::io(format!("Redis write failed: {}", e)))?;
        }

        // Echo request as response
        Ok(vec![0x06, data[0], data[1], data[2], data[3]])
    }

    /// Handle FC15: Write Multiple Coils
    async fn handle_write_multiple_coils(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 5 {
            return Ok(vec![0x8F, 0x03]); // Illegal data value
        }

        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);
        let byte_count = data[4] as usize;

        if data.len() < 5 + byte_count {
            return Ok(vec![0x8F, 0x03]); // Illegal data value
        }

        // Write to Redis
        if let Some(redis_client) = &self.redis_client {
            let mut conn = redis_client
                .get_multiplexed_async_connection()
                .await
                .map_err(|e| ComSrvError::io(format!("Redis connection failed: {}", e)))?;

            let redis_key = format!("comsrv:{}:C", self.channel_id);

            for i in 0..quantity {
                let byte_idx = (i / 8) as usize;
                let bit_idx = i % 8;
                let bit_value = (data[5 + byte_idx] >> bit_idx) & 1 == 1;

                let point_id = start_address + i;
                let _: () = redis::cmd("HSET")
                    .arg(&redis_key)
                    .arg(point_id.to_string())
                    .arg(if bit_value { "1" } else { "0" })
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| ComSrvError::io(format!("Redis write failed: {}", e)))?;
            }
        }

        // Response: echo address and quantity
        Ok(vec![0x0F, data[0], data[1], data[2], data[3]])
    }

    /// Handle FC16: Write Multiple Registers
    async fn handle_write_multiple_registers(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 5 {
            return Ok(vec![0x90, 0x03]); // Illegal data value
        }

        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);
        let byte_count = data[4] as usize;

        if data.len() < 5 + byte_count || byte_count != (quantity * 2) as usize {
            return Ok(vec![0x90, 0x03]); // Illegal data value
        }

        // Write to Redis
        if let Some(redis_client) = &self.redis_client {
            let mut conn = redis_client
                .get_multiplexed_async_connection()
                .await
                .map_err(|e| ComSrvError::io(format!("Redis connection failed: {}", e)))?;

            let redis_key = format!("comsrv:{}:A", self.channel_id);

            for i in 0..quantity {
                let byte_idx = (i * 2) as usize;
                let value = u16::from_be_bytes([data[5 + byte_idx], data[6 + byte_idx]]);

                let point_id = start_address + i;
                let _: () = redis::cmd("HSET")
                    .arg(&redis_key)
                    .arg(point_id.to_string())
                    .arg(value.to_string())
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| ComSrvError::io(format!("Redis write failed: {}", e)))?;
            }
        }

        // Response: echo address and quantity
        Ok(vec![0x10, data[0], data[1], data[2], data[3]])
    }
}

#[async_trait]
impl ComBase for ModbusServer {
    fn name(&self) -> &str {
        &self.name
    }

    fn protocol_type(&self) -> &str {
        "modbus-server"
    }

    fn get_channel_id(&self) -> u16 {
        self.channel_id
    }

    async fn get_status(&self) -> ChannelStatus {
        self.status.read().await.clone()
    }

    async fn initialize(&mut self, channel_config: Arc<ChannelConfig>) -> Result<()> {
        info!(
            "Initializing Modbus server for channel {}",
            channel_config.id
        );
        self.channel_config = Some(channel_config);

        // Update status
        self.status.write().await.points_count = self
            .channel_config
            .as_ref()
            .map(|c| {
                c.telemetry_points.len()
                    + c.signal_points.len()
                    + c.control_points.len()
                    + c.adjustment_points.len()
            })
            .unwrap_or(0);

        Ok(())
    }

    async fn read_four_telemetry(&self, telemetry_type: TelemetryType) -> Result<PointDataMap> {
        // Server reads from Redis
        self.read_from_redis(telemetry_type).await
    }
}

#[async_trait]
impl ComServer for ModbusServer {
    fn is_running(&self) -> bool {
        self.is_running
            .try_read()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    async fn start(&mut self) -> Result<()> {
        info!("Starting Modbus server for channel {}", self.channel_id);

        // Get bind address from configuration
        let bind_addr = self
            .channel_config
            .as_ref()
            .and_then(|c| c.parameters.get("bind_address"))
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0.0:502");

        // Create TCP listener
        let listener = TcpListener::bind(bind_addr)
            .await
            .map_err(|e| ComSrvError::io(format!("Failed to bind to {}: {}", bind_addr, e)))?;

        info!("Modbus server listening on {}", bind_addr);
        self.listener = Some(Arc::new(listener));

        // Start server task
        let listener = self.listener.as_ref().unwrap().clone();
        let is_running = self.is_running.clone();
        let clients = self.clients.clone();
        let ip_whitelist = self.ip_whitelist.clone();
        let channel_id = self.channel_id;
        let status = self.status.clone();

        // Clone self for the server task
        let server_self = Arc::new(self.clone());

        let server_task = tokio::spawn(async move {
            *is_running.write().await = true;

            loop {
                // Check if still running
                if !*is_running.read().await {
                    break;
                }

                // Accept new connection
                match listener.accept().await {
                    Ok((mut stream, addr)) => {
                        // Check IP whitelist
                        let ip = addr.ip();
                        let whitelist = ip_whitelist.read().await;
                        if !whitelist.is_empty() && !whitelist.contains(&ip) {
                            warn!("Rejected connection from {} (not in whitelist)", addr);
                            continue;
                        }

                        info!("Accepted Modbus connection from {}", addr);

                        // Add to clients map
                        {
                            let mut clients_guard = clients.write().await;
                            clients_guard.insert(
                                addr,
                                ClientInfo {
                                    addr: addr.to_string(),
                                    connected_at: chrono::Utc::now().timestamp(),
                                    last_request: chrono::Utc::now().timestamp(),
                                    request_count: 0,
                                },
                            );
                        }

                        // Update status
                        status.write().await.success_count += 1;

                        // Handle client in separate task
                        let server = server_self.clone();
                        let clients = clients.clone();
                        let status = status.clone();

                        tokio::spawn(async move {
                            let mut buffer = vec![0u8; 1024];

                            loop {
                                use tokio::io::AsyncReadExt;
                                match stream.read(&mut buffer).await {
                                    Ok(0) => {
                                        // Connection closed
                                        info!("Client {} disconnected", addr);
                                        break;
                                    },
                                    Ok(n) => {
                                        // Handle request
                                        if let Err(e) = server
                                            .handle_modbus_request(&mut stream, &buffer[..n], addr)
                                            .await
                                        {
                                            error!("Error handling request from {}: {}", addr, e);
                                            status.write().await.error_count += 1;
                                        }
                                    },
                                    Err(e) => {
                                        error!("Error reading from client {}: {}", addr, e);
                                        break;
                                    },
                                }
                            }

                            // Remove from clients map
                            clients.write().await.remove(&addr);
                        });
                    },
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                        status.write().await.error_count += 1;
                    },
                }
            }

            info!("Modbus server stopped for channel {}", channel_id);
        });

        *self.server_handle.write().await = Some(server_task);
        *self.is_running.write().await = true;
        self.status.write().await.is_connected = true;

        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        info!("Stopping Modbus server for channel {}", self.channel_id);

        *self.is_running.write().await = false;

        // Abort server task
        if let Some(handle) = self.server_handle.write().await.take() {
            handle.abort();
        }

        // Clear clients
        self.clients.write().await.clear();

        self.status.write().await.is_connected = false;

        Ok(())
    }

    fn verify_client(&self, client_addr: SocketAddr) -> bool {
        let whitelist = self
            .ip_whitelist
            .try_read()
            .map(|guard| guard.clone())
            .unwrap_or_default();

        // If whitelist is empty, allow all
        if whitelist.is_empty() {
            return true;
        }

        // Check if client IP is in whitelist
        whitelist.contains(&client_addr.ip())
    }

    async fn handle_read_request(
        &self,
        address: u16,
        count: u16,
        telemetry_type: TelemetryType,
    ) -> Result<Vec<RedisValue>> {
        // Read from Redis
        let data = self.read_from_redis(telemetry_type).await?;

        // Extract requested range
        let mut results = Vec::new();
        for i in 0..count {
            let point_id = (address + i) as u32;
            let value = data
                .get(&point_id)
                .map(|p| p.value.clone())
                .unwrap_or(RedisValue::Null);
            results.push(value);
        }

        Ok(results)
    }

    async fn handle_write_request(
        &mut self,
        address: u16,
        value: RedisValue,
        telemetry_type: TelemetryType,
    ) -> Result<bool> {
        // Write to Redis
        if let Some(redis_client) = &self.redis_client {
            let mut conn = redis_client
                .get_multiplexed_async_connection()
                .await
                .map_err(|e| ComSrvError::io(format!("Redis connection failed: {}", e)))?;

            let redis_key = format!("comsrv:{}:{}", self.channel_id, telemetry_type.as_str());

            let value_str = match value {
                RedisValue::Float(f) => f.to_string(),
                RedisValue::Integer(i) => i.to_string(),
                RedisValue::Bool(b) => if b { "1" } else { "0" }.to_string(),
                RedisValue::String(s) => s.to_string(),
                RedisValue::Null => return Ok(false),
            };

            let _: () = redis::cmd("HSET")
                .arg(&redis_key)
                .arg(address.to_string())
                .arg(value_str)
                .query_async(&mut conn)
                .await
                .map_err(|e| ComSrvError::io(format!("Redis write failed: {}", e)))?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    async fn get_connected_clients(&self) -> Vec<ClientInfo> {
        self.clients.read().await.values().cloned().collect()
    }
}

// Implement Clone for server cloning in async tasks
impl Clone for ModbusServer {
    fn clone(&self) -> Self {
        Self {
            name: Arc::clone(&self.name),
            channel_id: self.channel_id,
            channel_config: self.channel_config.clone(),
            is_running: self.is_running.clone(),
            listener: self.listener.clone(),
            clients: self.clients.clone(),
            ip_whitelist: self.ip_whitelist.clone(),
            server_handle: self.server_handle.clone(),
            status: self.status.clone(),
            redis_client: self.redis_client.clone(),
        }
    }
}
