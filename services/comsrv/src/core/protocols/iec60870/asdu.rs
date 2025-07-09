use crate::core::protocols::iec60870::common::{CauseOfTransmission, IecError, IecResult};
use byteorder::ReadBytesExt;
/// ASDU - Application Service Data Unit Implementation
use std::io::Cursor;

/// Information Object Addresses can be 1, 2, or 3 bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoObjAddrSize {
    /// 1 byte address
    OneOctet = 1,
    /// 2 byte address
    TwoOctets = 2,
    /// 3 byte address
    ThreeOctets = 3,
}

/// Common Address of ASDU can be 1 or 2 bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonAddrSize {
    /// 1 byte address
    OneOctet = 1,
    /// 2 byte address
    TwoOctets = 2,
}

/// Type Identification (TI) for ASDUs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TypeId {
    /// Single-point information (M_SP_NA_1)
    SinglePoint = 1,
    /// Single-point information with time tag (M_SP_TA_1)
    SinglePointWithTime = 2,
    /// Double-point information (M_DP_NA_1)
    DoublePoint = 3,
    /// Double-point information with time tag (M_DP_TA_1)
    DoublePointWithTime = 4,
    /// Step position information (M_ST_NA_1)
    StepPosition = 5,
    /// Step position information with time tag (M_ST_TA_1)
    StepPositionWithTime = 6,
    /// Bitstring of 32 bits (M_BO_NA_1)
    Bitstring32Bit = 7,
    /// Bitstring of 32 bits with time tag (M_BO_TA_1)
    Bitstring32BitWithTime = 8,
    /// Measured value, normalized value (M_ME_NA_1)
    MeasuredValueNormal = 9,
    /// Measured value, normalized value with time tag (M_ME_TA_1)
    MeasuredValueNormalWithTime = 10,
    /// Measured value, scaled value (M_ME_NB_1)
    MeasuredValueScaled = 11,
    /// Measured value, scaled value with time tag (M_ME_TB_1)
    MeasuredValueScaledWithTime = 12,
    /// Measured value, short floating point number (M_ME_NC_1)
    MeasuredValueFloat = 13,
    /// Measured value, short floating point number with time tag (M_ME_TC_1)
    MeasuredValueFloatWithTime = 14,
    /// Integrated totals (M_IT_NA_1)
    IntegratedTotals = 15,
    /// Integrated totals with time tag (M_IT_TA_1)
    IntegratedTotalsWithTime = 16,
    /// Event of protection equipment (M_EP_TA_1)
    EventOfProtectionEquipment = 17,
    /// Packed start events of protection equipment (M_EP_TB_1)
    PackedStartEventsOfProtectionEquipment = 18,
    /// Packed output circuit info of protection equipment (M_EP_TC_1)
    PackedOutputCircuitInfo = 19,
    /// Packed single-point info with status change detection (M_PS_NA_1)
    PackedSinglePointWithScd = 20,
    /// Measured value, normalized value without quality descriptor (M_ME_ND_1)
    MeasuredValueNormalNoQuality = 21,

    // ...additional types for supervisory, parameter, file transfer...
    /// Single command (C_SC_NA_1)
    SingleCommand = 45,
    /// Double command (C_DC_NA_1)
    DoubleCommand = 46,
    /// Regulating step command (C_RC_NA_1)
    RegulatingStepCommand = 47,
    /// Set-point command, normalized value (C_SE_NA_1)
    SetpointCommandNormal = 48,
    /// Set-point command, scaled value (C_SE_NB_1)
    SetpointCommandScaled = 49,
    /// Set-point command, short floating point number (C_SE_NC_1)
    SetpointCommandFloat = 50,
    /// Bitstring of 32 bits (C_BO_NA_1)
    Bitstring32BitCommand = 51,

    // ... more command types ...
    /// Interrogation command (C_IC_NA_1)
    InterrogationCommand = 100,
    /// Counter interrogation command (C_CI_NA_1)
    CounterInterrogationCommand = 101,
    /// Read command (C_RD_NA_1)
    ReadCommand = 102,
    /// Clock synchronization command (C_CS_NA_1)
    ClockSyncCommand = 103,
    /// Test command (C_TS_NA_1)
    TestCommand = 104,
    /// Reset process command (C_RP_NA_1)
    ResetProcessCommand = 105,
    /// Delay acquisition command (C_CD_NA_1)
    DelayAcquisitionCommand = 106,
    /// Test command with time tag (C_TS_TA_1)
    TestCommandWithTime = 107,

    // ... more system types ...
    /// Parameter of measured value, normalized value (P_ME_NA_1)
    ParameterNormalValue = 110,
    /// Parameter of measured value, scaled value (P_ME_NB_1)
    ParameterScaledValue = 111,
    /// Parameter of measured value, short floating point number (P_ME_NC_1)
    ParameterFloatValue = 112,
    /// Parameter activation (P_AC_NA_1)
    ParameterActivation = 113,

    // ... file transfer types ...
    /// File ready (F_FR_NA_1)
    FileReady = 120,
    /// Section ready (F_SR_NA_1)
    SectionReady = 121,
    /// Call directory, select file, call file, call section (F_SC_NA_1)
    CallDirectory = 122,
    /// Last section, last segment (F_LS_NA_1)
    LastSection = 123,
    /// ACK file, ACK section (F_AF_NA_1)
    AckFile = 124,
    /// Segment (F_SG_NA_1)
    Segment = 125,
    /// Directory (F_DR_TA_1)
    Directory = 126,
    /// QueryLog, request archive file (F_SC_NB_1)
    QueryLog = 127,
}

