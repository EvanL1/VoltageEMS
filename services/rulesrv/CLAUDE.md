# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Development Commands

### Build and Run

```bash
# Build the service
cargo build -p rulesrv

# Run with debug logging
RUST_LOG=debug,rulesrv=trace cargo run -p rulesrv -- service

# Run with specific config
cargo run -p rulesrv -- service --config config/production.yml

# Check compilation
cargo check -p rulesrv

# Run tests
cargo test -p rulesrv -- --nocapture

# Fix all warnings
cargo fix --lib -p rulesrv
```

### Testing Rules

```bash
# Create a simple test rule
curl -X POST http://localhost:8083/api/v1/rules \
  -H "Content-Type: application/json" \
  -d @test_rule_simple.json

# List all rules
curl http://localhost:8083/api/v1/rules

# Manually execute a rule
curl -X POST http://localhost:8083/api/v1/rules/{rule_id}/execute \
  -H "Content-Type: application/json" \
  -d '{"context": {"temperature": 35}}'

# Monitor rule execution
python test_scripts/test_rule_trigger.py --action monitor --rule-id temperature_alarm

# Publish test data to trigger rules
python test_data_formats.py
```

### Redis Operations

```bash
# Monitor Redis channels for rule triggers
redis-cli psubscribe "modsrv:outputs:*" "alarm:*"

# Check rule definitions
redis-cli keys "rule:*" | xargs -I {} redis-cli get {}

# Monitor alarm publications
redis-cli subscribe "alarm:temperature:high"

# Clear test data
redis-cli keys "rulesrv:*" | xargs redis-cli del
```

## Architecture Overview

rulesrv is a rule engine service that subscribes to Redis channels and executes rules based on incoming data. It supports two rule formats:

1. **Simple Rules**: Basic condition-action rules with simple comparison operators
2. **DAG Rules**: Complex directed acyclic graph rules with multiple nodes and edges

### Key Components

- **RedisSubscriber** (`redis/subscriber.rs`): Subscribes to Redis channels using psubscribe for pattern matching
- **RuleExecutor** (`engine/executor.rs`): Evaluates rule conditions and executes actions
- **RedisStore** (`redis/store.rs`): Manages rule storage and retrieval from Redis
- **API Server** (`api/mod.rs`): REST API for rule management (port 8083)

### Data Flow

1. **Data Input**: Other services publish data to Redis channels (e.g., `modsrv:outputs:temperature`)
2. **Subscription**: RedisSubscriber receives messages from subscribed channels
3. **Rule Evaluation**: RuleExecutor evaluates conditions against the data
4. **Action Execution**: If conditions are met, actions are executed (publish, control, notification)

### Rule Structure (Simple Rules)

```json
{
  "id": "temperature_alarm",
  "name": "Temperature Alarm Rule",
  "condition": "temperature > 30",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:temperature:high",
      "message": "Temperature exceeded 30°C"
    }
  ],
  "enabled": true,
  "priority": 10
}
```

### Supported Condition Operators

- Simple comparisons: `>`, `<`, `>=`, `<=`, `==`, `!=`
- Combined conditions: `&&`, `||` (planned, not yet implemented)

### Action Types

1. **publish**: Publish message to Redis channel
2. **control**: Send control command to devices (via Redis)
3. **notification**: Send notifications (webhook, email, SMS)

### Redis Key Patterns

- Rule definitions: `rule:{rule_id}`
- Rule list: `rulesrv:rules` (set)
- Rule groups: `rule_group:{group_id}`
- Group list: `rulesrv:rule_groups` (set)
- Execution history: `rulesrv:history:{rule_id}` (list)

### Important Implementation Notes

1. **Pattern Subscription**: Uses `psubscribe` instead of `subscribe` to support wildcard patterns
2. **Error Handling**: Uses custom `RulesrvError` type with conversions from `anyhow::Error`
3. **Router Paths**: Use `{param}` format, not `:param` (e.g., `/api/v1/rules/{rule_id}`)
4. **Redis Store**: Requires two parameters: `redis_url` and optional `key_prefix`

### Common Issues and Fixes

1. **Port Conflicts**: Default port changed from 8086 to 8083
2. **Type Mismatches**: Convert `anyhow::Result` to `Result<T, RulesrvError>` using `.map_err(|e| RulesrvError::RedisError(e.to_string()))`
3. **Dead Code Warnings**: Many unused functions are for future DAG rule implementation

## Testing Infrastructure

### Test Scripts Location

All test scripts are in `test_scripts/` directory:
- `test_data_publisher.py`: Publishes test data to Redis
- `test_rule_trigger.py`: Triggers rules via API
- `monitor_redis.py`: Monitors Redis activity
- `test_rule_definition.py`: Creates test rules

### Test Data Formats

The service expects data in JSON format on Redis channels:

```python
# Simple value
{"temperature": 35.5}

# Multiple values
{
  "temperature": 32.5,
  "humidity": 65.2,
  "timestamp": 1736981591526
}

# With metadata
{
  "value": 85.5,
  "quality": "good",
  "timestamp": 1736981591526
}
```

### Creating Test Rules

Use `test_rule_simple.json` as a template:

```json
{
  "rule": {
    "id": "test_rule_001",
    "name": "Test Rule",
    "condition": "value > 100",
    "actions": [{
      "type": "publish",
      "channel": "test:alarm",
      "message": "Test alarm triggered"
    }],
    "enabled": true,
    "priority": 50
  }
}
```

## Configuration

Main configuration file: `config/default.yml`

Key settings:
- API port: 8083 (not 8080 as in README)
- Redis URL: `redis://localhost:6379`
- Default subscriptions: `["modsrv:outputs:*", "alarm:event:*"]`

Environment variables can override config:
- `REDIS_URL`
- `API_PORT`
- `RUST_LOG`

## Future DAG Implementation

The codebase includes extensive DAG (Directed Acyclic Graph) rule infrastructure that's not yet fully implemented:
- `RuleNode`, `RuntimeRule` structures
- `NodeType` enum (Input, Condition, Transform, Action, Aggregate)
- Graph execution logic using `petgraph`

Currently, only simple rules (non-DAG) are functional via `execute_simple_rule` method.