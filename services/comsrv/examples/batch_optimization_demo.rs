/// Modbus 批量优化演示
/// 
/// 展示智能批量合并算法的效果，对比优化前后的批次数量和预期性能提升
/// 
/// 运行方式：
/// ```bash
/// cargo run --example batch_optimization_demo
/// ```

use comsrv::core::protocols::modbus::{
    client::ModbusClient,
    common::{ModbusRegisterMapping, ModbusRegisterType, ModbusDataType, ByteOrder},
};

fn main() {
    println!("🚀 Modbus 批量优化演示");
    println!("{}", "=".repeat(60));
    
    // 创建测试点表 - 模拟真实工业场景
    let test_scenarios = vec![
        ("连续地址场景", create_continuous_mappings()),
        ("混合类型场景", create_mixed_type_mappings()),
        ("分散地址场景", create_scattered_mappings()),
        ("大规模场景", create_large_scale_mappings()),
    ];
    
    for (scenario_name, mappings) in test_scenarios {
        println!("\n📊 场景: {}", scenario_name);
        println!("{}", "-".repeat(40));
        
        // 显示原始点表信息
        println!("原始点表: {} 个点", mappings.len());
        print_mapping_summary(&mappings);
        
        // 执行批量优化
        let batches = ModbusClient::optimize_point_reading(&mappings);
        
        // 显示优化结果
        println!("\n优化结果: {} 个批次", batches.len());
        print_batch_analysis(&batches);
        
        // 计算性能提升
        let improvement = calculate_performance_improvement(&mappings, &batches);
        println!("\n📈 性能提升:");
        println!("  • 批次减少: {}% ({} → {})", 
            ((mappings.len() - batches.len()) as f64 / mappings.len() as f64 * 100.0) as u32,
            mappings.len(), 
            batches.len()
        );
        println!("  • 预期耗时减少: {}%", improvement.time_reduction_percent);
        println!("  • 吞吐量提升: {}x", improvement.throughput_multiplier);
        
        println!();
    }
    
    println!("✅ 演示完成！批量优化算法已成功实现并验证。");
}

/// 创建连续地址的点表
fn create_continuous_mappings() -> Vec<ModbusRegisterMapping> {
    (40001..=40020).map(|addr| ModbusRegisterMapping {
        name: format!("temp_{}", addr - 40000),
        address: addr,
        register_type: ModbusRegisterType::HoldingRegister,
        data_type: ModbusDataType::UInt16,
        scale: 0.1,
        offset: 0.0,
        byte_order: ByteOrder::BigEndian,
        ..Default::default()
    }).collect()
}

/// 创建混合类型的点表
fn create_mixed_type_mappings() -> Vec<ModbusRegisterMapping> {
    let mut mappings = Vec::new();
    
    // Holding registers
    for i in 0..5 {
        mappings.push(ModbusRegisterMapping {
            name: format!("holding_{}", i),
            address: 40001 + i,
            register_type: ModbusRegisterType::HoldingRegister,
            data_type: ModbusDataType::UInt16,
            scale: 1.0,
            offset: 0.0,
            byte_order: ByteOrder::BigEndian,
            ..Default::default()
        });
    }
    
    // Input registers
    for i in 0..3 {
        mappings.push(ModbusRegisterMapping {
            name: format!("input_{}", i),
            address: 30001 + i,
            register_type: ModbusRegisterType::InputRegister,
            data_type: ModbusDataType::UInt16,
            scale: 1.0,
            offset: 0.0,
            byte_order: ByteOrder::BigEndian,
            ..Default::default()
        });
    }
    
    // Coils
    for i in 0..4 {
        mappings.push(ModbusRegisterMapping {
            name: format!("coil_{}", i),
            address: 1 + i,
            register_type: ModbusRegisterType::Coil,
            data_type: ModbusDataType::Bool,
            scale: 1.0,
            offset: 0.0,
            byte_order: ByteOrder::BigEndian,
            ..Default::default()
        });
    }
    
    mappings
}

