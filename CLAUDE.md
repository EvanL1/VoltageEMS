# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Development Commands

### Workspace-Level Commands

```bash
# Check compilation without building (preferred over cargo build)
cargo check --workspace

# Format all code
cargo fmt --all

# Run clippy linting on all services
cargo clippy --all-targets --all-features -- -D warnings

# Build entire workspace (only when necessary)
cargo build --workspace

# Run all tests
cargo test --workspace

# Run specific service tests
cargo test -p {service_name}

# Build in release mode
cargo build --release --workspace
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

### Docker Development Environment

```bash
# Start complete test environment for a service
cd services/{service_name}
docker-compose -f docker-compose.test.yml up -d

# Monitor service logs
docker-compose -f docker-compose.test.yml logs -f {service_name}

# Stop test environment
docker-compose -f docker-compose.test.yml down

# Run complete integration tests
./scripts/run-integration-tests.sh
```

### Redis Operations

```bash
# Start Redis for development
docker run -d --name redis-dev -p 6379:6379 redis:7-alpine

# Monitor Redis activity
redis-cli monitor | grep {service_name}

# Check Hash data
redis-cli hgetall "comsrv:1001:m"      # View all measurements for channel 1001
redis-cli hget "comsrv:1001:m" "10001" # Get single point value
redis-cli hlen "comsrv:1001:m"         # Count points in channel

# Monitor Pub/Sub
redis-cli psubscribe "comsrv:*"        # Monitor all comsrv channels
redis-cli subscribe "comsrv:1001:m"    # Monitor specific channel
```

### Python Scripts (使用uv环境)

```bash
# Run Python scripts in uv environment
uv run python scripts/script_name.py

# Install dependencies
uv pip install -r requirements.txt
```

## Architecture Overview

VoltageEMS is a Rust-based microservices architecture for industrial IoT energy management. The system uses Redis as a central message bus and data store, with each service handling specific responsibilities.

```
┌─────────────────────────────────────────────────────────────┐
│                      Web Application                        │
│            Web UI | Mobile App | HMI/SCADA                  │
└─────────────────────┬───────────────────────────────────────┘
                          │
                   ┌──────┴──────┐
                   │ API Gateway │
                   └──────┬──────┘
                          │
┌─────────────────────────┴───────────────────────────────────┐
│                    Redis Message Bus                        │
│              Pub/Sub | Key-Value | Streams                  │
└──┬──────────┬────────┬─────────┬──────────┬──────────┬──────┘
   │          │        │         │          │          │
┌──┴───┐  ┌───┴──┐  ┌──┴───┐  ┌──┴───┐  ┌───┴────┐  ┌──┴──┐
│comsrv│  │modsrv│  │hissrv│  │netsrv│  │alarmsrv│  │ ... │
└──┬───┘  └──────┘  └──────┘  └──────┘  └────────┘  └─────┘
   │
