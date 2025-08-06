# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

VoltageEMS is a high-performance industrial IoT energy management system built with Rust microservices architecture. It supports multiple industrial protocols (Modbus TCP/RTU, Virtual, gRPC) and real-time data processing through a hybrid architecture combining Rust services with Redis Lua Functions for optimal performance.

## Workspace Structure

```
VoltageEMS/
├── Cargo.toml              # Workspace root
├── libs/                   # Shared libraries (voltage_libs crate)
├── services/               # Microservices
│   ├── comsrv/            # Communication service (protocols)
│   ├── modsrv/            # Model service (lightweight)
│   ├── alarmsrv/          # Alarm service (lightweight)
│   ├── rulesrv/           # Rule engine (lightweight)
│   ├── hissrv/            # Historical data (lightweight)
│   ├── apigateway/        # API gateway (minimal proxy)
│   └── netsrv/            # Network service
├── scripts/
│   ├── redis-functions/   # Lua functions for Redis
│   ├── quick-check.sh     # Run format, clippy, and compile checks
│   ├── dev.sh            # Development environment setup
│   └── validate-comsrv-config.sh  # Validate CSV configurations
├── apps/                   # Frontend applications
└── docker-compose.yml      # Container orchestration
```

## Architecture

### Service Architecture

The system uses a hybrid architecture combining Rust microservices with Redis Lua Functions:

```
                ┌─────────────┐
                │   Client    │
                └──────┬──────┘
                       │
                ┌──────▼──────┐
                │ Nginx (:80) │ ← Unified entry point, reverse proxy
                └──────┬──────┘
                       │
       ┌───────────────┴───────────────────────────┐
       │                                           │
       ▼                                           ▼
┌─────────────┐                         ┌──────────────────┐
│ API Gateway │                         │   Microservices  │
│   (:6005)   │                         │                  │
│ (Minimal)   │                         │ comsrv(:6000)    │
└─────────────┘                         │ modsrv(:6001)    │
                                        │ hissrv(:6004)    │
                                        │ alarmsrv(:6002)  │
                                        │ rulesrv(:6003)   │
                                        └──────────────────┘
                                                 │
                                                 ▼
                                    ┌─────────────────────────┐
                                    │ Redis(:6379) & Storage  │
                                    │   - Hash Storage        │
                                    │   - Lua Functions       │
                                    └─────────────────────────┘
```

### Hybrid Service Implementation

Services use a dual implementation approach:
1. **Lightweight Services** - Minimal Rust services that handle HTTP APIs and configuration
2. **Redis Lua Functions** - Core business logic executed directly in Redis for maximum performance

| Service | Lightweight Component | Redis Functions |
|---------|---------------------|-----------------|
| modsrv | Model configuration & API | model_* functions for data operations |
| alarmsrv | Alarm configuration & API | store_alarm, acknowledge_alarm, etc. |
| hissrv | History configuration & API | hissrv_* functions for data collection |
| rulesrv | Rule configuration & API | rule_* functions for rule execution |

### Fixed Port Assignments (Hardcoded)

All service ports are hardcoded in the source code and not configurable:
- **Nginx**: 80 (HTTP), 443 (HTTPS)
- **comsrv**: 6000
- **modsrv**: 6001
- **alarmsrv**: 6002
- **rulesrv**: 6003
- **hissrv**: 6004
- **apigateway**: 6005
- **netsrv**: 6006
- **Redis**: 6379

## Environment Settings

### Proxy Configuration for Local Testing
When running curl commands or API tests locally, always disable proxy:
```bash
export NO_PROXY="*"
export no_proxy="*"
unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY ALL_PROXY all_proxy
```

## Key Design Patterns

### 1. Plugin Architecture (comsrv)

The communication service uses a plugin-based architecture for protocol support:

- **ComBase trait** (`services/comsrv/src/core/combase/core.rs`): Core interface all protocols must implement
  - `init()`: Initialize the protocol
  - `read_batch()`: Read multiple points efficiently
  - `write_batch()`: Write multiple points
  - `process_command()`: Handle control commands
  
- **PluginStorage trait**: Abstraction for protocol-specific storage backends
- **Supported protocols**: 
  - Modbus TCP/RTU (`plugins/protocols/modbus/`)
  - Virtual protocol for testing (`plugins/protocols/virtual/`)
  - gRPC (extensible)
