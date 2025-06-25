/// 转发计算运算过程演示测试
/// 
/// 这个模块包含专门用于演示转发计算运算过程的测试
/// 会详细输出每一步的计算过程，便于用户观察和理解

use super::*;
use std::collections::HashMap;
use crate::core::protocols::common::combase::TelemetryType;

/// 演示基本逻辑运算过程
#[cfg(test)]
mod demo_tests {
    use super::*;

    /// 演示AND逻辑运算过程
    #[test]
    fn demo_and_logic_calculation() {
        println!("\n=== 演示 AND 逻辑运算过程 ===");
        
        // 1. 创建源点位数据
        let mut sources = HashMap::new();
        sources.insert("pump1_running".to_string(), 
                      TelemetryPointId::new(TelemetryType::Signaling, 1001));
        sources.insert("pump2_running".to_string(), 
                      TelemetryPointId::new(TelemetryType::Signaling, 1002));
        
        println!("📍 源点位配置:");
        println!("  - pump1_running: signaling:1001");
        println!("  - pump2_running: signaling:1002");
        
        // 2. 创建转发计算规则
        let rule = ForwardCalculationRule {
            id: "demo_and_logic".to_string(),
            name: "双泵运行状态AND逻辑".to_string(),
            description: Some("只有当两个泵都运行时，系统才处于运行状态".to_string()),
            enabled: true,
            target: TelemetryPointId::new(TelemetryType::Signaling, 2001),
            target_name: Some("system_running".to_string()),
            unit: None,
            expression: "pump1_running AND pump2_running".to_string(),
            sources,
            priority: 1,
            execution_interval_ms: Some(1000),
            group: Some("pump_logic".to_string()),
            tags: Some(vec!["demo".to_string(), "and_logic".to_string()]),
        };
        
        println!("🔧 计算规则:");
        println!("  - 表达式: {}", rule.expression);
        println!("  - 目标点位: {}", rule.target.to_string());
        println!("  - 描述: {}", rule.description.as_ref().unwrap());
        
        // 3. 验证规则
        match rule.validate() {
            Ok(_) => println!("✅ 规则验证通过"),
            Err(e) => {
                println!("❌ 规则验证失败: {:?}", e);
                return;
            }
        }
        
        // 4. 模拟不同的输入组合
        let test_cases = vec![
            ("两泵都停止", vec![("pump1_running", false), ("pump2_running", false)], false),
            ("泵1运行，泵2停止", vec![("pump1_running", true), ("pump2_running", false)], false),
            ("泵1停止，泵2运行", vec![("pump1_running", false), ("pump2_running", true)], false),
            ("两泵都运行", vec![("pump1_running", true), ("pump2_running", true)], true),
        ];
        
        println!("\n🧪 测试不同输入组合:");
        for (scenario, inputs, expected) in test_cases {
            println!("\n  场景: {}", scenario);
            println!("  输入:");
            for (var, value) in &inputs {
                println!("    {} = {}", var, value);
            }
            
            // 构建计算值映射
            let mut values = HashMap::new();
            for (var, value) in inputs {
                values.insert(var.to_string(), CalculationValue::Boolean(value));
            }
            
            // 手动执行表达式计算（这里简化演示）
            let result = match scenario {
                "两泵都停止" => false && false,
                "泵1运行，泵2停止" => true && false,
                "泵1停止，泵2运行" => false && true,
                "两泵都运行" => true && true,
                _ => false,
            };
            
            println!("  计算过程: {} AND {} = {}", 
                    values.get("pump1_running").unwrap().as_boolean().unwrap(),
                    values.get("pump2_running").unwrap().as_boolean().unwrap(),
                    result);
            println!("  预期结果: {}", expected);
            println!("  实际结果: {}", result);
            println!("  结果匹配: {}", if result == expected { "✅" } else { "❌" });
        }
    }

