# HisSrv - Historical Data Service

HisSrv is the historical data service of the VoltageEMS system. It aggregates real-time Redis data through Lua scripts and persists the aggregated results to InfluxDB.

## Features

- **Clean Design** - Streamlined code structure, easy to maintain
- **Lua Script Aggregation** - High-performance data pre-aggregation processing
- **Multi-level Time Windows** - Supports multi-level aggregation such as 1-minute, 5-minute
- **Polling Architecture** - Simple and reliable data collection mechanism
- **Batch Write Optimization** - Reduces InfluxDB write pressure
- **Configuration Management** - Supports runtime configuration modification and hot reload

## Architecture

```
Redis Real-time Data → Lua Script Aggregation → Aggregated Data → HisSrv Polling → InfluxDB
                               ↓
                      Cron Scheduled Trigger
```

## Quick Start

### Environment Requirements

- Rust 1.88+
- Redis 7.0+
- InfluxDB 2.x

### Initialize Lua Scripts

```bash
# Load aggregation scripts to Redis
cd services/hissrv/scripts
./init_scripts.sh
```

### Scheduled Task Configuration

#### Option 1: Built-in Timer in Container (Recommended)

HisSrv can have a built-in timer that automatically triggers Lua scripts:

```yaml
# services/hissrv/config/default.yml
aggregation:
  enabled: true
  intervals:
    - name: "1m"
      interval: 60s
      script: "aggregate_1m"
    - name: "5m"
      interval: 300s
      script: "aggregate_5m"
```

#### Option 2: Docker Compose with Cron

```yaml
version: '3.8'
services:
  hissrv:
    image: voltageems/hissrv
    environment:
      - INFLUXDB_TOKEN=${INFLUXDB_TOKEN}
    depends_on:
      - redis
      - influxdb
    
  hissrv-cron:
    image: voltageems/hissrv
    command: crond -f
    volumes:
      - ./scripts:/scripts
      - type: bind
        source: ./crontab
        target: /etc/crontabs/root
```

crontab文件：
```
* * * * * /scripts/hissrv_cron.sh 1m
*/5 * * * * /scripts/hissrv_cron.sh 5m
```

#### Option 3: Kubernetes CronJob

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: hissrv-1m-aggregation
spec:
  schedule: "* * * * *"
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: aggregator
            image: redis:7-alpine
            command:
            - redis-cli
            - EVALSHA
            - $(AGGREGATE_SCRIPT_SHA)
            - "0"
            - "aggregate_1m"
```

### Running the Service

```bash
# Set environment variables
export INFLUXDB_TOKEN=your_influxdb_token
export RUST_LOG=hissrv=info

# Development mode
cargo run -p hissrv

# Production mode
cargo run --release -p hissrv
```

### Configuration File

```yaml
# services/hissrv/config/default.yml
service:
  name: "hissrv"
  polling_interval: 10s    # Polling interval
  batch_size: 1000         # Batch write size

redis:
  url: "redis://localhost:6379"
  data_patterns:           # Data source patterns
    - "archive:1m:*"      # 1-minute aggregated data
    - "archive:5m:*"      # 5-minute aggregated data
    - "archive:pending"   # Pending queue

influxdb:
  url: "http://localhost:8086"
  org: "voltage"
  bucket: "ems"
  token: "${INFLUXDB_TOKEN}"
  
api:
  host: "0.0.0.0"
  port: 8082
```

### Data Mapping Configuration

```yaml
# Data mapping rules
mappings:
  - source_pattern: "comsrv:(\\d+):m"    # Redis key pattern
    measurement: "telemetry"              # InfluxDB measurement
    tags:
      - name: "channel"
        source: "capture"                 # From regex capture group
        index: 1
    field_mappings:
      "1": "voltage"                      # Point ID to field name mapping
      "2": "current"
      "3": "power"
      
  - source_pattern: "modsrv:([^:]+):measurement"
    measurement: "model_data"
    tags:
      - name: "model"
        source: "capture"
        index: 1
    field_mappings:
      "*": "direct"                       # Use original field names directly
