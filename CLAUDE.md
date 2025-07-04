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

# Run single test
cargo test test_name --package package_name
cargo test test_name --package comsrv -- --exact

# Test all services
cargo test --workspace

# Format and lint
cargo fmt
cargo fmt --check  # Check without modifying
cargo clippy -- -D warnings

# Run specific test
cargo test test_name

# Run with logging
RUST_LOG=debug cargo run
RUST_LOG=info,comsrv=debug cargo run  # Service-specific debug

# Check code formatting
cargo fmt --check

# Run clippy linting
cargo clippy -- -D warnings

# Build release version
cargo build --release

# Run release version
./target/release/{service_name}-rust
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

# Start services only (without demo data)
./start-services.sh

# View logs
docker-compose logs -f {service_name}
docker-compose -f frontend/grafana/docker-compose.grafana.yml logs -f

# Rebuild specific service
docker-compose build {service_name}

# Stop all services
docker-compose down
```

### Testing Tools

```bash
# Run Modbus simulator
cd tests
python modbus_server_simulator.py

# Run integration tests
./test_comsrv_integration.sh

# Performance testing (10,000 points)
cd scripts
./test_optimized_points.sh

# Check Redis data
./check_redis_points.sh
```

### Service-Specific Commands

#### ComSrv Testing
```bash
cd services/comsrv
./scripts/start_modbus_simulator.sh    # Start Modbus simulator
./scripts/integration_test.sh          # Run integration tests
./scripts/check_redis_points.sh        # Verify Redis data
./scripts/test_optimized_points.sh     # Performance test

# Run modbus tests specifically
cargo test --package comsrv modbus
```

#### ModSrv Testing
```bash
cd services/modsrv
./run-local-tests.sh           # Basic tests
./run-local-tests.sh -b        # Rebuild image
./run-local-tests.sh --debug   # Debug mode
python3 test-api.py            # API tests
python3 test-rules-api.py      # Rules engine tests
```

#### Running Individual Services
```bash
# Each service has a start script
cd services/{service_name}
./start.sh

# API Gateway with config service
cd services/apigateway
./start-with-config-service.sh
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

VoltageEMS is a microservices-based Industrial IoT Energy Management System designed for edge computing scenarios.

### Core Services (Rust)

- **comsrv**: Industrial protocol communication (Modbus TCP/RTU, CAN, IEC60870, GPIO)
  - Channel-based device management with CSV point tables (四遥: YC/YX/YK/YT)
  - Layered transport architecture with unified Transport trait
  - Real-time data stored in Redis with optimized point management
  - Protocol-specific polling engines (ModbusPollingEngine for Modbus)

- **modsrv**: Model computation service with DAG workflow engine
  - Maps communication data to internal models
  - Executes control logic and calculations
  - Supports device control dispatch
  - Template-based model instantiation

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
  - Config service integration

- **config-framework**: Configuration center for centralized config management

### Data Flow

```
Industrial Devices → comsrv → Redis → {modsrv, hissrv, netsrv, alarmsrv}
                                   ↓        ↓
                              apigateway   InfluxDB
                                   ↓        ↓
                              Frontend/Grafana
```

### Frontend Stack

- Vue.js 3 + Element Plus
- Electron for desktop application
- Embedded Grafana for visualization
- Real-time WebSocket updates
- Vue Flow for DAG visualization

## Key Technical Details

### Communication Service (comsrv)

- Uses **layered transport architecture** separating protocol logic from physical transport
- Supports industrial interfaces: TCP, Serial, GPIO (DI/DO), CAN bus
- Configuration via YAML files with CSV point tables
- Channel-based device management with point mapping
- Built-in Prometheus metrics and structured logging
- **Enhanced logging system** with configurable file output, target filtering, and compact format

### Transport Layer Implementation

All protocols share unified `Transport` trait:
- `connect()`, `disconnect()`, `send()`, `receive()`
- Factory pattern for transport creation
- Mock transport for protocol testing
- Industrial-grade error handling and statistics

### Configuration Management

- **Figment-based** hierarchical configuration (YAML/TOML/JSON/ENV)
- **CSV point tables** for device mapping:
  - `telemetry.csv` (YC): Measurement points
  - `signal.csv` (YX): Status signals
  - `control.csv` (YK): Control commands
  - `adjustment.csv` (YT): Set points
- **Channel parameters** specific to each protocol
- Validation and type safety throughout
- **Configuration Center Support** (hissrv, config-framework)
  - Environment variables: `CONFIG_CENTER_URL`, `ENVIRONMENT`
  - Automatic fallback to local configuration files

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

- Points stored with optimized `HashMap<u32, UniversalPointConfig>`
- Multi-level indexes for fast queries
- Batch operations with Pipeline mode
- Local cache with TTL management
- Key patterns:
  - Real-time data: `voltage:{service}:data:{point_id}`
  - Config: `voltage:{service}:config:{item}`
  - Status: `voltage:{service}:status:{channel_id}`

### Development Workflow

1. Feature branches: `feature/{service_name}`
2. Merge to `develop` when complete
3. Pull `develop` before new features
4. Update `services/{service}/docs/fixlog.md` after changes
5. Never auto-commit (manual commits only)
6. Use English for code comments and git commits
7. Use Chinese (中文) for user-facing documentation

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
- No quality attributes in data structures
- Enhanced logging with daily rotation and configurable paths
- Support multiple protocols in single deployment
- Each service has independent configuration management
- Each time you finish modifying a file, record it in the corresponding microservice's fixlog.md

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

## HisSrv Specific Details

### Configuration Center Integration

HisSrv supports dynamic configuration loading from a central configuration service:

```bash
# Using configuration center
export CONFIG_CENTER_URL=http://config-center:8080
export ENVIRONMENT=production
cargo run

# Using local configuration (fallback)
cargo run -- --config hissrv.yaml
```

### Storage Backend Support

- **InfluxDB**: Primary time-series storage
- **Redis**: Real-time data caching
- **PostgreSQL**: Structured data storage (optional)
- **MongoDB**: Document storage (optional)

### API Endpoints

- Query history: `GET /api/v1/history`
- Store data: `POST /api/v1/data`
- Delete data: `DELETE /api/v1/data`
- Get keys: `GET /api/v1/data/keys`
- Statistics: `GET /api/v1/admin/statistics`
- Health check: `GET /api/v1/health`

### Common Compilation Fixes

When encountering compilation errors after merging branches:

1. **Missing imports**: Check for `IntoParams`, `ErrorResponse`, etc.
2. **Deprecated APIs**: Update `base64::encode()` to `general_purpose::STANDARD.encode()`
3. **Redis async issues**: Use `redis::cmd()` for commands like `PING`, `INFO`
4. **Type annotations**: Add explicit types when compiler can't infer

## Workspace Structure

The project uses a Cargo workspace with the following members:
- services/alarmsrv
- services/apigateway
- services/comsrv
- services/config-framework
- services/hissrv
- services/modsrv
- services/netsrv

Each service is independently deployable and communicates via Redis pub/sub.

## Project Structure Key Points

- `services/`: All Rust microservices
- `frontend/`: Vue.js web application
- `config/`: Service configuration files
- `docs/`: Architecture and design documentation
- Each service has:
  - `src/`: Source code
  - `docs/fixlog.md`: Service-specific changelog
  - `start.sh`: Service startup script
  - Configuration examples