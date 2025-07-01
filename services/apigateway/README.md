# API Gateway Service

API Gateway service for VoltageEMS that provides a unified HTTP interface for all microservices.

## Overview

The API Gateway serves as the single entry point for all client requests, routing them to appropriate backend services (comsrv, modsrv, hissrv, netsrv, alarmsrv).

## Features

- Unified REST API endpoints under `/api/v1/{service}`
- Request routing to backend services
- Redis connection for shared data access
- CORS support for web clients
- Health check endpoints
- Request/response logging
- Error handling and status propagation

## Configuration

Configuration is loaded from `apigateway.yaml`:

```yaml
server:
  host: "0.0.0.0"
  port: 8080
  workers: 4

redis:
  url: "redis://127.0.0.1:6379"
  
services:
  comsrv:
    url: "http://localhost:8081"
  # ... other services
```

## API Endpoints

### Health Check
- `GET /health` - Service health status
- `GET /api/v1/health` - Detailed health check with dependencies

### Service Routing
- `/api/v1/comsrv/*` - Communication service endpoints
- `/api/v1/modsrv/*` - Model service endpoints
- `/api/v1/hissrv/*` - Historical service endpoints
- `/api/v1/netsrv/*` - Network service endpoints
- `/api/v1/alarmsrv/*` - Alarm service endpoints

## Running the Service

```bash
# Development
RUST_LOG=debug cargo run

# With custom config
cargo run -- --config custom.yaml

# Production
cargo build --release
./target/release/apigateway
```

## Testing

```bash
# Run tests
cargo test

# Test health endpoint
curl http://localhost:8080/health

# Test service routing
curl http://localhost:8080/api/v1/comsrv/status
```