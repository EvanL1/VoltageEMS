use rust_iec60870::iec104::{Server, ServerConfig};
use rust_iec60870::common::{Asdu, TypeId, CauseOfTransmission, InformationObject};
use tokio::time::{Duration, sleep};
use std::error::Error;

/// A simple IEC 60870-5-104 server example that listens for connections,
/// processes incoming commands, and periodically sends measurement data.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    env_logger::init();
    
    // Configure the server
    let config = ServerConfig::default()
        .with_bind_address("0.0.0.0:2404")  // Listen on all interfaces
        .with_t0(Duration::from_secs(30))   // Connection timeout
        .with_t1(Duration::from_secs(15))   // APDU timeout
        .with_t2(Duration::from_secs(10))   // Acknowledgment timeout
        .with_t3(Duration::from_secs(20));  // Test frame timeout
    
    println!("Starting IEC 60870-5-104 server on 0.0.0.0:2404...");
    
    // Create and start the server
    let mut server = Server::new(config);
    server.start().await?;
    println!("Server started, waiting for client connections...");
    
    // Accept a client connection
    let mut connection = server.accept().await?;
    println!("Client connected: {}", connection.peer_addr()?);
    
    // Spawn a task for periodic data updates
    let connection_clone = connection.clone();
    let periodic_task = tokio::spawn(async move {
        let mut counter = 0;
        loop {
            counter += 1;
            // Send periodic measurement data
            if let Err(e) = send_periodic_data(&connection_clone, counter).await {
                eprintln!("Error sending periodic data: {}", e);
                break;
            }
            
            // Wait for 5 seconds before sending next update
            sleep(Duration::from_secs(5)).await;
        }
    });
    
    // Handle incoming commands
    println!("Waiting for incoming commands...");
    while let Some(asdu) = connection.receive().await? {
        println!("Received ASDU: {:?}", asdu);
        
        // Process the ASDU based on its type
        match asdu.type_id() {
            TypeId::C_SC_NA_1 => {
                println!("Received single command");
                
                // Extract command information
                if let Some(InformationObject::SingleCommand { address, command, .. }) = asdu.information_objects().first() {
                    println!("  - Address: {}, Command value: {}", address, command);
                    
                    // Process the command (in a real application)
                    // ...
                    
                    // Send confirmation
                    let response = create_command_confirmation(&asdu);
                    connection.send_asdu(response).await?;
                    println!("  - Sent confirmation");
                    
                    // After some time, send execution completion
                    sleep(Duration::from_millis(500)).await;
                    let termination = create_command_termination(&asdu);
                    connection.send_asdu(termination).await?;
                    println!("  - Sent execution termination");
                }
            },
            TypeId::C_IC_NA_1 => {
                println!("Received interrogation command");
                
                // Send confirmation
                let response = Asdu::new(
                    TypeId::C_IC_NA_1,
                    false,
                    CauseOfTransmission::ActivationConfirm,
                    0,
                    asdu.common_address(),
                    asdu.information_objects().clone(),
                );
                connection.send_asdu(response).await?;
                
                // Send current values of all data points
                send_all_data_points(&connection).await?;
                
                // Send termination
                let termination = Asdu::new(
                    TypeId::C_IC_NA_1,
                    false,
                    CauseOfTransmission::ActivationTermination,
                    0,
                    asdu.common_address(),
                    asdu.information_objects().clone(),
                );
                connection.send_asdu(termination).await?;
            },
            _ => {
                println!("Received unsupported command type: {:?}", asdu.type_id());
            }
        }
    }
    
    // Cancel the periodic task when the connection is closed
    periodic_task.abort();
    println!("Connection closed.");
    
    Ok(())
}

/// Creates a command confirmation ASDU
fn create_command_confirmation(original: &Asdu) -> Asdu {
    Asdu::new(
        original.type_id(),
        false,
        CauseOfTransmission::ActivationConfirm,
        0,
        original.common_address(),
        original.information_objects().clone(),
    )
}

/// Creates a command termination ASDU
fn create_command_termination(original: &Asdu) -> Asdu {
    Asdu::new(
        original.type_id(),
        false,
        CauseOfTransmission::ActivationTermination,
        0,
        original.common_address(),
        original.information_objects().clone(),
    )
}

/// Sends all current data points
async fn send_all_data_points(connection: &Server) -> Result<(), Box<dyn Error>> {
    // Send single point information
    let single_points = Asdu::new(
        TypeId::M_SP_NA_1,
        false,
        CauseOfTransmission::Spontaneous,
        0,
        1,
        vec![
            InformationObject::SinglePoint {
                address: 1001,
                value: true,
                quality: 0,
            },
            InformationObject::SinglePoint {
                address: 1002,
                value: false,
                quality: 0,
            },
        ],
    );
    connection.send_asdu(single_points).await?;
    
    // Send measured values
    let measured_values = Asdu::new(
        TypeId::M_ME_NB_1,
        false,
        CauseOfTransmission::Spontaneous,
        0,
        1,
        vec![
            InformationObject::MeasuredValueScaled {
                address: 2001,
                value: 75,
                quality: 0,
            },
            InformationObject::MeasuredValueScaled {
                address: 2002,
                value: 50,
                quality: 0,
            },
        ],
    );
    connection.send_asdu(measured_values).await?;
    
    Ok(())
}

/// Sends periodic measurement data
async fn send_periodic_data(connection: &Server, counter: i16) -> Result<(), Box<dyn Error>> {
    // Create an ASDU with measured values that change over time
    let measurement = Asdu::new(
        TypeId::M_ME_NB_1,
        false,
        CauseOfTransmission::Periodic,
        0,
        1,
        vec![
            InformationObject::MeasuredValueScaled {
                address: 2001,
                value: counter,
                quality: 0,
            },
            InformationObject::MeasuredValueScaled {
                address: 2002,
                value: counter * 2,
                quality: 0,
            },
        ],
    );
    
    connection.send_asdu(measurement).await?;
    println!("Sent periodic measurement data, counter = {}", counter);
    
    Ok(())
} 