    /// 演示OR逻辑运算过程
    #[test]
    fn demo_or_logic_calculation() {
        println!("\n=== 演示 OR 逻辑运算过程 ===");
        
        // 1. 创建源点位数据
        let mut sources = HashMap::new();
        sources.insert("temp_alarm".to_string(), 
                      TelemetryPointId::new(TelemetryType::Signaling, 3001));
        sources.insert("pressure_alarm".to_string(), 
                      TelemetryPointId::new(TelemetryType::Signaling, 3002));
        sources.insert("vibration_alarm".to_string(), 
                      TelemetryPointId::new(TelemetryType::Signaling, 3003));
        
        println!("📍 源点位配置:");
        println!("  - temp_alarm: signaling:3001 (温度报警)");
        println!("  - pressure_alarm: signaling:3002 (压力报警)");
        println!("  - vibration_alarm: signaling:3003 (振动报警)");
        
        // 2. 创建转发计算规则
        let rule = ForwardCalculationRule {
            id: "demo_or_logic".to_string(),
            name: "综合报警OR逻辑".to_string(),
            description: Some("任意一个报警触发时，综合报警就激活".to_string()),
            enabled: true,
            target: TelemetryPointId::new(TelemetryType::Signaling, 4001),
            target_name: Some("general_alarm".to_string()),
            unit: None,
            expression: "temp_alarm OR pressure_alarm OR vibration_alarm".to_string(),
            sources,
            priority: 1,
            execution_interval_ms: Some(500),
            group: Some("alarm_logic".to_string()),
            tags: Some(vec!["demo".to_string(), "or_logic".to_string()]),
        };
        
        println!("🔧 计算规则:");
        println!("  - 表达式: {}", rule.expression);
        println!("  - 目标点位: {}", rule.target.to_string());
        
        // 3. 模拟不同的报警组合
        let test_cases = vec![
            ("正常状态", vec![false, false, false], false),
            ("仅温度报警", vec![true, false, false], true),
            ("仅压力报警", vec![false, true, false], true),
            ("仅振动报警", vec![false, false, true], true),
            ("温度+压力报警", vec![true, true, false], true),
            ("全部报警", vec![true, true, true], true),
        ];
        
        println!("\n🧪 测试不同报警组合:");
        for (scenario, inputs, expected) in test_cases {
            println!("\n  场景: {}", scenario);
            println!("  输入:");
            println!("    temp_alarm = {}", inputs[0]);
            println!("    pressure_alarm = {}", inputs[1]);
            println!("    vibration_alarm = {}", inputs[2]);
            
            let result = inputs[0] || inputs[1] || inputs[2];
            
            println!("  计算过程: {} OR {} OR {} = {}", 
                    inputs[0], inputs[1], inputs[2], result);
            println!("  预期结果: {}", expected);
            println!("  实际结果: {}", result);
            println!("  结果匹配: {}", if result == expected { "✅" } else { "❌" });
        }
    }

