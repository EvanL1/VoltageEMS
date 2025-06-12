# Protocol Factory

The protocol factory is a core component of the communication service providing a unified interface for creating and configuring protocol clients.

## Highlights
- Lock-free access using `DashMap`
- Parallel creation of protocol instances
- Trait-based factory architecture supporting dynamic registration
- Lifecycle management with channel creation, cleanup and metrics
- Built-in factories for Modbus TCP and IEC104 with room for custom protocols

## Usage Example
```rust
use comsrv::core::protocol_factory::create_default_factory;
let factory = create_default_factory();
let protocols = factory.supported_protocols();
println!("Supported protocols: {:?}", protocols);
```
