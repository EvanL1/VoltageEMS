# VoltageEMS Configuration Library

Central configuration library for all VoltageEMS services, providing a single source of truth for SQL queries, Redis keys, table names, and configuration constants.

## Overview

This library serves as the **Contract Layer** between VoltageEMS services, ensuring consistency and maintainability across the distributed system. All hardcoded strings, SQL queries, and configuration keys are centralized here, eliminating the need to update multiple services when schema or configuration changes occur.

## Architecture Philosophy

### Why Centralized Configuration?

1. **Single Point of Maintenance**: When database schemas or Redis key patterns change, update only this library instead of hunting through multiple services
2. **Compile-time Safety**: Type-safe constants prevent typos and runtime errors
3. **Clear Contracts**: Services share well-defined contracts without tight coupling
4. **Documentation**: All configuration is documented in one place

### Microservices Still Matter

While configuration is centralized, services remain decoupled:
- Each service maintains its own business logic
- Services can scale independently
- Fault isolation is preserved
- Technology stack flexibility remains

## Module Structure

### comsrv - Communication Service Configuration

```rust
use voltage_config::comsrv::{ProtocolQueries, RedisKeys, TableNames, ConfigKeys};
```

- **ProtocolQueries**: All SQL queries for protocol plugins (CAN, Modbus, Virtual)
- **RedisKeys**: Key patterns for channel data storage and command queues
- **TableNames**: Database table names for points and mappings
- **ConfigKeys**: Configuration file keys for protocol mappings

### modsrv - Model Service Configuration

```rust
use voltage_config::modsrv::{ModsrvQueries, RedisKeys, TableNames};
```

- **ModsrvQueries**: SQL queries for instance and product management
- **RedisKeys**: Key patterns for instance measurements and actions
- **TableNames**: Database table names for products and instances

### Common Configuration Patterns

All modules follow consistent patterns:
- SQL queries are complete, parameterized statements
- Redis keys use format strings with {} placeholders
- Table names are simple string constants
- Configuration keys match YAML/CSV structure

## Usage Examples

### Using SQL Queries

```rust
use voltage_config::comsrv::ProtocolQueries;
use sqlx::SqlitePool;

// Load CAN signal points
let points = sqlx::query_as::<_, PointConfig>(ProtocolQueries::CAN_POINTS)
    .bind(&channel_id)
    .fetch_all(&pool)
    .await?;
```

### Using Redis Keys

```rust
use voltage_config::comsrv::RedisKeys;

// Format channel data key
let key = format!(RedisKeys::CHANNEL_DATA, channel_id, data_type);
// Results in: "comsrv:1001:T"

// Format control TODO queue key
let todo_key = format!(RedisKeys::CONTROL_TODO, channel_id);
// Results in: "comsrv:1001:C:TODO"
```

### Using Table Names

```rust
use voltage_config::comsrv::TableNames;

// Query from points table
let query = format!("SELECT * FROM {} WHERE channel_id = ?", TableNames::POINTS);
```

## Query Patterns

### Points Query Structure

All protocol point queries follow the same SELECT structure:
```sql
SELECT
    p.point_id,
    p.signal_name,
    p.scale,
    p.offset,
    p.unit,
    p.reverse,
    p.data_type,
    -- Protocol-specific mapping fields
FROM points p
LEFT JOIN {protocol}_mappings m ON p.channel_id = m.channel_id AND p.point_id = m.point_id
WHERE p.channel_id = ?
```

### Instance Query Patterns

Model service queries for instance management:
```sql
-- Check existence
SELECT EXISTS(SELECT 1 FROM instances WHERE instance_id = ?)

-- Count products
SELECT COUNT(*) FROM products

-- List instances
SELECT instance_id, product_name, properties FROM instances
```

## Redis Key Patterns

### Channel Data Storage
```
comsrv:{channel_id}:{type}  # Type: T (Telemetry), S (Signal), C (Control), A (Adjustment)
```

### Command Queues
```
comsrv:{channel_id}:C:TODO  # Control commands
comsrv:{channel_id}:A:TODO  # Adjustment commands
```

### Instance Data
```
# Address strings used in routing tables (not value storage):
modsrv:{instance_name}:M:{point_id}
modsrv:{instance_name}:A:{point_id}

# Runtime value storage (hash):
modsrv:{instance_name}:M   # field = point_id
modsrv:{instance_name}:A   # field = point_id
```

## Migration Guide

### Before (Hardcoded in each service)
```rust
// In each protocol plugin
const QUERY: &str = "SELECT * FROM points WHERE ...";
const REDIS_KEY: &str = "comsrv:{}:{}";
```

### After (Centralized)
```rust
// In voltage-config
use voltage_config::comsrv::{ProtocolQueries, RedisKeys};

// In service
let query = ProtocolQueries::CAN_POINTS;
let key = format!(RedisKeys::CHANNEL_DATA, id, type);
```

## Benefits

1. **Maintainability**: Update queries in one place when schema changes
2. **Consistency**: All services use the same key patterns and table names
3. **Type Safety**: Compile-time checking prevents configuration errors
4. **Documentation**: All configuration is self-documenting through code
5. **Testing**: Easy to unit test with known constants

## Contributing

When adding new configuration:
1. Add constants to the appropriate module (comsrv, modsrv, etc.)
2. Use clear, descriptive names
3. Add doc comments explaining usage
4. Update this README with new patterns
5. Run `cargo doc` to verify documentation

## Version Compatibility

This library version must be compatible with all services in the workspace. When making breaking changes:
1. Update all dependent services
2. Run full test suite
3. Document migration steps