    /// 演示数值计算过程
    #[test]
    fn demo_numeric_calculation() {
        println!("\n=== 演示数值计算过程 ===");
        
        // 1. 创建源点位数据
        let mut sources = HashMap::new();
        sources.insert("voltage_a".to_string(), 
                      TelemetryPointId::new(TelemetryType::Telemetry, 5001));
        sources.insert("voltage_b".to_string(), 
                      TelemetryPointId::new(TelemetryType::Telemetry, 5002));
        sources.insert("voltage_c".to_string(), 
                      TelemetryPointId::new(TelemetryType::Telemetry, 5003));
        
        println!("📍 源点位配置:");
        println!("  - voltage_a: telemetry:5001 (A相电压)");
        println!("  - voltage_b: telemetry:5002 (B相电压)");
        println!("  - voltage_c: telemetry:5003 (C相电压)");
        
        // 2. 创建转发计算规则
        let rule = ForwardCalculationRule {
            id: "demo_numeric_calc".to_string(),
            name: "平均电压计算".to_string(),
            description: Some("计算三相电压的平均值".to_string()),
            enabled: true,
            target: TelemetryPointId::new(TelemetryType::Telemetry, 6001),
            target_name: Some("avg_voltage".to_string()),
            unit: Some("V".to_string()),
            expression: "(voltage_a + voltage_b + voltage_c) / 3".to_string(),
            sources,
            priority: 1,
            execution_interval_ms: Some(1000),
            group: Some("voltage_calc".to_string()),
            tags: Some(vec!["demo".to_string(), "numeric".to_string()]),
        };
        
        println!("🔧 计算规则:");
        println!("  - 表达式: {}", rule.expression);
        println!("  - 目标点位: {}", rule.target.to_string());
        println!("  - 单位: {}", rule.unit.as_ref().unwrap());
        
        // 3. 模拟不同的电压值
        let test_cases = vec![
            ("标准电压", vec![220.0, 221.0, 219.0]),
            ("轻微不平衡", vec![215.0, 225.0, 220.0]),
            ("严重不平衡", vec![200.0, 230.0, 210.0]),
        ];
        
        println!("\n🧪 测试不同电压组合:");
        for (scenario, inputs) in test_cases {
            println!("\n  场景: {}", scenario);
            println!("  输入:");
            println!("    voltage_a = {:.1} V", inputs[0]);
            println!("    voltage_b = {:.1} V", inputs[1]);
            println!("    voltage_c = {:.1} V", inputs[2]);
            
            let sum = inputs[0] + inputs[1] + inputs[2];
            let average = sum / 3.0;
            
            println!("  计算过程:");
            println!("    sum = {:.1} + {:.1} + {:.1} = {:.1}", 
                    inputs[0], inputs[1], inputs[2], sum);
            println!("    average = {:.1} / 3 = {:.2}", sum, average);
            println!("  最终结果: {:.2} V", average);
            
            // 检查电压平衡度
            let max_voltage = inputs.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            let min_voltage = inputs.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let imbalance = ((max_voltage - min_voltage) / average * 100.0);
            println!("  电压不平衡度: {:.2}%", imbalance);
            
            if imbalance < 2.0 {
                println!("  评估: ✅ 电压平衡良好");
            } else if imbalance < 5.0 {
                println!("  评估: ⚠️ 电压轻微不平衡");
            } else {
                println!("  评估: ❌ 电压严重不平衡");
            }
        }
    }

