# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Development Commands

### Workspace-Level Commands

```bash
# Format all code
cargo fmt --all

# Run clippy linting on all services
cargo clippy --all-targets --all-features -- -D warnings

# Build entire workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Run specific service tests
cargo test -p {service_name}

# Build in release mode
cargo build --release --workspace

# Run local CI checks
./scripts/local-ci.sh

# Run all services locally
./scripts/run-all.sh start
./scripts/run-all.sh stop
./scripts/run-all.sh status
```

### Service-Specific Commands

```bash
# Build and run individual service
cd services/{service_name}
cargo build
cargo run

# Run with specific log level
RUST_LOG=debug cargo run
RUST_LOG={service_name}=debug cargo run

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name -- --exact --nocapture

# Watch for changes and auto-rebuild
cargo watch -x run
```

### Redis Operations

```bash
# Start Redis for development
docker run -d --name redis-dev -p 6379:6379 redis:7-alpine

# Monitor Redis activity
redis-cli monitor | grep {service_name}

# Check specific keys
redis-cli keys "point:*" | head -20
redis-cli hgetall "point:1"
```

## Architecture Overview

VoltageEMS is a Rust-based microservices architecture for industrial IoT energy management. The system uses Redis as a central message bus and data store, with each service handling specific responsibilities.

### Service Communication Pattern

All services communicate exclusively through Redis pub/sub and key-value storage:
- No direct service-to-service HTTP calls
- Real-time data flows through Redis channels
- State persistence in Redis with optional InfluxDB for historical data

### Core Services

**comsrv** - Industrial Protocol Gateway
- Manages all device communication (Modbus, CAN, IEC60870)
- Plugin architecture for protocol extensibility
- Unified transport layer supporting TCP, Serial, CAN, GPIO
- Publishes telemetry to Redis: `point:{id}` keys
- Subscribes to control commands: `cmd:*` channels

**modsrv** - Computation Engine
- Executes DAG-based calculation workflows
- Subscribes to telemetry updates from Redis
- Publishes calculated values back to Redis
- No longer uses hybrid_store or memory_store - Redis only

**hissrv** - Historical Data Service
- Bridges Redis real-time data to InfluxDB
- Batch writes for performance
- Manages data retention policies
- Provides query API for historical data

**netsrv** - Cloud Gateway
- Forwards data to external systems (AWS IoT, Alibaba Cloud)
- Protocol transformation (MQTT, HTTP)
- Configurable data formatting and filtering
- Retry logic for reliability

**alarmsrv** - Alarm Management
- Real-time alarm detection and classification
- Stores alarm state in Redis
- Manages alarm lifecycle and notifications

**apigateway** - REST API Gateway
- Single entry point for frontend
- JWT authentication
- Routes requests to appropriate services via Redis
- Note: Uses actix-web while other services use axum

### Shared Libraries

**voltage-common** (`libs/voltage-common`)
- Unified error handling
- Redis client wrapper (async/sync)
- Logging configuration
- Common data types
- Metrics collection

### Key Design Patterns

1. **Protocol Plugin System** (comsrv)
   - Each protocol implements `ProtocolPlugin` trait
   - Transport abstraction allows mock testing
   - Configuration via YAML + CSV point tables

2. **Point Management**
   - Points identified by u32 IDs for performance
   - Multi-level indexing for O(1) lookups
   - Point data includes value, quality, timestamp

3. **Configuration Hierarchy**
   - Figment-based configuration merging
   - Environment variables override files
   - CSV files for point mappings

4. **Logging Architecture**
   - Service-level and channel-level configuration
   - Daily rotation with retention policies
   - Separate log files per channel

## Protocol Address Format

Modbus addresses use colon-separated format: `slave_id:function_code:register_address`

Example parsing:
```rust
let parts: Vec<&str> = address.split(':').collect();
let slave_id = parts[0].parse::<u8>()?;
let function_code = parts[1].parse::<u8>()?;
let register = parts[2].parse::<u16>()?;
```

## Development Workflow

1. Create feature branch from `develop`
2. Make changes and test locally
3. Run `./scripts/local-ci.sh` before committing
4. Update `docs/fixlog/fixlog_{date}.md` with changes
5. Create PR to `develop` branch

## Testing Infrastructure

### Unit Tests
- Mock transports for protocol testing
- Test utilities in `voltage-common::test_utils`
- Use `#[tokio::test]` for async tests

### Integration Tests
```bash
# Start test infrastructure
./scripts/start-test-servers.sh

# Run integration tests
cargo test --features integration

# Clean up
./scripts/stop-test-servers.sh
```

### Protocol Simulators
- `tests/modbus_server_simulator.py` - Modbus TCP server
- Supports all point types (YC/YX/YK/YT)
- Generates realistic test data

## Common Issues and Solutions

### Platform-Specific Dependencies
- `rppal` (Raspberry Pi GPIO) is Linux-only
- `socketcan` requires Linux for CAN support
- Use feature flags to conditionally compile

### Redis Connection
- Services require Redis on localhost:6379
- Use Docker for local development
- Check connectivity: `redis-cli ping`

### Build Warnings
- config-framework temporarily excluded from workspace
- Some dead_code warnings are expected
- Use `#[allow(dead_code)]` sparingly

## Configuration Files

### Service Configuration
```yaml
# services/{service}/config/default.yml
service:
  name: "comsrv"
  redis:
    url: "redis://localhost:6379"
  logging:
    level: "info"
    file: "logs/comsrv.log"
```

### Channel Configuration
```yaml
# Point to CSV files
csv_base_path: "./config"
channels:
  - id: 1
    protocol_type: "modbus_tcp"
    points_config:
      base_path: "ModbusTCP_Test_01"
```

### CSV Point Tables
Located in `config/{Protocol}_Test_{ID}/`:
- `telemetry.csv` - Measurements (YC)
- `signal.csv` - Status signals (YX)  
- `control.csv` - Commands (YK)
- `adjustment.csv` - Setpoints (YT)

## Local CI Tools

The project includes several CI tools:
- **Earthly** - Container-based builds (needs fixes)
- **Lefthook** - Git hooks for pre-commit checks
- **Act** - Run GitHub Actions locally
- **local-ci.sh** - Bash script for all checks

Install with:
```bash
brew install earthly/earthly/earthly lefthook act
```