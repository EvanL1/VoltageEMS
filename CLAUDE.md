# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Development Commands

### Rust Services (comsrv, modsrv, hissrv, netsrv, alarmsrv)

```bash
# Build individual service
cd services/{service_name}
cargo build
cargo build --release    # Production build

# Run individual service
cargo run
RUST_LOG=debug cargo run    # With debug logging

# Run tests
cargo test
cargo test -- --nocapture   # Show print output

# Code quality
cargo fmt               # Format code
cargo clippy           # Lint code
```

### Frontend (Vue.js)

```bash
cd frontend
npm install
npm run serve    # Development server on port 8081
npm run build    # Production build
npm run lint     # ESLint checking
```

### Docker Development

```bash
# Start specific service
docker-compose -f services/{service_name}/docker-compose.yml up -d

# Start Grafana and InfluxDB
docker-compose -f frontend/grafana/docker-compose.grafana.yml up -d

# View logs
docker-compose logs -f {service_name}

# Demo environment
./start-demo.sh      # Start all services with demo data
./stop-demo.sh       # Stop all services
./restart-demo.sh    # Restart services
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

- **frontend**: Vue.js 3 + Element Plus + Tailwind CSS web application with embedded Grafana visualization
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
- Main branch: `feature/comsrv` (current)
- Write English code comments and git commit messages
- Log fixes to `{service}/docs/fixlog.md`
- Never auto-commit changes

### Testing

- Unit tests: `cargo test` in service directories
- Integration tests: `tests/test_comsrv_integration.sh`
- Mock simulators: `tests/modbus_server_simulator.py`
- API tests: `test-api.py` scripts in service directories
- Performance tests: `scripts/test_optimized_points.sh`

### Configuration Structure

```
config/
├── {service}.yml         # Service configuration
└── point_tables/         # CSV point definitions
    ├── telemetry.csv
    ├── control.csv
    ├── adjustment.csv
    └── signal.csv
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
```

### Important Notes

- Redis runs on port 6379 as real-time database
- Services communicate only via Redis, not direct calls
- Use Chinese for user-facing documentation
- Record file modifications in `{service}/docs/fixlog.md`
- Support for multiple industrial protocols in single deployment
- Service ports: Frontend (8081), Grafana (3000), APIs (809x series)

### Data Structures and Optimization

#### Point Management Optimization
- Use `HashMap<u32, UniversalPointConfig>` for better performance
- Implement multi-level indexes using `HashSet<u32>` for type grouping
- Add `name_to_id` mapping for name-based queries
- All query operations optimized to O(1) complexity

#### Protocol Mapping Structure
- **ProtocolMapping**: Uses address string format `slave_id:function_code:register_address`
- **UnifiedPointMapping**: Uses ProtocolAddress enum for different protocols
- Address parsing from protocol_params HashMap

#### Redis Optimization
- Implement local cache layer with TTL management
- Use batch operations with Pipeline mode
- Replace KEYS command with SCAN to avoid blocking
- Support batch update API: `batch_update_values`

### Redis Key Naming Convention
- Communication data: `voltage:com:*`
- Model data: `voltage:mod:*`
- Alarm data: `voltage:alarm:*`
- Real-time data: `voltage:data:*`

### Testing Tools

#### Modbus Testing
- `tests/modbus_server_simulator.py` - Full Modbus TCP server simulator
- `tests/test_modbus_client.py` - Test client for verification
- `scripts/start_modbus_simulator.sh` - Server startup script

#### Performance Testing
- `examples/optimized_points_demo.rs` - 10,000 point stress test
- `scripts/test_optimized_points.sh` - Performance test script
- `scripts/check_redis_points.sh` - Redis data validation script