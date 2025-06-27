//! IEC60870-5-104 Protocol Implementation
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, mpsc, RwLock};
use tokio::time::sleep;
use chrono::{DateTime, Utc};
use tracing::{debug, error, info, warn};

use crate::asdu::{ASDU, CommonAddrSize, TypeId};
use crate::common::{CauseOfTransmission, IecError, IecResult};

/// Control field codes for APCI
pub const START_DT_ACT: u8 = 0x07; // Start data transfer activation
pub const START_DT_CON: u8 = 0x0B; // Start data transfer confirmation
pub const STOP_DT_ACT: u8 = 0x13;  // Stop data transfer activation
pub const STOP_DT_CON: u8 = 0x23;  // Stop data transfer confirmation
pub const TEST_FR_ACT: u8 = 0x43;  // Test frame activation
pub const TEST_FR_CON: u8 = 0x83;  // Test frame confirmation

/// APCI structure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApciType {
    /// I-format (information transfer format)
    IFrame { 
        send_seq: u16, 
        recv_seq: u16 
    },
    /// S-format (supervisory format)
    SFrame { 
        recv_seq: u16 
    },
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
            },
            ApciType::SFrame { recv_seq } => {
                // Control fields for S-frame
                buffer.push(0x01);
                buffer.push(0x00);
                buffer.push(((recv_seq << 1) & 0xFE) as u8);
                buffer.push((recv_seq >> 7) as u8);
            },
            ApciType::UFrame(code) => {
                // Control fields for U-frame
                buffer.push(code);
                buffer.push(0x00);
                buffer.push(0x00);
                buffer.push(0x00);
            },
        }
        
        // Update length (excluding start character and length byte)
        let length = buffer.len() - 2;
        buffer[1] = length as u8;
        
        Ok(buffer)
    }
    
    /// Decode APDU from bytes
    pub fn decode(data: &[u8], common_addr_size: CommonAddrSize) -> IecResult<Self> {
        if data.len() < 6 {
            return Err(IecError::ProtocolError(
                "APDU data too short".to_string(),
            ));
        }
        
        // Check start character
        if data[0] != 0x68 {
            return Err(IecError::ProtocolError(
                format!("Invalid start character: {:02X}", data[0]),
            ));
        }
        
        // Check length
        let length = data[1] as usize;
        if data.len() < length + 2 {
            return Err(IecError::ProtocolError(
                format!("APDU data too short. Expected {} bytes, got {}", length + 2, data.len()),
            ));
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
            Err(IecError::ProtocolError(
                format!("Invalid control field: {:02X}", control1),
            ))
        }
    }
}

/// IEC-104 client configuration
#[derive(Debug, Clone)]
pub struct Iec104ClientConfig {
    /// Server hostname or IP address
    pub host: String,
    /// Server TCP port
    pub port: u16,
    /// Connection timeout in seconds
    pub timeout: Duration,
    /// Maximum connection retries
    pub max_retries: u32,
    /// Common address size (1 or 2 bytes)
    pub common_addr_size: CommonAddrSize,
    /// Information object address size (1, 2, or 3 bytes)
    pub info_obj_addr_size: usize,
    /// Connection keepalive interval
    pub keepalive_interval: Duration,
    /// Acknowledge timeout (t1)
    pub t1: Duration,
    /// Frame retransmit timeout (t2)
    pub t2: Duration,
    /// Test frame timeout (t3)
    pub t3: Duration,
}

impl Default for Iec104ClientConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 2404,
            timeout: Duration::from_secs(5),
            max_retries: 3,
            common_addr_size: CommonAddrSize::TwoOctets,
            info_obj_addr_size: 3,
            keepalive_interval: Duration::from_secs(20),
            t1: Duration::from_secs(15),
            t2: Duration::from_secs(10),
            t3: Duration::from_secs(20),
        }
    }
}

impl Iec104ClientConfig {
    /// Create a new default configuration
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set the host
    pub fn host<S: Into<String>>(mut self, host: S) -> Self {
        self.host = host.into();
        self
    }
    
    /// Set the port
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
    
    /// Set the connection timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    
    /// Set the maximum connection retries
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }
    
    /// Set the common address size
    pub fn common_addr_size(mut self, size: CommonAddrSize) -> Self {
        self.common_addr_size = size;
        self
    }
    
    /// Set the information object address size
    pub fn info_obj_addr_size(mut self, size: usize) -> Self {
        if size >= 1 && size <= 3 {
            self.info_obj_addr_size = size;
        }
        self
    }
    
    /// Build the configuration
    pub fn build(self) -> IecResult<Self> {
        // Validate configuration
        if self.info_obj_addr_size < 1 || self.info_obj_addr_size > 3 {
            return Err(IecError::ConfigError(
                format!("Invalid info_obj_addr_size: {}", self.info_obj_addr_size)
            ));
        }
        
        Ok(self)
    }
} 