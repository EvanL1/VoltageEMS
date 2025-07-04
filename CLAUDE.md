# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Development Commands

### Rust Services (comsrv, apigateway, modsrv, hissrv, netsrv, alarmsrv, config-framework)

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

# Service-specific commands
# API Gateway (port 8080)
cd services/apigateway && ./test_api.sh

# Model Service
cd services/modsrv && ./run-local-tests.sh

# Historical Service
cd services/Hissrv && ./start.sh

# Alarm Service
cd services/alarmsrv && ./start.sh

# Communication Service integration test
cd services/comsrv && tests/test_comsrv_integration.sh
```

### Frontend (Vue.js)

```bash
cd frontend
npm install
npm run serve    # Development server (port 8082)
npm run build    # Production build
npm run lint     # ESLint checking
```

### Docker Development

```bash
# Start Grafana (port 3050) and InfluxDB (port 8086)
docker-compose -f frontend/grafana/docker-compose.grafana.yml up -d

# View logs
docker-compose -f frontend/grafana/docker-compose.grafana.yml logs -f

# Stop services
docker-compose -f frontend/grafana/docker-compose.grafana.yml down

# Default credentials:
# Grafana: admin/admin
# InfluxDB: admin/password123
```

### Quick Start Scripts

```bash
# Start complete demo environment (includes data simulation)
./start-demo.sh

# Start basic services only
./start-services.sh

# Stop demo environment
./stop-demo.sh

# Restart services
./restart-demo.sh
```

## Architecture Overview

VoltageEMS is a microservices-based IoT Energy Management System with the following components:

### Core Services (Rust-based)

- **comsrv**: Industrial communication service supporting Modbus TCP/RTU, CAN, IEC60870, and GPIO interfaces
- **apigateway**: REST API gateway providing unified access to all services (port 8080)
- **modsrv**: Model service executing real-time calculations and control logic via DAG workflows  
- **hissrv**: Historical data service writing Redis data to InfluxDB
- **netsrv**: Network service forwarding data to external systems via MQTT/HTTP, supporting AWS IoT Core and Alibaba Cloud IoT
- **alarmsrv**: Intelligent alarm management with classification, Redis storage, and cloud integration
- **config-framework**: Unified configuration management framework with SQLite storage
- **config-cli**: Command-line tool for configuration management

### Frontend & Configuration

- **frontend**: Vue.js + Element Plus web application with embedded Grafana visualization
- **Electron integration**: Cross-platform desktop application wrapper

### Data Flow Architecture

```
Devices (Modbus/CAN/IEC60870) → comsrv → Redis → {apigateway, modsrv, hissrv, netsrv, alarmsrv}
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
- **Polling interval configuration**: 
  - Modbus protocol has dedicated `ModbusPollingEngine` with slave-level polling support
  - Other protocols use generic `UniversalPollingEngine`
  - Polling interval is NOT configured at channel level anymore
  - Each protocol can have protocol-specific polling configuration

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
├── comsrv.yml            # Communication service configuration
├── apigateway.yml        # API gateway configuration
├── modsrv.yml            # Model service configuration
├── netsrv.yml            # Network service configuration
├── alarmsrv.yml          # Alarm service configuration
└── {Protocol}_Test_{ID}/ # Protocol-specific CSV tables
    ├── telemetry.csv     # Telemetry points (YC)
    ├── control.csv       # Control points (YK)
    ├── adjustment.csv    # Adjustment points (YT)
    ├── signal.csv        # Signal points (YX)
    └── mapping_*.csv     # Protocol mappings
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
- Each time you finish modifying a file, record it in the corresponding microservice’s fixlog.md.
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
- `scripts/integration_test.sh` - Full system integration test

### Service Access URLs

- **Frontend Application**: http://localhost:8082
- **API Gateway**: http://localhost:8080
- **Grafana Dashboard**: http://localhost:3050 (admin/admin)
- **InfluxDB**: http://localhost:8086 (admin/password123)
- **Redis**: localhost:6379 (no auth by default)

## API Gateway Documentation

### Authentication

API Gateway uses JWT-based authentication. Default credentials:
- Username: `admin`
- Password: `admin123`

#### Login Flow
```bash
# 1. Login to get tokens
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin123"}'

# Response:
{
  "success": true,
  "data": {
    "access_token": "eyJ...",
    "refresh_token": "eyJ...",
    "expires_in": 3600,
    "token_type": "Bearer",
    "user": {
      "id": "1",
      "username": "admin",
      "roles": ["admin"]
    }
  }
}

# 2. Use access token for API calls
curl http://localhost:8080/api/v1/comsrv/api/channels \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"

# 3. Refresh token when expired
curl -X POST http://localhost:8080/api/v1/auth/refresh \
  -H "Content-Type: application/json" \
  -d '{"refresh_token": "YOUR_REFRESH_TOKEN"}'
```

### API Endpoints

#### Public Endpoints (No Auth Required)
- `GET /health` - Basic health check
- `GET /api/v1/health` - Service status
- `GET /api/v1/health/detailed` - Detailed health with all services
- `POST /api/v1/auth/login` - User login

#### Protected Endpoints (Auth Required)
- `POST /api/v1/auth/logout` - Logout
- `POST /api/v1/auth/refresh` - Refresh token
- `GET /api/v1/auth/me` - Current user info