impl TypeId {
    /// Create TypeId from a byte
    pub fn from_byte(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::SinglePoint),
            2 => Some(Self::SinglePointWithTime),
            3 => Some(Self::DoublePoint),
            4 => Some(Self::DoublePointWithTime),
            5 => Some(Self::StepPosition),
            6 => Some(Self::StepPositionWithTime),
            7 => Some(Self::Bitstring32Bit),
            8 => Some(Self::Bitstring32BitWithTime),
            9 => Some(Self::MeasuredValueNormal),
            10 => Some(Self::MeasuredValueNormalWithTime),
            11 => Some(Self::MeasuredValueScaled),
            12 => Some(Self::MeasuredValueScaledWithTime),
            13 => Some(Self::MeasuredValueFloat),
            14 => Some(Self::MeasuredValueFloatWithTime),
            15 => Some(Self::IntegratedTotals),
            16 => Some(Self::IntegratedTotalsWithTime),
            17 => Some(Self::EventOfProtectionEquipment),
            18 => Some(Self::PackedStartEventsOfProtectionEquipment),
            19 => Some(Self::PackedOutputCircuitInfo),
            20 => Some(Self::PackedSinglePointWithScd),
            21 => Some(Self::MeasuredValueNormalNoQuality),
            45 => Some(Self::SingleCommand),
            46 => Some(Self::DoubleCommand),
            47 => Some(Self::RegulatingStepCommand),
            48 => Some(Self::SetpointCommandNormal),
            49 => Some(Self::SetpointCommandScaled),
            50 => Some(Self::SetpointCommandFloat),
            51 => Some(Self::Bitstring32BitCommand),
            100 => Some(Self::InterrogationCommand),
            101 => Some(Self::CounterInterrogationCommand),
            102 => Some(Self::ReadCommand),
            103 => Some(Self::ClockSyncCommand),
            104 => Some(Self::TestCommand),
            105 => Some(Self::ResetProcessCommand),
            106 => Some(Self::DelayAcquisitionCommand),
            107 => Some(Self::TestCommandWithTime),
            110 => Some(Self::ParameterNormalValue),
            111 => Some(Self::ParameterScaledValue),
            112 => Some(Self::ParameterFloatValue),
            113 => Some(Self::ParameterActivation),
            120 => Some(Self::FileReady),
            121 => Some(Self::SectionReady),
            122 => Some(Self::CallDirectory),
            123 => Some(Self::LastSection),
            124 => Some(Self::AckFile),
            125 => Some(Self::Segment),
            126 => Some(Self::Directory),
            127 => Some(Self::QueryLog),
            _ => None,
        }
    }

    /// Convert TypeId to a byte
    pub fn to_byte(self) -> u8 {
        self as u8
    }

    /// Check if this type ID is for a command
    pub fn is_command(self) -> bool {
        let val = self as u8;
        val >= 45 && val <= 107
    }

    /// Check if this type ID is for a parameter
    pub fn is_parameter(self) -> bool {
        let val = self as u8;
        val >= 110 && val <= 113
    }

    /// Check if this type ID is for a file transfer
    pub fn is_file_transfer(self) -> bool {
        let val = self as u8;
        val >= 120 && val <= 127
    }
}

