# Monarch CLI

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

Unified management tool for [VoltageEMS](https://github.com/EvanL1/VoltageEMS) — configuration, monitoring, and remote operations for industrial energy management systems.

## Installation

### Bun / npm (recommended, cross-platform)

```bash
bun install -g @voltage/monarch
# or
npm install -g @voltage/monarch
```

### GitHub Releases

Download pre-built binaries from [Releases](https://github.com/EvanL1/VoltageEMS/releases):

```bash
# Linux / macOS
curl -fsSL https://github.com/EvanL1/VoltageEMS/releases/latest/download/monarch-$(uname -s)-$(uname -m).tar.gz | tar xz -C /usr/local/bin/

# Windows (PowerShell)
Invoke-WebRequest -Uri "https://github.com/EvanL1/VoltageEMS/releases/latest/download/monarch-windows-x86_64.zip" -OutFile monarch.zip
Expand-Archive monarch.zip -DestinationPath $env:USERPROFILE\.local\bin
```

### From Source

```bash
cargo install --path tools/monarch
```

## Quick Start

```bash
# Check system health
monarch doctor

# Local operations
monarch channels list
monarch models instances list
monarch rules list

# Remote machine
monarch --host 192.168.30.21 channels list

# Interactive dashboard
monarch --host 192.168.30.21 top
```

## Commands

### Configuration

| Command | Description |
|---------|-------------|
| `monarch init` | Initialize SQLite database schema |
| `monarch sync` | Sync YAML/CSV config to database |
| `monarch sync --dry-run` | Validate config without writing |
| `monarch export` | Export config from database to files |
| `monarch status` | Show configuration status |
| `monarch doctor` | Full system health check |

### Channels (comsrv)

| Command | Description |
|---------|-------------|
| `monarch channels list` | List all communication channels |
| `monarch channels status <id>` | Channel runtime status and statistics |
| `monarch channels control <id> <pt> <0\|1>` | Send binary control command |
| `monarch channels adjust <id> <pt> <value>` | Send float setpoint |
| `monarch channels reload` | Hot-reload channel configuration |
| `monarch channels health` | Service health check |

### Templates (comsrv)

| Command | Description |
|---------|-------------|
| `monarch templates list` | List channel configuration templates |
| `monarch templates get <id>` | Template details |
| `monarch templates snapshot <ch_id>` | Snapshot channel as reusable template |
| `monarch templates apply <tpl_id> <ch_id>` | Apply template to target channel |
| `monarch templates delete <id>` | Delete template |

### Models (modsrv)

| Command | Description |
|---------|-------------|
| `monarch models products list` | List built-in product types |
| `monarch models instances list` | List device instances |
| `monarch models instances create <product> <name>` | Create device instance |
| `monarch models instances get <name>` | Instance details |
| `monarch models instances delete <name>` | Delete instance |

### Rules (modsrv)

| Command | Description |
|---------|-------------|
| `monarch rules list` | List business rules |
| `monarch rules get <id>` | Rule details with flow definition |
| `monarch rules enable <id>` | Enable rule |
| `monarch rules disable <id>` | Disable rule |
| `monarch rules test <id>` | Test rule conditions without executing |
| `monarch rules execute <id>` | Execute rule manually |

### RTDB (Redis)

| Command | Description |
|---------|-------------|
| `monarch rtdb get <key>` | Get Redis value |
| `monarch rtdb set <key> <value>` | Set Redis value |
| `monarch rtdb scan <pattern>` | Scan keys by glob pattern |
| `monarch rtdb inspect <key>` | Inspect key type and content |
| `monarch rtdb del <key>` | Delete key(s) |
| `monarch rtdb patterns` | Show VoltageEMS key patterns reference |

### Infrastructure

| Command | Description |
|---------|-------------|
| `monarch services start` | Start Docker services |
| `monarch services stop` | Stop services |
| `monarch services status` | Service status |
| `monarch services logs <svc>` | View service logs |
| `monarch logs level <svc> <level>` | Dynamic log level adjustment |
| `monarch shm top` | Local shared memory TUI monitor |

### Interactive Dashboard

```bash
monarch top                          # Local
monarch --host 192.168.30.21 top    # Remote
```

| Key | Action |
|-----|--------|
| `←` `→` / `Tab` | Switch views (Channels / Instances / Rules) |
| `↑` `↓` / `j` `k` | Navigate within list |
| `Enter` | Drill into detail (points, live data, routing) |
| `Esc` | Back to parent view |
| `1` `2` `3` | Jump to view directly |
| `z` | Toggle hide zero values |
| `r` | Force refresh |
| `q` | Quit |

## Global Flags

| Flag | Description |
|------|-------------|
| `--host <IP>` | Target remote machine (overrides localhost) |
| `--json` | Structured JSON output for scripts and AI agents |
| `--verbose` | Enable debug logging |
| `--no-color` | Disable colored output |
| `--config-path <path>` | Override config directory |
| `--db-path <path>` | Override database directory |

## JSON Output

All commands support `--json` for structured output:

```bash
monarch --json channels list
# {"success": true, "data": [...]}

monarch --json rtdb scan "inst:*"
# {"success": true, "data": {"pattern": "inst:*", "total": 13, "keys": [...]}}
```

Set `MONARCH_JSON=1` to enable JSON by default:

```bash
export MONARCH_JSON=1
monarch channels list    # Outputs JSON without --json flag
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `MONARCH_JSON` | Force JSON output | — |
| `VOLTAGE_REDIS_URL` | Redis connection URL | `redis://localhost:6379` |
| `VOLTAGE_COMSRV_URL` | Comsrv HTTP URL | `http://localhost:6001` |
| `VOLTAGE_MODSRV_URL` | Modsrv HTTP URL | `http://localhost:6002` |
| `VOLTAGE_CONFIG_PATH` | Config directory path | Auto-detect |
| `VOLTAGE_DATA_PATH` | Data directory path | Auto-detect |

## Platform Support

| Platform | Architecture | Status |
|----------|-------------|--------|
| Linux | x86_64, aarch64 | Supported |
| macOS | x86_64, Apple Silicon | Supported |
| Windows | x86_64 | Supported |
| WSL | x86_64 | Supported (use Linux binary) |

## License

MIT — see [LICENSE](../../LICENSE)
