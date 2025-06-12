use rust_iec60870::iec104::{Client, ClientConfig};
use rust_iec60870::common::{Asdu, TypeId, CauseOfTransmission, InformationObject};
use tokio::time::Duration;
use std::error::Error;

/// A simple IEC 60870-5-104 client example that connects to a server,
/// sends a command, and handles responses.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    env_logger::init();
    
    // Configure the client
    let config = ClientConfig::default()
        .with_address("192.168.1.100:2404") // Replace with your server address
        .with_t0(Duration::from_secs(30))   // Connection timeout
        .with_t1(Duration::from_secs(15))   // APDU timeout
        .with_t2(Duration::from_secs(10))   // Acknowledgment timeout
        .with_t3(Duration::from_secs(20));  // Test frame timeout
    
    println!("Connecting to IEC 60870-5-104 server...");
    
    // Connect to the server
    let mut client = Client::connect(config).await?;
    println!("Connected successfully!");
    
    // Start STARTDT (data transfer) procedure
    println!("Starting data transfer...");
    client.start_dt().await?;
    println!("Data transfer started.");
    
    // Send a command (single command)
    println!("Sending single command...");
    let command = create_single_command(1001, true);
    client.send_asdu(command).await?;
    println!("Command sent successfully.");
    
    // Receive and process data for 30 seconds
    println!("Waiting for responses (press Ctrl+C to exit)...");
    let mut count = 0;
    
    while let Some(asdu) = client.receive().await? {
        count += 1;
        println!("Received ASDU #{}: {:?}", count, asdu);
        
        // Handle different types of ASDU
        process_asdu(&asdu);
    }
    
    println!("Connection closed.");
    Ok(())
}

/// Creates a single command ASDU
fn create_single_command(address: u32, command_value: bool) -> Asdu {
    Asdu::new(
        TypeId::C_SC_NA_1,           // Single command
        false,                        // Not a sequence
        CauseOfTransmission::Activation,
        0,                           // Originator address
        1,                           // Common address of ASDU
        vec![
            InformationObject::SingleCommand {
                address,
                command: command_value, // ON/OFF
                select: false,          // Execute directly (not select)
                qualifier: 0,           // No additional qualifier
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
        TypeId::M_ME_NB_1 => {
            println!("  - Received measured value (scaled)");
            
            // Process information objects
            for info_obj in asdu.information_objects() {
                if let InformationObject::MeasuredValueScaled { address, value, quality } = info_obj {
                    println!("    - Address: {}, Value: {}, Quality: {}", address, value, quality);
                }
            }
        },
        TypeId::C_SC_NA_1 => {
            println!("  - Received single command confirmation");
            
            // Check if it's a command confirmation
            if asdu.cause_of_transmission() == CauseOfTransmission::ActivationConfirm {
                println!("    - Command was confirmed by the remote device");
            } else if asdu.cause_of_transmission() == CauseOfTransmission::ActivationTermination {
                println!("    - Command execution was completed");
            }
        },
        _ => {
            println!("  - Received other type ({})", asdu.type_id() as u8);
        }
    }
} 