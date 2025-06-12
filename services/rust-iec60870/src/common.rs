/// IEC60870 Constants and Common Data Types
use std::fmt;
use thiserror::Error;

/// IEC60870 protocol versions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IecProtocolVersion {
    /// IEC 60870-5-101
    Iec101,
    /// IEC 60870-5-104
    Iec104,
}

impl fmt::Display for IecProtocolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IecProtocolVersion::Iec101 => write!(f, "IEC 60870-5-101"),
            IecProtocolVersion::Iec104 => write!(f, "IEC 60870-5-104"),
        }
    }
}

/// IEC60870 Error Types
#[derive(Error, Debug)]
pub enum IecError {
    /// Error in connection
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    /// Timeout error
    #[error("Timeout error: {0}")]
    TimeoutError(String),
    
    /// Protocol error
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    
    /// Data conversion error
    #[error("Data conversion error: {0}")]
    DataConversionError(String),
    
    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    /// Client not connected
    #[error("Client not connected")]
    NotConnected,
    
    /// Data transfer not started
    #[error("Data transfer not started")]
    DataTransferNotStarted,
}

/// Common protocol result type
pub type IecResult<T> = Result<T, IecError>;

/// Quality Descriptor Flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualityDescriptor {
    /// Reserved bit
    pub reserved: bool,
    /// Blocked: the value is blocked for transmission by a local lock
    pub blocked: bool,
    /// Substituted: the value has been provided by the operator
    pub substituted: bool,
    /// Not topical: the value is outdated
    pub not_topical: bool,
    /// Invalid: the value is invalid
    pub invalid: bool,
}

impl QualityDescriptor {
    /// Create a new quality descriptor with default values
    pub fn new() -> Self {
        Self {
            reserved: false,
            blocked: false,
            substituted: false,
            not_topical: false,
            invalid: false,
        }
    }
    
    /// Create a quality descriptor from a byte
    pub fn from_byte(value: u8) -> Self {
        Self {
            reserved: (value & 0x01) != 0,
            blocked: (value & 0x10) != 0,
            substituted: (value & 0x20) != 0,
            not_topical: (value & 0x40) != 0,
            invalid: (value & 0x80) != 0,
        }
    }
    
    /// Convert quality descriptor to a byte
    pub fn to_byte(&self) -> u8 {
        let mut value = 0u8;
        if self.reserved { value |= 0x01; }
        if self.blocked { value |= 0x10; }
        if self.substituted { value |= 0x20; }
        if self.not_topical { value |= 0x40; }
        if self.invalid { value |= 0x80; }
        value
    }
}

impl Default for QualityDescriptor {
    fn default() -> Self {
        Self::new()
    }
}