- **Factory pattern** (`core/combase/factory.rs`): Dynamic plugin instantiation based on protocol type

### 2. Redis Data Structure
- Hash-based storage for O(1) access: `{service}:{channelID}:{type}`
- Types: T (telemetry), S (signal), C (control), A (adjustment)
- Point IDs start from 1 (sequential numbering)
- 6 decimal precision standardization

### 3. Shared Libraries (libs/)
- Common Redis client with connection pooling
- InfluxDB client for time-series data
- Unified error handling and configuration loading (`ConfigLoader`)
- Shared types across all services

### 4. Redis Functions Architecture
- Located in `scripts/redis-functions/`
- Service-specific functions for each microservice
- Loaded via `load_functions.sh` (replaces load_all_functions.sh)
- Functions handle core business logic for performance

## Common Development Commands

### Quick Checks
```bash
# Run all checks (format, clippy, compilation) - USE THIS BEFORE COMMITTING
./scripts/quick-check.sh

# Development mode with auto-reload
./scripts/dev.sh

# Build all services
./scripts/build.sh

# Run tests
cargo test --workspace
cargo test -p {service_name}
cargo test test_name -- --exact --nocapture

# Run specific test with debug output
RUST_LOG=debug cargo test test_name -- --nocapture
```

### Code Quality
```bash
# Format code
cargo fmt --all

# Lint with clippy (strict mode)
cargo clippy --all-targets --all-features -- -D warnings

# Check compilation without building
cargo check --workspace
```

### Service Development
```bash
# Run individual service
cd services/{service_name}
RUST_LOG=debug cargo run
RUST_LOG={service_name}=trace cargo run

# With specific configuration
cargo run -- -c config/custom.yaml

# Watch mode (requires cargo-watch)
cargo watch -x run
```

### Redis Operations
```bash
# Start Redis
docker run -d --name redis-dev -p 6379:6379 redis:8-alpine

# Load Redis functions (REQUIRED for lightweight services)
cd scripts/redis-functions
./load_functions.sh

# Verify functions loaded
./verify_functions.sh

# Monitor activity
redis-cli monitor | grep {service_name}

# Check data
redis-cli hgetall "comsrv:1001:T"    # View telemetry values
redis-cli hget "comsrv:1001:T" "1"   # Get point ID 1
redis-cli hlen "comsrv:1001:T"       # Count points

# Call Redis function directly
redis-cli FCALL model_upsert 1 "model_001" '{"name":"Test Model"}'
```

### Docker Development
```bash
# Build individual service
cd services/{service_name}
docker build -t {service_name} .

# Start entire system
docker-compose up -d

# Rebuild specific service
docker-compose build {service_name}

# View logs
docker logs -f voltageems-{service_name}

# Clean up Docker resources
docker-compose down -v
docker system prune -f
```

## Configuration System

### Environment Variable Priority
The `ConfigLoader` system provides unified configuration loading with the following priority:
1. Default values (lowest)
2. YAML file configuration
3. Environment variables (highest)

### Service-Specific Environment Variables
Each service supports global and service-specific environment variables:
- Global: `VOLTAGE_REDIS_URL`
- Service-specific: `{SERVICE}_REDIS_URL` (e.g., `COMSRV_REDIS_URL`)

### Comsrv Configuration Structure (Simplified)

Recent refactoring has simplified the configuration structure:

```yaml
csv_base_path: "${CSV_BASE_PATH:-/app/config}"

channels:
  - id: 1001
    name: "modbus_tcp_channel_1"
    protocol: "modbus_tcp"
    enabled: true
    parameters:
      host: "192.168.1.100"
      port: 502
      slave_id: 1  # Default, can be overridden per point in CSV
    polling_config:
      interval_ms: 1000
      batch_size: 100
```

The CSV files are now auto-discovered based on channel ID:
- Point definitions: `{csv_base_path}/channel_{id}/{type}.csv`
- Protocol mappings: `{csv_base_path}/channel_{id}/protocol/{type}_mapping.csv`

### CSV Point Configuration
All telemetry types use unified CSV format:
```csv
point_id,signal_name,scale,offset,unit,reverse,data_type
1,Temperature,1.0,0.0,℃,false,float
2,Status,1.0,0.0,,false,bool
```

