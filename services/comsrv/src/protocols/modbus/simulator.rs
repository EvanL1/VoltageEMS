//! Modbus TCP simulator for testing
//!
//! A simple in-memory Modbus TCP server for integration testing

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Simple Modbus TCP simulator
pub struct ModbusSimulator {
    /// Holding registers (40001-49999)
    holding_registers: Arc<RwLock<HashMap<u16, u16>>>,
    /// Input registers (30001-39999)  
    input_registers: Arc<RwLock<HashMap<u16, u16>>>,
    /// Coils (00001-09999)
    coils: Arc<RwLock<HashMap<u16, bool>>>,
    /// Discrete inputs (10001-19999)
    discrete_inputs: Arc<RwLock<HashMap<u16, bool>>>,
    listener_addr: Option<SocketAddr>,
}

impl Default for ModbusSimulator {
    fn default() -> Self {
        Self {
            holding_registers: Arc::new(RwLock::new(HashMap::new())),
            input_registers: Arc::new(RwLock::new(HashMap::new())),
            coils: Arc::new(RwLock::new(HashMap::new())),
            discrete_inputs: Arc::new(RwLock::new(HashMap::new())),
            listener_addr: None,
        }
    }
}

impl ModbusSimulator {
    /// Create new simulator
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize with test data
    pub async fn init_test_data(&self) {
        // Initialize holding registers (40001-40100)
        let mut holding = self.holding_registers.write().await;
        for i in 1..=100 {
            holding.insert(i, i * 10); // 10, 20, 30...
        }
        drop(holding);

        // Initialize input registers (30001-30100)
        let mut input = self.input_registers.write().await;
        for i in 1..=100 {
            input.insert(i, i * 5); // 5, 10, 15...
        }
        drop(input);

        // Initialize coils (1-100)
        let mut coils = self.coils.write().await;
        for i in 1..=100 {
            coils.insert(i, i % 2 == 0); // Even = true, Odd = false
        }
        drop(coils);

        // Initialize discrete inputs (10001-10100)
        let mut discrete = self.discrete_inputs.write().await;
        for i in 1..=100 {
            discrete.insert(i, i % 3 == 0); // Divisible by 3 = true
        }
    }

    /// Start simulator server
    pub async fn start(mut self, port: u16) -> Result<SocketAddr, Box<dyn std::error::Error>> {
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&addr).await?;
        let local_addr = listener.local_addr()?;

        info!("Modbus simulator listening on {}", local_addr);
        self.listener_addr = Some(local_addr);

        let sim = Arc::new(self);

