# Instance Logging Configuration

## Overview

Instance logging system provides runtime data logging for each model instance, including:
- **Measurement (M) Points**: Periodic snapshots of all measurement values
- **Action (A) Points**: Change tracking with source and metadata

## Architecture

```
logs/modsrv/instances/
├── pv_inverter_01/
│   └── pv_inverter_01.log.2025-10-09
├── battery_01/
│   └── battery_01.log.2025-10-09
└── ...
```

## Environment Variables

### INSTANCE_LOG_INTERVAL
- **Purpose**: Snapshot interval in seconds
- **Default**: `60` (1 minute)
- **Example**: `INSTANCE_LOG_INTERVAL=300` (5 minutes)
- **Impact**: How often complete snapshots are written to logs

### INSTANCE_LOG_VERBOSE
- **Purpose**: Enable verbose logging mode (includes point names and extra metadata)
- **Default**: `false`
- **Example**: `INSTANCE_LOG_VERBOSE=true`
- **Impact**: Adds additional context to log entries for debugging

## Log Format

### Snapshot Entry (Periodic)
```
[2025-10-09 14:30:00.000] SNAPSHOT | M_count=25, A_count=5, uptime=3600s
  M: {"1":"750.5","2":"350.2","3":"125.8",...}
  A: {"10":"1","11":"0","12":"1"}
```

**Fields**:
- `M_count`: Number of measurement points
- `A_count`: Number of action points
- `uptime`: Seconds since last snapshot
- `M`: Measurement point values (point_id: value)
- `A`: Action point values (point_id: value)

### Action Change Entry (Immediate)
```
[2025-10-09 14:30:15.456] A-SET | point_id=10, value=1→0, source=rulesrv, rule_id=battery_protect, user=admin
```

**Fields**:
- `point_id`: Action point identifier
- `value`: Old value → New value
- `source`: Service that triggered the change (rulesrv, modsrv, comsrv)
- **Metadata** (optional): Additional context fields like `rule_id`, `user`, etc.

### Action Initialization Entry
```
[2025-10-09 14:25:00.000] A-INIT | point_id=10, value=0, source=modsrv
```

First time an action point is seen for this instance.

## Configuration Examples

### Development Environment (.env)
```bash
# Instance Logging Configuration
INSTANCE_LOG_INTERVAL=60          # Snapshot every 60 seconds
INSTANCE_LOG_VERBOSE=false        # Standard logging mode
```

### Production Environment (.env)
```bash
# Instance Logging Configuration
INSTANCE_LOG_INTERVAL=300         # Snapshot every 5 minutes
INSTANCE_LOG_VERBOSE=false        # Production mode (minimal output)
```

### Debugging Environment (.env)
```bash
# Instance Logging Configuration
INSTANCE_LOG_INTERVAL=10          # Snapshot every 10 seconds (more frequent)
INSTANCE_LOG_VERBOSE=true         # Verbose mode with extra metadata
```

### Docker Compose
```yaml
services:
  modsrv:
    environment:
      - INSTANCE_LOG_INTERVAL=300
      - INSTANCE_LOG_VERBOSE=false
    volumes:
      - ./logs/modsrv:/app/logs/modsrv
```

## Log Rotation

Instance logs use the same rotation policy as service logs:
- **Daily Rotation**: New log file created each day
- **Compression**: Logs older than 7 days are compressed to `.gz`
- **Deletion**: Compressed logs older than 365 days are deleted

## Integration with modsrv_set_action_point

When using the VoltageRedis Lua function `modsrv_set_action_point`, action changes are automatically logged with the `source` parameter:

```lua
-- Example: Rule engine setting an action point
redis.call('FCALL', 'modsrv_set_action_point', 0,
    'pv_inverter_01',  -- instance_id
    '10',              -- point_id
    '1',               -- value
    'rulesrv',         -- source
    'rule_id=battery_protect,user=admin'  -- metadata
)
```

This will generate:
```
[2025-10-09 14:30:15.456] A-SET | point_id=10, value=0→1, source=rulesrv, rule_id=battery_protect, user=admin
```

## Monitoring and Analysis

### View Real-time Logs
```bash
# Watch all instances
tail -f logs/modsrv/instances/*/*.log.*

# Watch specific instance
tail -f logs/modsrv/instances/pv_inverter_01/*.log.*
```

### Search for Action Changes
```bash
# Find all action changes for a specific point
grep "A-SET | point_id=10" logs/modsrv/instances/*/*.log.*

# Find action changes from specific source
grep "source=rulesrv" logs/modsrv/instances/*/*.log.*
```

### Extract Snapshots
```bash
# Get all snapshots for today
grep "SNAPSHOT" logs/modsrv/instances/pv_inverter_01/*.log.$(date +%Y-%m-%d)
```

## Performance Considerations

- **Snapshot Frequency**: Lower values increase disk I/O but provide finer granularity
- **Verbose Mode**: Adds minimal overhead, suitable for debugging
- **Disk Space**: Each instance generates ~10-50 KB per day depending on point count and snapshot frequency

## Troubleshooting

### No Logs Generated
1. Check if `VIRTUAL_CALC_INTERVAL_MS` is set (default: 1000ms)
2. Verify instances are loaded in database: `SELECT * FROM instances;`
3. Check logs directory permissions: `ls -la logs/modsrv/instances/`

### Missing Snapshots
1. Verify `INSTANCE_LOG_INTERVAL`: `echo $INSTANCE_LOG_INTERVAL`
2. Check virtual calculation is running: Look for "Virtual point calculation task started" in service logs
3. Check instance has measurement data in Redis: `redis-cli HGETALL modsrv:{instance_name}:M`

### Action Changes Not Logged
1. Ensure actions are being written via `modsrv_set_action_point` Lua function
2. Check A-point data exists in Redis: `redis-cli HGETALL modsrv:{instance_name}:A`
3. Verify source parameter is provided in action writes

## API Integration

Currently, instance logs are file-based only. Future enhancements may include:
- REST API to query instance log history
- Real-time log streaming via WebSocket
- Log aggregation and analysis dashboard
