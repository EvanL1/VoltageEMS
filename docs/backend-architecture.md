# VoltageEMS Backend Architecture Documentation

## Executive Summary

VoltageEMS is a high-performance industrial IoT energy management system built on a hybrid architecture combining Rust microservices with Redis Lua Functions. This design delivers sub-millisecond data processing latency while maintaining the safety and reliability guarantees of Rust.

## 1. Hybrid Architecture Overview

### 1.1 Architecture Philosophy

The system employs a **dual-layer architecture**:

1. **Lightweight HTTP Layer** (Rust/Axum): Handles API endpoints, configuration management, and service orchestration
2. **High-Performance Data Layer** (Redis Lua Functions): Executes core business logic directly in Redis for maximum performance

```
┌─────────────────────────────────────────────────┐
│                    Clients                      │
│         (Web UI, SCADA, External Systems)       │
└─────────────────────────┬───────────────────────┘
                         │
                    ┌────▼─────┐
                    │  Nginx    │
                    │  (:80)    │
                    └────┬─────┘
                         │
         ┌───────────────┴──────────────────┐
         │                                  │
    ┌────▼────┐                    ┌───────▼────────┐
    │   API   │                    │  Microservices │
    │ Gateway │                    │                │
    │ (:6005) │                    │ comsrv (:6000) │
    └─────────┘                    │ modsrv (:6001) │
                                   │ alarmsrv(:6002)│
                                   │ rulesrv (:6003)│
                                   │ hissrv (:6004) │
                                   │ netsrv (:6006) │
                                   └────────┬───────┘
                                           │
                            ┌──────────────▼──────────────┐
                            │        Redis (:6379)        │
                            │  ┌─────────────────────┐   │
                            │  │   Lua Functions     │   │
                            │  │  - model_*          │   │
                            │  │  - alarm_*          │   │
                            │  │  - rule_*           │   │
                            │  │  - hissrv_*         │   │
                            │  └─────────────────────┘   │
                            │  ┌─────────────────────┐   │
                            │  │   Hash Storage      │   │
                            │  │  {service}:{id}:{T} │   │
                            │  └─────────────────────┘   │
                            └──────────────┬──────────────┘
                                          │
                                    ┌─────▼──────┐
                                    │  InfluxDB   │
                                    │   (:8086)   │
                                    └─────────────┘
```

### 1.2 Service Port Allocation

All service ports are **hardcoded** in the source code for consistency:

| Service | Port | Protocol | Purpose |
|---------|------|----------|---------|
| Nginx | 80/443 | HTTP/HTTPS | Unified entry point, reverse proxy |
| comsrv | 6000 | HTTP | Communication service API |
| modsrv | 6001 | HTTP | Model management API |
| alarmsrv | 6002 | HTTP | Alarm configuration API |
| rulesrv | 6003 | HTTP | Rule engine API |
| hissrv | 6004 | HTTP | Historical data API |
| apigateway | 6005 | HTTP | API aggregation gateway |
| netsrv | 6006 | HTTP | Network forwarding service |
| Redis | 6379 | RESP | Data storage & Lua execution |
| InfluxDB | 8086 | HTTP | Time-series database |

## 2. Service Components Analysis

### 2.1 Communication Service (comsrv)

**Purpose**: Industrial protocol communication hub with plugin architecture

**Architecture**:
```rust
trait ComBase {
    async fn initialize(&mut self, config: &ChannelConfig) -> Result<()>;
    async fn connect(&mut self) -> Result<()>;
    async fn read_batch(&mut self, points: Vec<PointConfig>) -> Result<Vec<PointData>>;
    async fn write_point(&mut self, point_id: u32, value: f64) -> Result<()>;
}
```

**Key Features**:
- Plugin-based protocol support (Modbus TCP/RTU, Virtual, gRPC)
- Connection pooling and automatic reconnection
- Batch operations for efficiency
- CSV-based point table configuration
- Real-time data publishing to Redis

**Data Flow**:
1. Protocol plugins read from industrial devices
2. Data normalized to 6-decimal precision
3. Stored in Redis Hash: `comsrv:{channelID}:{type}`
4. Published to Redis channels for subscribers

### 2.2 Model Service (modsrv)

**Purpose**: Template-based model management with dynamic point mapping