/// ASDU Structure
#[derive(Debug, Clone)]
pub struct ASDU {
    /// Type ID of ASDU
    pub type_id: TypeId,
    /// Variable structure qualifier
    pub vsq: u8,
    /// Cause of transmission
    pub cot: CauseOfTransmission,
    /// Originator address (in bits 8-15 of COT)
    pub originator: u8,
    /// Common address of ASDU
    pub common_addr: u16,
    /// Raw information object data
    pub data: Vec<u8>,
}

impl ASDU {
    /// Create a new ASDU
    pub fn new(
        type_id: TypeId,
        vsq: u8,
        cot: CauseOfTransmission,
        originator: u8,
        common_addr: u16,
        data: Vec<u8>,
    ) -> Self {
        Self {
            type_id,
            vsq,
            cot,
            originator,
            common_addr,
            data,
        }
    }

    /// Get the number of objects from VSQ
    pub fn num_objects(&self) -> u8 {
        self.vsq & 0x7F
    }

    /// Check if the sequential addressing bit is set in VSQ
    pub fn is_sequence(&self) -> bool {
        (self.vsq & 0x80) != 0
    }

    /// Encode ASDU to bytes using the specified sizes for addresses
    pub fn encode(&self, common_addr_size: CommonAddrSize) -> IecResult<Vec<u8>> {
        let mut buffer = Vec::new();

        // Write type ID
        buffer.push(self.type_id as u8);

        // Write VSQ
        buffer.push(self.vsq);

        // Write COT with originator
        let cot_byte = self.cot.to_byte();
        buffer.push(cot_byte);
        buffer.push(self.originator);

        // Write common address
        match common_addr_size {
            CommonAddrSize::OneOctet => {
                buffer.push(self.common_addr as u8);
            }
            CommonAddrSize::TwoOctets => {
                buffer.push((self.common_addr & 0xFF) as u8);
                buffer.push(((self.common_addr >> 8) & 0xFF) as u8);
            }
        }

        // Append information object data
        buffer.extend_from_slice(&self.data);

        Ok(buffer)
    }

    /// Decode ASDU from bytes using the specified sizes for addresses
    pub fn decode(data: &[u8], common_addr_size: CommonAddrSize) -> IecResult<Self> {
        if data.len() < 4 + common_addr_size as usize {
            return Err(IecError::ProtocolError("ASDU data too short".to_string()));
        }

        let mut cursor = Cursor::new(data);

        // Read type ID
        let type_id_byte = cursor.read_u8()?;
        let type_id = TypeId::from_byte(type_id_byte)
            .ok_or_else(|| IecError::ProtocolError(format!("Unknown TypeId: {}", type_id_byte)))?;

        // Read VSQ
        let vsq = cursor.read_u8()?;

        // Read COT with originator
        let cot_byte = cursor.read_u8()?;
        let cot = CauseOfTransmission::from_byte(cot_byte)
            .ok_or_else(|| IecError::ProtocolError(format!("Unknown COT: {}", cot_byte)))?;
        let originator = cursor.read_u8()?;

        // Read common address
        let common_addr = match common_addr_size {
            CommonAddrSize::OneOctet => cursor.read_u8()? as u16,
            CommonAddrSize::TwoOctets => {
                let low = cursor.read_u8()? as u16;
                let high = cursor.read_u8()? as u16;
                low | (high << 8)
            }
        };

        // Read remaining data as information objects
        let position = cursor.position() as usize;
        let data_slice = &data[position..];
        let data = data_slice.to_vec();

        Ok(Self {
            type_id,
            vsq,
            cot,
            originator,
            common_addr,
            data,
        })
    }
}
