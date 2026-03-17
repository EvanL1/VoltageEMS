# VoltageEMS

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.90%2B-orange.svg)](https://www.rust-lang.org/)

[中文版本](README-CN.md) | [Documentation](docs/README.md)

Industrial IoT energy management system built with Rust. Multi-protocol data acquisition, real-time processing via shared memory, rule engine execution, and full-stack monitoring for industrial energy scenarios.

## Features

- **Multi-Protocol Support** — Modbus TCP/RTU, IEC 60870-5-104, OPC UA, MQTT, HTTP, DL/T 645, CAN, J1939, GPIO
- **Zero-Copy Shared Memory** — High-performance data path between services via `/dev/shm`, bypassing serialization overhead
- **Rule Engine** — Visual rule editing (Vue Flow) with real-time execution, expression evaluation, and scheduling
- **Time-Series Integration** — InfluxDB 3.x for historical data persistence and trend analysis
- **Full-Stack Visualization** — Vue.js 3 + ECharts dashboard with real-time WebSocket updates

## Architecture

```
                        ┌─────────────────────────────────────────────┐
                        │              voltage-redis(:6379)           │
                        │          Real-time Data Store + Routing     │
                        └──────┬──────────────────────┬───────────────┘
                               │                      │
  Devices ─────► comsrv(:6001) ┤                      ├─► modsrv(:6002)
   Modbus        Communication │    SHM + UDS         │   Rules / Calc
   IEC104        & Collection  ◄──────────────────────┘   Instances
   OPC UA                      │
   MQTT/HTTP                   │
   DL645/CAN          apigateway(:6005) ──── apps(:8080)
   J1939/GPIO            API Gateway          Vue.js Frontend
                               │
                       hissrv(:6004) ◄── InfluxDB(:8181)
                       Historical Data       Time-Series DB
                               │
                     alarmsrv(:6007)    netsrv(:6006)
                     Alarm Management   MQTT Networking
```

### Service Ports

| Service | Port | Language | Description |
|---------|------|----------|-------------|
| comsrv | 6001 | Rust | Communication service — industrial protocol drivers, channel management |
| modsrv | 6002 | Rust | Model service — product definitions, device instances, rule engine |
| hissrv | 6004 | Python | Historical data service — InfluxDB 3.x time-series persistence |
| apigateway | 6005 | Python | API gateway — unified REST API, WebSocket, JWT auth |
| netsrv | 6006 | Python | Network service — MQTT broker integration |
| alarmsrv | 6007 | Python | Alarm service — alarm rules and notifications |
| apps | 8080 | Vue.js | Frontend — ECharts dashboards, Vue Flow rule editor |
| voltage-redis | 6379 | — | Real-time data store and message routing |
| InfluxDB | 8181 | — | Time-series database for historical data |

## Quick Start

### Prerequisites

- Rust 1.90+ | Docker & Docker Compose | Redis 8+

### Development

```bash
# Clone
git clone https://github.com/EvanL1/VoltageEMS.git
cd VoltageEMS

# Build Monarch CLI
cargo build --release -p monarch

# Initialize and sync config
./target/release/monarch init
./target/release/monarch sync

# Start services
./target/release/monarch services start

# Check system health
./target/release/monarch doctor
```

### Docker Deployment

```bash
docker compose up -d
docker compose ps
```

## Project Structure

```
VoltageEMS/
├── services/
│   ├── comsrv/              # Communication service (Rust)
│   ├── modsrv/              # Model service + rules (Rust)
│   └── python-services/
│       ├── hissrv/          # Historical data (Python/FastAPI)
│       ├── apigateway/      # API gateway (Python/FastAPI)
│       ├── netsrv/          # MQTT networking (Python/FastAPI)
│       └── alarmsrv/        # Alarm management (Python/FastAPI)
├── libs/                    # 13 shared Rust libraries
├── tools/
│   ├── monarch/             # CLI config & service manager
│   └── simulator/           # Modbus TCP/RTU slave simulator
├── apps/                    # Vue.js 3 frontend (Element Plus + ECharts)
├── firmware/                # Embedded firmware prototype (ARM/STM32)
├── config/                  # YAML/CSV configuration
└── docs/                    # Documentation
```

## Libraries

### Core

| Library | Description |
|---------|-------------|
| voltage-core | Core types and codecs — `no_std` compatible for embedded firmware |
| voltage-model | Model layer — calculations, product definitions, instance management |
| voltage-infra | Infrastructure — Redis and SQLite integration |
| common | Service bootstrap, config management, and shared utilities |
| errors | Unified error types across all services |

### Data Layer

| Library | Description |
|---------|-------------|
| voltage-rtdb | Real-time database abstraction — Redis and in-memory backends |
| voltage-rtdb-shm | Shared memory RTDB implementation — zero-copy data sharing |
| voltage-shm | Platform-agnostic shared memory readers/writers |
| voltage-routing | Data flow routing — comsrv ↔ modsrv message routing |

### Extensions

| Library | Description |
|---------|-------------|
| voltage-calc | Expression evaluation engine with built-in functions |
| voltage-rules | Rule engine — Vue Flow rule parsing, execution, and scheduling |
| voltage-sim | Waveform generator for device simulation |
| voltage-schema-macro | Proc macro — auto-generates SQL DDL from Rust structs |

## Data Flow

### Upstream (Device → Cloud)

```
Device → comsrv → Redis (route:c2m) → modsrv
                   channel data         rule execution
                   "comsrv:{ch_id}:T"   instance calc
```

### Downstream (Cloud → Device)

```
Primary: modsrv → SHM + UDS notify → comsrv ShmCommandListener → Device
```

The primary path uses shared memory (`/dev/shm/voltage-rtdb.shm`) with Unix Domain Socket notifications for minimal latency. UDS reconnects automatically with exponential backoff (1-30s) if comsrv restarts.

## Monarch CLI

Monarch is the unified management tool for VoltageEMS — configuration, monitoring, and remote operations across all services.

### Installation

```bash
# One-line install (Linux / macOS / WSL)
curl -fsSL https://raw.githubusercontent.com/EvanL1/VoltageEMS/develop/tools/monarch/install.sh | bash

# Via Bun/npm (cross-platform including Windows)
bun install -g @voltage/monarch

# From source
cargo install --path tools/monarch
```

### Configuration Management

```bash
monarch init                   # Initialize database
monarch sync                   # Sync YAML/CSV config to SQLite
monarch sync --dry-run         # Preview changes
monarch export                 # Export config from database
monarch status                 # Show config status
```

### Service Operations

```bash
monarch channels list          # List all channels
monarch channels status 1      # Channel runtime status
monarch models instances list  # List device instances
monarch rules list             # List business rules
monarch templates list         # List channel templates
monarch templates snapshot 1   # Snapshot channel as template
monarch templates apply 1 2    # Apply template to channel
```

### Remote Management

```bash
# Target a remote machine (all service ports auto-resolved)
monarch --host 192.168.30.21 channels list
monarch --host 192.168.30.21 models instances list
monarch --host 192.168.30.21 rules list
monarch --host 192.168.30.21 logs level all debug
```

### Interactive TUI Dashboard

```bash
monarch top                              # Local monitoring
monarch --host 192.168.30.21 top         # Remote monitoring

# Navigation:
#   ←→ / Tab    Switch views (Channels / Instances / Rules)
#   ↑↓ / j/k    Navigate within list
#   Enter        Drill into detail (points, live data, routing)
#   Esc          Back to parent view
#   z            Toggle hide zero values
#   r            Force refresh
#   q            Quit
```

### JSON Output (for AI Agents & Scripts)

```bash
# All commands support --json for structured output
monarch --json channels list
monarch --json --host 192.168.30.21 rtdb scan "inst:*"

# Or set environment variable
export MONARCH_JSON=1
monarch channels list          # Outputs JSON automatically
```

### Infrastructure

```bash
monarch services start         # Start Docker services
monarch services status        # Check service status
monarch doctor                 # Full system health check
monarch logs level all debug   # Dynamic log level adjustment
monarch rtdb scan "inst:*"     # Direct Redis operations
monarch shm top                # Local shared memory TUI
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `VOLTAGE_REDIS_URL` | Redis connection | `redis://localhost:6379` |
| `VOLTAGE_COMSRV_URL` | Comsrv URL | `http://localhost:6001` |
| `VOLTAGE_MODSRV_URL` | Modsrv URL | `http://localhost:6002` |
| `VOLTAGE_CONFIG_PATH` | Config directory | Auto-detect |
| `VOLTAGE_DATA_PATH` | Data directory | Auto-detect |
| `MONARCH_JSON` | Force JSON output | — |

## Development

```bash
# Build
cargo build --workspace

# Test
cargo test --workspace

# Quick check (fmt + clippy + test + frontend)
./scripts/quick-check.sh
```

## License

MIT License - see [LICENSE](LICENSE)

## Links

- Repository: [https://github.com/EvanL1/VoltageEMS](https://github.com/EvanL1/VoltageEMS)
- Issues: [https://github.com/EvanL1/VoltageEMS/issues](https://github.com/EvanL1/VoltageEMS/issues)
