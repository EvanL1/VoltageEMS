# API Gateway Configuration Client Guide

## Overview

The API Gateway integrates with the central configuration service to enable dynamic configuration management. This guide explains how the configuration client works and how to use it effectively.

## Configuration Client Architecture

```
┌─────────────────────────────────────────────────┐
│              API Gateway Process                 │
│                                                 │
│  ┌─────────────────┐    ┌──────────────────┐  │
│  │  ConfigClient   │    │   Main Service    │  │
│  │                 │    │                    │  │
│  │  - Fetch Config │───▶│  - Apply Config   │  │
│  │  - Watch Updates│    │  - Handle Requests│  │
│  │  - Cache Config │    │  - Route to Services│
│  └────────┬────────┘    └──────────────────┘  │
│           │                                     │
└───────────┼─────────────────────────────────────┘
            │
            │ HTTP/REST
            ▼
    ┌───────────────────┐
    │  Config Service   │
    │   (Port 8000)     │
    └───────────────────┘
```

## Configuration Flow

### 1. Startup Configuration

```rust
// On API Gateway startup
1. Create ConfigClient instance
2. Try to fetch configuration from config service
3. If successful: Use remote configuration
4. If failed: Fall back to local configuration file
5. Start background update checker
```

### 2. Dynamic Update Flow

```rust
// Every 30 seconds (configurable)
1. Check configuration version with config service
2. If new version available:
   a. Fetch updated configuration
   b. Validate configuration
   c. Apply new configuration
   d. Update internal cache
```

## Usage Guide

### Environment Variables

```bash
# Required
CONFIG_SERVICE_URL=http://localhost:8000  # Config service endpoint

# Optional
CONFIG_UPDATE_INTERVAL=30                 # Update check interval (seconds)
CONFIG_CACHE_TTL=300                     # Cache TTL (seconds)
CONFIG_RETRY_ATTEMPTS=3                  # Retry attempts on failure
```

### Starting API Gateway

#### With Config Service (Recommended)

```bash
# Using the provided script
./start-with-config-service.sh

# Or manually
export CONFIG_SERVICE_URL=http://localhost:8000
export RUST_LOG=info
cargo run --release
```

#### With Local Configuration (Fallback)

```bash
# Will use apigateway.yaml in current directory
cargo run --release
```

## Configuration Structure

### Complete Configuration Example

```yaml
# Server configuration
server:
  host: "0.0.0.0"
  port: 8080
  workers: 4

# Redis configuration
redis:
  url: "redis://localhost:6379"
  pool_size: 10
  timeout_seconds: 5

# Backend services
services:
  comsrv:
    url: "http://localhost:8001"
    timeout_seconds: 30
  modsrv:
    url: "http://localhost:8002"
    timeout_seconds: 30
  hissrv:
    url: "http://localhost:8003"
    timeout_seconds: 30
  netsrv:
    url: "http://localhost:8004"
    timeout_seconds: 30
  alarmsrv:
    url: "http://localhost:8005"
    timeout_seconds: 30

# CORS configuration
cors:
  allowed_origins:
    - "http://localhost:8082"  # Frontend
    - "http://localhost:3000"  # Dev server
    - "http://localhost:5173"  # Vite dev
  allowed_methods:
    - "GET"
    - "POST"
    - "PUT"
    - "DELETE"
    - "OPTIONS"
  allowed_headers:
    - "Content-Type"
    - "Authorization"
  max_age: 3600

# Logging configuration
logging:
  level: "info"
  format: "json"
```

## API Reference

### ConfigClient Methods

#### `new(config_service_url: String, service_name: String) -> Self`
Creates a new configuration client instance.

#### `fetch_config() -> Result<Config, ApiGatewayError>`
Fetches the latest configuration from the config service.

#### `check_for_updates() -> Result<bool, ApiGatewayError>`
Checks if a new configuration version is available.

#### `start_watch_loop(update_interval: Duration)`
Starts the background configuration update checker.

#### `get_cached_config() -> Option<Config>`
Returns the currently cached configuration.

