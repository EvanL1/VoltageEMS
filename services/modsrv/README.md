# ModSrv - Device Model Computation Engine

ModSrv is the core computation engine of the VoltageEMS system, responsible for executing real-time data calculations based on DAG and device model management.

## Features

- **DAG Computation Engine** - Supports complex data flow computation graphs
- **Device Model System** - Unified device abstraction and management
- **Real-time Data Processing** - Millisecond-level computation latency
- **Built-in Function Library** - sum, avg, min, max, scale, etc.
- **Redis Integration** - High-performance data storage and publishing

## Architecture

```
Redis Hash (comsrv data) → ModSrv Engine → Redis Hash (results)
                               ↓
                        Alarm Trigger → AlarmSrv
                        Rule Trigger → RuleSrv
```

## Quick Start

### Environment Requirements

- Rust 1.88+
- Redis 7.0+

### Running the Service

```bash
# Development mode
cargo run -p modsrv

# Production mode
cargo run --release -p modsrv

# Specify log level
RUST_LOG=modsrv=debug cargo run -p modsrv
```

### Configuration File

```yaml
# services/modsrv/config/default.yml
service_name: "modsrv"
version: "2.0.0"

redis:
  url: "redis://localhost:6379"
  key_prefix: "modsrv:"

api:
  host: "0.0.0.0"
  port: 8092

models:
  - id: "power_meter_demo"
    name: "Demo Power Meter Model"
    description: "Simple power meter monitoring model for demonstration"
    monitoring:
      voltage_a:
        description: "Phase A voltage"
        unit: "V"
      current_a:
        description: "Phase A current"
        unit: "A"
      power:
        description: "Active power"
        unit: "kW"
    control:
      main_switch:
        description: "Main switch"
      power_limit:
        description: "Power limit setting"
        unit: "kW"
```

### Point Mapping

```json
// services/modsrv/mappings/power_meter_demo.json
{
  "monitoring": {
    "voltage_a": {
      "channel": 1001,
      "point": 1,      // Note: Point IDs start from 1
      "type": "m"
    },
    "current_a": {
      "channel": 1001,
      "point": 2,
      "type": "m"
    }
  },
  "control": {
    "main_switch": {
      "channel": 1001,
      "point": 1,      // Control points also start from 1
      "type": "c"
    }
  }
}
```

## API Endpoints

### Health Check

```bash
curl http://localhost:8092/health
```

### Get Model List

```bash
curl http://localhost:8092/models
```

### Get Model Data

```bash
curl http://localhost:8092/models/power_meter_demo
```

### Send Control Command

```bash
curl -X POST http://localhost:8092/models/power_meter_demo/control/main_switch \
  -H "Content-Type: application/json" \
  -d '{"value": 1}'
```

## DAG Computation Example

```rust
// Internal computation logic example
let dag = DAGBuilder::new()
    .add_node("voltage", Source::Redis("comsrv:1001:T", "1"))
    .add_node("current", Source::Redis("comsrv:1001:T", "2"))
    .add_node("power", Function::Multiply(vec!["voltage", "current"]))
    .add_node("scaled_power", Function::Scale("power", 0.001))  // W to kW
    .build();

// Execute computation
let results = dag.execute().await?;
```

## Data Flow

1. **Data Input**: Read from Redis Hash `comsrv:{channelID}:{type}`
2. **Computation Processing**: Execute DAG-defined computation flow
3. **Result Storage**: Write to `modsrv:{modelname}:measurement`
4. **Event Publishing**: Publish to `modsrv:{modelname}:update`

## Monitoring and Debugging

### View Redis Data

```bash
# View input data
redis-cli hgetall "comsrv:1001:T"

# View computation results
redis-cli hgetall "modsrv:power_meter_demo:measurement"

# Monitor data updates
redis-cli subscribe "modsrv:power_meter_demo:update"
```

### Log Monitoring

```bash
# View service logs
tail -f logs/modsrv.log

# Debug mode
RUST_LOG=modsrv=trace cargo run
```

## Performance Optimization

- **Batch Reading**: Use HGETALL to reduce Redis round trips
- **Computation Caching**: Avoid recalculating the same nodes
- **Parallel Processing**: Execute independent computation branches in parallel
- **Connection Pooling**: Redis connection reuse

## Development Guide

### Adding New Functions

```rust
// Add to Function enum
pub enum Function {
    // ...
    MyNewFunction(Vec<String>),  // Input parameter list
}

// Implement computation logic
impl Function {
    pub fn execute(&self, inputs: &HashMap<String, f64>) -> Result<f64> {
        match self {
            Function::MyNewFunction(params) => {
                // Implement function logic
            }
        }
    }
}
```

### Testing

```bash
# Run unit tests
cargo test -p modsrv

# Run specific test
cargo test -p modsrv test_dag_calculation

# Run integration tests
cargo test -p modsrv --test integration
```

## Troubleshooting

### Common Issues

1. **No Data Output**
   - Check Redis connection
   - Verify point mapping configuration
   - Confirm comsrv data exists

2. **Computation Errors**
   - Check DAG definition for circular dependencies
   - Verify input data format
   - Check error logs

3. **Performance Issues**
   - Monitor Redis operation latency
   - Check computation graph complexity
   - Optimize batch operations

## Configuration Reference

### Environment Variables

```bash
RUST_LOG=modsrv=info      # Log level
REDIS_URL=redis://localhost:6379
MODSRV_PORT=8092
```

### Advanced Configuration

```yaml
# Computation engine configuration
compute:
  max_dag_depth: 10        # Maximum DAG depth
  cache_ttl: 60            # Cache time (seconds)
  batch_size: 100          # Batch processing size
  
# Performance tuning
performance:
  worker_threads: 4        # Number of worker threads
  queue_size: 1000        # Task queue size
```

## License

MIT License