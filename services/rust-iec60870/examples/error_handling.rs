use rust_iec60870::iec104::{Client, ClientConfig};
use rust_iec60870::error::Iec60870Error;
use rust_iec60870::common::{Asdu, TypeId, CauseOfTransmission, InformationObject};
use tokio::time::Duration;
use std::error::Error;

/// An example showing various error handling approaches with the rust-iec60870 library.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    env_logger::init();
    
    // Example 1: Basic error propagation using the ? operator
    println!("Example 1: Basic error propagation with ?");
    match basic_error_propagation().await {
        Ok(_) => println!("Success!"),
        Err(e) => println!("Error: {}", e),
    }
    
    // Example 2: Custom error handling with error variants
    println!("\nExample 2: Custom error handling with match");
    match detailed_error_handling().await {
        Ok(_) => println!("Success!"),
        Err(Iec60870Error::ConnectionFailed(err)) => {
            println!("Connection failed: {}", err);
        },
        Err(Iec60870Error::Timeout) => {
            println!("Operation timed out");
        },
        Err(Iec60870Error::ProtocolViolation(details)) => {
            println!("Protocol violation: {}", details);
        },
        Err(e) => println!("Other error: {}", e),
    }
    
    // Example 3: Using Result combinators
    println!("\nExample 3: Using Result combinators");
    result_combinators().await
        .map(|_| println!("Success!"))
        .unwrap_or_else(|e| println!("Error: {}", e));
    
    // Example 4: Try-catch style with anyhow (if available in library)
    println!("\nExample 4: Using try blocks for more concise error handling");
    if let Err(e) = try_block_style().await {
        println!("Error: {}", e);
    } else {
        println!("Success!");
    }
    
    Ok(())
}

/// Example of using the ? operator for error propagation
async fn basic_error_propagation() -> Result<(), Iec60870Error> {
    let config = ClientConfig::default()
        .with_address("192.168.1.100:2404") // Non-existent server
        .with_t0(Duration::from_secs(5));   // Short timeout
    
    // This will likely fail, but ? will propagate the error
    let mut client = Client::connect(config).await?;
    
    // This won't execute if above fails
    client.start_dt().await?;
    
    Ok(())
}

/// Example of handling specific error types
async fn detailed_error_handling() -> Result<(), Iec60870Error> {
    let config = ClientConfig::default()
        .with_address("192.168.1.100:2404")
        .with_t0(Duration::from_secs(1)); // Very short timeout
    
    // Try to connect with a short timeout
    let client_result = Client::connect(config).await;
    
    // Handle connection errors specifically
    let mut client = match client_result {
        Ok(client) => {
            println!("Connected successfully");
            client
        },
        Err(Iec60870Error::ConnectionFailed(e)) => {
            println!("Connection failed: {}. Retrying with longer timeout...", e);
            
            // Retry with longer timeout
            let config = ClientConfig::default()
                .with_address("192.168.1.100:2404")
                .with_t0(Duration::from_secs(10));
                
            match Client::connect(config).await {
                Ok(client) => client,
                Err(e) => return Err(e),
            }
        },
        Err(e) => return Err(e),
    };
    
    // Send an invalid command to demonstrate protocol violation error
    let invalid_command = Asdu::new(
        TypeId::C_SC_NA_1,
        false,
        CauseOfTransmission::Activation,
        0,
        1,
        vec![],  // Empty information objects - this might cause a protocol violation
    );
    
    match client.send_asdu(invalid_command).await {
        Ok(_) => println!("Command sent successfully"),
        Err(Iec60870Error::ProtocolViolation(details)) => {
            println!("Protocol violation: {}. This is handled specially.", details);
            // Continue execution instead of returning error
        },
        Err(e) => return Err(e),
    }
    
    Ok(())
}

/// Example of using Result combinators
async fn result_combinators() -> Result<(), Iec60870Error> {
    let config = ClientConfig::default()
        .with_address("192.168.1.100:2404");
    
    // Using map_err and other combinators
    let result = Client::connect(config).await
        .map_err(|e| {
            println!("Connection error transformed: {}", e);
            e
        });
        
    if let Ok(mut client) = result {
        // Since we can't use .and_then with async closures directly,
        // we'll handle this manually
        println!("Connected, now starting data transfer");
        match client.start_dt().await {
            Ok(_) => Ok(()),
            Err(e) => {
                println!("Error handled manually: {}", e);
                Err(e)
            }
        }
    } else {
        result
    }
}

/// Example of using try blocks for more concise error handling
async fn try_block_style() -> Result<(), Iec60870Error> {
    let config = ClientConfig::default()
        .with_address("192.168.1.100:2404");
    
    // More concise approach with early returns
    let mut client = Client::connect(config).await?;
    println!("Connected successfully");
    
    // Try to do several operations
    if let Err(e) = client.start_dt().await {
        println!("Failed to start data transfer: {}", e);
        // We can choose to continue despite this error
    }
    
    // Send command
    let command = Asdu::new(
        TypeId::C_SC_NA_1,
        false,
        CauseOfTransmission::Activation,
        0,
        1,
        vec![
            InformationObject::SingleCommand {
                address: 1001,
                command: true,
                select: false,
                qualifier: 0,
            }
        ],
    );
    
    // Try to send, but don't propagate error
    if let Err(e) = client.send_asdu(command).await {
        println!("Failed to send command: {}", e);
        // We might take alternative action here
    } else {
        println!("Command sent successfully");
    }
    
    Ok(())
} 