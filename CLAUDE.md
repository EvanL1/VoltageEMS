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
- Built-in Prometheus metrics and structured logging

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

### Important Notes

- Redis runs on port 6379 as real-time database
- No quality attributes needed in data structures
- Use Chinese for user-facing documentation
- Services communicate only via Redis, not direct calls
- Support for multiple industrial protocols in single deployment