#### `update_config(key: &str, value: serde_json::Value) -> Result<(), ApiGatewayError>`
Updates a specific configuration key (requires appropriate permissions).

## Error Handling

### Configuration Fetch Errors

```rust
pub enum ApiGatewayError {
    ConfigFetchError(String),      // Network or service errors
    ConfigParseError(String),      // Invalid configuration format
    ConfigUpdateError(String),     // Failed to update configuration
    ConfigChecksumError,          // Checksum verification failed
    ConfigSubscriptionError(String), // Notification subscription failed
}
```

### Fallback Mechanism

1. **Primary**: Fetch from config service
2. **Fallback**: Use local `apigateway.yaml` file
3. **Emergency**: Use hardcoded defaults

## Monitoring and Debugging

### Health Check Endpoint

```bash
# Check API Gateway health (includes config status)
curl http://localhost:8080/api/v1/health/detailed

# Response includes:
{
  "config_source": "remote|local|default",
  "config_version": 12,
  "last_update": "2024-01-15T10:30:00Z",
  "update_status": "ok|error"
}
```

### Logging

```bash
# Enable debug logging for configuration
export RUST_LOG=apigateway::config_client=debug

# Log examples:
INFO  Configuration update detected, fetching new config
DEBUG Checking for updates, current version: 12
INFO  Configuration updated successfully to version: 13
ERROR Failed to fetch updated configuration: connection refused
```

### Metrics

The following metrics are available for monitoring:

- `config_fetch_total`: Total configuration fetch attempts
- `config_fetch_errors`: Failed configuration fetches
- `config_version`: Current configuration version
- `config_update_duration`: Time taken to update configuration

## Best Practices

### 1. Configuration Updates

- Keep update interval reasonable (30-60 seconds)
- Implement proper error handling for update failures
- Log configuration changes for audit purposes

### 2. Performance

- Cache configuration locally to reduce network calls
- Use connection pooling for HTTP client
- Implement exponential backoff for retries

### 3. Security

- Always use HTTPS for config service in production
- Implement proper authentication for config service
- Never log sensitive configuration values

### 4. Testing

```bash
# Test configuration fetch
curl -H "X-Service-Name: apigateway" \
     http://localhost:8000/api/v1/config/apigateway

# Test version check
curl -H "X-Service-Name: apigateway" \
     http://localhost:8000/api/v1/config/apigateway/version

# Force configuration refresh (admin endpoint)
curl -X POST http://localhost:8080/api/v1/admin/refresh-config
```

## Troubleshooting

### Common Issues

1. **Config service unreachable**
   - Check CONFIG_SERVICE_URL is correct
   - Verify config service is running
   - Check network connectivity

2. **Configuration not updating**
   - Check update loop is running (see logs)
   - Verify version has actually changed
   - Check for validation errors

3. **Performance issues**
   - Increase update interval
   - Check network latency
   - Enable configuration caching

### Debug Commands

```bash
# Check current configuration
curl http://localhost:8080/api/v1/debug/config

# Check configuration version
curl http://localhost:8080/api/v1/debug/config-version

# Force configuration reload
curl -X POST http://localhost:8080/api/v1/admin/reload-config
```

## Migration from Static Configuration

### Step 1: Export Current Configuration

```bash
# Convert apigateway.yaml to config service format
cat apigateway.yaml | \
  curl -X POST http://localhost:8000/api/v1/config/apigateway/import \
  -H "Content-Type: application/x-yaml" \
  --data-binary @-
```

### Step 2: Verify Configuration

```bash
# Check imported configuration
curl http://localhost:8000/api/v1/config/apigateway | jq .
```

### Step 3: Update Startup Script

```bash
# Update systemd service or docker-compose
Environment="CONFIG_SERVICE_URL=http://config-service:8000"
ExecStart=/usr/local/bin/apigateway
```

### Step 4: Test and Deploy

```bash
# Test with config service
./start-with-config-service.sh

# Monitor logs
tail -f logs/apigateway.log | grep config
```