**Lightweight Component (Rust)**:
```rust
struct Model {
    id: String,
    template: String,
    mapping: Mapping {
        channel: u32,
        data: HashMap<String, u32>,
        action: HashMap<String, u32>,
    }
}
```

**Redis Functions**:
```lua
-- model_upsert: Create/update model instance
-- model_get: Retrieve model configuration  
-- model_expand: Expand template with mappings
-- model_reverse_lookup: Find model by point ID
```

**Features**:
- Template expansion for model inheritance
- Reverse mapping for point-to-model lookup
- Auto-generation of point offsets
- Metadata support for extensibility

### 2.3 Alarm Service (alarmsrv)

**Purpose**: Multi-level alarm management with lifecycle tracking

**Alarm Levels**:
- Critical: System failures, safety issues
- Major: Service degradation, equipment failures  
- Minor: Performance issues, warnings
- Warning: Informational alerts
- Info: Status notifications

**Redis Functions**:
```lua
-- store_alarm: Create new alarm with indexing
-- acknowledge_alarm: Update alarm status
-- resolve_alarm: Mark alarm as resolved
-- get_active_alarms: Query active alarms by level/source
-- cleanup_old_alarms: Automatic retention management
```

**Data Structure**:
```
alarmsrv:alarm:{alarm_id} → Alarm details (Hash)
alarmsrv:alarms:active → Active alarm IDs (Set)
alarmsrv:alarms:by_level:{level} → Alarms by level (Set)
alarmsrv:alarms:by_source:{source} → Alarms by source (Set)
```

### 2.4 Rule Engine Service (rulesrv)

**Purpose**: Real-time rule evaluation with complex condition support

**Rule Structure**:
```yaml
rule:
  id: "rule_001"
  name: "High Temperature Alert"
  conditions:
    - source: "comsrv:1001:T:1"
      operator: ">"
      value: 80
  actions:
    - type: "create_alarm"
      params:
        level: "Warning"
        title: "Temperature exceeds threshold"
```

**Redis Functions**:
```lua
-- rule_evaluate: Evaluate single rule
-- rule_batch_evaluate: Process multiple rules
-- rule_trigger_action: Execute rule actions
-- rule_update_state: Track rule state changes
```

**Features**:
- Support for AND/OR logic combinations
- Data source abstraction (comsrv, models, custom)
- Action types: alarms, control commands, notifications
- State tracking to prevent duplicate triggers

### 2.5 Historical Service (hissrv)

**Purpose**: Time-series data collection and InfluxDB integration

**Data Pipeline**:
```
Redis Data → Lua Transform → Line Protocol → InfluxDB
```

**Redis Functions**:
```lua
-- configure_mapping: Set up data source mappings
-- convert_to_line_protocol: Transform to InfluxDB format
-- batch_collect: Batch data for efficient writes
-- get_batch_status: Monitor collection status
```

**Features**:
- Configurable collection intervals
- Field mapping and transformation
- Tag extraction from data sources
- Batch optimization for InfluxDB writes
- Dead letter queue for failed writes

### 2.6 API Gateway (apigateway)

**Purpose**: Minimal API aggregation and routing layer

**Responsibilities**:
- Health check aggregation
- Service discovery (hardcoded endpoints)
- CORS handling
- Future: Authentication/Authorization
- Future: Request/Response transformation

**Current Implementation**:
```rust
Router::new()
    .route("/health", get(health_check))
    .route("/health/detailed", get(detailed_health))
    .nest("/api", api_routes)
    .layer(CorsLayer::new())
```

### 2.7 Network Service (netsrv)

**Purpose**: IoT data forwarding to cloud/external systems

**Status**: Skeleton implementation (core functionality pending)

**Planned Features**:
- HTTP/MQTT protocol support
- Configurable data formatters (JSON, ASCII)
- Retry logic with exponential backoff
- Cloud connectivity status tracking

## 3. Data Flow Architecture

### 3.1 Real-time Data Flow

```
Industrial Device → comsrv → Redis Hash → Subscribers
                            ↓
                    Lua Functions
                    (Processing)
                            ↓
                    ┌───────┴──────────┐
                    │                  │
                alarmsrv          rulesrv
                (Alarms)         (Actions)
```

### 3.2 Data Storage Structure

**Redis Hash Structure**:
```
Key Format: {service}:{channelID}:{type}
Example: comsrv:1001:T

Field → Value:
1 → "23.456789"  (Point ID 1, Temperature)
2 → "1.000000"   (Point ID 2, Status)
```

