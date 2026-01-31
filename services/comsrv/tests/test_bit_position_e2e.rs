//! 端到端测试：验证 bit_position 从配置文件到最终值的完整流程
//!
//! 这个测试文件验证 comsrv 的 bit_position 解析是否正确：
//! 1. decode_registers() 函数的位提取逻辑
//! 2. 各种边界条件 (bit 0-15)
//! 3. 错误处理 (bit > 15)

#![allow(clippy::disallowed_methods)] // Tests use unwrap for clarity

use comsrv::protocols::codec::decode_registers;
use comsrv::protocols::core::point::{ByteOrder, DataFormat};

/// 测试寄存器 0x8421 的各位提取
///
/// 0x8421 的二进制表示:
/// ```text
/// bit: 15 14 13 12 | 11 10  9  8 |  7  6  5  4 |  3  2  1  0
///       1  0  0  0 |  0  1  0  0 |  0  0  1  0 |  0  0  0  1
///       ↑              ↑              ↑              ↑
///      bit15=1       bit10=1        bit5=1         bit0=1
/// ```
#[test]
fn test_bit_position_extraction_0x8421() {
    let reg = [0x8421u16];

    // bit0 = 1 (LSB)
    let result = decode_registers(&reg, DataFormat::Bool, ByteOrder::Abcd, Some(0)).unwrap();
    assert_eq!(result.as_bool(), Some(true), "bit0 should be 1");

    // bit1 = 0
    let result = decode_registers(&reg, DataFormat::Bool, ByteOrder::Abcd, Some(1)).unwrap();
    assert_eq!(result.as_bool(), Some(false), "bit1 should be 0");

    // bit5 = 1
    let result = decode_registers(&reg, DataFormat::Bool, ByteOrder::Abcd, Some(5)).unwrap();
    assert_eq!(result.as_bool(), Some(true), "bit5 should be 1");

    // bit10 = 1
    let result = decode_registers(&reg, DataFormat::Bool, ByteOrder::Abcd, Some(10)).unwrap();
    assert_eq!(result.as_bool(), Some(true), "bit10 should be 1");

    // bit15 = 1 (MSB)
    let result = decode_registers(&reg, DataFormat::Bool, ByteOrder::Abcd, Some(15)).unwrap();
    assert_eq!(result.as_bool(), Some(true), "bit15 should be 1");

    // 验证为 0 的位
    for bit in [2, 3, 4, 6, 7, 8, 9, 11, 12, 13, 14] {
        let result = decode_registers(&reg, DataFormat::Bool, ByteOrder::Abcd, Some(bit)).unwrap();
        assert_eq!(
            result.as_bool(),
            Some(false),
            "bit{} should be 0 for 0x8421",
            bit
        );
    }
}

/// 测试边界条件：全 1 寄存器
#[test]
fn test_bit_position_all_ones() {
    let reg = [0xFFFFu16]; // 所有位为 1

    for bit in 0..=15 {
        let result = decode_registers(&reg, DataFormat::Bool, ByteOrder::Abcd, Some(bit)).unwrap();
        assert_eq!(
            result.as_bool(),
            Some(true),
            "bit {} should be true for 0xFFFF",
            bit
        );
    }
}

/// 测试边界条件：全 0 寄存器
#[test]
fn test_bit_position_all_zeros() {
    let reg = [0x0000u16]; // 所有位为 0

    for bit in 0..=15 {
        let result = decode_registers(&reg, DataFormat::Bool, ByteOrder::Abcd, Some(bit)).unwrap();
        assert_eq!(
            result.as_bool(),
            Some(false),
            "bit {} should be false for 0x0000",
            bit
        );
    }
}

/// 测试单个位为 1 的情况
#[test]
fn test_bit_position_single_bit_set() {
    for target_bit in 0..=15u8 {
        let reg = [1u16 << target_bit];

        for check_bit in 0..=15u8 {
            let result =
                decode_registers(&reg, DataFormat::Bool, ByteOrder::Abcd, Some(check_bit)).unwrap();
            let expected = target_bit == check_bit;
            assert_eq!(
                result.as_bool(),
                Some(expected),
                "reg=0x{:04X}: bit{} should be {} (only bit{} is set)",
                reg[0],
                check_bit,
                expected,
                target_bit
            );
        }
    }
}