```

## API Endpoints

### Health Check

```bash
curl http://localhost:8082/health
```

### Get Service Status

```bash
curl http://localhost:8082/status
```

Response example:
```json
{
  "status": "running",
  "processed": 150000,
  "failed": 12,
  "queue_size": 0,
  "last_sync": "2024-01-29T10:30:00Z"
}
```

### Trigger Manual Sync

```bash
curl -X POST http://localhost:8082/sync
```

### Query Historical Data

```bash
# Query data from the last 1 hour
curl "http://localhost:8082/query?measurement=telemetry&channel=1001&range=1h"
```

## Data Flow

1. **Raw Data Collection**: comsrv writes real-time data to `comsrv:{channelID}:m`
2. **Lua Script Aggregation**: Cron triggers Lua scripts for data aggregation
   - 1-minute aggregation: Calculate avg/min/max, store to `archive:1m:*`
   - 5-minute aggregation: Secondary aggregation from 1-minute data to `archive:5m:*`
3. **HisSrv Polling**: Periodically scan aggregated data with `archive:*` pattern
4. **Batch Write**: Accumulate data and batch write to InfluxDB
5. **Clean Expired Data**: Redis aggregated data expires automatically (2 hours)

## Performance Optimization

### Batch Write Strategy

```yaml
performance:
  batch_size: 1000          # Data volume per batch
  batch_timeout: 5s         # Maximum wait time
  write_buffer_size: 10000  # Write buffer size
  max_retries: 3            # Maximum retry count
```

### Redis Scan Optimization

- Use SCAN command to avoid blocking
- Process multiple data sources in parallel
- Intelligently skip unchanged data

### InfluxDB Optimization

- Set reasonable retention policy
- Use tag indexed fields
- Avoid high cardinality tags

## Monitoring and Debugging

### View Processing Logs

```bash
# Detailed logs
RUST_LOG=hissrv=debug cargo run

# View real-time logs
tail -f logs/hissrv.log
```

### Redis Monitoring

```bash
# View aggregated data
redis-cli keys "archive:1m:*"
redis-cli hgetall "archive:1m:1704067200:1001"

# Monitor Lua script execution
redis-cli monitor | grep EVALSHA

# View pending queue
redis-cli llen "archive:pending"
```

### InfluxDB Query

```bash
# Using influx CLI
influx query 'from(bucket:"ems") 
  |> range(start: -1h) 
  |> filter(fn: (r) => r._measurement == "telemetry")'
```

## Troubleshooting

### Common Issues

1. **Data Not Written to InfluxDB**
   - Check INFLUXDB_TOKEN environment variable
   - Verify InfluxDB connection and permissions
   - Check error logs

2. **Data Delay**
   - Adjust polling_interval
   - Increase batch_size
   - Check Redis and InfluxDB performance

3. **High Memory Usage**
   - Reduce write_buffer_size
   - Optimize data mapping rules
   - Enable data compression

## Advanced Configuration

### Data Aggregation

```yaml
aggregations:
  - name: "1m_avg"
    interval: 1m
    function: "mean"
    sources: ["telemetry"]
    
  - name: "5m_max"
    interval: 5m
    function: "max"
    sources: ["telemetry"]
```

### Data Filtering

```yaml
filters:
  - source: "comsrv:*:m"
    conditions:
      - field: "quality"
        operator: "eq"
        value: "good"
```

### Alert Integration

```yaml
alerts:
  - name: "sync_failure"
    condition: "failed_count > 100"
    action: "webhook"
    url: "http://alert-service/webhook"
```

## Deployment Recommendations

### Docker Deployment

```dockerfile
FROM rust:1.88 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p hissrv

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/hissrv /usr/local/bin/
COPY services/hissrv/config /etc/hissrv
CMD ["hissrv"]
```

### Resource Requirements

- CPU: 1-2 cores
- Memory: 512MB-1GB
- Storage: Depends on log retention policy

### High Availability Deployment

- Multi-instance deployment using Redis distributed locks
- Load balance different data source patterns
- Regular backup of InfluxDB data

## Development Guide

### Adding New Data Sources

1. Add new pattern in configuration
2. Implement corresponding mapping rules
3. Test data flow

### Custom Aggregation Functions

```rust
// Implement new aggregation function
impl AggregationFunction {
    pub fn custom_percentile(&self, values: &[f64], p: f64) -> f64 {
        // Implement percentile calculation
    }
}
```

## Testing

```bash
# Unit tests
cargo test -p hissrv

# Integration tests
cargo test -p hissrv --test integration

# Performance tests
cargo bench -p hissrv
```

## License

MIT License