use std::fs;
use tempfile::TempDir;
use comsrv::core::config::config_manager::ConfigManager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 测试分离表配置系统");

    // 创建临时目录和文件
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("test_config.yaml");
    
    // 创建目录结构
    let table_dir = temp_dir.path().join("config/TankFarmModbusTCP");
    fs::create_dir_all(&table_dir)?;
    
    println!("📁 创建测试目录: {}", table_dir.display());
    
    // 创建四遥CSV文件
    create_telemetry_files(&table_dir)?;
    create_mapping_files(&table_dir)?;
    
    // 创建主配置文件
    let yaml_content = r#"
service:
  name: "test-separated-tables"
  description: "测试分离表配置系统"
  api:
    enabled: true
    bind_address: "127.0.0.1:8080"
  redis:
    url: "redis://127.0.0.1:6379/1"
    database: 1
  logging:
    level: "info"

channels:
  - id: 1001
    name: "TankFarmModbusTCP"
    description: "油罐区Modbus TCP通信"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      slave_id: 1
      timeout_ms: 1000
    
    table_config:
      four_telemetry_route: "config/TankFarmModbusTCP"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"
        signal_file: "signal.csv"
        adjustment_file: "adjustment.csv"
        control_file: "control.csv"
      
      protocol_mapping_route: "config/TankFarmModbusTCP"
      protocol_mapping_files:
        telemetry_mapping: "mapping_telemetry.csv"
        signal_mapping: "mapping_signal.csv"
        adjustment_mapping: "mapping_adjustment.csv"
        control_mapping: "mapping_control.csv"