**Point Types**:
- T: Telemetry (measurements)
- S: Signal (digital states)
- C: Control (commands)
- A: Adjustment (setpoints)

### 3.3 Data Precision Standards

- All numeric values: **6 decimal places**
- Point IDs: Start from **1** (sequential)
- Timestamps: Unix epoch (milliseconds)
- Boolean values: scale=1.0, offset=0.0

## 4. Integration Points

### 4.1 Service-to-Service Communication

**HTTP REST APIs**:
```
GET  /health              - Service health check
GET  /api/channels        - List channels (comsrv)
POST /api/models          - Create model (modsrv)
POST /api/alarms          - Create alarm (alarmsrv)
GET  /api/rules           - List rules (rulesrv)
POST /api/history/query   - Query historical data (hissrv)
```

### 4.2 Redis Lua Function Calls

**Direct Function Invocation**:
```rust
// From Rust service
redis_client.fcall("model_upsert", 
    keys: ["model_001"],
    args: [model_json]
).await?;
```

**Function Categories**:
- `model_*`: Model operations
- `alarm_*`: Alarm management
- `rule_*`: Rule evaluation
- `hissrv_*`: Data collection

### 4.3 InfluxDB Integration

**Write Path**:
```
hissrv → collect_batch → Line Protocol → InfluxDB Write API
```

**Query Path**:
```
Client → hissrv API → InfluxDB Query → JSON Response
```

### 4.4 Configuration System

**ConfigLoader Hierarchy**:
1. Default values (lowest priority)
2. YAML configuration files
3. Environment variables (highest priority)

**Environment Variable Pattern**:
```bash
# Global
VOLTAGE_REDIS_URL=redis://localhost:6379

# Service-specific (overrides global)
COMSRV_REDIS_URL=redis://redis-master:6379
MODSRV_CONFIG_FILE=/custom/path/models.yaml
```

## 5. Key Design Patterns

### 5.1 Plugin Architecture (ComBase)

**Benefits**:
- Protocol independence
- Easy extension for new protocols
- Isolated failure domains
- Hot-swappable implementations

**Implementation**:
```rust
pub struct PluginManager {
    plugins: HashMap<String, Box<dyn ComBase>>,
    storage: Arc<dyn PluginStorage>,
}
```

### 5.2 Lightweight Service Pattern

**Rust Service Responsibilities**:
- HTTP API endpoints
- Configuration management
- Input validation
- Redis Function invocation

**Redis Lua Responsibilities**:
- Core business logic
- Data transformations
- Index management
- Complex queries

### 5.3 Shared Libraries Pattern

**libs/ Contents**:
- `redis/client.rs`: Connection pooling, standard operations
- `influxdb/client.rs`: Time-series operations
- `config/loader.rs`: Unified configuration loading
- `error.rs`: Common error types
- `types.rs`: Shared data structures

### 5.4 Event-Driven Updates

**Redis Pub/Sub Channels**:
```
comsrv:updates:{channelID} - Data updates
alarmsrv:new - New alarms
rulesrv:triggered - Rule triggers
```

## 6. Performance Optimizations

### 6.1 Redis Optimizations

- **Hash operations** instead of individual keys
- **Lua Functions** for atomic operations
- **Pipelining** for batch operations
- **Connection pooling** via ConnectionManager

### 6.2 Service Optimizations

- **Async/await** throughout
- **Batch processing** for efficiency
- **Channel-based** concurrency
- **Zero-copy** where possible

### 6.3 Data Flow Optimizations

- **6-decimal precision** standardization
- **Direct Redis writes** (no intermediate caching)
- **Lazy loading** of configuration
- **Compile-time optimization** flags

## 7. Deployment Architecture

### 7.1 Container Orchestration

```yaml
services:
  nginx:        # Entry point
  redis:        # Data layer + Lua
  influxdb:     # Time-series
  apigateway:   # API aggregation
  comsrv:       # Protocol communication
  modsrv:       # Model management
  alarmsrv:     # Alarm handling
  rulesrv:      # Rule engine
  hissrv:       # Historical data
  netsrv:       # Network forwarding
```

### 7.2 Startup Dependencies

```
1. Redis (with Lua Functions loaded)
2. InfluxDB
3. Core services (parallel):
   - comsrv
   - modsrv
   - alarmsrv
   - rulesrv
   - hissrv
4. API Gateway
5. Nginx
```