┌──┴──────────────────────────────┐
│            Devices              │
│   Modbus | IEC60870 | CAN | ... │
└─────────────────────────────────┘
```

## Redis Data Architecture (v3.2)

### Hash Storage Format
```
comsrv:{channelID}:{type}   # type: m(measurement), s(signal), c(control), a(adjustment)
modsrv:{modelname}:{type}   # type: measurement, control
alarm:{alarmID}             # Alarm data
rulesrv:rule:{ruleID}       # Rule definitions
```

### Data Standards
- **Float Precision**: 6 decimal places (e.g., "25.123456")
- **Hash Access**: O(1) field queries
- **Batch Operations**: HGETALL, HMGET for efficiency
- **No Quality Field**: Data quality removed from all structures

### Pub/Sub Channels
```
comsrv:{channelID}:{type}   # Message format: "{pointID}:{value:.6f}"
modsrv:{modelname}:{type}   # Calculation results
cmd:{channelID}:control     # Control commands
cmd:{channelID}:adjustment  # Adjustment commands
```

## Core Services

### comsrv - Industrial Protocol Gateway
- Manages all device communication (Modbus, CAN, IEC60870)
- Plugin architecture for protocol extensibility
- Publishes data to Redis Hash: `comsrv:{channelID}:{type}`
- Subscribes to control commands: `cmd:{channelID}:control`
- Command subscription handled at framework level, not in protocol plugins

### modsrv - Device Model Engine
- Executes DAG-based calculation workflows
- Device model system (DeviceModel)
- Real-time data flow processing
- Built-in functions: sum, avg, min, max, scale
- Stores results in Hash: `modsrv:{modelname}:{type}`

### hissrv - Historical Data Service
- Bridges Redis real-time data to InfluxDB
- Batch writes for performance
- Manages data retention policies
- No quality field processing

### netsrv - Cloud Gateway
- Forwards data to external systems (AWS IoT, Alibaba Cloud)
- Protocol transformation (MQTT, HTTP)
- Configurable data formatting and filtering

### alarmsrv - Alarm Management
- Real-time alarm detection and classification
- Stores alarm state in Redis
- Manages alarm lifecycle and notifications
- Subscribes to modsrv calculation results

### rulesrv - Rule Engine
- DAG-based rule definitions (JSON)
- Reads modsrv Redis keys
- Executes control actions
- Manages rule scheduling

### apigateway - REST API Gateway
- Single entry point for frontend
- JWT authentication
- Routes requests to appropriate services via Redis
- WebSocket support for real-time data
- Uses axum framework (unified across all services)

## Shared Libraries

### voltage-libs
- `voltage_libs::types`: StandardFloat, PointData
- `voltage_libs::error`: Unified error handling
- `voltage_libs::redis`: Redis client wrapper
- Common logging and metrics

## Key Design Patterns

### 1. Hash Storage Architecture
- Real-time data: `comsrv:{channelID}:{type}` → Hash{pointID: value}
- Configuration: `cfg:{channelID}:{type}:{pointID}`
- O(1) access performance, supports millions of points
- 30%+ memory savings vs string keys

### 2. Protocol Plugin System (comsrv)
- Each protocol implements `ProtocolPlugin` trait
- Transport abstraction for testing
- YAML configuration + CSV point tables
- Framework handles command subscription

### 3. Data Type Standards
```rust
use voltage_libs::types::{StandardFloat, PointData};

// All float values use 6 decimal precision
let value = StandardFloat::new(25.123456);
let point = PointData::new(value);

// Redis storage
let redis_value = point.to_redis_value();  // "25.123456"
```

### 4. Configuration
- Figment-based configuration merging
- Environment variables override files
- CSV files for point mappings
- Service-specific YAML configs

## Development Workflow

1. Check compilation with `cargo check --workspace`
2. Create worktree branch from `develop`
3. Make changes and test locally
4. Update `docs/fixlog/fixlog_{date}.md` with changes
5. Create PR to `develop` branch
6. No Claude-related information in git commits

## Testing

### Unit Tests
```bash
cargo test --workspace
cargo test -p {service_name}
```

### Integration Tests
```bash
# Docker-based testing
cd services/{service_name}
docker-compose -f docker-compose.test.yml up -d
docker-compose -f docker-compose.test.yml exec test-runner cargo test
```

### Performance Testing
```bash
# Benchmarks (e.g., modsrv)
cargo bench -p modsrv
cargo bench -p modsrv -- --quick
```

## Common Issues and Solutions

### Platform-Specific Dependencies
- `rppal` (Raspberry Pi GPIO) is Linux-only
- `socketcan` requires Linux for CAN support
- macOS M3: Cannot compile Linux-specific features locally
- Use Docker for cross-platform testing

### Redis Connection
- Services require Redis on localhost:6379
- Use Docker: `docker run -d -p 6379:6379 redis:7-alpine`
- Check connectivity: `redis-cli ping`

### Build Errors
- netsrv workspace error: Temporary, being fixed
- Use `cargo check` instead of `cargo build` for verification
- Some warnings expected (deprecated functions, unused imports)

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
# services/{service}/config/channels.yml
channels:
  - id: 1001
    name: "ModbusTCP Channel 1001"
    protocol_type: "modbus_tcp"
    enabled: true
    table_config:
      # Four telemetry point tables path
      four_telemetry_route: "ModbusTCP_CH1001"
      four_telemetry_files:
        measurement_file: "measurement.csv"    # YC - Telemetry measurements
        signal_file: "signal.csv"              # YX - Status signals
        adjustment_file: "adjustment.csv"      # YT - Adjustment setpoints
        control_file: "control.csv"            # YK - Control commands

      # Protocol mapping path
      protocol_mapping_route: "ModbusTCP_CH1001/mappings"
      protocol_mapping_file:
        measurement_mapping: "modbus_measurement.csv"
        signal_mapping: "modbus_signal.csv"
        adjustment_mapping: "modbus_adjustment.csv"
        control_mapping: "modbus_control.csv"
```