"#;
    
    fs::write(&config_path, yaml_content)?;
    
    println!("📝 配置文件已创建: {}", config_path.display());
    
    // 测试加载配置
    println!("🔄 加载配置...");
    match ConfigManager::from_file(&config_path) {
        Ok(manager) => {
            println!("✅ 配置加载成功!");
            
            // 验证基本配置
            println!("📊 服务名称: {}", manager.service().name);
            println!("📊 通道数量: {}", manager.channels().len());
            
            if let Some(channel) = manager.channels().first() {
                println!("📊 通道ID: {}", channel.id);
                println!("📊 通道名称: {}", channel.name);
                println!("📊 协议类型: {}", channel.protocol);
                
                if channel.table_config.is_some() {
                    println!("✅ 分离表配置已加载");
                    
                    // 测试组合点访问
                    let combined_points = manager.get_combined_points(1001);
                    println!("📊 组合点数量: {}", combined_points.len());
                    
                    // 显示前几个点的信息
                    for (i, point) in combined_points.iter().take(3).enumerate() {
                        println!("  点 {}: {} - {}", 
                            i + 1, 
                            point.telemetry.signal_name, 
                            point.telemetry.chinese_name
                        );
                        println!("    地址: {}, 数据类型: {}", 
                            point.mapping.address, 
                            point.mapping.data_type
                        );
                        if let Some(scale) = point.telemetry.scale {
                            println!("    系数: {}", scale);
                        }
                        if let Some(reverse) = point.telemetry.reverse {
                            println!("    取反: {}", reverse);
                        }
                    }
                    
                    // 测试按类型获取点
                    let yc_points = manager.get_four_telemetry_points(1001, "YC");
                    let yx_points = manager.get_four_telemetry_points(1001, "YX");
                    let yt_points = manager.get_four_telemetry_points(1001, "YT");
                    let yk_points = manager.get_four_telemetry_points(1001, "YK");
                    
                    println!("📊 YC(遥测)点数量: {}", yc_points.len());
                    println!("📊 YX(遥信)点数量: {}", yx_points.len());
                    println!("📊 YT(遥调)点数量: {}", yt_points.len());
                    println!("📊 YK(遥控)点数量: {}", yk_points.len());
                    
                } else {
                    println!("❌ 分离表配置未找到");
                }
            }
            
            println!("🎉 测试完成!");
        }
        Err(e) => {
            println!("❌ 配置加载失败: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}

fn create_telemetry_files(table_dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    // 遥测 (YC) - 带scale/offset/unit
    let telemetry_csv = r#"point_id,signal_name,chinese_name,scale,offset,unit
1,TANK_01_LEVEL,1号罐液位,0.1,0,m
2,TANK_01_TEMP,1号罐温度,0.1,-40,°C
3,TANK_02_LEVEL,2号罐液位,0.1,0,m
4,TANK_02_TEMP,2号罐温度,0.1,-40,°C"#;
    fs::write(table_dir.join("telemetry.csv"), telemetry_csv)?;
    
    // 遥信 (YX) - 带reverse
    let signal_csv = r#"point_id,signal_name,chinese_name,reverse
1,PUMP_01_STATUS,1号泵状态,0
2,PUMP_02_STATUS,2号泵状态,0
3,EMERGENCY_STOP,紧急停机,1
4,FIRE_ALARM,火灾报警,0"#;
    fs::write(table_dir.join("signal.csv"), signal_csv)?;
    
    // 遥调 (YT) - 带scale/offset/unit
    let adjustment_csv = r#"point_id,signal_name,chinese_name,scale,offset,unit
1,PUMP_01_SPEED,1号泵转速,1,0,rpm
2,PUMP_02_SPEED,2号泵转速,1,0,rpm"#;
    fs::write(table_dir.join("adjustment.csv"), adjustment_csv)?;
    
    // 遥控 (YK) - 带reverse
    let control_csv = r#"point_id,signal_name,chinese_name,reverse
1,PUMP_01_START,1号泵启动,0
2,PUMP_01_STOP,1号泵停止,0
3,PUMP_02_START,2号泵启动,0
4,PUMP_02_STOP,2号泵停止,0"#;
    fs::write(table_dir.join("control.csv"), control_csv)?;
    
    println!("✅ 四遥CSV文件已创建");
    Ok(())
}

fn create_mapping_files(table_dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    // 遥测映射
    let telemetry_mapping_csv = r#"point_id,signal_name,address,data_type,data_format,number_of_bytes,bit_location,description
1,TANK_01_LEVEL,40001,uint16,ABCD,2,1,1号罐液位传感器
2,TANK_01_TEMP,40002,int16,ABCD,2,1,1号罐温度传感器
3,TANK_02_LEVEL,40003,uint16,ABCD,2,1,2号罐液位传感器
4,TANK_02_TEMP,40004,int16,ABCD,2,1,2号罐温度传感器"#;
    fs::write(table_dir.join("mapping_telemetry.csv"), telemetry_mapping_csv)?;
    
    // 遥信映射
    let signal_mapping_csv = r#"point_id,signal_name,address,data_type,data_format,number_of_bytes,bit_location,description
1,PUMP_01_STATUS,2001,bool,ABCD,1,1,1号泵运行状态
2,PUMP_02_STATUS,2002,bool,ABCD,1,2,2号泵运行状态
3,EMERGENCY_STOP,2003,bool,ABCD,1,3,紧急停机按钮状态
4,FIRE_ALARM,2004,bool,ABCD,1,4,火灾探测器报警"#;
    fs::write(table_dir.join("mapping_signal.csv"), signal_mapping_csv)?;
    
    // 遥调映射
    let adjustment_mapping_csv = r#"point_id,signal_name,address,data_type,data_format,number_of_bytes,bit_location,description
1,PUMP_01_SPEED,40101,uint16,ABCD,2,1,1号泵转速设定
2,PUMP_02_SPEED,40102,uint16,ABCD,2,1,2号泵转速设定"#;
    fs::write(table_dir.join("mapping_adjustment.csv"), adjustment_mapping_csv)?;
    
    // 遥控映射
    let control_mapping_csv = r#"point_id,signal_name,address,data_type,data_format,number_of_bytes,bit_location,description
1,PUMP_01_START,1,bool,ABCD,1,1,1号泵启动命令
2,PUMP_01_STOP,2,bool,ABCD,1,2,1号泵停止命令
3,PUMP_02_START,3,bool,ABCD,1,3,2号泵启动命令
4,PUMP_02_STOP,4,bool,ABCD,1,4,2号泵停止命令"#;
    fs::write(table_dir.join("mapping_control.csv"), control_mapping_csv)?;
    
    println!("✅ 协议映射CSV文件已创建");
    Ok(())
} 