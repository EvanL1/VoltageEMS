# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Development Commands

### Rust Services (comsrv, modsrv, hissrv, netsrv, alarmsrv)

```bash
# Build individual service
cd services/{service_name}
cargo build

# Run individual service
cargo run

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run

# Check code formatting
cargo fmt --check

# Run clippy linting
cargo clippy -- -D warnings
```

### Frontend (Vue.js)

```bash
cd frontend
npm install
npm run serve    # Development server
npm run build    # Production build
npm run lint     # ESLint checking
```

### Docker Development

```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f {service_name}

# Rebuild specific service
docker-compose build {service_name}
```

## Architecture Overview

VoltageEMS is a microservices-based IoT Energy Management System with the following components:

### Core Services (Rust-based)

- **comsrv**: Industrial communication service supporting Modbus TCP/RTU, CAN, IEC60870, and GPIO interfaces
- **modsrv**: Model service executing real-time calculations and control logic via DAG workflows  
- **hissrv**: Historical data service writing Redis data to InfluxDB
- **netsrv**: Network service forwarding data to external systems via MQTT/HTTP, supporting AWS IoT Core and Alibaba Cloud IoT
- **alarmsrv**: Intelligent alarm management with classification, Redis storage, and cloud integration

### Frontend & Configuration

- **frontend**: Vue.js + Element Plus web application with embedded Grafana visualization
- **Electron integration**: Cross-platform desktop application wrapper

### Data Flow Architecture

```
Devices (Modbus/CAN/IEC60870) → comsrv → Redis → {modsrv, hissrv, netsrv, alarmsrv}
                                              ↓
                                         InfluxDB ← hissrv
                                              ↓
                                         Frontend/Grafana
```

## Key Technical Details

### Communication Service (comsrv)

- Uses **layered transport architecture** separating protocol logic from physical transport
- Supports industrial interfaces: TCP, Serial, GPIO (DI/DO), CAN bus
- Configuration via YAML files with CSV point tables
- Channel-based device management with point mapping
- Built-in Prometheus metrics and optimized structured logging
- **Enhanced logging system** with configurable file output, target filtering, and compact format

### Transport Layer Implementation

All protocols share unified `Transport` trait:
- `connect()`, `disconnect()`, `send()`, `receive()`
- Factory pattern for transport creation
- Mock transport for protocol testing
- Industrial-grade error handling and statistics

### Configuration Management

- **Figment-based** hierarchical configuration (YAML/TOML/JSON/ENV)
- **CSV point tables** for telemetry, control, adjustment, and signal points
- **Channel parameters** specific to each protocol
- Validation and type safety throughout

### Development Workflow

- Use feature branches: `feature/{service_name}` for development
- Merge to `develop` branch when complete
- Merge `develop` before starting new features
- Write English code comments and git commit messages
- Log fixes to `{service}/docs/fixlog.md`
- Never auto-commit changes

### Testing

- Unit tests: `cargo test` in service directories
- Integration tests available in `tests/` directories
- Mock simulators: `modbus_simulator.py`, protocol-specific test tools
- Real hardware testing supported via configuration

### Configuration Structure

```
config/
├── default.yml           # Global configuration
├── point_map.yml         # Point mapping definitions
└── {Protocol}_Test_{ID}/ # Protocol-specific CSV tables
    ├── telemetry.csv
    ├── control.csv
    ├── adjustment.csv
    ├── signal.csv
    └── mapping_*.csv
```

### Logging System Configuration

#### Service-Level Logging
```yaml
service:
  logging:
    level: "debug"
    file: "logs/comsrv.log"        # Configurable file path
    max_size: 10485760             # 10MB file size limit
    max_files: 5                   # Max number of rotated files
    console: true                  # Enable console output
```

#### Channel-Level Logging
```yaml
channels:
  - logging:
      enabled: true
      level: "debug"
      log_dir: "logs/modbus_tcp_demo"    # Custom log directory
      max_file_size: 5242880             # 5MB per file
      max_files: 3                       # Keep 3 files
      retention_days: 7                  # Keep for 7 days
      console_output: true               # Also output to console
      log_messages: true                 # Log protocol messages
```

#### Logging Features
- **Daily log rotation** with configurable retention
- **Compact format** without redundant target information  
- **Mixed output** supporting both console and file simultaneously
- **Channel-specific logs** in separate directories
- **Configurable paths** for flexible deployment

### Important Notes

- Redis runs in container at port 6379 as real-time database
- No quality attributes needed in data structures
- Use Chinese for user-facing documentation
- Each time you finish modifying a file, record it in the corresponding microservice's fixlog.md.
- Services communicate only via Redis, not direct calls
- Support for multiple industrial protocols in single deployment
- **Enhanced logging** provides clear, non-redundant output for debugging and monitoring

### Data Structures and Optimization

#### Point Management Optimization
- Use `HashMap<u32, UniversalPointConfig>` instead of `HashMap<String, UniversalPointConfig>` for better performance
- Implement multi-level indexes using `HashSet<u32>` for type grouping and permission checks
- Add `name_to_id` mapping for name-based queries
- All query operations optimized to O(1) complexity

#### Protocol Mapping Structure
Multiple mapping structures exist in the codebase:

1. **ProtocolMapping** (in config_manager.rs and types/channel.rs)
   - Does not directly contain slave_id and function_code fields
   - These values are stored in the address string field or protocol_params HashMap
   - Address format: `slave_id:function_code:register_address` (colon-separated)

2. **ProtocolMappingRecord** (in loaders/csv_loader.rs)
   - Directly contains slave_id, function_code, register_address fields
   - Raw data structure loaded from CSV files

3. **UnifiedPointMapping** (in types/protocol.rs)
   - Uses ProtocolAddress enum for different protocol addresses
   - Modbus variant includes slave_id, function_code, register, bit fields

#### Address Parsing Logic
```rust
// Extract address from protocol_params
let address = cp.protocol_params.get("address").unwrap_or(&default_address);
// Parse according to "slave_id:function_code:register_address" format
let address_parts: Vec<&str> = address.split(':').collect();
let slave_id = address_parts[0].parse::<u8>()?;
let function_code = address_parts[1].parse::<u8>()?;
let register_address = address_parts[2].parse::<u16>()?;
```

#### Redis Optimization
- Implement local cache layer with TTL management
- Use batch operations with Pipeline mode
- Replace KEYS command with SCAN to avoid blocking
- Support batch update API: `batch_update_values`

### Testing Tools

#### Modbus Testing
- `tests/modbus_server_simulator.py` - Full Modbus TCP server simulator
  - Supports all four remote types (YC/YX/YK/YT)
  - Matches comsrv configuration for slave IDs and addresses
  - Real-time data updates with sine wave simulation
- `tests/test_modbus_client.py` - Test client for verification
- `scripts/start_modbus_simulator.sh` - Server startup script
- `tests/test_comsrv_integration.sh` - Integration test script

#### Performance Testing
- `examples/optimized_points_demo.rs` - 10,000 point stress test
- `scripts/test_optimized_points.sh` - Performance test script
- `scripts/check_redis_points.sh` - Redis data validation script
# important-instruction-reminders
Do what has been asked; nothing more, nothing less.
NEVER create files unless they're absolutely necessary for achieving your goal.
ALWAYS prefer editing an existing file to creating a new one.
NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.

      
      IMPORTANT: this context may or may not be relevant to your tasks. You should not respond to this context or otherwise consider it in your response unless it is highly relevant to your task. Most of the time, it is not relevant.