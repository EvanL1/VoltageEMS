# ComSrv Service Configuration Guide

## Configuration File Structure

ComSrv uses the following configuration structure:

```yaml
# Service configuration (Note: must be contained under the service node)
service:
  name: "comsrv"
  version: "0.0.1"
  description: "Communication Service"
  api:
    enabled: true
    port: 8081
    bind_addr: "0.0.0.0"
  redis:
    url: "redis://localhost:6379"
    pool_size: 20
    key_prefix: "comsrv"
  logging:
    level: "info"
    format: "json"
    file_enabled: false
    console_enabled: true

# Channel configuration
channels:
  - id: 1001
    name: "Test Channel 1"
    protocol: "modbus"
    enabled: true
    config:
      protocol: "tcp"
      address: "192.168.1.100:502"
      slave_id: 1
      timeout_ms: 3000
      retry_count: 3
```

## Configuration Loading Methods

1. The service will load the configuration file from the path specified in command line arguments
2. By default, it looks for `config/comsrv.yaml`
3. Supports environment variable substitution (using `${VAR_NAME:-default}` format)

## Important Notes

- All service-related configurations must be under the `service` node
- The `key_prefix` in Redis configuration is required
- Channel configuration is an array, allowing multiple channels to be defined