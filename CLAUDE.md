# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

VoltageEMS is a high-performance industrial IoT energy management system built with Rust microservices architecture. It supports multiple industrial protocols (Modbus TCP/RTU, Virtual, gRPC) and real-time data processing through a hybrid architecture combining Rust services with Redis Lua Functions for optimal performance.

## Architecture & Design Patterns

### Core Architecture Decisions
- **Hybrid Processing**: Rust services handle I/O and protocol communication, Redis Lua functions handle business logic for ultra-low latency
- **Service Simplification**: Most services (modsrv, alarmsrv, rulesrv, hissrv) use single-file main.rs architecture - avoid over-engineering
- **Plugin System**: Communication service (comsrv) uses plugin-based protocol architecture in `services/comsrv/src/plugins/`
- **Unified Entry**: All external traffic goes through Nginx on port 80, which routes to individual services

### Redis Data Storage Patterns
```
comsrv:{channel_id}:T         # Telemetry data for channel
comsrv:{channel_id}:S         # Signal data
comsrv:{channel_id}:C         # Control data
comsrv:{channel_id}:A         # Adjustment data
```

### Service Port Allocation
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

# Build release mode
cargo build --release --workspace

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

# Run tests with backtrace for debugging
RUST_BACKTRACE=1 cargo test failing_test_name

# Run ignored tests
cargo test -- --ignored

# Run both regular and ignored tests
cargo test -- --include-ignored
```

### Code Quality
```bash
# Format all code
cargo fmt --all

# Check format without modifying
cargo fmt --all -- --check

# Run clippy with strict checks
cargo clippy --all-targets --all-features -- -D warnings

# Check compilation
cargo check --workspace

# Check specific package
cargo check -p comsrv
```

### Docker Operations

#### Building Images
```bash
# Build base dependency image first (caches all dependencies)
docker build -f Dockerfile.base -t voltageems-dependencies:latest .

# Build all service images using cached dependencies
docker-compose build --parallel

# Build specific service
docker-compose build comsrv
```

#### Running Services
```bash
# Start all services
docker-compose up -d

# Start specific services
docker-compose up -d redis comsrv

# View logs
docker-compose logs -f comsrv

# Stop all services
docker-compose down

# Stop and remove volumes
docker-compose down -v
```

### Redis Lua Functions
```bash
# Load all Lua functions to Redis
cd scripts/redis-functions
./load_functions.sh

