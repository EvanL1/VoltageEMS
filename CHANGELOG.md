# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-03-18

### Breaking Changes — Full Rust Migration
- **All services now Rust** — hissrv, apigateway, netsrv, alarmsrv migrated from Python/FastAPI to Rust/Axum
- **Python services removed** — `services/python-services/` directory deleted; all 6 services live under `services/`
- **Pluggable storage replaces InfluxDB** — hissrv now uses a runtime-configurable backend (PostgreSQL / TimescaleDB) via `PUT /hisApi/storage`
- **Unified Docker image** — all Rust services share a single `voltageems:latest` Alpine-based image (105MB total)

### Added
- **monarch**: Remote management CLI with `--host` flag for all commands
- **monarch**: Interactive TUI dashboard (`monarch top`) with local and remote monitoring
- **monarch**: JSON output mode (`--json`) for AI agent and script integration
- **monarch**: Channel template API — snapshot, apply, list templates
- **monarch**: Cross-platform release pipeline (Linux ARM64/AMD64, macOS)
- **apigateway**: JWT authentication, WebSocket proxy, unified REST API (Rust rewrite)
- **hissrv**: Pluggable storage backend (PostgreSQL / TimescaleDB) with runtime configuration via REST API
- **netsrv**: MQTT client with TLS support, device telemetry forwarding
- **alarmsrv**: Alarm rule evaluation, notification management, CSV export

### Refactored
- Net reduction of ~7,000 lines of code despite adding 4 new Rust services
- Purge 23 unused Cargo dependencies + dead functions/types across workspace
- Remove tombstone comments, ghost imports, zombie Redis writes
- Clean up dead VecRtdb, RingBuffer, snapshot_info, health_check code

### Fixed
- CI: multi-arch build fixes, NPM_TOKEN handling, monarch tag filtering
- Tests: fix trailing comma in calculations DDL causing 16 integration test failures
- Remove dead CanMappingConfig exposed by Linux-only can feature gate

## [0.1.11] - 2026-03-12

### Performance
- **shm**: Zero-cost seqlock on x86 — eliminate mfence/lock instructions, use compiler fences only

## [0.1.10] - 2026-03-11

### Fixed
- **modsrv**: Harden M2C dispatch safety — error propagation, stale writer clear, refresh lock
- **modsrv**: Correct SHM refresh ordering, remove dead code
- **modsrv**: Propagate comsrv reload error, remove phantom TODO queue references
- **modsrv**: Address code review — DispatchOutcome observability, SHM consistency
- **modsrv**: Eliminate TOCTOU races, harden error handling and API safety
- **comsrv**: Filter by point_type in Modbus write_control/write_adjustment
- **comsrv**: Use OS-assigned port in integration tests to eliminate flaky port conflicts
- **rtdb-shm**: Fix unsafe soundness in ring buffer and snapshot restore
- **shm**: Harden shared memory safety for ARM64 weak memory model
- **e2e**: Harden Phase 6 routing and Phase 9 reset verification; fix Redis socket permission

### Refactored
- **modsrv**: Replace DispatchOutcome 3-bool struct with enum
- **modsrv**: Extract infra/runtime layers, wire ShmDispatch + ComsrvCoordinator
- **modsrv**: Delete unwired skeletons, wire DynamicSlotRuntime
- **modsrv**: Delete dead ShmCommandPoller; purge all stale M2C fallback/polling references
- **modsrv**: Deduplicate Redis sync via Acquire trait generalization
- **comsrv**: Extract ChannelPollContext to reduce function parameter count

### Added
- **simulator**: Device state machine with Modbus write hooks; CAN/J1939 E2E scenarios
- **simulator**: HTTP state API, CAN LYNK sender, J1939 sender; Modbus protocol readback
- **comsrv/modsrv**: Refactor rtdb-shm notification API; enable Redis Unix socket hot path
- **comsrv**: Watchdog with auto-recovery, heartbeat liveness, and health endpoint
- **comsrv**: JSON mapper with JSONPath extraction and script fallback
- **comsrv**: Add protocol field to GET /api/channels/list response

## [0.1.9] - 2026-02-26

### Added
- **comsrv**: BLE (Bluetooth Low Energy) protocol adapter
- **comsrv**: Zigbee protocol adapter via TCP gateway
- **comsrv**: Matter protocol adapter with UDP transport
- **comsrv**: Channel template API for point-table snapshot and apply
- **modsrv**: Auto-reload services after monarch sync
- **comsrv**: OpenAPI/Swagger docs for template API

### Fixed
- Stabilize flaky seqlock concurrent test for CI coverage runs
- Soften topology hierarchy validation to warn-only for flexible topologies
- CI: install libdbus-1-dev for BLE adapter compilation

## [0.1.8] - 2026-02-26

