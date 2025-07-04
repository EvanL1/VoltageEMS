# ComsRV Configuration Guide

## Overview

ComsRV supports a flexible multi-source configuration system that allows you to manage configurations from multiple sources with clear precedence rules.

## Configuration Sources

### 1. Default Values (Lowest Priority)
Built-in default values ensure the service can start even without any configuration file.

### 2. Local Configuration File
The service looks for configuration files in the following order:
- Command-line specified: `comsrv --config /path/to/config.yaml`
- Default locations:
  - `./config/comsrv.yaml`
  - `./comsrv.yaml`
  - `./config/default.yaml`
  - `/etc/comsrv/config.yaml`

Supported formats:
- YAML (.yaml, .yml)
- TOML (.toml)
- JSON (.json)

### 3. Configuration Center
If `CONFIG_CENTER_URL` environment variable is set, the service will try to fetch configuration from:
```
GET {CONFIG_CENTER_URL}/api/v1/config/service/comsrv
```

### 4. Environment Variables (Highest Priority)
Any configuration value can be overridden using environment variables with the `COMSRV_` prefix.

## Configuration Structure

```yaml
# Configuration version
version: "2.0"

# Service configuration
service:
  name: "comsrv"
  description: "Industrial Communication Service"
  
  # API server configuration
  api:
    enabled: true
    bind_address: "127.0.0.1:3000"
    version: "v1"
  
  # Redis configuration
  redis:
    enabled: true
    url: "redis://127.0.0.1:6379"
    db: 0
    key_prefix: "voltageems:"
    timeout_ms: 5000
    retry_attempts: 3
    retry_delay_ms: 100
    pool_size: 10
    
  # Logging configuration
  logging:
    level: "info"
    file: "logs/comsrv.log"
    max_size: 10485760
    max_files: 5
    console: true

# Default paths
defaults:
  channels_root: "channels"
  combase_dir: "combase"
  protocol_dir: "protocol"

# Communication channels
channels:
  - id: 1
    name: "Channel_1"
    protocol: "modbus_tcp"
    parameters:
      host: "127.0.0.1"
      port: 502
    # ... channel specific config
```

## Environment Variable Override Examples

```bash
# Service configuration
export COMSRV_SERVICE_NAME=production-comsrv
export COMSRV_SERVICE_DESCRIPTION="Production Communication Service"

# API configuration
export COMSRV_SERVICE_API_ENABLED=true
export COMSRV_SERVICE_API_BIND_ADDRESS=0.0.0.0:8080

# Redis configuration
export COMSRV_SERVICE_REDIS_URL=redis://redis-cluster:6379
export COMSRV_SERVICE_REDIS_KEY_PREFIX=prod:
export COMSRV_SERVICE_REDIS_POOL_SIZE=50

# Logging configuration
export COMSRV_SERVICE_LOGGING_LEVEL=warn
export COMSRV_SERVICE_LOGGING_FILE=/var/log/comsrv/app.log
export COMSRV_SERVICE_LOGGING_CONSOLE=false
```

## Configuration Center Integration

### Enabling Config Center
```bash
export CONFIG_CENTER_URL=http://config-service:8080
```

### Expected Response Format
```json
{
  "status": "success",
  "message": null,
  "version": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:30:00Z",
  "data": {
    // Complete AppConfig structure
  }
}
```

### Fallback Behavior
If the config center is unavailable:
1. A warning is logged
2. The service continues with local configuration
3. No retry attempts are made (future enhancement)

## Best Practices

### Development Environment
```bash
# Use local config file with debug logging
./comsrv --config config/dev.yaml
export COMSRV_SERVICE_LOGGING_LEVEL=debug
```

### Staging Environment
```bash
# Use config center with some overrides
export CONFIG_CENTER_URL=http://config.staging:8080
export COMSRV_SERVICE_REDIS_URL=redis://redis.staging:6379
./comsrv
```

### Production Environment
```bash
# Use config center with production overrides
export CONFIG_CENTER_URL=http://config.prod:8080
export COMSRV_SERVICE_REDIS_URL=redis://redis.prod:6379
export COMSRV_SERVICE_LOGGING_LEVEL=warn
export COMSRV_SERVICE_LOGGING_FILE=/var/log/comsrv/app.log
./comsrv
```

## Migration from Legacy Configuration

If you're migrating from the old configuration format:

1. The new system is backward compatible with existing YAML files
2. Environment variable names have changed:
   - Old: `REDIS_URL`
   - New: `COMSRV_SERVICE_REDIS_URL`
3. The configuration structure remains largely the same

## Debugging Configuration Issues

To see which configuration sources are being used:
```bash
export RUST_LOG=comsrv::core::config::loader=debug
./comsrv
```

This will show:
- Which configuration files were loaded
- Whether config center was contacted
- Which environment variables were applied

## Future Enhancements

1. **Hot Reload**: Configuration changes without restart
2. **WebSocket Notifications**: Real-time config updates from config center
3. **Configuration Validation**: Schema validation before applying changes
4. **Encrypted Values**: Support for encrypted configuration values
5. **Configuration History**: Track configuration changes over time