### 7.3 Health Monitoring

**Service Health Endpoints**:
```
GET /health - Basic health check
GET /health/detailed - Detailed status with dependencies
```

**Health Check Components**:
- Redis connectivity
- InfluxDB connectivity (hissrv)
- Lua Functions availability
- Configuration validity

## 8. Security Considerations

### 8.1 Current Implementation

- Internal network communication (no TLS)
- Hardcoded ports (no dynamic allocation)
- No authentication on service APIs
- Redis without password (development)

### 8.2 Production Recommendations

- Enable Redis AUTH
- Implement JWT authentication
- Use TLS for external communication
- Network segmentation via Docker networks
- Rate limiting on API Gateway
- Input validation at service boundaries

## 9. Testing Strategy Foundation

### 9.1 Unit Testing

- Test individual Lua Functions
- Mock Redis operations
- Validate data transformations
- Protocol plugin testing

### 9.2 Integration Testing Points

**Critical Paths**:
1. Device → comsrv → Redis → modsrv
2. Data update → rulesrv → alarmsrv
3. Redis → hissrv → InfluxDB
4. API Gateway → Services → Redis

**Test Scenarios**:
- Multi-channel data collection
- Rule cascade triggering
- Alarm lifecycle management
- Model template expansion
- Historical data aggregation

### 9.3 Performance Testing

**Metrics to Monitor**:
- Data ingestion rate (points/second)
- Rule evaluation latency (ms)
- API response times (p50, p95, p99)
- Redis memory usage
- InfluxDB write throughput

## 10. Common Operations

### 10.1 Adding New Protocol

1. Implement `ComBase` trait
2. Register in `PluginManager`
3. Add protocol-specific configuration
4. Create CSV mapping files
5. Test with simulator

### 10.2 Defining New Model Template

1. Create template in `modsrv` config
2. Define base points and actions
3. Test template expansion
4. Verify reverse mappings
5. Validate with real data

### 10.3 Creating Custom Rule

1. Define rule in YAML
2. Specify data sources
3. Configure conditions and operators
4. Define actions
5. Test with simulated data

## 11. Troubleshooting Guide

### 11.1 Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| "Script attempted to access nonexistent global variable" | Redis Functions not loaded | Run `./scripts/redis-functions/load_functions.sh` |
| Port already in use | Hardcoded port conflict | Check for other services on ports 6000-6006 |
| No data in Redis | comsrv not connected | Check device connectivity and protocol config |
| Rules not triggering | Invalid data source | Verify source format matches `service:channel:type:point` |
| Historical data missing | InfluxDB not configured | Check hissrv environment variables |

### 11.2 Debug Commands

```bash
# Check Redis data
redis-cli hgetall "comsrv:1001:T"

# Monitor Redis activity
redis-cli monitor | grep alarmsrv

# View service logs
docker logs -f voltageems-comsrv

# Test Lua Function
redis-cli FCALL model_get 1 "model_001"

# Check InfluxDB data
influx query 'from(bucket:"ems_data") |> range(start: -1h)'
```

## 12. Future Enhancements

### 12.1 Planned Features

- **netsrv implementation**: Complete IoT forwarding
- **Authentication system**: JWT-based auth
- **Distributed deployment**: Multi-node support
- **Protocol additions**: IEC 60870-5-104, OPC UA
- **Advanced analytics**: ML-based anomaly detection

### 12.2 Architecture Evolution

- **GraphQL API**: Replace REST with GraphQL
- **Event streaming**: Kafka integration
- **Distributed tracing**: OpenTelemetry support
- **Service mesh**: Istio/Linkerd integration
- **Multi-tenancy**: Namespace isolation

## Conclusion

VoltageEMS's hybrid architecture combines the safety and reliability of Rust with the performance of Redis Lua Functions to create a robust, high-performance industrial IoT platform. The clear separation of concerns, plugin architecture, and standardized data flow patterns provide a solid foundation for both current operations and future enhancements.

The architecture is optimized for:
- **Sub-millisecond latency** for critical operations
- **Horizontal scalability** through service isolation
- **Extensibility** via plugin architecture
- **Maintainability** through clear service boundaries
- **Testability** with well-defined integration points

This documentation serves as the authoritative reference for understanding, extending, and testing the VoltageEMS backend system.