//! IEC 60870-5-104 Configuration

use serde::{Deserialize, Serialize};

/// IEC 60870-5-104 configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iec104Config {
    /// Server host address
    pub host: String,
    /// Server port
    pub port: u16,
    /// Common address of ASDU
    pub common_addr: u16,
    /// Cause of transmission field size (1 or 2 bytes)
    pub cot_size: u8,
    /// Common address field size (1 or 2 bytes)
    pub coa_size: u8,
    /// Information object address field size (1, 2 or 3 bytes)
    pub ioa_size: u8,
    /// Connection establishment timeout (ms)
    pub t0_timeout: u64,
    /// Send or test APDU timeout (ms)
    pub t1_timeout: u64,
    /// Acknowledgement timeout when no data (ms)
    pub t2_timeout: u64,
    /// Test frame timeout (ms)
    pub t3_timeout: u64,
    /// Maximum number of outstanding I format APDUs
    pub k_value: u16,
    /// Latest acknowledgement after receiving w I format APDUs
    pub w_value: u16,
}

impl Default for Iec104Config {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 2404,
            common_addr: 1,
            cot_size: 2,
            coa_size: 2,
            ioa_size: 3,
            t0_timeout: 30000,
            t1_timeout: 15000,
            t2_timeout: 10000,
            t3_timeout: 20000,
            k_value: 12,
            w_value: 8,
        }
    }
}