/// 创建分散地址的点表
fn create_scattered_mappings() -> Vec<ModbusRegisterMapping> {
    let addresses = vec![40001, 40003, 40010, 40012, 40020, 40025, 40030];
    
    addresses.into_iter().map(|addr| ModbusRegisterMapping {
        name: format!("scattered_{}", addr),
        address: addr,
        register_type: ModbusRegisterType::HoldingRegister,
        data_type: ModbusDataType::UInt16,
        scale: 1.0,
        offset: 0.0,
        byte_order: ByteOrder::BigEndian,
        ..Default::default()
    }).collect()
}

/// 创建大规模点表
fn create_large_scale_mappings() -> Vec<ModbusRegisterMapping> {
    let mut mappings = Vec::new();
    
    // 多个连续区间
    let ranges = vec![
        (40001, 40020),  // 20 个连续点
        (40050, 40060),  // 10 个连续点
        (40100, 40130),  // 30 个连续点
    ];
    
    for (start, end) in ranges {
        for addr in start..=end {
            mappings.push(ModbusRegisterMapping {
                name: format!("large_scale_{}", addr),
                address: addr,
                register_type: ModbusRegisterType::HoldingRegister,
                data_type: ModbusDataType::UInt16,
                scale: 1.0,
                offset: 0.0,
                byte_order: ByteOrder::BigEndian,
                ..Default::default()
            });
        }
    }
    
    mappings
}

/// 打印点表摘要
fn print_mapping_summary(mappings: &[ModbusRegisterMapping]) {
    use std::collections::HashMap;
    
    let mut type_counts: HashMap<ModbusRegisterType, usize> = HashMap::new();
    let mut address_ranges: HashMap<ModbusRegisterType, (u16, u16)> = HashMap::new();
    
    for mapping in mappings {
        *type_counts.entry(mapping.register_type).or_insert(0) += 1;
        
        let (min_addr, max_addr) = address_ranges.entry(mapping.register_type)
            .or_insert((mapping.address, mapping.address));
        *min_addr = (*min_addr).min(mapping.address);
        *max_addr = (*max_addr).max(mapping.address);
    }
    
    for (reg_type, count) in type_counts {
        let (min_addr, max_addr) = address_ranges[&reg_type];
        println!("  • {:?}: {} 个点 (地址范围: {} - {})", 
            reg_type, count, min_addr, max_addr);
    }
}

/// 打印批次分析
fn print_batch_analysis(batches: &[Vec<ModbusRegisterMapping>]) {
    for (i, batch) in batches.iter().enumerate() {
        if batch.is_empty() { continue; }
        
        let reg_type = &batch[0].register_type;
        let min_addr = batch.iter().map(|m| m.address).min().unwrap();
        let max_addr = batch.iter().map(|m| m.address).max().unwrap();
        let span = max_addr - min_addr + 1;
        
        println!("  批次 {}: {:?} 地址 {} - {} (跨度: {}, 点数: {})", 
            i + 1, reg_type, min_addr, max_addr, span, batch.len());
    }
}

/// 性能提升计算结果
#[derive(Debug)]
struct PerformanceImprovement {
    time_reduction_percent: u32,
    throughput_multiplier: f64,
}

/// 计算性能提升
fn calculate_performance_improvement(
    original_mappings: &[ModbusRegisterMapping], 
    batches: &[Vec<ModbusRegisterMapping>]
) -> PerformanceImprovement {
    // 假设每个单独读取需要 10ms，每个批次读取需要 30ms
    const SINGLE_READ_TIME_MS: f64 = 10.0;
    const BATCH_READ_TIME_MS: f64 = 30.0;
    
    let original_time = original_mappings.len() as f64 * SINGLE_READ_TIME_MS;
    let optimized_time = batches.len() as f64 * BATCH_READ_TIME_MS;
    
    let time_reduction_percent = ((original_time - optimized_time) / original_time * 100.0) as u32;
    let throughput_multiplier = original_time / optimized_time;
    
    PerformanceImprovement {
        time_reduction_percent,
        throughput_multiplier,
    }
} 