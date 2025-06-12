//! CAN Frame Definition and Handling
//! 
//! This module defines the CAN frame structure and provides utilities
//! for parsing and constructing CAN messages.

use serde::{Deserialize, Serialize};

/// CAN frame identifier type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CanId {
    /// Standard 11-bit identifier
    Standard(u16),
    /// Extended 29-bit identifier  
    Extended(u32),
}

impl CanId {
    /// Get the raw identifier value
    pub fn raw(&self) -> u32 {
        match self {
            CanId::Standard(id) => *id as u32,
            CanId::Extended(id) => *id,
        }
    }
    
    /// Check if this is an extended identifier
    pub fn is_extended(&self) -> bool {
        matches!(self, CanId::Extended(_))
    }
}

/// CAN frame data
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanFrame {
    /// Frame identifier
    pub id: CanId,
    /// Frame data (0-8 bytes)
    pub data: Vec<u8>,
    /// Remote transmission request flag
    pub rtr: bool,
    /// Error frame flag
    pub err: bool,
}

impl CanFrame {
    /// Create a new CAN frame
    pub fn new(id: CanId, data: Vec<u8>) -> Result<Self, String> {
        if data.len() > 8 {
            return Err("CAN frame data cannot exceed 8 bytes".to_string());
        }
        
        Ok(CanFrame {
            id,
            data,
            rtr: false,
            err: false,
        })
    }
    
    /// Create a new standard CAN frame
    pub fn new_standard(id: u16, data: Vec<u8>) -> Result<Self, String> {
        if id > 0x7FF {
            return Err("Standard CAN ID cannot exceed 0x7FF".to_string());
        }
        Self::new(CanId::Standard(id), data)
    }
    
    /// Create a new extended CAN frame
    pub fn new_extended(id: u32, data: Vec<u8>) -> Result<Self, String> {
        if id > 0x1FFFFFFF {
            return Err("Extended CAN ID cannot exceed 0x1FFFFFFF".to_string());
        }
        Self::new(CanId::Extended(id), data)
    }
    
    /// Create a remote transmission request frame
    pub fn new_rtr(id: CanId, dlc: u8) -> Result<Self, String> {
        if dlc > 8 {
            return Err("DLC cannot exceed 8".to_string());
        }
        
        Ok(CanFrame {
            id,
            data: vec![0; dlc as usize],
            rtr: true,
            err: false,
        })
    }
    
    /// Get data length code
    pub fn dlc(&self) -> u8 {
        self.data.len() as u8
    }
    
    /// Check if this is a remote transmission request
    pub fn is_rtr(&self) -> bool {
        self.rtr
    }
    
    /// Check if this is an error frame
    pub fn is_error(&self) -> bool {
        self.err
    }
}

/// CAN message filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanFilter {
    /// Filter ID
    pub id: u32,
    /// Filter mask
    pub mask: u32,
    /// Apply to extended frames
    pub extended: bool,
}

impl CanFilter {
    /// Create a new CAN filter
    pub fn new(id: u32, mask: u32, extended: bool) -> Self {
        CanFilter { id, mask, extended }
    }
    
    /// Check if a frame matches this filter
    pub fn matches(&self, frame: &CanFrame) -> bool {
        let frame_extended = frame.id.is_extended();
        
        // Extended flag must match
        if self.extended != frame_extended {
            return false;
        }
        
        // Apply mask to both IDs and compare
        let frame_id = frame.id.raw();
        (frame_id & self.mask) == (self.id & self.mask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_frame_creation() {
        let frame = CanFrame::new_standard(0x123, vec![1, 2, 3, 4]).unwrap();
        assert_eq!(frame.id, CanId::Standard(0x123));
        assert_eq!(frame.data, vec![1, 2, 3, 4]);
        assert_eq!(frame.dlc(), 4);
        assert!(!frame.is_rtr());
    }
    
    #[test]
    fn test_can_filter() {
        let filter = CanFilter::new(0x100, 0xFF0, false);
        
        let frame1 = CanFrame::new_standard(0x123, vec![]).unwrap();
        let frame2 = CanFrame::new_standard(0x200, vec![]).unwrap();
        
        assert!(filter.matches(&frame1));
        assert!(!filter.matches(&frame2));
    }
} 