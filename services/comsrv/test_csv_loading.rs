use std::path::PathBuf;

// 引入必要的模块
use comsrv::core::config::config_manager::{CombaseConfigManager, TelemetryType};

fn main() {
    env_logger::init();
    
    println!("Testing CSV loading...");
    
    let base_dir = PathBuf::from("config/channels/channel_1");
    println!("Base directory: {:?}", base_dir);
    
    if !base_dir.exists() {
        println!("ERROR: Base directory does not exist!");
        return;
    }
    
    let mut manager = CombaseConfigManager::new(base_dir);
    
    match manager.load_all_configs() {
        Ok(()) => {
            println!("Successfully loaded all configs!");
            
            let stats = manager.get_statistics();
            println!("Statistics: {:?}", stats);
            
            // 检查每种类型的点
            for telemetry_type in [TelemetryType::Telemetry, TelemetryType::Signaling, TelemetryType::Control, TelemetryType::Setpoint] {
                if let Some(points) = manager.get_points_by_type(telemetry_type) {
                    println!("{:?}: {} points", telemetry_type, points.len());
                    for (id, config) in points {
                        println!("  - {} ({}): {}", id, config.name(), config.chinese_name());
                    }
                } else {
                    println!("{:?}: No points loaded", telemetry_type);
                }
            }
        }
        Err(e) => {
            println!("ERROR loading configs: {}", e);
        }
    }
} 