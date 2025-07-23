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
```

### Redis Operations

```bash
# Start Redis for development
docker run -d --name redis-dev -p 6379:6379 redis:7-alpine

# Monitor Redis activity
redis-cli monitor | grep {service_name}

# Check specific keys
redis-cli keys "1001:m:*" | head -20  # 查看通道1001的测量点
redis-cli get "1001:m:10001"          # 获取单个点值
redis-cli keys "cfg:*" | head -20      # 查看配置数据

# Monitor comsrv data publishing
./services/comsrv/scripts/monitor-redis.sh

# Verify data flow
./services/comsrv/scripts/verify-data-flow.sh
```

### Python Scripts (使用uv环境)

```bash
# Run Python scripts in uv environment
uv run python scripts/script_name.py

# Install dependencies
uv pip install -r requirements.txt
```

### Performance Testing

```bash
# Run benchmarks (modsrv example)
cargo bench -p modsrv

# Quick benchmark mode
cargo bench -p modsrv -- --quick
```

## Architecture Overview

VoltageEMS is a Rust-based microservices architecture for industrial IoT energy management. The system uses Redis as a central message bus and data store, with each service handling specific responsibilities.

```
┌─────────────────────────────────────────────────────────────┐
│                      Web Application                        │
│            Web UI | Mobile App │ HMI/SCADA                  │
└─────────────────────┬───────────────────────────────────────┘
                          │
                   ┌──────┴──────┐
                   │ API Gateway │
                   └──────┬──────┘
                          │
┌─────────────────────────┴───────────────────────────────────┐
│                    Redis Message                            │
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
### 重要架构变更 (2025年7月)

系统已从原有的分层哈希存储迁移到**扁平化键值存储架构**：

**旧架构**: `point:1001` → HashMap → 所有点数据
**新架构**: `1001:m:10001` → 单个点值 (直接访问，O(1))

### Service Communication Pattern

All services communicate exclusively through Redis pub/sub and key-value storage:
- No direct service-to-service HTTP calls
- Real-time data flows through Redis channels
- State persistence in Redis with optional InfluxDB for historical data
- 使用Redis直接映射替代HTTP调用

### Core Services

**comsrv** - Industrial Protocol Gateway
- Manages all device communication (Modbus, CAN, IEC60870)
- Plugin architecture for protocol extensibility
- Unified transport layer supporting TCP, Serial, CAN, GPIO
- 发布遥测数据到Redis: `{channelID}:{type}:{pointID}` 格式
- 订阅控制命令: `cmd:{channel_id}:control` 和 `cmd:{channel_id}:adjustment` 通道
- combase框架层处理命令订阅，协议层保持独立
- **Platform-specific features**: `socketcan` (Linux), `rppal` (Linux/RPi), `i2cdev`/`spidev` (industrial I/O)

**modsrv** - Device Model Engine
- Executes DAG-based calculation workflows
- 订阅遥测更新从Redis
- Publishes calculated values back to Redis
- 新增物模型映射系统（device_model模块）
- 支持实时数据流处理和自动计算触发
- Includes performance benchmarks (`cargo bench -p modsrv`)

**rulesrv** - Control Rule Engine
- 通过Json文件定义DAG(有向无环图)从而定义触发规则
- 原则上只对modsrv的redis键进行读取、控制

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
- **Note**: Uses actix-web while other services use axum

### Shared Libraries

**voltage-common** (`libs/voltage-common`)
- Unified error handling
- Redis client wrapper (async/sync)
- Logging configuration
- Common data types (包含PointData结构)
- Metrics collection
- Feature flags: `async` (default), `sync`, `metrics`, `http`, `test-utils`

### Key Design Patterns
1. **扁平化存储架构**
   - 键格式: `{channelID}:{type}:{pointID}` (实时数据)
   - 配置格式: `cfg:{channelID}:{type}:{pointID}` (配置数据)
   - 类型映射: m=测量(YC), s=信号(YX), c=控制(YK), a=调节(YT)
   - 单点查询O(1)，支持百万级点位

