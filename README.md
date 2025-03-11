# VoltageEMS - Industrial IoT Energy Management System

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![Docker](https://img.shields.io/badge/docker-ready-blue.svg)](https://www.docker.com/)

[中文版本](README-CN.md)

VoltageEMS is a high-performance industrial IoT energy management system built with Rust microservices architecture. It provides real-time data collection, processing, and monitoring capabilities for industrial energy management scenarios.

## 🚀 Features

- **High Performance**: Built with Rust for optimal performance and memory safety
- **Centralized Configuration**: All configuration constants and queries managed in `voltage-config` library
- **Web Dashboard**: Vue.js frontend with real-time data visualization
- **Hybrid Architecture**: Rust services for I/O, Redis Lua functions for business logic
- **Real-time Data Flow**: Automatic data routing from devices to models via Redis mappings
- **Multi-Protocol Support**: Modbus TCP/RTU, Virtual, gRPC, CAN bus with plugin system
- **Hardware Access**: Support for serial ports and CAN bus for industrial control
- **Model-based System**: Instance-based data modeling with hierarchical products
- **Zero-polling Design**: Event-driven data flow using Redis Lua functions
- **RESTful APIs**: Standard HTTP/JSON interfaces for all services
- **Docker Ready**: Fully containerized deployment with hardware access support
- **CLI Tools**: Comprehensive command-line tools including Monarch for configuration management

## 🏗️ Architecture

### System Architecture
```
                ┌─────────────┐
                │   Devices   │ (Modbus, Virtual, gRPC, CAN, GPIO)
                └──────┬──────┘
                       │
                ┌──────▼──────┐
                │   Frontend  │ (Vue.js Web Application)
                └──────┬──────┘
                       │
       ┌───────────────┴───────────────────────────┐
       │                                           │
       ▼                                           ▼
                                          ┌──────────────────┐
                                          │   Microservices  │
                                          │                  │
                                          │ comsrv(:6001)    │
                                          │ modsrv(:6002)    │
                                          │ rulesrv(:6003)   │
                                          └──────────────────┘
                                                 │
                                                 ▼
                                    ┌───────────────────────────────┐
                                    │ Redis(:6379)                  │
                                    └───────────────────────────────┘
```

### Data Flow Architecture
```
Device → comsrv → Redis Hash → Lua Function → modsrv instance
        (Plugin)  (comsrv:ch:T) (Auto Route)  (Real-time)
                                 ↓
                           route:c2m mapping
                           (Channel→Instance)
```

## 📦 Services

| Service | Port | Description |
|---------|------|-------------|
| **comsrv** | 6001 | Communication service - handles industrial protocols |
| **modsrv** | 6002 | Model service - manages data models and calculations |
| **rulesrv** | 6003 | Rule engine - executes business rules |
| **redis** | 6379 | In-memory data store & Lua functions |

Note: The provided docker-compose runs comsrv/modsrv/rulesrv plus Redis by default.

## 🛠️ Technology Stack

- **Language**: Rust 1.85+
- **Web Framework**: Axum
- **Database**: Redis 8+, InfluxDB 2.x
- **Container**: Docker, Docker Compose
- **Message Format**: JSON, Protocol Buffers
- **Build Tool**: Cargo

## 🚦 Quick Start

### Prerequisites

- Rust 1.85+ ([Install Rust](https://rustup.rs/))
- Docker & Docker Compose
- Redis 8+ (for development)

### Development Setup

1. Clone the repository:
```bash
git clone https://github.com/your-org/VoltageEMS.git
cd VoltageEMS
```

2. Start development environment:
```bash
./scripts/dev.sh
```

3. Load Redis Lua functions (Critical for data flow):
```bash
cd scripts/redis-functions && ./load_functions.sh
```

4. Run a specific service:
```bash
RUST_LOG=debug cargo run --bin comsrv
```

### Docker Deployment

1. Build and start all services:
```bash
# Build Docker image
docker build -t voltageems:latest .

# Start all services (will build automatically)
docker compose up -d

# Check service status
docker compose ps
```

2. Verify data flow:
```bash
# Check logs
docker compose logs -f comsrv modsrv

# Test data flow
docker exec voltageems-redis redis-cli FCALL comsrv_batch_update 0 "1001" "T" '{"1":100}'

# Check mapped data (runtime storage uses hash)
docker exec voltageems-redis redis-cli HGET "modsrv:pv_inverter_01:M" "1"
```

### Frontend Application

The web dashboard is built with Vue 3 + TypeScript + Vite:

1. Install dependencies:
```bash
cd apps
npm install
```

2. Development mode:
```bash
npm run dev
# Frontend will be available at http://localhost:5173
```

3. Build for production:
```bash
npm run build
# Output will be in apps/dist/
```

4. Frontend Features:
- **Real-time Monitoring**: Live data from PV, battery, diesel generators
- **Alarm Management**: Real-time alarm display and historical records
- **User Management**: Role-based access control
- **Data Visualization**: Charts and graphs using ECharts
- **Responsive Design**: Works on desktop and mobile devices

## 📝 Configuration

### Configuration Sources & Priority
- **Priority Order (highest to lowest)**:
  1. Configuration files (YAML/SQLite) - Takes precedence when explicitly configured
  2. Environment variables - Used as fallback for unspecified/default values
  3. Default values - Built-in defaults when nothing is configured
  
- Services load runtime configuration primarily from a unified SQLite database:
  - `VOLTAGE_DB_PATH` (default `data/voltage.db`) - Unified database for all services

- **Deprecated environment variables** (no longer used):
  - `COMSRV_DB_PATH`, `MODSRV_DB_PATH`, `RULESRV_DB_PATH` - Replaced by `VOLTAGE_DB_PATH`
  
- **Note**: Environment variables only override when configuration uses default values. For example:
  - If SQLite has `port=6001` (default), ENV can override
  - If SQLite has `port=7001` (non-default), it takes precedence over ENV
  
- YAML under `config/` serves as reference and is synced to SQLite via Monarch tool.

### Service Configuration (YAML)
```yaml
# config/comsrv/comsrv.yaml
channels:
  - id: 1001
    name: "pv_inverter_channel"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      timeout_secs: 5
      polling_interval_ms: 1000
      
  - id: 1002
    name: "pcs_channel"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.101"
      port: 502
      polling_interval_ms: 1000
```

### Channel Data Structure
```
config/comsrv/
├── comsrv.yaml                     # Channel definitions
├── {channel_id}/                    # e.g., 1001
│   ├── telemetry.csv               # T-type point definitions
│   ├── signal.csv                  # S-type point definitions
│   ├── control.csv                 # C-type point definitions
│   ├── adjustment.csv              # A-type point definitions
│   └── mapping/
│       ├── telemetry_mapping.csv   # Protocol mappings for T points
│       ├── signal_mapping.csv      # Protocol mappings for S points
│       ├── control_mapping.csv     # Protocol mappings for C points
│       └── adjustment_mapping.csv  # Protocol mappings for A points
```

### Point Definition Example (CSV)
```csv
# config/comsrv/1001/telemetry.csv
point_id,signal_name,scale,offset,unit,reverse,data_type
1,DC_Voltage,0.1,0,V,false,float32
2,DC_Current,0.01,0,A,false,float32
```

### Protocol Mapping Example (CSV)
```csv
# config/comsrv/1001/mapping/telemetry_mapping.csv
point_id,slave_id,function_code,register_address,data_type,byte_order
1,1,3,0,float32,ABCD
2,1,3,2,float32,ABCD
```

### Instance Configuration (YAML)
```yaml
# config/modsrv/instances.yaml
instances:
  pv_inverter_01:
    product_name: pv_inverter
    config:
      rated_power: 100.0
      efficiency: 0.98
```

### Channel-Instance Mapping (CSV)
```csv
# config/modsrv/instances/pv_inverter_01/channel_mappings.csv
channel_id,channel_type,channel_point_id,instance_type,instance_point_id,description
1001,T,1,M,1,DC Voltage Input
1001,T,2,M,2,DC Current Input
```

Note:
- Runtime enum for 四遥: FourRemote (T/S/C/A; IEC aliases: YC/YX/YK/YT)
- JSON APIs use field name `four_remote`（例如 modsrv 路由相关 API）。
- CSV/DB 列名保持 `channel_type`（单字母）。

## 🔧 Development

### Project Structure

```
VoltageEMS/
├── services/           # Microservices
│   ├── comsrv/        # Communication service (plugin architecture)
│   ├── modsrv/        # Model service (single-file architecture)
│   └── rulesrv/       # Rule engine (single-file architecture)
├── tools/             # CLI tools
│   └── monarch/       # Configuration management (YAML/CSV ↔ SQLite)
├── libs/              # Shared libraries
│   ├── common/        # Common utilities
│   └── voltage-config/ # Centralized configuration (SQL, Redis keys, tables)
├── scripts/           # Utility scripts
│   ├── redis-functions/  # Redis Lua functions
│   ├── dev.sh            # Development environment setup
│   └── quick-check.sh    # Pre-commit checks
├── config/            # Configuration files
│   ├── comsrv/        # Communication service configs
│   └── modsrv/        # Model service configs
│       └── instances/ # Instance mapping configs
├── docker/            # Docker related files
└── docker-compose.yml # Service orchestration
```

### Building

```bash
# Check compilation
cargo check --workspace

# Build all services
cargo build --workspace

# Run tests
cargo test --workspace

# Format code
cargo fmt --all

# Run clippy
cargo clippy --all-targets --all-features -- -D warnings
```

### Testing

```bash
# Quick quality check (format, clippy, compile)
./scripts/quick-check.sh

# Run all unit tests
cargo test --workspace

# Run specific service tests
cargo test -p comsrv

# Run with output
cargo test -- --nocapture

# Integration testing
./scripts/test-all-services.sh      # Test all services
./scripts/test-docker.sh test        # Docker integration test
./scripts/test-lua-functions.sh      # Test Redis Lua functions
```

### Offline Build (ARM64)

- CLI (tools/*) ARM64 MUSL binaries
  - Requirements: `zig`, `rustup target add aarch64-unknown-linux-musl`, `cargo install cargo-zigbuild`
  - Build: `scripts/offline/build_cli_arm64.sh`
  - Output: `offline-bundle/cli/linux-aarch64/bin/`

- Docker images (linux/arm64) via buildx
  - Requirements: Docker buildx and binfmt
  - Build: `VERSION=arm64-v1 scripts/offline/build_images_arm64.sh`
  - Output: `offline-bundle/docker/images/*.tar`

### Maintenance Scripts

```bash
# Clean up deprecated Redis meta structures
# Use after migrating to point-level timestamps and raw values
./scripts/cleanup-meta-structure.sh

# This removes old comsrv:{channel}:meta keys that were replaced by:
# - comsrv:{channel}:{type}:ts for point-level timestamps
# - comsrv:{channel}:{type}:raw for raw values
```

### Data Structures (ComSrv)
- Keys and Types
  - `comsrv:{channel}:{type}` (Hash)
    - Engineering values after scaling; field=`{point_id}`, value=`{string}` (formatted with 6 decimal places)
    - `{type}` ∈ `T`(telemetry), `S`(signal), `C`(control), `A`(adjustment)
  - `comsrv:{channel}:{type}:ts` (Hash)
    - Point-level timestamps; field=`{point_id}`, value=`{unix_timestamp}` (milliseconds since epoch)
  - `comsrv:{channel}:{type}:raw` (Hash, optional)
    - Raw values before scaling; field=`{point_id}`, value=`{string}`
  - `comsrv:{channel}:{C|A}:TODO` (List, FIFO)
    - Pending command queue (RPUSH enqueue, BLPOP consume)
    - Item JSON includes: `command_id`, `channel_id`, `command_type` (C/A), `point_id`, `value`, `timestamp`, `source` (optional `priority`)

- Data Flow
  - Ingestion: `comsrv_batch_update(channel, T|S, updates_json, [raw_values_json])`
    - Batch HSET `comsrv:{channel}:{T|S}` → engineering values
    - Batch HSET `comsrv:{channel}:{T|S}:ts` → timestamps
    - Batch HSET `comsrv:{channel}:{T|S}:raw` → raw values (if provided)
    - Route to ModSrv via `route:c2m` mapping
  - Query: `GET /api/channels/{channel}/{type}/{point_id}`
    - Returns JSON with value, timestamp, and raw value (REST endpoint backed by Rust)
  - Commands: HTTP `POST /api/channels/{channel_id}/points/{point_id}/{control|adjustment}` or internal producers
    - HSET `comsrv:{channel}:{C|A}` (latest state) → RPUSH `comsrv:{channel}:{C|A}:TODO`
    - Protocol layer consumes via BLPOP to execute on device

- Mapping Index (maintained by ModSrv, used by ComSrv routing)
  - `route:c2m` (Hash): `comsrv:{channel}:{type}:{point}` → `modsrv:{instance_name}:{M|A}:{point}`
  - `route:m2c` (Hash): `modsrv:{instance_name}:{M|A}:{point}` → `comsrv:{channel}:{C|A}:{point}`

- Examples
  - Point write: `HSET comsrv:1001:T "1" "230.5"`
  - Command enqueue: `RPUSH comsrv:1001:A:TODO '{"point_id":7,"value":12.3,"timestamp":...}'`

### Data Structures (ModSrv)
- Mapping Index (single source of truth at runtime)
  - `route:c2m` (Hash): `comsrv:{channel}:{type}:{point}` → `modsrv:{instance_name}:{M|A}:{point}`
  - `route:m2c` (Hash): `modsrv:{instance_name}:{M|A}:{point}` → `comsrv:{channel}:{C|A}:{point}`

- Instance Directory (management/display)
  - `instance:index` (Set): all instance names
  - `instance:{instance_name}:info` (Hash): `id`, `product_name`, `properties`(JSON), `created_at`, `updated_at`
  - `instance:{instance_name}:parameters` (Hash): runtime parameters (k→v)
  - `instance:{instance_name}:mappings` (Hash, optional): `M:{pid}`/`A:{pid}` → Redis key

- Product Directory (read-only cache)
  - `modsrv:products` (Set)
  - `modsrv:product:{pid}` (Hash): `definition`(JSON), `updated_at`
  - `modsrv:product:{pid}:measurements|actions|properties` (Hash): definitions

- Runtime State
  - `modsrv:{instance_name}:M` (Hash): measurements (field=`{point_id}`, value=`{string}`)
  - `modsrv:{instance_name}:A` (Hash): action target values (for visibility)
  - `modsrv:{instance_name}:status` (Hash): `state`, `last_update`, `health`, `errors`
  - `modsrv:{instance_name}:config` (Hash): config cache copied from properties
  - Stats: `modsrv:stats:routed` (Hash): routed counts by `channel_id` (diagnostic)

- Actions (instance → device command)
  - Entry: ModSrv API (`modsrv_execute_action`) or RuleSrv instance action
  - Path: write `modsrv:{instance_name}:A` → lookup `route:m2c` → `RPUSH comsrv:{channel}:{C|A}:TODO`

- Example
  - `FCALL modsrv_execute_action 0 "pv_inv_01" '{"action_id":"7","value":1}'`

### Data Structures (RuleSrv)
- Rule definitions persist in SQLite `rules` table (`id`, `name`, `description`, `flow_json`, `enabled`, `priority`, timestamps).
- Rule management uses REST endpoints (`/api/rules/*`) for list/create/update/delete/enable/disable; direct FCALLs are no longer available.
- Runtime field references still follow ModSrv syntax: `{instance}.{M|A}.{point}` with aggregates `SUM/AVG/MAX/MIN/COUNT(...)`.


## 📊 API Documentation

All services expose RESTful APIs. Here are some common endpoints:

### Health Check
```bash
GET /health
```

### Communication Service (comsrv)
```bash
# Get all channels
GET /api/channels

# Get channel status
GET /api/channels/:id/status
```

### Model Service (modsrv)
```bash
# Apply model
POST /api/models/apply
{
  "model_id": "energy_calc",
  "inputs": {...}
}
```

## 🎯 Key Features & Improvements

### Real-time Data Flow (2025-09)
- **Automatic Data Routing**: comsrv_batch_update Lua function auto-routes data to modsrv
- **Instance-based Modeling**: Meaningful instance names loaded from instances.yaml
- **Zero-polling Architecture**: Event-driven data flow via Redis mappings
- **Channel-to-Instance Mapping**: CSV-based configuration for flexible data routing

### Performance Optimizations
- **Hybrid Processing**: Rust for I/O, Redis Lua for business logic (µs latency)
- **Single-file Services**: Simplified architecture for modsrv, rulesrv
- **Direct Redis Operations**: Eliminated unnecessary abstractions
- **Optimized Docker Build**: Unified image with all services (~20% smaller)

## 🔍 Monitoring

### Logs
```bash
# View service logs
docker logs -f voltageems-comsrv

# With debug level
RUST_LOG=debug cargo run --bin comsrv
```

### Redis Monitoring
```bash
# Monitor Redis activity
redis-cli monitor | grep comsrv

# Check data
redis-cli hgetall "comsrv:1001:T"
```

## 🤝 Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Web framework: [Axum](https://github.com/tokio-rs/axum)
- In-memory database: [Redis](https://redis.io/)
- Time-series database: [InfluxDB](https://www.influxdata.com/)

## 📞 Contact

- Project Link: [https://github.com/your-org/VoltageEMS](https://github.com/your-org/VoltageEMS)
- Issues: [https://github.com/your-org/VoltageEMS/issues](https://github.com/your-org/VoltageEMS/issues)
