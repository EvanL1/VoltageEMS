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
├── config/                 # Unified configuration directory (2025-08-11)
│   ├── comsrv/            # comsrv configs and CSV files
│   ├── modsrv/            # modsrv configs
│   ├── alarmsrv/          # alarmsrv configs
│   ├── rulesrv/           # rulesrv configs
│   ├── hissrv/            # hissrv configs
│   ├── apigateway/        # apigateway configs
│   └── netsrv/            # netsrv configs
├── logs/                   # Unified log directory (2025-08-11)
│   ├── comsrv/            # comsrv logs
│   ├── modsrv/            # modsrv logs
│   └── ...                # other service logs
├── scripts/
│   ├── redis-functions/   # Lua functions for Redis
│   ├── quick-check.sh     # Run format, clippy, and compile checks
│   ├── dev.sh            # Development environment setup
│   └── validate-comsrv-config.sh  # Validate CSV configurations
├── apps/                   # Frontend applications
└── docker-compose.yml      # Container orchestration
```

## Development Commands

### Quick Quality Check
```bash
# Run format, clippy, and compile checks in one command
./scripts/quick-check.sh
```

### Building & Running
```bash
# Development environment setup
./scripts/dev.sh

# Build all services
cargo build --workspace

# Run specific service with debug logging
RUST_LOG=debug,comsrv=trace cargo run --bin comsrv

# Run with specific config
cargo run --bin comsrv -- -c config/comsrv/comsrv.yaml
```

### Testing
```bash
# Run all unit tests
cargo test --workspace

# Run specific service tests with output
cargo test -p comsrv -- --nocapture

# Run single test
cargo test test_plugin_manager_initialization -- --nocapture

# Docker integration tests
./scripts/test-docker.sh test
```

### Code Quality
```bash
# Format all code
cargo fmt --all

# Run clippy with strict checks
cargo clippy --all-targets --all-features -- -D warnings

# Check compilation
cargo check --workspace
```

### Docker Operations

#### Optimized Build Strategy (2025-08-11)
```bash
# Build base dependency image first (caches all dependencies)
docker build -f Dockerfile.base -t voltageems-dependencies:latest .

# Build all service images using cached dependencies
docker-compose build --parallel

# Start all services
docker-compose up -d

# View service logs
docker logs -f voltageems-comsrv
```

#### Docker Image Optimization Results
- Base dependency image: ~2.42GB (contains all pre-compiled dependencies)
- Service images: 85-88MB each (very lean)
- Build time: ~1 minute per service (vs several minutes before)
- Strategy: Multi-stage builds with dependency layer caching

### Redis Functions
```bash
# Load all Lua functions to Redis
cd scripts/redis-functions
./load_functions.sh

# Validate comsrv CSV configurations
./scripts/validate-comsrv-config.sh services/comsrv/config
```

## Architecture & Key Concepts

### Service Architecture
- **Hybrid Design**: Rust services handle I/O and protocol communication, Redis Lua functions handle business logic for ultra-low latency
- **Simplified Services**: Most services (modsrv, alarmsrv, rulesrv, hissrv) use single-file main.rs architecture
- **Nginx Entry Point**: All external traffic goes through Nginx on port 80, which routes to individual services

### Communication Service (comsrv)
- Core service handling industrial protocols (Modbus TCP/RTU, Virtual, gRPC)
- Plugin-based protocol architecture in `services/comsrv/src/plugins/`
- CSV-based point table configuration for 四遥 (telemetry, signal, control, adjustment)
- Redis storage pattern: `comsrv:{channel_id}:{type}` where type is T/S/C/A

### Redis Data Patterns
```
comsrv:1001:T         # Telemetry data for channel 1001
comsrv:1001:S         # Signal data
comsrv:1001:C         # Control data
comsrv:1001:A         # Adjustment data
```

### Configuration System

#### Directory Structure (Unified since 2025-08-11)
```
config/
├── comsrv/
│   ├── comsrv.yaml       # Main service config
│   ├── telemetry.csv     # Point definitions
│   ├── signal.csv        # Signal definitions
│   └── mapping/          # Protocol mappings
├── modsrv/
│   └── modsrv.yaml       # Model service config
├── alarmsrv/
│   └── alarmsrv.yaml     # Alarm service config
└── ...                   # Other services

