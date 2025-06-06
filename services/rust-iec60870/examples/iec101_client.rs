use rust_iec60870::iec101::{BalancedClient, BalancedClientConfig, LinkLayerParameters};
use rust_iec60870::common::{Asdu, TypeId, CauseOfTransmission, InformationObject};
use tokio::time::Duration;
use std::error::Error;

/// A simple IEC 60870-5-101 balanced mode client example.
/// This connects to a device via serial port and sends an interrogation command.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    env_logger::init();
    
    // Create link layer parameters
    let link_params = LinkLayerParameters::default()
        .with_address_field_size(1)      // Single byte address field
        .with_time_format(2);            // CP56Time2a (time format B)
    
    // Configure the balanced communication client
    let config = BalancedClientConfig::default()
        .with_serial_port("/dev/ttyS0")  // Adjust to your serial port
        .with_baud_rate(9600)            // Common baud rate for IEC 101
        .with_link_address(1)            // Link address
        .with_link_parameters(link_params);
    
    println!("Connecting to IEC 60870-5-101 device on /dev/ttyS0...");
    
    // Connect to the device
    let mut client = BalancedClient::connect(config).await?;
    println!("Connected successfully!");
    
    // Send reset process command
    println!("Sending reset process command...");
    let reset_cmd = create_reset_process_command();
    client.send_asdu(reset_cmd).await?;
    
    // Wait for confirmation
    match client.receive().await? {
        Some(asdu) => {
            println!("Received response: {:?}", asdu);
            if asdu.type_id() == TypeId::C_RP_NA_1 && 
               asdu.cause_of_transmission() == CauseOfTransmission::ActivationConfirm {
                println!("Reset process command confirmed.");
            }
        },
        None => println!("No response received for reset command."),
    }
    
    // Send an interrogation command
    println!("Sending station interrogation command...");
    let command = create_interrogation_command();
    client.send_asdu(command).await?;
    
    // Process received data
    println!("Waiting for data (press Ctrl+C to exit)...");
    let mut count = 0;
    
    while let Some(asdu) = client.receive().await? {
        count += 1;
        println!("Received ASDU #{}: {:?}", count, asdu);
        
        // Check if interrogation is completed
        if asdu.type_id() == TypeId::C_IC_NA_1 && 
           asdu.cause_of_transmission() == CauseOfTransmission::ActivationTermination {
            println!("Station interrogation completed.");
            break;  // You can continue receiving data if needed
        }
        
        // Process data points
        process_asdu(&asdu);
    }
    
    println!("Connection closed.");
    Ok(())
}

/// Creates a reset process command ASDU
fn create_reset_process_command() -> Asdu {
    Asdu::new(
        TypeId::C_RP_NA_1,          // Reset process command
        false,                       // Not a sequence
        CauseOfTransmission::Activation,
        0,                          // Originator address
        1,                          // Common address of ASDU
        vec![
            InformationObject::ResetProcessCommand {
                address: 0,
                qualifier: 1,        // General reset
            }
        ],
    )
}

/// Creates a station interrogation command ASDU
fn create_interrogation_command() -> Asdu {
    Asdu::new(
        TypeId::C_IC_NA_1,          // Interrogation command
        false,                       // Not a sequence
        CauseOfTransmission::Activation,
        0,                          // Originator address
        1,                          // Common address of ASDU
        vec![
            InformationObject::InterrogationCommand {
                address: 0,
                qualifier: 20,       // Station interrogation (20 = default value)
            }
        ],
    )
}

/// Processes a received ASDU based on its type ID
fn process_asdu(asdu: &Asdu) {
    match asdu.type_id() {
        TypeId::M_SP_NA_1 => {
            println!("  - Received single point information");
            
            // Process information objects
            for info_obj in asdu.information_objects() {
                if let InformationObject::SinglePoint { address, value, quality } = info_obj {
                    println!("    - Address: {}, Value: {}, Quality: {}", address, value, quality);
                }
            }
        },
        TypeId::M_DP_NA_1 => {
            println!("  - Received double point information");
            
            // Process information objects
            for info_obj in asdu.information_objects() {
                if let InformationObject::DoublePoint { address, value, quality } = info_obj {
                    let state = match value {
                        0 => "Indeterminate or intermediate",
                        1 => "OFF",
                        2 => "ON",
                        3 => "Indeterminate",
                        _ => "Invalid value"
                    };
                    println!("    - Address: {}, Value: {} ({}), Quality: {}", 
                             address, value, state, quality);
                }
            }
        },
        TypeId::M_ME_NA_1 => {
            println!("  - Received normalized value");
            
            // Process information objects
            for info_obj in asdu.information_objects() {
                if let InformationObject::MeasuredValueNormalized { address, value, quality } = info_obj {
                    println!("    - Address: {}, Value: {}, Quality: {}", address, value, quality);
                }
            }
        },
        TypeId::M_ME_NB_1 => {
            println!("  - Received scaled value");
            
            // Process information objects
            for info_obj in asdu.information_objects() {
                if let InformationObject::MeasuredValueScaled { address, value, quality } = info_obj {
                    println!("    - Address: {}, Value: {}, Quality: {}", address, value, quality);
                }
            }
        },
        TypeId::M_ME_NC_1 => {
            println!("  - Received floating point value");
            
            // Process information objects
            for info_obj in asdu.information_objects() {
                if let InformationObject::MeasuredValueFloat { address, value, quality } = info_obj {
                    println!("    - Address: {}, Value: {}, Quality: {}", address, value, quality);
                }
            }
        },
        _ => {
            println!("  - Received other type: {:?}", asdu.type_id());
        }
    }
} 