# Load functions to Docker Redis
for lua in scripts/redis-functions/*.lua; do
    cat "$lua" | docker exec -i voltageems-redis redis-cli -x FUNCTION LOAD REPLACE
done

# Test a function
redis-cli FCALL function_name 0 arg1 arg2

# List loaded functions
redis-cli FUNCTION LIST
```

## Configuration System

### Configuration Priority (highest to lowest)
1. YAML files in `config/{service}/`
2. Environment variables
3. Default values in code

### Directory Structure
```
config/
├── comsrv/
│   ├── comsrv.yaml       # Main service config
│   ├── telemetry.csv     # Point definitions
│   ├── signal.csv        # Signal definitions
│   └── mapping/          # Protocol mappings
├── modsrv/modsrv.yaml
├── alarmsrv/alarmsrv.yaml
└── ...

logs/                      # Auto-created by services
├── comsrv/
├── modsrv/
└── ...
```

### CSV Configuration for comsrv

Point tables define data points:
```csv
point_id,signal_name,scale,offset,unit,reverse,data_type
1,Temperature,0.1,0,°C,false,float32
```

Mapping files define protocol-specific details:
```csv
point_id,slave_id,function_code,register_address,data_type,byte_order
1,1,3,100,float32,ABCD
```

**Important**: Slave ID is in mapping files, NOT channel configuration.

### Validate CSV Configuration
```bash
./scripts/validate-comsrv-config.sh config/comsrv
```

## Common Development Tasks

### Adding a New Protocol Plugin
1. Create plugin file in `services/comsrv/src/plugins/`
2. Implement `Protocol` trait from `core::combase`
3. Register in `ProtocolFactory::create()` method
4. Add configuration support in channel config
5. Add CSV mapping files if needed

### Modifying Redis Lua Functions
1. Edit function in `scripts/redis-functions/*.lua`
2. Reload: `./scripts/redis-functions/load_functions.sh`
3. Test: `redis-cli FCALL function_name 0 args`
4. Check logs: `redis-cli MONITOR | grep function_name`

### Debugging Service Issues
```bash
# Enable detailed logging
RUST_LOG=debug,service_name=trace cargo run --bin service_name

# Check port availability
lsof -i :6000

# Test Redis connection
redis-cli ping

# Monitor Redis operations
redis-cli MONITOR

# Check service health
curl http://localhost:6000/health
```

### Performance Profiling
```bash
# Generate flamegraph
cargo install flamegraph
cargo flamegraph --bin comsrv

# Run with release optimizations
cargo run --release --bin comsrv

# Benchmark specific code
cargo bench
```

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

### Integration Test Structure
```rust
// tests/integration_test.rs
use voltage_libs::test_utils::setup_test_env;

#[tokio::test]
async fn test_full_flow() {
    let env = setup_test_env().await;
    // Test implementation
}
```

## Project Structure

```
VoltageEMS/
├── Cargo.toml              # Workspace root
├── libs/                   # Shared library (voltage_libs crate)
│   └── src/
│       ├── redis/         # Redis client abstractions
│       ├── config/        # Configuration utilities
│       └── errors/        # Common error types
├── services/              
│   ├── comsrv/           # Complex service with plugins
│   │   └── src/
│   │       ├── main.rs
│   │       ├── plugins/  # Protocol implementations
│   │       ├── core/     # Core abstractions
│   │       └── api/      # HTTP endpoints
│   ├── modsrv/           # Single-file service
│   │   └── src/main.rs
│   └── ...               # Other lightweight services
├── scripts/
│   ├── redis-functions/  # Lua business logic
│   ├── quick-check.sh    # Pre-commit checks
│   ├── dev.sh           # Dev environment
│   └── build-docker.sh  # Docker builds
├── config/              # Service configurations
├── docker/              # Docker files
│   ├── redis/          # Redis with Lua
│   └── modbus-sim/     # Test simulator
└── docker-compose.yml   # Service orchestration
```

## Key Dependencies (Cargo.toml)

- **Async Runtime**: tokio with full features
- **Web Framework**: axum 0.8.4 with tower middleware
- **Serialization**: serde, serde_json, serde_yaml
- **Database**: redis 0.32 with tokio support
- **gRPC**: tonic 0.11, prost 0.12
- **Error Handling**: anyhow, thiserror
- **Logging**: tracing, tracing-subscriber

## Known Issues & Solutions

### Redis Connection Failed
```bash
# Check Redis is running
docker-compose ps redis
redis-cli ping

# Check environment variable
echo $REDIS_URL
# Should be: redis://localhost:6379 or redis://redis:6379 in Docker
```

### CSV Files Not Loading
```bash
# Check CSV_BASE_PATH
echo $CSV_BASE_PATH

# Validate CSV format
./scripts/validate-comsrv-config.sh config/comsrv

# Check file permissions
ls -la config/comsrv/*.csv
```

### Axum Route Syntax
- Use `/{id}` not `/:id` for path parameters in axum 0.8+
- Example: `/api/channels/{id}/status`

### Port Already in Use
```bash
# Find process using port
lsof -i :6000

# Kill process
kill -9 <PID>

# Or use different port
SERVICE_PORT=6100 cargo run --bin service_name
```

## Development Principles

- **Simplicity First**: Avoid over-engineering, use single-file services where possible
- **Performance**: Delegate hot-path logic to Redis Lua functions
- **Compatibility**: When modifying code, change directly without compatibility concerns
- **Logging**: Maintain comprehensive logging for debugging
- **Testing**: Write tests for critical paths and protocol implementations
- **Configuration**: Use environment variables for infrastructure, YAML for business logic