logs/
├── comsrv/               # Service logs
├── modsrv/               # Automatically created
└── ...                   # When services run
```

#### Features
- YAML configuration files for each service
- Environment variable override support
- CSV files for point tables and protocol mappings
- Smart defaults to minimize configuration
- Volume mapping in docker-compose.yml for easy management

### Service Ports
| Service | Port | Purpose |
|---------|------|---------|
| nginx | 80 | Unified entry point |
| comsrv | 6000 | Communication protocols |
| modsrv | 6001 | Model calculations |
| alarmsrv | 6002 | Alarm monitoring |
| rulesrv | 6003 | Rule engine |
| hissrv | 6004 | Historical data |
| apigateway | 6005 | API aggregation |
| netsrv | 6006 | External comms |
| redis | 6379 | Data storage |

## Common Development Tasks

### Configuration Best Practices
1. **Infrastructure config** (ports, Redis URL) → Use environment variables
2. **Business config** → Use YAML files in `config/` directory
3. **Config files are optional** → Services work with defaults
4. **Partial config is supported** → Only override what you need
5. **Priority order**: Default values < Environment variables < YAML files (highest)

### Adding a New Protocol
1. Create plugin in `services/comsrv/src/plugins/`
2. Implement `Protocol` trait from `core::combase`
3. Register in `ProtocolFactory`
4. Add configuration support
5. Note: Slave IDs are in CSV mapping files, not channel config

### Modifying Redis Lua Functions
1. Edit functions in `scripts/redis-functions/`
2. Run `./load_functions.sh` to reload
3. Test with `redis-cli FCALL function_name`

### CSV Configuration for comsrv
Point tables (`telemetry.csv`, `signal.csv`, etc.):
```csv
point_id,signal_name,scale,offset,unit,reverse,data_type
1,Temperature,0.1,0,°C,false,float32
```

Mapping files (`telemetry_mapping.csv`, etc.):
```csv
point_id,slave_id,function_code,register_address,data_type,byte_order
1,1,3,100,float32,ABCD
```

**Important**: Slave ID is defined in the CSV mapping files, NOT in the channel configuration. Each point can have its own slave_id in the mapping file.

## Testing Patterns

### Unit Test with Redis Mock
```rust
#[cfg(test)]
mod tests {
    use voltage_libs::redis::MockRedisClient;

    #[tokio::test]
    async fn test_with_redis() {
        let redis = MockRedisClient::new();
        // Test implementation
    }
}
```

### Integration Test with Docker
```bash
# Start test environment
docker-compose -f docker-compose.test.yml up -d

# Run integration tests
cargo test --features integration

# Cleanup
docker-compose -f docker-compose.test.yml down
```

## Performance Optimization

### Recent Improvements
- Lua functions reduced by ~40% for better performance
- Service code simplified to single-file architecture where appropriate
- Docker images optimized with multi-stage builds
- Removed unnecessary abstractions and dependencies

### Best Practices
- Use Redis Lua functions for business logic (near-zero latency)
- Batch Redis operations when possible
- Use connection pooling for all external connections
- Profile with `cargo flamegraph` for performance analysis

## Data Flow Validation

### Quick Data Flow Test
```bash
# 1. Start infrastructure services
docker-compose up -d redis influxdb modbus-sim

# 2. Load Redis Lua functions
for lua in scripts/redis-functions/*.lua; do
    cat "$lua" | docker exec -i voltageems-redis redis-cli -x FUNCTION LOAD REPLACE
done

# 3. Start comsrv (may need configuration adjustment)
docker-compose up -d comsrv

# 4. Check data in Redis
docker exec voltageems-redis redis-cli KEYS "comsrv:*"
docker exec voltageems-redis redis-cli HGETALL "comsrv:1001:T"
```

### Known Issues
- Some services have axum routing syntax issues (`:id` should be `{id}`)
- Services need proper CSV configuration files in the expected paths
- Redis Lua functions may need adjustment for proper data initialization

## Troubleshooting

### Common Issues

**Redis connection failed**
```bash
# Check Redis is running
redis-cli ping

# Check Redis URL in environment
echo $REDIS_URL
```

**CSV configuration not loading**
```bash
# Validate CSV files
./scripts/validate-comsrv-config.sh config/comsrv

# Check CSV_BASE_PATH environment variable
echo $CSV_BASE_PATH
```

**Service not starting**
```bash
# Check with debug logging
RUST_LOG=debug cargo run --bin service_name

# Check port availability
lsof -i :6000  # Replace with service port
```

## Development Principles

- When modifying code, do not consider compatibility, change directly.
- Keep services lightweight and focused
- Delegate business logic to Redis Lua functions
- Use environment variables for configuration override
- Maintain comprehensive logging for debugging