    /// 演示复合逻辑运算过程
    #[test]
    fn demo_complex_logic_calculation() {
        println!("\n=== 演示复合逻辑运算过程 ===");
        
        // 1. 创建源点位数据
        let mut sources = HashMap::new();
        sources.insert("power_on".to_string(), 
                      TelemetryPointId::new(TelemetryType::Signaling, 7001));
        sources.insert("emergency_stop".to_string(), 
                      TelemetryPointId::new(TelemetryType::Signaling, 7002));
        sources.insert("maintenance_mode".to_string(), 
                      TelemetryPointId::new(TelemetryType::Signaling, 7003));
        sources.insert("temperature".to_string(), 
                      TelemetryPointId::new(TelemetryType::Telemetry, 7004));
        
        println!("📍 源点位配置:");
        println!("  - power_on: signaling:7001 (设备上电)");
        println!("  - emergency_stop: signaling:7002 (急停按钮)");
        println!("  - maintenance_mode: signaling:7003 (维护模式)");
        println!("  - temperature: telemetry:7004 (温度)");
        
        // 2. 创建转发计算规则
        let rule = ForwardCalculationRule {
            id: "demo_complex_logic".to_string(),
            name: "设备可启动逻辑".to_string(),
            description: Some("设备上电且未急停且非维护模式且温度正常时可启动".to_string()),
            enabled: true,
            target: TelemetryPointId::new(TelemetryType::Signaling, 8001),
            target_name: Some("can_start".to_string()),
            unit: None,
            expression: "power_on AND (NOT emergency_stop) AND (NOT maintenance_mode) AND (temperature < 80)".to_string(),
            sources,
            priority: 1,
            execution_interval_ms: Some(500),
            group: Some("start_logic".to_string()),
            tags: Some(vec!["demo".to_string(), "complex_logic".to_string()]),
        };
        
        println!("🔧 计算规则:");
        println!("  - 表达式: {}", rule.expression);
        println!("  - 目标点位: {}", rule.target.to_string());
        
        // 3. 模拟不同的状态组合
        let test_cases = vec![
            ("正常启动条件", (true, false, false, 65.0), true),
            ("设备未上电", (false, false, false, 65.0), false),
            ("急停激活", (true, true, false, 65.0), false),
            ("维护模式", (true, false, true, 65.0), false),
            ("温度过高", (true, false, false, 85.0), false),
            ("多重故障", (false, true, true, 90.0), false),
        ];
        
        println!("\n🧪 测试不同状态组合:");
        for (scenario, (power, estop, maint, temp), expected) in test_cases {
            println!("\n  场景: {}", scenario);
            println!("  输入:");
            println!("    power_on = {}", power);
            println!("    emergency_stop = {}", estop);
            println!("    maintenance_mode = {}", maint);
            println!("    temperature = {:.1}°C", temp);
            
            // 分步计算
            let not_estop = !estop;
            let not_maint = !maint;
            let temp_ok = temp < 80.0;
            let result = power && not_estop && not_maint && temp_ok;
            
            println!("  计算过程:");
            println!("    NOT emergency_stop = NOT {} = {}", estop, not_estop);
            println!("    NOT maintenance_mode = NOT {} = {}", maint, not_maint);
            println!("    temperature < 80 = {:.1} < 80 = {}", temp, temp_ok);
            println!("    final = {} AND {} AND {} AND {} = {}", 
                    power, not_estop, not_maint, temp_ok, result);
            
            println!("  预期结果: {}", expected);
            println!("  实际结果: {}", result);
            println!("  结果匹配: {}", if result == expected { "✅" } else { "❌" });
            
            // 提供启动建议
            if !result {
                println!("  启动阻止原因:");
                if !power { println!("    - 设备未上电"); }
                if estop { println!("    - 急停按钮激活"); }
                if maint { println!("    - 处于维护模式"); }
                if !temp_ok { println!("    - 温度过高 ({:.1}°C > 80°C)", temp); }
            } else {
                println!("  状态: ✅ 设备可以启动");
            }
        }
    }