        // Spawn accept loop
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        debug!("New connection from {}", addr);
                        let sim_clone = sim.clone();
                        tokio::spawn(async move {
                            if let Err(e) = sim_clone.handle_connection(stream).await {
                                error!("Connection error: {}", e);
                            }
                        });
                    },
                    Err(e) => {
                        error!("Accept error: {}", e);
                        break;
                    },
                }
            }
        });

        Ok(local_addr)
    }

    /// Handle a single connection
    async fn handle_connection(
        &self,
        mut stream: TcpStream,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut buffer = vec![0u8; 260]; // Max Modbus TCP frame size

        loop {
            let n = match stream.read(&mut buffer).await {
                Ok(0) => break, // Connection closed
                Ok(n) => n,
                Err(e) => {
                    debug!("Read error: {}", e);
                    break;
                },
            };

            debug!("Received {} bytes", n);

            // Parse Modbus TCP frame
            if n >= 12 {
                // Minimum frame size
                let transaction_id = u16::from_be_bytes([buffer[0], buffer[1]]);
                let protocol_id = u16::from_be_bytes([buffer[2], buffer[3]]);
                let _length = u16::from_be_bytes([buffer[4], buffer[5]]);
                let unit_id = buffer[6];
                let function_code = buffer[7];

                if protocol_id != 0 {
                    continue; // Not Modbus protocol
                }

                debug!(
                    "Transaction: {}, Unit: {}, Function: 0x{:02X}",
                    transaction_id, unit_id, function_code
                );

                // Build response based on function code
                let response = match function_code {
                    0x03 => {
                        // Read Holding Registers
                        let start_addr = u16::from_be_bytes([buffer[8], buffer[9]]);
                        let count = u16::from_be_bytes([buffer[10], buffer[11]]);
                        self.read_holding_registers(transaction_id, unit_id, start_addr, count)
                            .await
                    },
                    0x04 => {
                        // Read Input Registers
                        let start_addr = u16::from_be_bytes([buffer[8], buffer[9]]);
                        let count = u16::from_be_bytes([buffer[10], buffer[11]]);
                        self.read_input_registers(transaction_id, unit_id, start_addr, count)
                            .await
                    },
                    0x06 => {
                        // Write Single Register
                        let addr = u16::from_be_bytes([buffer[8], buffer[9]]);
                        let value = u16::from_be_bytes([buffer[10], buffer[11]]);
                        self.write_single_register(transaction_id, unit_id, addr, value)
                            .await
                    },
                    _ => {
                        // Unsupported function - return exception
                        self.build_exception(transaction_id, unit_id, function_code, 0x01)
                    },
                };

                if let Err(e) = stream.write_all(&response).await {
                    debug!("Write error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Read holding registers (Function 0x03)
    async fn read_holding_registers(&self, tid: u16, uid: u8, start: u16, count: u16) -> Vec<u8> {
        let holding = self.holding_registers.read().await;
        let mut response = Vec::new();

        // MBAP Header
        response.extend_from_slice(&tid.to_be_bytes());
        response.extend_from_slice(&[0x00, 0x00]); // Protocol ID
        response.extend_from_slice(&(3 + count * 2).to_be_bytes()); // Length
        response.push(uid);
        response.push(0x03); // Function code
        response.push((count * 2) as u8); // Byte count

        // Data
        for i in 0..count {
            let addr = start + i + 1; // Modbus addresses start at 1
            let value = holding.get(&addr).copied().unwrap_or(0);
            response.extend_from_slice(&value.to_be_bytes());
        }

        response
    }

    /// Read input registers (Function 0x04)
    async fn read_input_registers(&self, tid: u16, uid: u8, start: u16, count: u16) -> Vec<u8> {
        let input = self.input_registers.read().await;
        let mut response = Vec::new();

        // MBAP Header
        response.extend_from_slice(&tid.to_be_bytes());
        response.extend_from_slice(&[0x00, 0x00]); // Protocol ID
        response.extend_from_slice(&(3 + count * 2).to_be_bytes()); // Length
        response.push(uid);
        response.push(0x04); // Function code
        response.push((count * 2) as u8); // Byte count

        // Data
        for i in 0..count {
            let addr = start + i + 1;
            let value = input.get(&addr).copied().unwrap_or(0);
            response.extend_from_slice(&value.to_be_bytes());
        }

        response
    }

    /// Write single register (Function 0x06)
    async fn write_single_register(&self, tid: u16, uid: u8, addr: u16, value: u16) -> Vec<u8> {
        let mut holding = self.holding_registers.write().await;
        holding.insert(addr + 1, value); // Modbus addresses start at 1

        // Echo back the request as response
        let mut response = Vec::new();
        response.extend_from_slice(&tid.to_be_bytes());
        response.extend_from_slice(&[0x00, 0x00]); // Protocol ID
        response.extend_from_slice(&6u16.to_be_bytes()); // Length
        response.push(uid);
        response.push(0x06); // Function code
        response.extend_from_slice(&addr.to_be_bytes());
        response.extend_from_slice(&value.to_be_bytes());

        response
    }

    /// Build exception response
    fn build_exception(&self, tid: u16, uid: u8, func: u8, exception: u8) -> Vec<u8> {
        let mut response = Vec::new();
        response.extend_from_slice(&tid.to_be_bytes());
        response.extend_from_slice(&[0x00, 0x00]); // Protocol ID
        response.extend_from_slice(&3u16.to_be_bytes()); // Length
        response.push(uid);
        response.push(func | 0x80); // Set error bit
        response.push(exception);
        response
    }

    /// Get holding register value (for testing)
    pub async fn get_holding_register(&self, addr: u16) -> Option<u16> {
        let holding = self.holding_registers.read().await;
        holding.get(&addr).copied()
    }
}

// NOTE: The previous integration-style test that started a local TCP simulator
// was removed from unit tests to avoid failures in restricted environments.
// It should be implemented as an integration test under tests/ and run only
// in environments that allow opening local sockets.
