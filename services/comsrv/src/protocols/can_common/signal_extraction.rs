//! Signal extraction utilities for CAN data
//!
//! Handles extraction of signals from CAN message data based on bit positions

use super::byte_order::{bytes_to_f32, bytes_to_f64, bytes_to_u32, bytes_to_u64};
use super::types::{ByteOrder, SignalDataType};
use crate::utils::bytes::extract_bits;

/// Extract signal value from CAN data
#[allow(clippy::too_many_arguments)] // All parameters are required for CAN signal extraction
pub fn extract_signal_from_data(
    data: &[u8],
    start_bit: u8,
    bit_length: u8,
    byte_order: &ByteOrder,
    data_type: &SignalDataType,
    signed: bool,
    scale: f64,
    offset: f64,
) -> f64 {
    // Extract the relevant bytes based on start_bit and bit_length
    let start_byte = (start_bit / 8) as usize;
    let end_byte = ((start_bit + bit_length - 1) / 8) as usize;

    if start_byte >= data.len() {
        return 0.0;
    }

    let byte_count = (end_byte - start_byte + 1).min(data.len() - start_byte);
    let extracted_bytes = &data[start_byte..start_byte + byte_count];

    // Convert based on data type
    let raw_value = match data_type {
        SignalDataType::Boolean => {
            let bit_value = extract_bits(data, start_bit as u16, 1);
            bit_value as f64
        },
        SignalDataType::UInt8 | SignalDataType::Int8 => {
            let bits = extract_bits(data, start_bit as u16, bit_length.min(8));
            if signed && bit_length <= 8 {
                // Sign extend for signed 8-bit
                let sign_bit = 1 << (bit_length - 1);
                if bits & sign_bit != 0 {
                    (bits | (0xFFFFFFFFFFFFFF00u64 << bit_length)) as i8 as f64
                } else {
                    bits as f64
                }
            } else {
                bits as f64
            }
        },
        SignalDataType::UInt16 | SignalDataType::Int16 => {
            let bits = extract_bits(data, start_bit as u16, bit_length.min(16));
            if signed && bit_length <= 16 {
                // Sign extend for signed 16-bit
                let sign_bit = 1 << (bit_length - 1);
                if bits & sign_bit != 0 {
                    (bits | (0xFFFFFFFFFFFF0000u64 << bit_length)) as i16 as f64
                } else {
                    bits as f64
                }
            } else {
                bits as f64
            }
        },
        SignalDataType::UInt32 | SignalDataType::Int32 => {
            if bit_length <= 32 {
                let bits = extract_bits(data, start_bit as u16, bit_length);
                if signed {
                    // Sign extend for signed 32-bit
                    let sign_bit = 1 << (bit_length - 1);
                    if bits & sign_bit != 0 {
                        (bits | (0xFFFFFFFF00000000u64 << bit_length)) as i32 as f64
                    } else {
                        bits as f64
                    }
                } else {
                    bits as f64
                }
            } else {
                // Use byte order for multi-byte extraction
                let value = bytes_to_u32(extracted_bytes, byte_order);
                if signed {
                    value as i32 as f64
                } else {
                    value as f64
                }
            }
        },
        SignalDataType::UInt64 | SignalDataType::Int64 => {
            if bit_length <= 64 {
                let bits = extract_bits(data, start_bit as u16, bit_length);
                if signed {
                    // Sign extend for signed 64-bit
                    let sign_bit = 1u64 << (bit_length - 1);
                    if bits & sign_bit != 0 {
                        // Sign extend by setting all higher bits
                        let mask = !((1u64 << bit_length) - 1);
                        (bits | mask) as i64 as f64
                    } else {
                        bits as f64
                    }
                } else {
                    bits as f64
                }
            } else {
                let value = bytes_to_u64(extracted_bytes, byte_order);
                if signed {
                    value as i64 as f64
                } else {
                    value as f64
                }
            }
        },
        SignalDataType::Float32 => bytes_to_f32(extracted_bytes, byte_order) as f64,
        SignalDataType::Float64 => bytes_to_f64(extracted_bytes, byte_order),
        SignalDataType::String | SignalDataType::Bytes => {
            // CAN signals typically don't use string/bytes types
            // Return 0.0 or handle as needed for your use case
            0.0
        },
    };

    // Apply scale and offset
    raw_value * scale + offset
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_extract_bits() {
        let data = vec![0b10110101, 0b11001100];

        // Extract 4 bits starting at bit 2
        let result = extract_bits(&data, 2, 4);
        assert_eq!(result, 0b1101);

        // Extract 8 bits starting at bit 4
        let result = extract_bits(&data, 4, 8);
        assert_eq!(result, 0b11001011);
    }

    #[test]
    fn test_extract_signal_uint16() {
        let data = vec![0x12, 0x34, 0x56, 0x78];

        let value = extract_signal_from_data(
            &data,
            0,
            16,
            &ByteOrder::ABCD,
            &SignalDataType::UInt16,
            false,
            1.0,
            0.0,
        );

        // Should extract 0x3412 = 13330 (due to byte order handling)
        assert_eq!(value, 13330.0);
    }
}
