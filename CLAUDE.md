# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

VoltageEMS is a high-performance industrial IoT energy management system built with Rust microservices architecture. It supports multiple industrial protocols and real-time data processing.

## Architecture

### Service Architecture
The system uses a microservice architecture with Redis as the central message bus:

```
Web App → API Gateway (8089) → Redis Message Bus → Services → Devices
                                       ↓
                          ┌────────────┼────────────┐
                          │            │            │
                      comsrv(3000)  modsrv(8092)  hissrv(8082)
                      alarmsrv(8080) rulesrv(8080) netsrv(TBD)
```

### Key Design Patterns

1. **Plugin Architecture (comsrv)**
   - `ComBase` trait: Core interface for all communication protocols
   - `PluginStorage` trait: Abstraction for protocol-specific storage
   - Protocol plugins: Modbus, Virtual, gRPC (extensible)
   - Dynamic plugin loading and management

2. **Redis Data Structure**
   - Hash-based storage for O(1) access: `{service}:{channelID}:{type}`
   - Types: m (measurement), s (signal), c (control), a (adjustment)
   - Point IDs start from 1 (not 10001)
   - 6 decimal precision standardization

3. **Shared Libraries (libs/)**
   - Common Redis client with connection pooling
   - InfluxDB client for time-series data
   - Unified error handling and configuration loading
   - Shared types across all services

4. **Redis Functions Architecture**
   - Located in `scripts/redis-functions/`
   - Core functions for data operations
   - Service-specific functions for each microservice
   - Loaded via `load_all_functions.sh`

## Version Information

All services use version 0.0.1:
- libs (voltage-libs): 0.0.1
- All services: 0.0.1

## Common Development Commands

### Build & Test Commands

```bash
# Check compilation (preferred over build)
cargo check --workspace

# Format and lint
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test --workspace
cargo test -p {service_name}
cargo test test_name -- --exact --nocapture

```

### Service Development

```bash
# Run individual service
cd services/{service_name}
RUST_LOG=debug cargo run
RUST_LOG={service_name}=trace cargo run

# Watch mode
cargo watch -x run
```

### Redis Operations

```bash
# Start Redis
docker run -d --name redis-dev -p 6379:6379 redis:8-alpine

# Load Redis functions
cd scripts/redis-functions
./load_all_functions.sh

# Monitor activity
redis-cli monitor | grep {service_name}

# Check data
redis-cli hgetall "comsrv:1001:m"    # View measurements
redis-cli hget "comsrv:1001:m" "1"   # Get point ID 1
redis-cli hlen "comsrv:1001:m"       # Count points

# Pub/Sub monitoring
redis-cli psubscribe "comsrv:*"
```

### Docker Environment

```bash
# Each service has its own Dockerfile
cd services/{service_name}
docker build -t {service_name} .

# No docker-compose files in the project (removed during cleanup)
```

### Python Scripts

```bash
# Uses uv for Python environment
uv run python scripts/script_name.py
uv pip install -r requirements.txt
```

## Key Implementation Details

### comsrv Plugin System
- Protocols implement `ComBase` trait
- Storage implements `PluginStorage` trait
- Dynamic loading via `PluginManager`
- Channel-based configuration in CSV files

### Redis Hash Storage
- Measurement: `comsrv:{channel}:m` → `{pointId: value}`
- Signal: `comsrv:{channel}:s` → `{pointId: 0/1}`
- Control: `comsrv:{channel}:c` → `{pointId: value}`
- Adjustment: `comsrv:{channel}:a` → `{pointId: value}`

### Service Communication
- Pub/Sub for real-time events
- Hash storage for state persistence
- Redis functions for atomic operations
- No direct service-to-service calls

### Configuration
- YAML-based: `services/{service}/config/default.yml`
- CSV point mapping: `config/{protocol}_CH{id}/*.csv`
- Environment overrides supported

## Comsrv Configuration

### YAML Configuration Structure
```yaml
# CSV base path (can be overridden by environment variable)
csv_base_path: "${CSV_BASE_PATH:-/app/config}"

channels:
  - id: 1
    name: "modbus_tcp_channel_1"
    protocol: "modbus"
    enabled: true
    transport_config:
      tcp:
        address: "192.168.1.100:502"
    polling_config:
      interval_ms: 1000
      batch_size: 100
    table_config:
      # Points to comsrv directory for four telemetry files
      four_telemetry_route: "comsrv"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"
        signal_file: "signal.csv"
        control_file: "control.csv"
        adjustment_file: "adjustment.csv"
      # Protocol mappings in protocol subdirectory
      protocol_mapping_route: "comsrv/protocol"
      protocol_mapping_files:
        telemetry_mapping: "telemetry_mapping.csv"
        signal_mapping: "signal_mapping.csv"
        control_mapping: "control_mapping.csv"
        adjustment_mapping: "adjustment_mapping.csv"
```

### Unified CSV Format (v0.0.1+)
All four telemetry types use the same CSV format:

```csv
point_id,signal_name,scale,offset,unit,reverse,data_type
1,油温,1.0,0.0,℃,false,float
2,断路器状态,1.0,0.0,,false,bool
```

- **point_id**: Sequential from 1
- **scale/offset**: For value conversion (actual = raw × scale + offset)
- **reverse**: For bool types (true = inverted logic)
- **data_type**: float/int/bool

### Protocol Mapping CSV
Maps points to protocol-specific parameters:

```csv
point_id,slave_id,function_code,register_address,bit_position,data_type,byte_order
1,1,3,0,,float32,ABCD
2,1,1,0,0,bool,
```

### File Organization
```
config/
└── comsrv/
    ├── telemetry.csv          # Measurements
    ├── signal.csv             # Digital inputs
    ├── control.csv            # Control outputs
    ├── adjustment.csv         # Setpoints
    └── protocol/              # Protocol mappings
        ├── telemetry_mapping.csv
        ├── signal_mapping.csv
        ├── control_mapping.csv
        └── adjustment_mapping.csv
```

## Development Guidelines

- Each service has exactly one Dockerfile
- Use `cargo check` instead of `cargo build` during development
- Point IDs start from 1 (sequential numbering)
- All numeric values use 6 decimal precision
- Prefer Hash operations over Keys scanning
- For bool types in CSV: scale=1.0, offset=0.0
