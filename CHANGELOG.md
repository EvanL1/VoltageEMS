# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-12-04

### 🎉 First Stable Release

This is the first stable release of VoltageEMS, an Industrial Energy Management System designed for edge computing environments.

### Core Services (Rust)

#### comsrv (Communication Service) - Port 6001
- **Protocol Support**
  - Modbus TCP client with configurable polling intervals
  - Modbus RTU over serial port (RS-485/RS-232)
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
- Historical data storage with InfluxDB 2.x
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
- **InfluxDB 2** - Time-series database
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

| voltage-rtdb | 0.1.0 | Redis abstraction layer |
| voltage-routing | 0.1.0 | M2C routing shared library |
| voltage-model | 0.1.0 | Model calculation library |
| voltage-rules | 0.1.0 | Rule engine library |
| voltage-schema-macro | 0.1.0 | Schema derivation macros |
| common | 0.1.0 | Shared utilities |

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
- CAN bus protocol support
- Lua scripting for custom calculations
- Enhanced rule engine with state machines
- Multi-tenant support
- Cloud synchronization