#### Service Proxy Endpoints
All backend services are accessed through API Gateway:

##### Communication Service (comsrv) - Port 8001
```bash
# Base path: /api/v1/comsrv/*

# Get all channels
GET /api/v1/comsrv/api/channels

# Get channel status
GET /api/v1/comsrv/api/channels/{id}/status

# Control channel (start/stop/restart)
POST /api/v1/comsrv/api/channels/{id}/control
Body: {"action": "start|stop|restart"}

# Read point value
GET /api/v1/comsrv/api/channels/{channel_id}/points/{point_table}/{point_name}

# Write point value
POST /api/v1/comsrv/api/channels/{channel_id}/points/{point_table}/{point_name}
Body: {"value": 123.45}

# Get all points for a channel
GET /api/v1/comsrv/api/channels/{channel_id}/points

# Get telemetry table view
GET /api/v1/comsrv/api/channels/{channel_id}/telemetry_tables
```

##### Model Service (modsrv) - Port 8002
```bash
# Base path: /api/v1/modsrv/*

# Rules management
GET    /api/v1/modsrv/rules              # List all rules
GET    /api/v1/modsrv/rules/{id}         # Get rule details
POST   /api/v1/modsrv/rules              # Create rule
PUT    /api/v1/modsrv/rules/{id}         # Update rule
DELETE /api/v1/modsrv/rules/{id}         # Delete rule
POST   /api/v1/modsrv/rules/{id}/execute # Execute rule

# Templates and instances
GET  /api/v1/modsrv/templates            # List templates
GET  /api/v1/modsrv/templates/{id}       # Get template
POST /api/v1/modsrv/instances            # Create instance

# Operations
GET  /api/v1/modsrv/operations           # List operations
POST /api/v1/modsrv/operations/{id}/control  # Control operation
POST /api/v1/modsrv/operations/{id}/execute  # Execute operation
```

##### Historical Service (hissrv) - Port 8003
```bash
# Base path: /api/v1/hissrv/*

# Query historical data
GET /api/v1/hissrv/history/query
Query params: source_id, start_time, end_time, limit

# Data sources
GET /api/v1/hissrv/history/sources
GET /api/v1/hissrv/history/sources/{source_id}

# Statistics and export
GET  /api/v1/hissrv/history/statistics
POST /api/v1/hissrv/history/export
GET  /api/v1/hissrv/history/export/{job_id}

# Admin
GET /api/v1/hissrv/admin/config
GET /api/v1/hissrv/admin/storage-stats
```

##### Network Service (netsrv) - Port 8004
```bash
# Base path: /api/v1/netsrv/*

# Configuration management
GET  /api/v1/netsrv/config                    # Get current config
PUT  /api/v1/netsrv/config                    # Update config
POST /api/v1/netsrv/config/validate           # Validate config
POST /api/v1/netsrv/config/reload             # Reload config

# Version control
GET    /api/v1/netsrv/config/versions         # List versions
GET    /api/v1/netsrv/config/versions/{ver}   # Get version
POST   /api/v1/netsrv/config/versions/{ver}   # Save version
DELETE /api/v1/netsrv/config/versions/{ver}   # Delete version
POST   /api/v1/netsrv/config/rollback/{ver}   # Rollback to version

# Export and optimization
GET  /api/v1/netsrv/config/export             # Export config
POST /api/v1/netsrv/config/aws/optimize       # AWS optimization
```

##### Alarm Service (alarmsrv) - Port 8005
```bash
# Base path: /api/v1/alarmsrv/*

# Alarm management
GET  /api/v1/alarmsrv/alarms                 # List alarms
POST /api/v1/alarmsrv/alarms                 # Create alarm
POST /api/v1/alarmsrv/alarms/{id}/ack        # Acknowledge alarm
POST /api/v1/alarmsrv/alarms/{id}/resolve    # Resolve alarm

# Classification and statistics
POST /api/v1/alarmsrv/alarms/classify        # Classify alarms
GET  /api/v1/alarmsrv/alarms/categories      # Get categories
GET  /api/v1/alarmsrv/status                 # Service status
GET  /api/v1/alarmsrv/stats                  # Statistics
```

### API Gateway Configuration

Configuration file: `config/apigateway.yml`

```yaml
server:
  host: "0.0.0.0"
  port: 8080
  workers: 4

redis:
  url: "redis://localhost:6379"
  pool_size: 10
  timeout_seconds: 5

services:
  comsrv:
    url: "http://localhost:8001"
    timeout_seconds: 30
  modsrv:
    url: "http://localhost:8002"
    timeout_seconds: 30
  hissrv:
    url: "http://localhost:8003"
    timeout_seconds: 30
  netsrv:
    url: "http://localhost:8004"
    timeout_seconds: 30
  alarmsrv:
    url: "http://localhost:8005"
    timeout_seconds: 30

cors:
  allowed_origins:
    - "http://localhost:8082"  # Frontend
    - "http://localhost:3000"  # Dev server
    - "http://localhost:5173"  # Vite dev
  allowed_methods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"]
  allowed_headers: ["Content-Type", "Authorization"]
  max_age: 3600

logging:
  level: "info"
  format: "json"
```

Environment variable override: `APIGATEWAY_SERVER_PORT=8080`