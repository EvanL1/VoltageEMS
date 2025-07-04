# Configuration Service REST API

## Overview

The Configuration Service provides centralized configuration management for all VoltageEMS microservices. It supports dynamic configuration updates, version management, and real-time notifications.

## Base URL

```
http://localhost:8000/api/v1
```

## Authentication

All requests must include the service name header:
```
X-Service-Name: {service_name}
```

## API Endpoints

### 1. Get Service Configuration

**GET** `/config/{service_name}`

Fetch the complete configuration for a specific service.

**Response:**
```json
{
  "version": 12,
  "data": {
    "server": {
      "host": "0.0.0.0",
      "port": 8080,
      "workers": 4
    },
    "redis": {
      "url": "redis://localhost:6379",
      "pool_size": 10,
      "timeout_seconds": 5
    },
    "services": {
      "comsrv": {
        "url": "http://localhost:8001",
        "timeout_seconds": 30
      },
      "modsrv": {
        "url": "http://localhost:8002",
        "timeout_seconds": 30
      },
      "hissrv": {
        "url": "http://localhost:8003",
        "timeout_seconds": 30
      },
      "netsrv": {
        "url": "http://localhost:8004",
        "timeout_seconds": 30
      },
      "alarmsrv": {
        "url": "http://localhost:8005",
        "timeout_seconds": 30
      }
    },
    "cors": {
      "allowed_origins": ["http://localhost:8082"],
      "allowed_methods": ["GET", "POST", "PUT", "DELETE"],
      "allowed_headers": ["Content-Type", "Authorization"],
      "max_age": 3600
    },
    "logging": {
      "level": "info",
      "format": "json"
    }
  },
  "checksum": "sha256:abcdef..."
}
```

### 2. Get Configuration Version

**GET** `/config/{service_name}/version`

Check the current version of a service's configuration.

**Response:**
```json
{
  "version": 12,
  "last_updated": "2025-07-04T10:30:00Z"
}
```

### 3. Update Configuration

**PUT** `/config/{service_name}/update`

Update a specific configuration key.

**Request Body:**
```json
{
  "key": "services.comsrv.url",
  "value": "http://localhost:8091",
  "reason": "Service port changed"
}
```

**Response:**
```json
{
  "success": true,
  "version": 13,
  "message": "Configuration updated successfully"
}
```

### 4. Get Configuration History

**GET** `/config/{service_name}/history`

Retrieve configuration change history.

**Query Parameters:**
- `limit` (optional): Number of records to return (default: 20)
- `offset` (optional): Pagination offset (default: 0)

**Response:**
```json
{
  "history": [
    {
      "version": 13,
      "timestamp": "2025-07-04T10:30:00Z",
      "operation": "update",
      "key": "services.comsrv.url",
      "old_value": "http://localhost:8001",
      "new_value": "http://localhost:8091",
      "reason": "Service port changed",
      "user": "admin"
    }
  ],
  "total": 50
}
```

### 5. Subscribe to Configuration Changes

**POST** `/config/subscribe`

Subscribe to real-time configuration change notifications.

**Request Body:**
```json
{
  "service": "apigateway",
  "callback_url": "http://localhost:8080/config/notify",
  "events": ["update", "delete"]
}
```

**Response:**
```json
{
  "success": true,
  "subscription_id": "sub_123456",
  "message": "Subscription created successfully"
}
```

### 6. Unsubscribe from Notifications

**DELETE** `/config/subscribe/{subscription_id}`

Remove a configuration change subscription.

**Response:**
```json
{
  "success": true,
  "message": "Subscription removed successfully"
}
```

### 7. Rollback Configuration

**POST** `/config/{service_name}/rollback`

Rollback configuration to a previous version.

**Request Body:**
```json
{
  "target_version": 11,
  "reason": "Reverting problematic changes"
}
```

**Response:**
```json
{
  "success": true,
  "version": 14,
  "message": "Configuration rolled back to version 11"
}
```

### 8. Export Configuration

**GET** `/config/{service_name}/export`

Export configuration in specified format.

**Query Parameters:**
- `format`: Export format (yaml|json|toml)

**Response:**
```yaml
# For format=yaml
server:
  host: 0.0.0.0
  port: 8080
  workers: 4

redis:
  url: redis://localhost:6379
  pool_size: 10
  timeout_seconds: 5
```

### 9. Import Configuration

**POST** `/config/{service_name}/import`

Import configuration from file.

**Request Body:**
```json
{
  "format": "yaml",
  "content": "server:\n  host: 0.0.0.0\n  port: 8080\n...",
  "merge": true,
  "reason": "Importing from development environment"
}
```

### 10. Validate Configuration

**POST** `/config/{service_name}/validate`

Validate configuration without applying it.

**Request Body:**
```json
{
  "config": {
    "server": {
      "host": "0.0.0.0",
      "port": 8080,
      "workers": 4
    }
  }
}
```

**Response:**
```json
{
  "valid": true,
  "errors": [],
  "warnings": []
}
```

## Notification Webhook

When subscribed to configuration changes, the config service will send notifications to the registered callback URL.

**POST** `{callback_url}`

**Webhook Payload:**
```json
{
  "event": "update",
  "service": "apigateway",
  "version": 13,
  "timestamp": "2025-07-04T10:30:00Z",
  "changes": [
    {
      "key": "services.comsrv.url",
      "old_value": "http://localhost:8001",
      "new_value": "http://localhost:8091"
    }
  ]
}
```

## Error Responses

All error responses follow this format:
```json
{
  "success": false,
  "error": {
    "code": "CONFIG_NOT_FOUND",
    "message": "Configuration not found for service: unknown_service",
    "details": null
  }
}
```

Common error codes:
- `CONFIG_NOT_FOUND`: Service configuration doesn't exist
- `INVALID_KEY`: Configuration key is invalid
- `VALIDATION_ERROR`: Configuration validation failed
- `VERSION_CONFLICT`: Configuration version conflict
- `UNAUTHORIZED`: Missing or invalid service name header