/// Cause of Transmission (COT)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CauseOfTransmission {
    /// Periodic, cyclic
    Periodic = 1,
    /// Background scan
    Background = 2,
    /// Spontaneous
    Spontaneous = 3,
    /// Initialized
    Initialized = 4,
    /// Request or requested
    Request = 5,
    /// Activation
    Activation = 6,
    /// Activation confirmation
    ActivationConfirmation = 7,
    /// Deactivation
    Deactivation = 8,
    /// Deactivation confirmation
    DeactivationConfirmation = 9,
    /// Activation termination
    ActivationTermination = 10,
    /// Return information caused by a remote command
    RemoteCommand = 11,
    /// Return information caused by a local command
    LocalCommand = 12,
    /// File transfer
    FileTransfer = 13,
    /// Authentication
    Authentication = 14,
    /// Maintenance of auth. session key
    SessionKey = 15,
    /// Maintenance of user role and its auth. key
    UserRoleAndAuthKey = 16,
    /// Interrogated by station interrogation
    StationInterrogation = 20,
    /// Interrogated by group 1 interrogation
    Group1Interrogation = 21,
    /// Interrogated by group 2 interrogation
    Group2Interrogation = 22,
    /// Interrogated by group 3 interrogation
    Group3Interrogation = 23,
    /// Interrogated by group 4 interrogation
    Group4Interrogation = 24,
    /// Interrogated by group 5 interrogation
    Group5Interrogation = 25,
    /// Interrogated by group 6 interrogation
    Group6Interrogation = 26,
    /// Interrogated by group 7 interrogation
    Group7Interrogation = 27,
    /// Interrogated by group 8 interrogation
    Group8Interrogation = 28,
    /// Interrogated by group 9 interrogation
    Group9Interrogation = 29,
    /// Interrogated by group 10 interrogation
    Group10Interrogation = 30,
    /// Interrogated by group 11 interrogation
    Group11Interrogation = 31,
    /// Interrogated by group 12 interrogation
    Group12Interrogation = 32,
    /// Interrogated by group 13 interrogation
    Group13Interrogation = 33,
    /// Interrogated by group 14 interrogation
    Group14Interrogation = 34,
    /// Interrogated by group 15 interrogation
    Group15Interrogation = 35,
    /// Interrogated by group 16 interrogation
    Group16Interrogation = 36,
    /// Requested by general counter request
    GeneralCounterRequest = 37,
    /// Requested by group 1 counter request
    Group1CounterRequest = 38,
    /// Requested by group 2 counter request
    Group2CounterRequest = 39,
    /// Requested by group 3 counter request
    Group3CounterRequest = 40,
    /// Requested by group 4 counter request
    Group4CounterRequest = 41,
    /// Unknown type identification
    UnknownTypeIdentification = 44,
    /// Unknown cause of transmission
    UnknownCauseOfTransmission = 45,
    /// Unknown common address of ASDU
    UnknownCommonAddress = 46,
    /// Unknown information object address
    UnknownInfoObjAddress = 47,
}

impl CauseOfTransmission {
    /// Create a COT from a byte
    pub fn from_byte(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Periodic),
            2 => Some(Self::Background),
            3 => Some(Self::Spontaneous),
            4 => Some(Self::Initialized),
            5 => Some(Self::Request),
            6 => Some(Self::Activation),
            7 => Some(Self::ActivationConfirmation),
            8 => Some(Self::Deactivation),
            9 => Some(Self::DeactivationConfirmation),
            10 => Some(Self::ActivationTermination),
            11 => Some(Self::RemoteCommand),
            12 => Some(Self::LocalCommand),
            13 => Some(Self::FileTransfer),
            14 => Some(Self::Authentication),
            15 => Some(Self::SessionKey),
            16 => Some(Self::UserRoleAndAuthKey),
            20 => Some(Self::StationInterrogation),
            21 => Some(Self::Group1Interrogation),
            22 => Some(Self::Group2Interrogation),
            23 => Some(Self::Group3Interrogation),
            24 => Some(Self::Group4Interrogation),
            25 => Some(Self::Group5Interrogation),
            26 => Some(Self::Group6Interrogation),
            27 => Some(Self::Group7Interrogation),
            28 => Some(Self::Group8Interrogation),
            29 => Some(Self::Group9Interrogation),
            30 => Some(Self::Group10Interrogation),
            31 => Some(Self::Group11Interrogation),
            32 => Some(Self::Group12Interrogation),
            33 => Some(Self::Group13Interrogation),
            34 => Some(Self::Group14Interrogation),
            35 => Some(Self::Group15Interrogation),
            36 => Some(Self::Group16Interrogation),
            37 => Some(Self::GeneralCounterRequest),
            38 => Some(Self::Group1CounterRequest),
            39 => Some(Self::Group2CounterRequest),
            40 => Some(Self::Group3CounterRequest),
            41 => Some(Self::Group4CounterRequest),
            44 => Some(Self::UnknownTypeIdentification),
            45 => Some(Self::UnknownCauseOfTransmission),
            46 => Some(Self::UnknownCommonAddress),
            47 => Some(Self::UnknownInfoObjAddress),
            _ => None,
        }
    }
    
    /// Convert COT to a byte
    pub fn to_byte(&self) -> u8 {
        *self as u8
    }
} 