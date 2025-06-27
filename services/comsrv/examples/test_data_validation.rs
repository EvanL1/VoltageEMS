use std::path::Path;
use comsrv::core::config::config_manager::{ConfigManager, ProtocolMapping, DataTypeRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Testing Data Format Validation");
    println!("=====================================");
    
    // Test data type rules
    println!("\n📋 Available Data Type Rules:");
    let rules = DataTypeRule::get_validation_rules();
    for rule in &rules {
        println!("  {} -> {} bytes, formats: {:?}, max_bit: {}", 
            rule.data_type, rule.expected_bytes, rule.valid_formats, rule.max_bit_location);
    }
    
    // Test valid protocol mappings
    println!("\n✅ Testing Valid Protocol Mappings:");
    let valid_mappings = vec![
        ProtocolMapping {
            point_id: 1,
            signal_name: "TANK_LEVEL".to_string(),
            address: "40001".to_string(),
            data_type: "float32".to_string(),
            data_format: "ABCD".to_string(),
            number_of_bytes: 4,
            bit_location: Some(1),
            description: Some("Tank level sensor".to_string()),
        },
        ProtocolMapping {
            point_id: 2,
            signal_name: "PUMP_STATUS".to_string(),
            address: "2001".to_string(),
            data_type: "bool".to_string(),
            data_format: "ABCD".to_string(),
            number_of_bytes: 1,
            bit_location: Some(5),
            description: Some("Pump status bit".to_string()),
        },
        ProtocolMapping {
            point_id: 3,
            signal_name: "TEMP_SENSOR".to_string(),
            address: "40010".to_string(),
            data_type: "uint16".to_string(),
            data_format: "CDBA".to_string(),
            number_of_bytes: 2,
            bit_location: Some(16),
            description: Some("Temperature sensor".to_string()),
        },
    ];
    
    for mapping in &valid_mappings {
        match mapping.validate() {
            Ok(()) => println!("  ✅ {} ({}) - Valid", mapping.signal_name, mapping.data_type),
            Err(e) => println!("  ❌ {} ({}) - Error: {}", mapping.signal_name, mapping.data_type, e),
        }
    }
    
    // Test invalid protocol mappings
    println!("\n❌ Testing Invalid Protocol Mappings:");
    let invalid_mappings = vec![
        ProtocolMapping {
            point_id: 1,
            signal_name: "INVALID_FORMAT".to_string(),
            address: "40001".to_string(),
            data_type: "float32".to_string(),
            data_format: "INVALID".to_string(), // Invalid format
            number_of_bytes: 4,
            bit_location: Some(1),
            description: Some("Invalid format test".to_string()),
        },
        ProtocolMapping {
            point_id: 2,
            signal_name: "WRONG_BYTES".to_string(),
            address: "40002".to_string(),
            data_type: "uint16".to_string(),
            data_format: "ABCD".to_string(),
            number_of_bytes: 4, // Should be 2 for uint16
            bit_location: Some(1),
            description: Some("Wrong byte count test".to_string()),
        },
        ProtocolMapping {
            point_id: 3,
            signal_name: "INVALID_BIT_LOC".to_string(),
            address: "40003".to_string(),
            data_type: "bool".to_string(),
            data_format: "ABCD".to_string(),
            number_of_bytes: 1,
            bit_location: Some(20), // Invalid bit location (max 16)
            description: Some("Invalid bit location test".to_string()),
        },
        ProtocolMapping {
            point_id: 4,
            signal_name: "UNSUPPORTED_TYPE".to_string(),
            address: "40004".to_string(),
            data_type: "unknown_type".to_string(), // Unsupported data type
            data_format: "ABCD".to_string(),
            number_of_bytes: 1,
            bit_location: Some(1),
            description: Some("Unsupported type test".to_string()),
        },
    ];
    
    for mapping in &invalid_mappings {
        match mapping.validate() {
            Ok(()) => println!("  ⚠️  {} ({}) - Should have failed!", mapping.signal_name, mapping.data_type),
            Err(e) => println!("  ✅ {} ({}) - Correctly failed: {}", mapping.signal_name, mapping.data_type, e),
        }
    }
    
    // Test bit location defaults
    println!("\n🔢 Testing Bit Location Defaults:");
    let mapping_no_bit = ProtocolMapping {
        point_id: 1,
        signal_name: "NO_BIT_LOC".to_string(),
        address: "40001".to_string(),
        data_type: "uint16".to_string(),
        data_format: "ABCD".to_string(),
        number_of_bytes: 2,
        bit_location: None,
        description: None,
    };
    
    println!("  Default bit location: {}", mapping_no_bit.get_bit_location());
    
    // Test with actual CSV file if it exists
    println!("\n📁 Testing CSV File Validation:");
    let csv_path = Path::new("config/TankFarmModbusTCP/mapping_telemetry.csv");
    if csv_path.exists() {
        match ConfigManager::from_file("examples/bridge_config_separated.yaml") {
            Ok(config_manager) => {
                println!("  ✅ Configuration loaded successfully!");
                
                // Check channel mappings
                if let Some(channel) = config_manager.get_channel(1) {
                    println!("  📊 Channel 1 has {} combined points", channel.combined_points.len());
                    
                    for point in &channel.combined_points {
                        println!("    - {} ({}) @ {} with format {}", 
                            point.mapping.signal_name,
                            point.mapping.data_type,
                            point.mapping.address,
                            point.mapping.data_format);
                    }
                } else {
                    println!("  ⚠️  Channel 1 not found");
                }
            },
            Err(e) => println!("  ❌ Failed to load configuration: {}", e),
        }
    } else {
        println!("  ⚠️  CSV file not found: {}", csv_path.display());
    }
    
    println!("\n🎉 Data validation testing completed!");
    Ok(())
} 