### CSV Point Tables

#### Four Telemetry Tables (in config/{Protocol}_CH{ChannelID}/)

**measurement.csv** (YC - Telemetry):
```csv
point_id,signal_name,data_type,scale,offset,unit,description
10001,voltage_a,float,0.1,0,V,Phase A voltage
10002,current_a,float,0.01,0,A,Phase A current
10003,power_active,float,1.0,0,kW,Active power
10004,power_reactive,float,1.0,0,kVar,Reactive power
```

**signal.csv** (YX - Status):
```csv
point_id,signal_name,data_type,scale,offset,unit,description
20001,breaker_status,bool,1.0,0,,Breaker open/close status
20002,fault_alarm,bool,1.0,0,,Fault alarm signal
20003,communication_ok,bool,1.0,0,,Communication status
```

**adjustment.csv** (YT - Adjustment):
```csv
point_id,signal_name,data_type,scale,offset,unit,description
40001,voltage_setpoint,float,0.1,0,V,Voltage setpoint
40002,power_limit,float,1.0,0,kW,Power limit setpoint
```

**control.csv** (YK - Control):
```csv
point_id,signal_name,data_type,scale,offset,unit,description
30001,breaker_control,bool,1.0,0,,Breaker open/close control
30002,reset_alarm,bool,1.0,0,,Reset alarm command
```

#### Protocol Mapping Tables (in config/{Protocol}_CH{ChannelID}/mappings/)

**modbus_measurement.csv**:
```csv
point_id,slave_id,function_code,register_address,data_type,byte_order
10001,1,3,0,float32,ABCD
10002,1,3,2,float32,ABCD
10003,1,3,4,float32,ABCD
10004,1,3,6,float32,ABCD
```

**modbus_signal.csv**:
```csv
point_id,slave_id,function_code,register_address,bit_position
20001,1,2,0,0
20002,1,2,0,1
20003,1,2,0,2
```

## Performance Optimization

### Redis Operations
- Use Hash structures for O(1) access
- Batch operations with pipeline
- Minimize key count (thousands vs millions)
- 6 decimal precision for all floats

### Service Design
- Async/await throughout
- Connection pooling
- Efficient serialization (bincode where applicable)
- Minimal data copying

## Monitoring

### Logs
- Structured logging with tracing
- Service and channel level configuration
- Daily rotation with retention

### Metrics
- Prometheus-compatible metrics
- Service health endpoints
- Redis operation counters

### Health Checks
```bash
curl http://localhost:8001/health  # comsrv
curl http://localhost:8080/health  # apigateway
```

## Security Notes

- JWT tokens for API authentication
- Redis ACL for service isolation (when configured)
- No secrets in code or logs
- Environment variables for sensitive config

## Critical Reminders

1. **Always use `cargo check` before building**
2. **Update fixlog for all changes**
3. **No Claude references in commits**
4. **Use uv for Python, never system Python**
5. **Test with Docker for platform compatibility**
6. **All services use axum framework**
7. **Hash storage for all real-time data**
8. **6 decimal precision for all numeric values**
