# Configuration Center API Documentation

## Overview

The comsrv service supports loading configuration from a remote configuration center via HTTP API. This allows centralized configuration management across multiple service instances.

## API Endpoint

```
GET /api/v1/config/service/comsrv
```

## Response Format

The configuration center should return a JSON response in the following format:

```json
{
  "status": "success",
  "message": null,
  "version": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:30:00Z",
  "data": {
    "version": "2.0",
    "service": {
      "name": "comsrv",
      "description": "Industrial Communication Service",
      "api": {
        "enabled": true,
        "bind_address": "0.0.0.0:3000",
        "version": "v1"
      },
      "redis": {
        "enabled": true,
        "url": "redis://redis-cluster:6379",
        "db": 0,
        "key_prefix": "voltageems:",
        "timeout_ms": 5000,
        "retry_attempts": 3,
        "retry_delay_ms": 100,
        "pool_size": 20
      },
      "logging": {
        "level": "info",
        "file": "/var/log/comsrv/comsrv.log",
        "max_size": 104857600,
        "max_files": 10,
        "console": false
      }
    },
    "defaults": {
      "channels_root": "channels",
      "combase_dir": "combase",
      "protocol_dir": "protocol"
    },
    "channels": [
      {
        "id": 1,
        "name": "Production_Line_1",
        "description": "Production line 1 Modbus devices",
        "protocol": "modbus_tcp",
        "parameters": {
          "host": "10.0.1.100",
          "port": 502,
          "timeout_ms": 5000,
          "polling_interval_ms": 1000,
          "enable_batch_reading": true,
          "max_batch_size": 100
        },
        "logging": {
          "enabled": true,
          "level": "info",
          "log_dir": "/var/log/comsrv/channels/line1"
        },
        "table_config": {
          "four_telemetry_route": "config/production/line1",
          "mapping_route": "config/production/line1"
        }
      }
    ]
  }
}
```

## Error Response

If an error occurs, the response should be:

```json
{
  "status": "error",
  "message": "Configuration not found for service: comsrv",
  "data": null
}
```

## Configuration Loading Priority

1. **Environment Variables** (highest priority)
   - Prefix: `COMSRV_`
   - Example: `COMSRV_SERVICE_REDIS_URL=redis://override:6379`

2. **Configuration Center**
   - Loaded if `CONFIG_CENTER_URL` environment variable is set
   - Example: `CONFIG_CENTER_URL=http://config-service:8080`

3. **Local Configuration File**
   - Default: `config/comsrv.yaml`
   - Can be overridden via command line: `comsrv --config /path/to/config.yaml`

4. **Default Values** (lowest priority)
   - Built-in defaults in the application

## Environment Variable Examples

```bash
# Override service name
export COMSRV_SERVICE_NAME=comsrv-prod

# Override Redis configuration
export COMSRV_SERVICE_REDIS_URL=redis://redis-prod:6379
export COMSRV_SERVICE_REDIS_KEY_PREFIX=prod:

# Override logging
export COMSRV_SERVICE_LOGGING_LEVEL=debug
export COMSRV_SERVICE_LOGGING_FILE=/var/log/comsrv/debug.log

# Enable config center
export CONFIG_CENTER_URL=http://config-center.internal:8080
```

## Implementation Notes

- The configuration center URL must be accessible from the comsrv service
- HTTP timeout is set to 10 seconds
- If the config center is unavailable, the service will fall back to local configuration
- The service does not cache config center responses (future enhancement)
- WebSocket support for configuration updates is planned for future releases