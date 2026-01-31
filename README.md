# VoltageEMS

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.90%2B-orange.svg)](https://www.rust-lang.org/)

[中文版本](README-CN.md) | [Documentation](docs/README.md)

Industrial IoT energy management system built with Rust. Real-time data collection, processing, and monitoring for industrial energy scenarios.

## Architecture

```
Device (Modbus/Virtual/gRPC) → comsrv(:6001) → Redis(:6379) → modsrv(:6002)
```

| Service | Port | Description |
|---------|------|-------------|
| comsrv | 6001 | Communication - industrial protocols |
| modsrv | 6002 | Model service + rule engine |
| Redis | 6379 | Real-time data store |

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

## Monarch CLI

Monarch is the configuration management tool for VoltageEMS.

```bash
# Initialize database
monarch init

# Sync YAML/CSV config to SQLite
monarch sync
monarch sync comsrv    # Sync specific service
monarch sync --dry-run # Preview changes

# Service management
monarch services start
monarch services stop
monarch services status

# System health check
monarch doctor

# Channel management
monarch channels list
monarch channels status 1001

# Help
monarch --help
monarch <command> --help
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `VOLTAGE_REDIS_URL` | Redis connection | `redis://localhost:6379` |
| `VOLTAGE_COMSRV_URL` | Comsrv URL | `http://localhost:6001` |
| `VOLTAGE_MODSRV_URL` | Modsrv URL | `http://localhost:6002` |
| `VOLTAGE_CONFIG_PATH` | Config directory | Auto-detect |
| `VOLTAGE_DATA_PATH` | Data directory | Auto-detect |

## Project Structure

```
VoltageEMS/
├── services/comsrv/     # Communication service
├── services/modsrv/     # Model service + rules
├── tools/monarch/       # CLI tool
├── libs/                # Shared libraries
├── apps/                # Vue.js frontend
├── config/              # YAML/CSV config
└── docs/                # Documentation
```

## Development

```bash
# Build
cargo build --workspace

# Test
cargo test --workspace

# Quick check (fmt + clippy + test)
./scripts/quick-check.sh
```

## License

MIT License - see [LICENSE](LICENSE)

## Links

- Repository: [https://github.com/EvanL1/VoltageEMS](https://github.com/EvanL1/VoltageEMS)
- Issues: [https://github.com/EvanL1/VoltageEMS/issues](https://github.com/EvanL1/VoltageEMS/issues)
