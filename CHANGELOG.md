# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-12-04

### 🎉 First Stable Release

This is the first stable release of VoltageEMS, an Industrial Energy Management System designed for edge computing environments.

### Core Services (Rust)

#### comsrv (Communication Service) - Port 6001
- **Protocol Support** (10 protocols)
  - Modbus TCP/RTU over Ethernet and serial port (RS-485/RS-232)
  - IEC 60870-5-104
  - OPC UA
  - MQTT (publish/subscribe with JSON mapping)
  - HTTP (polling and webhook with JSON mapping)
  - DL/T 645-2007 (smart meter protocol)
  - CAN bus
  - J1939 (vehicle network over CAN)
  - GPIO (digital I/O)
  - Virtual protocol for testing and simulation
- **Point Types (Four-Remote)**
  - Telemetry (T): Real-time analog measurements
  - Signal (S): Digital status indicators
  - Control (C): Digital control commands
  - Adjustment (A): Analog setpoint adjustments
- **Features**
  - Batch data upload to Redis with configurable intervals
  - Hot-reload configuration via REST API (`POST /api/channels/reload`)
  - Swagger UI documentation at `/swagger-ui/`

#### modsrv (Model Service) - Port 6002
- **Product & Instance Management**
  - Hierarchical product definitions with measurement/adjustment points
  - Instance lifecycle management (create, update, delete)
  - Dynamic property configuration per instance
- **Routing Engine**
  - C2M (Channel-to-Model): Device data to instance mapping
  - M2C (Model-to-Channel): Control command routing
  - C2C (Channel-to-Channel): Direct device forwarding
- **Rule Engine**
  - Time-based triggers (cron expressions)
  - Condition-based triggers (OnChange, OnCondition)
  - Vue Flow compatible rule definitions (JSON)
- **Virtual Points**
  - Expression-based calculations using evalexpr
  - Support for arithmetic, logical, and comparison operators
- **API**
  - Full REST API for all operations
  - Swagger UI documentation at `/swagger-ui/`

#### monarch (CLI Tool)
- **Configuration Management**
  - `monarch init <service>` - Initialize database tables
  - `monarch sync <service>` - Sync YAML/CSV to SQLite
  - `monarch status` - Check synchronization status
  - `monarch validate` - Validate configuration files
- **Service Management**
  - `monarch services start` - Start all services
  - `monarch services stop` - Stop services
  - `monarch services restart` - Restart services
  - `monarch services refresh --smart` - Smart refresh (detect image changes)
  - `monarch services logs <service>` - View service logs
  - `monarch services reload` - Hot-reload configuration
- **Routing Commands**
  - `monarch services set-action` - Execute M2C routing
  - `monarch services routing-show` - Display routing table

### Python Services

#### hissrv (History Service) - Port 6004
- Historical data storage with InfluxDB 3.x
- Time-series data aggregation and queries
- REST API for data retrieval

#### apigateway (API Gateway) - Port 6005
- Unified API gateway for all backend services
- WebSocket proxy for real-time updates
- Authentication and authorization

#### netsrv (Network Service) - Port 6006
- MQTT client for cloud connectivity
- HTTP forwarding for external integrations
- Message queue management

#### alarmsrv (Alarm Service) - Port 6007
- Alarm rule configuration and evaluation
- Notification management
- Alarm history and acknowledgment

### Frontend

#### apps (Web Interface) - Port 8080
- Vue.js 3 with TypeScript
- Real-time dashboard with WebSocket updates
- Configuration management UI
- Responsive design for desktop and tablet

### Infrastructure

- **Redis 8** - High-performance data store
  - Real-time point data (Hash)
  - Routing tables (Hash)
  - Control command queues (List)
  - Unix socket support for better performance
- **InfluxDB 3** - Time-series database
  - Historical data storage
  - Configurable retention policies
- **Docker Compose** - Unified orchestration
  - Host network mode for industrial environments
  - Volume mounts for configuration and data
  - Health checks and restart policies

### Configuration System

- **SQLite** - Unified configuration database
  - Single `voltage.db` shared by all services
  - Atomic transactions for configuration updates
- **YAML/CSV Sources**
  - Human-readable configuration files
  - Version control friendly
  - Monarch CLI for synchronization
- **Configuration Hierarchy**
  - Service-specific > Global > Environment variables > Defaults

### Libraries

| Library | Version | Description |
|---------|---------|-------------|
| voltage-core | 0.1.0 | Core types and codecs — no_std compatible for embedded firmware |
| voltage-model | 0.1.0 | Model layer — calculations, product definitions, instance management |
| voltage-routing | 0.1.0 | Data flow routing — comsrv ↔ modsrv message routing |
| voltage-rtdb | 0.1.0 | Real-time database abstraction — Redis and in-memory backends |
| voltage-rtdb-shm | 0.1.0 | Shared memory RTDB — zero-copy data sharing via /dev/shm |
| voltage-shm | 0.1.0 | Platform-agnostic shared memory readers/writers |
| voltage-infra | 0.1.0 | Infrastructure layer — Redis and SQLite integration |
| voltage-calc | 0.1.0 | Expression evaluation engine with built-in functions |
| voltage-rules | 0.1.0 | Rule engine — Vue Flow rule parsing, execution, and scheduling |
| voltage-sim | 0.1.0 | Waveform generator for device simulation |
| voltage-schema-macro | 0.1.0 | Proc macro — auto-generates SQL DDL from Rust structs |
| common | 0.1.0 | Service bootstrap, config management, and shared utilities |
| errors | 0.1.0 | Unified error types across all services |

### Documentation

- Comprehensive README (English and Chinese)
- Architecture documentation
- API reference via Swagger UI
- Configuration guides
- Operations log for knowledge preservation

### Testing

- Unit and integration tests with coverage
- Integration tests with Redis
- Pre-commit hooks for code quality
- CI/CD pipeline with GitHub Actions

---

## Future Releases

Features planned for future versions:
- Lua scripting for custom calculations
- Enhanced rule engine with state machines
- Multi-tenant support
- Cloud synchronization