2. **Protocol Plugin System** (comsrv)
   - Each protocol implements `ProtocolPlugin` trait
   - Transport abstraction allows mock testing
   - Configuration via YAML + CSV point tables
   - 命令订阅在框架层，不在协议插件层

3. **Point Management**
   - Points identified by u32 IDs for performance
   - 直接键值访问，无需二次哈希
   - Point data includes value, timestamp

4. **Configuration Hierarchy**
   - Figment-based configuration merging
   - Environment variables override files
   - CSV files for point mappings

5. **Logging Architecture**
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

1. Create worktree branch from `develop`
2. Make changes and test locally
3. 更新 `docs/fixlog/fixlog_{date}.md` 记录修改（使用date命令获取日期）
4. Create PR to `develop` branch
5. Git commit时不包含Claude相关信息

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

## Common Issues and Solutions

### Platform-Specific Dependencies
- `rppal` (Raspberry Pi GPIO) is Linux-only
- `socketcan` requires Linux for CAN support
- Use feature flags to conditionally compile
- macOS M3 users: Cannot compile Linux-specific features locally

### Redis Connection
- Services require Redis on localhost:6379
- Use Docker for local development
- Check connectivity: `redis-cli ping`
- modsrv使用RedisHandler包装器处理异步操作
- Redis ACL configured for service authentication (see `services/comsrv/test-configs/redis/users.acl`)

### Build Warnings
- config-framework temporarily excluded from workspace
- Some dead_code warnings are expected
- Use `#[allow(dead_code)]` sparingly

### Error Type Mapping (modsrv)
- CalculationError → ValidationError
- ParseError → FormatError
- 使用crate::error::ModelSrvError而非voltage_common::error::VoltageError

### Docker Build Notes
- Base image: `rust:1.88-bullseye` for building
- Runtime: `debian:bullseye-slim`
- Uses Aliyun mirrors for faster apt updates in China
- Multi-stage builds to reduce image size

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

## 物模型系统 (modsrv device_model)

### 核心组件
- **DeviceModel**: 设备模型定义（属性、遥测、命令、事件、计算）
- **InstanceManager**: 实例管理（创建、更新、查询）
- **CalculationEngine**: 计算引擎（内置函数：sum、avg、min、max、scale）
- **DataFlowProcessor**: 实时数据流处理（Redis订阅、自动计算触发）
- **DeviceModelSystem**: 系统集成（统一API接口）

### 使用示例
```rust
// 创建设备实例
let instance_id = device_system.create_instance(
    "power_meter_v1",
    "meter_001".to_string(),
    "Main Power Meter".to_string(),
    initial_properties,
).await?;

// 获取遥测数据
let voltage = device_system.get_telemetry(&instance_id, "voltage_a").await?;

// 执行命令
device_system.execute_command(&instance_id, "switch_on", params).await?;
```

## Service-Specific Notes

### comsrv Startup Architecture
- Uses async startup pattern to prevent blocking
- Communication service runs in separate tokio task
- API server starts immediately without waiting
- 30-second timeout for service initialization
- Redis connection handled asynchronously with 5-second timeout

### Tauri Desktop Application
- Located in `apps/tauri-desktop/`
- Vue 3 + TypeScript frontend
- Connects only to API Gateway via REST/WebSocket
- Build with: `npm run tauri:build`
- Development: `npm run tauri:dev`

## Critical Reminders

### Redis Data Architecture
- **Only comsrv writes telemetry data to Redis**
- Other services read data and may write computed/derived values
- Point data format: `{channelID}:{type}:{pointID}` → `{value, timestamp}`
- Configuration data: `cfg:{channelID}:{type}:{pointID}`

### Development Workflow
- Create fixlog entries: `docs/fixlog/fixlog_{date}.md`
- Use `date` command to get current date
- Git commits should not contain Claude-related information
- Always use uv for Python scripts, never system Python

## Data Quality Considerations

- 不允许有数据质量相关的代码，事实上无法检测数据质量，除非相关的协议有数据质量的东西，例如IEC-60870