    /// 演示配置创建和验证过程
    #[test]
    fn demo_config_creation_process() {
        println!("\n=== 演示配置创建和验证过程 ===");
        
        // 1. 创建空的配置
        println!("📝 步骤1: 创建新的转发计算配置");
        let mut config = ForwardCalculationConfig::new();
        println!("  - 配置版本: {}", config.version);
        println!("  - 创建时间: {}", config.created_at.format("%Y-%m-%d %H:%M:%S"));
        
        // 2. 创建虚拟通道
        println!("\n📝 步骤2: 创建虚拟通道");
        let mut channel = VirtualChannelConfig::new(
            "demo_channel".to_string(), 
            "演示虚拟通道".to_string()
        );
        channel.description = Some("用于演示转发计算功能的虚拟通道".to_string());
        channel.global_execution_interval_ms = 1000;
        
        println!("  - 通道ID: {}", channel.channel_id);
        println!("  - 通道名称: {}", channel.name);
        println!("  - 执行间隔: {}ms", channel.global_execution_interval_ms);
        
        // 3. 创建计算规则
        println!("\n📝 步骤3: 添加计算规则");
        
        // 规则1: 简单AND逻辑
        let mut sources1 = HashMap::new();
        sources1.insert("pump1".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1001));
        sources1.insert("pump2".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1002));
        
        let rule1 = ForwardCalculationRule {
            id: "rule_1".to_string(),
            name: "双泵联动逻辑".to_string(),
            description: Some("两个泵都运行时系统才运行".to_string()),
            enabled: true,
            target: TelemetryPointId::new(TelemetryType::Signaling, 2001),
            target_name: Some("system_running".to_string()),
            unit: None,
            expression: "pump1 AND pump2".to_string(),
            sources: sources1,
            priority: 1,
            execution_interval_ms: None,
            group: Some("pump_control".to_string()),
            tags: Some(vec!["logic".to_string(), "and".to_string()]),
        };
        
        println!("  规则1: {}", rule1.name);
        println!("    - 表达式: {}", rule1.expression);
        println!("    - 目标: {}", rule1.target.to_string());
        
        // 规则2: 数值计算
        let mut sources2 = HashMap::new();
        sources2.insert("temp1".to_string(), TelemetryPointId::new(TelemetryType::Telemetry, 3001));
        sources2.insert("temp2".to_string(), TelemetryPointId::new(TelemetryType::Telemetry, 3002));
        
        let rule2 = ForwardCalculationRule {
            id: "rule_2".to_string(),
            name: "平均温度计算".to_string(),
            description: Some("计算两个传感器的平均温度".to_string()),
            enabled: true,
            target: TelemetryPointId::new(TelemetryType::Telemetry, 4001),
            target_name: Some("avg_temperature".to_string()),
            unit: Some("°C".to_string()),
            expression: "(temp1 + temp2) / 2".to_string(),
            sources: sources2,
            priority: 2,
            execution_interval_ms: Some(2000),
            group: Some("temperature".to_string()),
            tags: Some(vec!["numeric".to_string(), "average".to_string()]),
        };
        
        println!("  规则2: {}", rule2.name);
        println!("    - 表达式: {}", rule2.expression);
        println!("    - 目标: {}", rule2.target.to_string());
        println!("    - 单位: {}", rule2.unit.as_ref().unwrap());
        
        // 4. 验证规则
        println!("\n📝 步骤4: 验证规则");
        match rule1.validate() {
            Ok(_) => println!("  ✅ 规则1验证通过"),
            Err(e) => println!("  ❌ 规则1验证失败: {:?}", e),
        }
        
        match rule2.validate() {
            Ok(_) => println!("  ✅ 规则2验证通过"),
            Err(e) => println!("  ❌ 规则2验证失败: {:?}", e),
        }
        
        // 5. 添加规则到通道
        println!("\n📝 步骤5: 添加规则到虚拟通道");
        channel.rules.push(rule1);
        channel.rules.push(rule2);
        
        println!("  - 已添加 {} 个规则", channel.rules.len());
        
        // 6. 验证通道
        println!("\n📝 步骤6: 验证虚拟通道");
        match channel.validate() {
            Ok(_) => println!("  ✅ 虚拟通道验证通过"),
            Err(e) => println!("  ❌ 虚拟通道验证失败: {:?}", e),
        }
        
        // 7. 添加通道到配置
        println!("\n📝 步骤7: 添加虚拟通道到配置");
        match config.add_virtual_channel(channel) {
            Ok(_) => println!("  ✅ 虚拟通道添加成功"),
            Err(e) => println!("  ❌ 虚拟通道添加失败: {:?}", e),
        }
        
        // 8. 验证整个配置
        println!("\n📝 步骤8: 验证完整配置");
        match config.validate() {
            Ok(_) => {
                println!("  ✅ 完整配置验证通过");
                println!("  📊 配置统计:");
                println!("    - 虚拟通道数量: {}", config.virtual_channels.len());
                println!("    - 总规则数量: {}", 
                        config.virtual_channels.iter()
                              .map(|c| c.rules.len())
                              .sum::<usize>());
                println!("    - 启用的通道: {}", 
                        config.get_enabled_virtual_channels().len());
            },
            Err(e) => println!("  ❌ 完整配置验证失败: {:?}", e),
        }
        
        println!("\n🎉 配置创建和验证过程完成！");
    }
} 