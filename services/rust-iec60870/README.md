# rust-iec60870

A Rust implementation of the IEC 60870-5 protocols for SCADA communications.

[![Crates.io](https://img.shields.io/crates/v/rust-iec60870.svg)](https://crates.io/crates/rust-iec60870)
[![Documentation](https://docs.rs/rust-iec60870/badge.svg)](https://docs.rs/rust-iec60870)
[![Build Status](https://github.com/voltage-llc/rust-iec60870/workflows/CI/badge.svg)](https://github.com/voltage-llc/rust-iec60870/actions)
[![License](https://img.shields.io/crates/l/rust-iec60870.svg)](https://github.com/voltage-llc/rust-iec60870)

## Features

- IEC 60870-5-104 protocol implementation (TCP/IP-based)
- Asynchronous API using Tokio
- Type-safe ASDU handling
- Comprehensive error handling
- Well-documented codebase
- Designed for embedded and server-side applications

Future plans include:

- IEC 60870-5-101 protocol implementation (serial-based)
- Master and Slave implementations
- Redundancy support

## Example Usage

```rust
use rust_iec60870::iec104::{Iec104Client, Iec104ClientConfig};
use rust_iec60870::common::CauseOfTransmission;
use rust_iec60870::asdu::{ASDU, TypeId};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure client
    let config = Iec104ClientConfig::new()
        .host("192.168.1.100")
        .port(2404)
        .timeout(Duration::from_secs(5))
        .max_retries(3)
        .build()?;
  
    // Create client
    let mut client = Iec104Client::new(config);
  
    // Connect to server
    client.connect().await?;
  
    // Start data transfer
    client.start_data_transfer().await?;
  
    // Send a general interrogation command
    let common_addr = 1;
    let asdu = ASDU::new(
        TypeId::InterrogationCommand,
        0x01, // Single sequence
        CauseOfTransmission::Activation,
        0, // Originator address
        common_addr,
        vec![20], // 20 = general interrogation
    );
  
    client.send_asdu(asdu).await?;
  
    // Process received data
    loop {
        let data = client.receive().await?;
        println!("Received ASDU: {:?}", data);
    }
}
```

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
rust-iec60870 = "0.1.0"
```

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributing

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

Bug reports, feature requests, and pull requests are welcome on GitHub at https://github.com/voltage-llc/rust-iec60870.

## Acknowledgments

This project draws inspiration from:

- [lib60870](https://github.com/mz-automation/lib60870) - C/C++ implementation
- [OpenMUC j60870](https://github.com/openmuc/j60870) - Java implementation
