use redis::{Client, Commands};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DataMapping {
    source_key: String,
    source_field: String,
    target_field: String,
    transform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelDefinition {
    id: String,
    name: String,
    description: String,
    input_mappings: Vec<DataMapping>,
    output_key: String,
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum ControlActionType {
    RemoteControl,
    RemoteAdjust,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ControlActionCondition {
    field: String,
    operator: String,
    value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ControlAction {
    id: String,
    name: String,
    action_type: ControlActionType,
    channel: String,
    point: String,
    value: String,
    conditions: Vec<ControlActionCondition>,
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelWithActions {
    model: ModelDefinition,
    actions: Vec<ControlAction>,
}

fn main() -> Result<(), Box<dyn Error>> {
    // Connect to Redis
    let client = Client::open("redis://localhost:6379")?;
    let mut conn = client.get_connection()?;

    // Create a battery model
    let battery_model = ModelDefinition {
        id: "battery_model".to_string(),
        name: "Battery State Model".to_string(),
        description: "Real-time battery state estimation model".to_string(),
        input_mappings: vec![
            DataMapping {
                source_key: "ems:data:bams".to_string(),
                source_field: "voltage".to_string(),
                target_field: "battery_voltage".to_string(),
                transform: Some("scale:0.001".to_string()),
            },
            DataMapping {
                source_key: "ems:data:bams".to_string(),
                source_field: "current".to_string(),
                target_field: "battery_current".to_string(),
                transform: None,
            },
            DataMapping {
                source_key: "ems:data:bams".to_string(),
                source_field: "temperature".to_string(),
                target_field: "battery_temperature".to_string(),
                transform: None,
            },
        ],
        output_key: "ems:model:output:battery".to_string(),
        enabled: true,
    };

    // Create a power flow model with control actions
    let power_flow_model = ModelDefinition {
        id: "power_flow_model".to_string(),
        name: "Power Flow Model".to_string(),
        description: "Real-time power flow model for the system".to_string(),
        input_mappings: vec![
            DataMapping {
                source_key: "ems:data:pcs".to_string(),
                source_field: "active_power".to_string(),
                target_field: "pcs_power".to_string(),
                transform: None,
            },
            DataMapping {
                source_key: "ems:data:diesel_meter".to_string(),
                source_field: "active_power".to_string(),
                target_field: "diesel_power".to_string(),
                transform: None,
            },
            DataMapping {
                source_key: "ems:data:bams".to_string(),
                source_field: "soc".to_string(),
                target_field: "battery_soc".to_string(),
                transform: None,
            },
        ],
        output_key: "ems:model:output:power_flow".to_string(),
        enabled: true,
    };

    // Create control actions for power flow model
    let power_flow_actions = vec![
        // 当SOC低于20%时，启动柴油发电机
        ControlAction {
            id: "start_diesel_generator".to_string(),
            name: "Start Diesel Generator".to_string(),
            action_type: ControlActionType::RemoteControl,
            channel: "Diesel_Serial".to_string(),
            point: "start_command".to_string(),
            value: "1".to_string(), // 1 = 启动
            conditions: vec![
                ControlActionCondition {
                    field: "battery_soc".to_string(),
                    operator: "<".to_string(),
                    value: "20".to_string(),
                },
            ],
            enabled: true,
        },
        // 当SOC高于90%时，停止柴油发电机
        ControlAction {
            id: "stop_diesel_generator".to_string(),
            name: "Stop Diesel Generator".to_string(),
            action_type: ControlActionType::RemoteControl,
            channel: "Diesel_Serial".to_string(),
            point: "start_command".to_string(),
            value: "0".to_string(), // 0 = 停止
            conditions: vec![
                ControlActionCondition {
                    field: "battery_soc".to_string(),
                    operator: ">".to_string(),
                    value: "90".to_string(),
                },
            ],
            enabled: true,
        },
        // 当PCS功率超过限制时，调整功率限制
        ControlAction {
            id: "adjust_pcs_power_limit".to_string(),
            name: "Adjust PCS Power Limit".to_string(),
            action_type: ControlActionType::RemoteAdjust,
            channel: "PCS".to_string(),
            point: "power_limit".to_string(),
            value: "5000".to_string(), // 限制为5000W
            conditions: vec![
                ControlActionCondition {
                    field: "pcs_power".to_string(),
                    operator: ">".to_string(),
                    value: "6000".to_string(),
                },
            ],
            enabled: true,
        },
    ];

    // Combine model and actions
    let power_flow_model_with_actions = ModelWithActions {
        model: power_flow_model,
        actions: power_flow_actions,
    };

    // Convert models to JSON and store in Redis
    let battery_model_json = serde_json::to_string(&battery_model)?;
    let power_flow_model_json = serde_json::to_string(&power_flow_model_with_actions)?;

    conn.set(
        "ems:model:config:battery_model",
        battery_model_json,
    )?;
    
    conn.set(
        "ems:model:config:power_flow_model",
        power_flow_model_json,
    )?;

    println!("Model configurations created successfully in Redis");

    // Create some sample data in Redis for testing
    let mut bams_data = HashMap::new();
    bams_data.insert("voltage", "48000");
    bams_data.insert("current", "100");
    bams_data.insert("temperature", "25");
    bams_data.insert("soc", "85");

    let mut pcs_data = HashMap::new();
    pcs_data.insert("active_power", "5000");
    pcs_data.insert("reactive_power", "1000");
    pcs_data.insert("frequency", "50.0");

    let mut diesel_meter_data = HashMap::new();
    diesel_meter_data.insert("active_power", "10000");
    diesel_meter_data.insert("reactive_power", "2000");
    diesel_meter_data.insert("frequency", "50.0");

    // Store sample data in Redis
    conn.hset_multiple("ems:data:bams", &bams_data)?;
    conn.hset_multiple("ems:data:pcs", &pcs_data)?;
    conn.hset_multiple("ems:data:diesel_meter", &diesel_meter_data)?;

    println!("Sample data created successfully in Redis");

    Ok(())
} 