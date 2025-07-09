# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Development Commands

### Rust Services (comsrv, modsrv, hissrv, netsrv, alarmsrv, apigateway)

```bash
# Build individual service
cd services/{service_name}
cargo build

# Run individual service
cargo run

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name -- --exact

# Run with logging
RUST_LOG=debug cargo run
RUST_LOG={service_name}=debug cargo run  # Service-specific debug

# Check code formatting
cargo fmt --check

# Run clippy linting
cargo clippy -- -D warnings

# Run benchmarks
cargo bench

# Generate documentation
cargo doc --no-deps --open
```

### Frontend (Vue.js)

```bash
cd frontend
npm install
npm run serve    # Development server (port 5173)
npm run build    # Production build
npm run lint     # ESLint checking
npm run preview  # Preview production build
```

### Docker Development

```bash
# Build all services
./scripts/build-all.sh [version] [registry]

# Start all services
docker-compose up -d

# View logs
docker-compose logs -f {service_name}

# Rebuild specific service
docker-compose build {service_name}

# Run integration tests
./scripts/run-integration-tests.sh
```

### Protocol Plugin Development (NEW)

```bash
# Create new protocol plugin
cd services/comsrv
cargo run -- new {protocol_name} --output src/core/protocols/

# List available plugins
cargo run -- list --verbose

# Generate protocol config template
cargo run -- config {protocol_id} --output config/

# Test protocol plugin
cargo run -- test {protocol_id} --config test_config.yaml

# Migrate configuration
cargo run -- migrate --from yaml --to sqlite config.yaml
```

## Architecture Overview

VoltageEMS is a microservices-based IoT Energy Management System with the following components:

### Core Services (Rust-based)

- **comsrv**: Industrial communication service supporting Modbus TCP/RTU, CAN, IEC60870, Virtual (testing), and GPIO interfaces
  - Plugin-based architecture for protocol extensibility
  - Unified transport layer abstraction (TCP, Serial, CAN, GPIO)
  - Enhanced logging with channel-specific outputs
  - Real-time telemetry via Prometheus metrics
- **apigateway**: REST API gateway providing unified access to all services
  - JWT authentication and authorization
  - Service routing and load balancing
  - Request/response transformation
  - Health monitoring endpoints
- **modsrv**: Model service executing real-time calculations and control logic via DAG workflows
  - Template-based model definitions
  - Rule engine for complex logic
  - Storage agent for data persistence
- **hissrv**: Historical data service writing Redis data to InfluxDB
  - Configurable data retention policies
  - Grafana integration for visualization
  - High-performance batch writing
- **netsrv**: Network service forwarding data to external systems via MQTT/HTTP
  - AWS IoT Core and Alibaba Cloud IoT support
  - Configurable data formatters (JSON, ASCII)
  - Reliable message delivery with retry logic
- **alarmsrv**: Intelligent alarm management with classification and storage
  - Multi-level alarm classification
  - Redis-based real-time storage
  - Cloud notification integration

### Frontend & Configuration

- **frontend**: Vue.js 3 + Element Plus web application
  - Real-time data visualization
  - Embedded Grafana dashboards
  - Responsive design for mobile devices
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
- **Environment variable support** for CSV base path via `COMSRV_CSV_BASE_PATH`

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
    ├── telemetry.csv     # 遥测点 (YC) - measurements
    ├── control.csv       # 遥控点 (YK) - commands
    ├── adjustment.csv    # 遥调点 (YT) - setpoints
    ├── signal.csv        # 遥信点 (YX) - status signals
    └── mapping_*.csv     # Protocol-specific mappings

# CSV Format Example (telemetry.csv):
# point_id,name,address,data_type,scale,offset,unit
# 1,电压A相,30001,float32,0.1,0,V
# 2,电流A相,30003,float32,0.01,0,A
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
- `services/comsrv/scripts/test_modbus.sh` - Modbus-specific tests
- `services/comsrv/scripts/run-integration-test.sh` - Full integration tests

#### Performance Testing
- `examples/optimized_points_demo.rs` - 10,000 point stress test
- `services/comsrv/scripts/test_protocol.sh` - Protocol performance tests

#### Test Execution Scripts
```bash
# Run all service tests
cd services/{service_name}
./scripts/run_all_tests.sh

# Run integration tests with test servers
./scripts/start-test-servers.sh
cargo test --features integration
./scripts/stop-test-servers.sh

# Generate test report
./scripts/generate_test_report.sh
```
### Plugin System Architecture (NEW)

#### Creating Protocol Plugins
The plugin system allows extending comsrv with new protocols:

1. **Plugin Structure**:
   ```
   src/core/protocols/{protocol_name}/
   ├── mod.rs         # Module definition
   ├── plugin.rs      # Plugin implementation
   ├── config.rs      # Configuration types
   ├── client.rs      # Protocol client logic
   └── common.rs      # Shared utilities
   ```

2. **Plugin Registration**:
   - Implement `ProtocolPlugin` trait
   - Register in `plugin_manager.rs`
   - Provide configuration template
   - Define CLI commands

3. **Transport Layer Integration**:
   - Use unified `Transport` trait
   - Support mock transport for testing
   - Handle connection lifecycle

### API Documentation

#### API Gateway Endpoints
- Health: `GET /api/v1/health`
- System Info: `GET /api/v1/system/info`
- Channels: `GET /api/v1/comsrv/channels`
- Points: `GET /api/v1/comsrv/points/{device_id}`
- Commands: `POST /api/v1/comsrv/command`
- History: `GET /api/v1/hissrv/query`
- Alarms: `GET /api/v1/alarmsrv/alarms`

#### Authentication
- JWT tokens required for all endpoints except health
- Token expiration: 24 hours
- Refresh token support

### Troubleshooting Common Issues

#### Redis Connection
```bash
# Check Redis connectivity
redis-cli -p 6379 ping

# Monitor Redis keys
redis-cli monitor | grep comsrv

# Check point data
redis-cli keys "point:*" | head -20
```

#### Service Debugging
```bash
# Enable debug logging for specific module
RUST_LOG=comsrv::core::protocols::modbus=debug cargo run

# Check channel-specific logs
tail -f logs/channel_{id}/channel_{id}.log

# Monitor Prometheus metrics
curl http://localhost:9090/metrics | grep comsrv
```

#### Protocol Issues
- Modbus timeout: Increase `timeout` parameter in channel config
- CAN buffer overflow: Adjust `buffer_size` in transport config
- IEC60870 sequence errors: Check `k`, `w` parameters