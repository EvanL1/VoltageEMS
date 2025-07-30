# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Version Information

All services and libraries in the VoltageEMS project use version 0.0.1 as the initial pre-demo version:
- libs (voltage-libs): 0.0.1
- alarmsrv: 0.0.1
- apigateway: 0.0.1
- comsrv: 0.0.1
- hissrv: 0.0.1
- modsrv: 0.0.1
- netsrv: 0.0.1
- rulesrv: 0.0.1

## Common Development Commands

### Workspace-Level Commands

```bash
# Check compilation without building (preferred over cargo build)
cargo check --workspace

# Format all code
cargo fmt --all

# Run clippy linting on all services
cargo clippy --all-targets --all-features -- -D warnings

# Build entire workspace (only when necessary)
cargo build --workspace

# Run all tests
cargo test --workspace

# Run specific service tests
cargo test -p {service_name}

# Build in release mode
cargo build --release --workspace
```

### Service-Specific Commands

```bash
# Build and run individual service
cd services/{service_name}
cargo build
cargo run

# Run with specific log level
RUST_LOG=debug cargo run
RUST_LOG={service_name}=debug cargo run

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name -- --exact --nocapture

# Watch for changes and auto-rebuild
cargo watch -x run
```

### Docker Development Environment

```bash
# Start complete test environment for a service
cd services/{service_name}
docker-compose -f docker-compose.test.yml up -d

# Monitor service logs
docker-compose -f docker-compose.test.yml logs -f {service_name}

# Stop test environment
docker-compose -f docker-compose.test.yml down

# Run complete integration tests
./scripts/run-integration-tests.sh
```

### Redis Operations

```bash
# Start Redis for development
docker run -d --name redis-dev -p 6379:6379 redis:7-alpine

# Monitor Redis activity
redis-cli monitor | grep {service_name}

# Check Hash data
redis-cli hgetall "comsrv:1001:m"      # View all measurements for channel 1001
redis-cli hget "comsrv:1001:m" "10001" # Get single point value
redis-cli hlen "comsrv:1001:m"         # Count points in channel

# Monitor Pub/Sub
redis-cli psubscribe "comsrv:*"        # Monitor all comsrv channels
redis-cli subscribe "comsrv:1001:m"    # Monitor specific channel
```

### Python Scripts (使用uv环境)

```bash
# Run Python scripts in uv environment
uv run python scripts/script_name.py

# Install dependencies
uv pip install -r requirements.txt
```

## Redis Development Notes

- Default development Redis: `redis:8-alpine`
  - Quick start: `docker run -d --name redis-dev -p 6379:6379 redis:8-alpine`