/// 测试非法 bit_position (>15) 应返回错误
#[test]
fn test_bit_position_invalid() {
    let reg = [0x0001u16];

    // bit_position = 16 应该失败
    assert!(
        decode_registers(&reg, DataFormat::Bool, ByteOrder::Abcd, Some(16)).is_err(),
        "bit_position=16 should return error"
    );

    // bit_position = 255 应该失败
    assert!(
        decode_registers(&reg, DataFormat::Bool, ByteOrder::Abcd, Some(255)).is_err(),
        "bit_position=255 should return error"
    );
}

/// 测试 bit_position=None 的默认行为 (应该使用 bit 0)
#[test]
fn test_bit_position_default() {
    let reg_bit0_set = [0x0001u16]; // bit0 = 1
    let reg_bit0_clear = [0xFFFEu16]; // bit0 = 0

    // None 应该等同于 Some(0)
    let result = decode_registers(&reg_bit0_set, DataFormat::Bool, ByteOrder::Abcd, None).unwrap();
    assert_eq!(
        result.as_bool(),
        Some(true),
        "None should default to bit0 (true)"
    );

    let result =
        decode_registers(&reg_bit0_clear, DataFormat::Bool, ByteOrder::Abcd, None).unwrap();
    assert_eq!(
        result.as_bool(),
        Some(false),
        "None should default to bit0 (false)"
    );
}

/// 测试交替位模式 (0xAAAA 和 0x5555)
#[test]
fn test_bit_position_alternating_patterns() {
    // 0xAAAA = 1010_1010_1010_1010 (偶数位为 0，奇数位为 1)
    let reg_aaaa = [0xAAAAu16];
    for bit in 0..=15u8 {
        let result =
            decode_registers(&reg_aaaa, DataFormat::Bool, ByteOrder::Abcd, Some(bit)).unwrap();
        let expected = bit % 2 == 1; // 奇数位为 1
        assert_eq!(
            result.as_bool(),
            Some(expected),
            "0xAAAA bit{} should be {}",
            bit,
            expected
        );
    }

    // 0x5555 = 0101_0101_0101_0101 (偶数位为 1，奇数位为 0)
    let reg_5555 = [0x5555u16];
    for bit in 0..=15u8 {
        let result =
            decode_registers(&reg_5555, DataFormat::Bool, ByteOrder::Abcd, Some(bit)).unwrap();
        let expected = bit % 2 == 0; // 偶数位为 1
        assert_eq!(
            result.as_bool(),
            Some(expected),
            "0x5555 bit{} should be {}",
            bit,
            expected
        );
    }
}

/// 验证实际公式: (register >> bit_position) & 1 == 1
/// 这是 byte_order.rs:37 的核心逻辑
#[test]
fn test_bit_extraction_formula() {
    // 随机测试值
    let test_cases: [(u16, u8, bool); 10] = [
        (0x0001, 0, true),  // bit0
        (0x0002, 1, true),  // bit1
        (0x0004, 2, true),  // bit2
        (0x8000, 15, true), // bit15
        (0x0000, 0, false), // all zeros
        (0xFFFF, 7, true),  // all ones, check middle
        (0x0100, 8, true),  // bit8 set
        (0x0100, 7, false), // bit8 set, check adjacent
        (0x8421, 0, true),  // our test pattern, bit0
        (0x8421, 15, true), // our test pattern, bit15
    ];

    for (reg_val, bit_pos, expected) in test_cases {
        let reg = [reg_val];
        let result =
            decode_registers(&reg, DataFormat::Bool, ByteOrder::Abcd, Some(bit_pos)).unwrap();

        // 验证结果与手动计算一致
        let manual_result = (reg_val >> bit_pos) & 1 == 1;
        assert_eq!(result.as_bool(), Some(expected));
        assert_eq!(
            manual_result, expected,
            "Manual formula verification failed for reg=0x{:04X}, bit={}",
            reg_val, bit_pos
        );
    }
}