## Testing Lightweight Services

When testing lightweight services (modsrv, alarmsrv, hissrv, rulesrv), ensure Redis Functions are loaded:

```bash
# 1. Start Redis
docker run -d --name redis-test -p 6379:6379 redis:8-alpine

# 2. Load functions
cd scripts/redis-functions
./load_functions.sh

# 3. Run the service
cd services/alarmsrv
cargo run -- -c config/alarms.yaml

# 4. Test the API
curl http://localhost:6002/health
curl -X POST http://localhost:6002/alarms \
  -H "Content-Type: application/json" \
  -d '{"title":"Test Alarm","description":"Test","level":"Warning"}'
```

## Development Guidelines

### Critical Rules
- **ALWAYS run `./scripts/quick-check.sh` before committing** - ensures format, clippy, and compilation
- Each service has exactly one Dockerfile at `services/{service_name}/Dockerfile`
- Point IDs start from 1 (sequential numbering)
- All numeric values use 6 decimal precision
- Prefer Hash operations over Keys scanning in Redis
- For bool types in CSV: scale=1.0, offset=0.0
- Redis Functions are loaded once and persist until Redis restart

### Code Style
- Use `cargo check` instead of `cargo build` during development for faster feedback
- Follow Rust naming conventions: snake_case for functions/variables, CamelCase for types
- Use the `?` operator for error propagation instead of unwrap() in production code
- Prefer `tracing` over `println!` for logging

## Troubleshooting

### Common Issues

1. **"Script attempted to access nonexistent global variable"**
   - Ensure Redis Functions are loaded: `./scripts/redis-functions/load_functions.sh`

2. **Port already in use**
   - All ports are hardcoded; check for conflicting services

3. **Service won't start**
   - Check Redis connection: `redis-cli ping`
   - Verify configuration file exists
   - Check logs: `RUST_LOG=debug cargo run`

4. **Clippy failures on macOS**
   - Remove `-fuse-ld=lld` from `.cargo/config.toml` if present

## Comsrv Configuration Rules

When creating or modifying comsrv CSV configuration files, ALWAYS follow these rules:

### CSV File Structure
1. **Four Telemetry Files** (点表定义):
   - `telemetry.csv`, `signal.csv`, `control.csv`, `adjustment.csv`
   - Required columns: `point_id,signal_name,scale,offset,unit,reverse,data_type`

2. **Protocol Mapping Files** (协议映射):
   - `telemetry_mapping.csv`, `signal_mapping.csv`, `control_mapping.csv`, `adjustment_mapping.csv`
   - Required columns: `point_id,slave_id,function_code,register_address,data_type,byte_order`
   - For bool types: add `bit_position` column

### Validation Rules
1. **bit_position**: 
   - Range: 0-15 (16-bit register)
   - Required for bool types in signal/control mappings
   - Default: 0 if not specified

2. **slave_id**: 
   - Defined in mapping CSV files, NOT in YAML
   - Each point can have different slave_id

3. **byte_order**:
   - 16-bit: AB or BA
   - 32-bit: ABCD, DCBA, BADC, CDAB
   - 64-bit: ABCDEFGH, etc.

### Auto-Validation
After creating/modifying comsrv config files, run:
```bash
./scripts/validate-comsrv-config.sh
```

### Minimal YAML Configuration
```yaml
csv_base_path: "/app/config"
channels:
  - id: 1001
    protocol: "modbus_tcp"
    parameters:
      host: "modbus-sim"
      port: 5020
```

Most parameters have defaults:
- `polling_config.interval_ms`: 1000ms
- `enabled`: true
- CSV file paths: auto-discovered based on channel ID

## Recent Changes (Important)

### Configuration Simplification (August 2025)
- Removed complex `table_config` structure - now auto-discovered
- CSV files organized by channel: `channel_{id}/` directory structure  
- Simplified YAML configuration to focus on connection parameters
- Protocol mappings now use consistent naming: `{type}_mapping.csv`

### Current Focus Areas
- Comsrv is the most actively developed service
- Lightweight services (modsrv, alarmsrv, hissrv, rulesrv) delegate to Redis Lua functions
- Test environment configurations have been removed (use Docker for testing)
- Command trigger functionality for write operations via Redis pub/sub