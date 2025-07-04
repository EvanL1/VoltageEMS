# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Development Commands

### Rust Services Build & Test

```bash
# Build all services (from project root)
cargo build --workspace
cargo build --release --workspace

# Run specific service
cargo run --bin comsrv
cargo run --bin modsrv
cargo run --bin hissrv
cargo run --bin alarmsrv
cargo run --bin apigateway
cargo run --bin netsrv

# Test all services
cargo test --workspace

# Format and lint
cargo fmt
cargo clippy -- -D warnings

# Run with logging
RUST_LOG=debug cargo run
RUST_LOG=info,comsrv=debug cargo run  # Service-specific debug
```

### Frontend Development

```bash
cd frontend
npm install
npm run serve    # Development server at http://localhost:8081
npm run build    # Production build
npm run lint     # ESLint checking
```

### Quick Start with Docker

```bash
# Start complete demo environment (includes Grafana, InfluxDB, mock data)
./start-demo.sh

# Stop demo
./stop-demo.sh

# Restart demo
./restart-demo.sh

# View logs
docker-compose logs -f {service_name}
```

### Service-Specific Commands

#### ComSrv Testing
```bash
cd services/comsrv
./scripts/start_modbus_simulator.sh    # Start Modbus simulator
./scripts/integration_test.sh          # Run integration tests
./scripts/check_redis_points.sh        # Verify Redis data
```

#### ModSrv Testing
```bash
cd services/modsrv
./run-local-tests.sh           # Basic tests
./run-local-tests.sh -b        # Rebuild image
./run-local-tests.sh --debug   # Debug mode
python3 test-api.py            # API tests
```

#### Running Individual Services
```bash
# Each service has a start script
cd services/{service_name}
./start.sh
```

## Configuration Management

VoltageEMS uses a configuration center architecture for microservices configuration:

### Configuration Loading Priority
1. Default values in code
2. Local configuration files (YAML/JSON)
3. Configuration center (HTTP API)
4. Environment variable overrides

### Service Configuration
```bash
# Using local config
cargo run --bin {service}

# Using config center
export CONFIG_CENTER_URL=http://config-center:8080
cargo run --bin {service}

# Environment overrides
export {SERVICE}_REDIS_URL=redis://production:6379
export {SERVICE}_LOG_LEVEL=debug
```

### Configuration Files
- Development: `config/{service}.yaml`
- Production: `/etc/voltageems/config/{service}/{service}.yaml`
- Examples: `config/{service}.example.yaml`

See `docs/CONFIG_CENTER_ARCHITECTURE.md` for detailed configuration management guide.

## Architecture Overview

VoltageEMS is a microservices-based Industrial IoT Energy Management System:

### Core Services (Rust)

- **comsrv**: Industrial protocol communication (Modbus TCP/RTU, CAN, IEC60870, GPIO)
  - Channel-based device management with CSV point tables
  - Layered transport architecture with unified Transport trait
  - Real-time data stored in Redis

- **modsrv**: Model computation service with DAG workflow engine
  - Maps communication data to internal models
  - Executes control logic and calculations
  - Supports device control dispatch

- **hissrv**: Time-series data storage
  - Subscribes to Redis real-time data
  - Writes to InfluxDB for historical analysis
  - Provides Grafana integration

- **netsrv**: Network forwarding service
  - MQTT/HTTP protocol support
  - Cloud integration (AWS IoT Core, Alibaba Cloud IoT)
  - Configurable data formatting

- **alarmsrv**: Intelligent alarm management
  - Multi-level alarm classification
  - Redis-based storage with cloud sync
  - Automatic alarm processing

- **apigateway**: Unified REST API gateway
  - JWT authentication
  - Service routing and aggregation
  - Health monitoring

### Data Flow

```
Industrial Devices → comsrv → Redis → {modsrv, hissrv, netsrv, alarmsrv}
                                   ↓
                              InfluxDB ← hissrv
                                   ↓
                           Frontend/Grafana
```

### Frontend Stack

- Vue.js 3 + Element Plus
- Electron for desktop application
- Embedded Grafana for visualization
- Real-time WebSocket updates

## Key Technical Details

### Configuration Management

- Hierarchical YAML/JSON configuration using Figment
- CSV point tables for device mapping:
  - `telemetry.csv`: Measurement points
  - `control.csv`: Control commands
  - `adjustment.csv`: Set points
  - `signal.csv`: Status signals

### Protocol Implementation

All protocols use unified Transport trait:
```rust
trait Transport {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn send(&mut self, data: &[u8]) -> Result<()>;
    async fn receive(&mut self, buffer: &mut [u8]) -> Result<usize>;
}
```

### Redis Data Structure

- Points stored with optimized HashMap<u32, UniversalPointConfig>
- Multi-level indexes for fast queries
- Batch operations with Pipeline mode
- Local cache with TTL management

### Development Workflow

1. Feature branches: `feature/{service_name}`
2. Merge to `develop` when complete
3. Pull `develop` before new features
4. Update `services/{service}/docs/fixlog.md` after changes
5. Never auto-commit (manual commits only)

## Testing Infrastructure

### Modbus Testing Tools
- `tests/modbus_server_simulator.py`: Full Modbus TCP simulator
- `scripts/start_modbus_simulator.sh`: Quick simulator startup
- `tests/test_comsrv_integration.sh`: End-to-end testing

### Performance Testing
- `examples/optimized_points_demo.rs`: 10,000 point stress test
- Supports concurrent channel testing
- Real-time metrics via Prometheus

## Important Development Notes

- Services communicate only via Redis (no direct service calls)
- Use English for code comments and git commits
- Use Chinese for user-facing documentation
- No quality attributes in data structures
- Enhanced logging with daily rotation and configurable paths
- Support multiple protocols in single deployment

## Access URLs

- Frontend: http://localhost:8081 or http://localhost:8082
- API Gateway: http://localhost:8080
- Grafana: http://localhost:3000 (admin/admin)
- InfluxDB: http://localhost:8086 (admin/password123)

## Configuration Examples

### Service Logging
```yaml
service:
  logging:
    level: "debug"
    file: "logs/comsrv.log"
    max_size: 10485760  # 10MB
    console: true
```

### Channel Configuration
```yaml
channels:
  - id: 1
    name: "Power Meter"
    protocol: modbus_tcp
    parameters:
      modbus_tcp:
        host: "192.168.1.100"
        port: 502
        timeout: 5000
```