### Added
- **modsrv**: Enforce instance topology hierarchy with cascade delete and topology API
- Comprehensive architecture documentation and CI hardening

### Fixed
- **comsrv**: Remove panics, unwraps, and reconnect backoff blocking
- **voltage-rtdb-shm**: Seqlock fallback torn read + ringbuffer push guard
- CI: ARM64 native runner; dependency-aware service restart; clippy threshold unification
- Large-scale dead code removal and deduplication across services
- Security, stability, and performance audit remediation; translate remaining Chinese log messages

### Refactored
- **comsrv**: Extract state mapping, cleanup deprecated code, convert macros to functions
- Large-scale simplification — deduplicate, extract, remove dead code

## [0.1.7] - 2026-02-11

### Added
- **comsrv**: Channel online status tracking (real-time heartbeat in Redis)
- **voltage-model**: Runtime ProductLibrary with external JSON overrides
- **voltage-model**: PVInverter product; align all product names
- **modsrv**: Propagate rule_name in execution results; normalize legacy product names
- **modsrv**: Batch_direct and routing_cache integration tests

### Fixed
- **comsrv**: Prevent CAN client deadlock; clarify protocol safety docs
- **modsrv**: Parameterize SQL queries to prevent injection
- **logging**: Enforce log retention and prevent disk overflow
- Pre-existing test failures resolved

### Refactored
- **modsrv**: Replace ghost table SQL with in-memory product lookups
- **comsrv**: Split point_handlers.rs into module directory
- **voltage-rtdb**: Extract SHM zero-dep modules to voltage-rtdb-shm crate
- **voltage-rtdb**: Remove unused Rtdb trait methods
- Move routing_cache to voltage-routing; complete SHM extraction
- **voltage-rtdb-shm**: Simplify ChannelToSlotIndex to slot indices

## [0.1.6] - 2026-01-13

### Refactored
- **comsrv**: Replace u32/4 internal ID encoding with explicit point_type field
- **comsrv**: Implement lock-free diagnostics and Arc\<DataBatch\> optimization
- **comsrv**: Implement lock-free polling with channel-based architecture
- **rules**: Optimize scheduler with Arc\<Rule\> and parallel execution
- Simplify modsrv errors and eliminate syncer.rs duplication

### Fixed
- **gpio**: Use MockGpioDriver in tests for hardware-independent testing
- Clippy: use is_multiple_of() instead of manual modulo check

## [0.1.5] - 2026-01-09

### Added
- **monarch**: shm command with TUI dashboard and SHM iteration API
- **install.sh**: Auto-start and cleanup on install
- **frontend**: Vue.js build and checks integration

### Fixed
- **install**: Improve architecture detection and Docker Compose compatibility
- **build**: Use musl for amd64 to fix Alpine compatibility
- **ci**: Support multi-arch Docker builds with TARGET_TRIPLE ARG
- **test**: Update assertions to match ryu float formatting
- **install**: Correct pattern matching for voltageems-ss image
- **monarch**: Cleanup redundant code and unused dependencies

## [0.1.4] - 2026-01-05

### Added
- DI/DO channel: internal_id conversion and improved logging

### Performance
- Multi-round performance optimization for core libraries (voltage-core, voltage-model, voltage-routing)

## [0.1.3] - 2026-01-04

### Fixed
- **comsrv**: Make start_flush_task async to avoid blocking_write panic
- **comsrv**: Add defensive Drop impl for IgwChannelWrapper; abort background tasks on hot-reload
- Improve type safety and error handling across concurrency paths
- Add safety bounds for Redis retry and integer conversions
- Improve task lifecycle management robustness
- Restore bash syntax after upgrading to bash 5
- Update igw to 0.2.16 (GPIO startup init)

## [0.1.2] - 2026-01-04

### Added
- Multi-arch installer support (ARM64 + AMD64)
- GPIO: use sysfs driver for simpler global numbering

### Fixed
- **comsrv**: Register point types for correct Redis key mapping
- **build**: Fail fast when monarch binary is missing; add --platform linux/arm64 flag
- **ci**: Replace non-existent monarch validate with sync --dry-run
- **rules**: Fail condition evaluation when variable is missing
- Remove dangerous Default implementations and validate critical IDs
- Correct volume mount path for data directory
- Update igw to 0.2.14–0.2.13; remove voltage_modbus and tokio-serial dependencies

### Refactored
- **comsrv**: Replace Chinese log messages with English
- **apigateway**: Unify WebSocket rule message format
- **install**: Change default path to /opt/MonarchEdge; enable auto mode by default
- Unify clippy lints to workspace level; optimize Cargo.toml dependencies

## [0.1.1] - 2025-12-25

### Refactored
- **comsrv**: Simplify point handlers with wrapper pattern

---

## [0.1.0] - 2025-12-04

### First Stable Release

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

### Python Services (migrated to Rust in v0.2.0)

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
