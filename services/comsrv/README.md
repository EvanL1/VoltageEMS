# Communication Service (comsrv)

The communication service provides unified industrial protocol support for EMS. It is designed as an extensible asynchronous framework.

## Features
- Supports Modbus TCP/RTU, CAN and more
- Unified async interface `ComBase`
- YAML/CSV configuration with hot reload
- REST API and Prometheus metrics
- Protocol factory pattern for easy extension

## Architecture Overview
```
comsrv/
├── src/
│   ├── core/
│   │   ├── config/           # configuration management
│   │   ├── protocols/        # protocol implementations
│   │   └── service/          # service layer
│   ├── utils/               # utilities
│   └── lib.rs
├── tests/                   # integration tests
└── examples/                # usage examples
```

## Design Principles
1. **Unified API** – all protocol clients implement the `ComBase` trait
2. **Factory pattern** – `ProtocolFactory` dynamically creates clients
3. **Async design** – built on `async/await` for high concurrency
4. **Extensibility** – traits make it simple to add new protocols
5. **Configuration driven** – channels and points defined in files
6. **Type safety** – strong typing ensures runtime safety

## Supported Protocols
- **Modbus TCP** – supports functions 1,2,3,4,5,6,15,16
- **Modbus RTU** – serial communication over RS485/232
- **CAN Bus** – SocketCAN, Peak CAN etc.
- **IEC 104** – planned
- **IEC 61850** – planned

## Quick Start Example
```rust
use comsrv::core::protocols::factory::create_default_factory;
use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let factory = create_default_factory();
    let mut cfg = factory.get_default_config(&ProtocolType::ModbusTcp)?;
    cfg.parameters.get_mut("host").map(|h| *h = serde_yaml::Value::String("192.168.1.100".into()));
    let client = factory.create_client("PLC_001", cfg).await?;
    client.set_running(true).await;
    println!("Client status: {:?}", client.status().await);
    Ok(())
}
```
