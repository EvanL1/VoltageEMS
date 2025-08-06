# VoltageEMS Testing Environment

This directory contains a comprehensive Docker-based testing infrastructure for VoltageEMS, designed to support unit tests, integration tests, load testing, and end-to-end testing scenarios.

## Overview

The testing environment provides:

- **Multi-container test setups** with isolated environments
- **Service dependencies** including Redis with Lua Functions, InfluxDB, and Modbus simulators
- **Test data management** with fixtures, mocks, and generators
- **Performance monitoring** with Prometheus, Grafana, and custom metrics
- **CI/CD integration** with GitHub Actions workflows
- **Comprehensive reporting** with HTML, JSON, and performance analysis

## Quick Start

### Prerequisites

- Docker and Docker Compose
- Bash shell
- Git

### Running All Tests

```bash
# From the tests directory
./run-tests.sh

# Or run specific test types
./run-tests.sh --type unit
./run-tests.sh --type integration  
./run-tests.sh --type load
```

### Running Tests from Project Root

```bash
# Run the existing test script (unit tests only)
./scripts/test.sh

# Or use the comprehensive test runner
./tests/run-tests.sh --type all --verbose
```

## Test Environment Architecture

### Directory Structure

```
tests/
├── run-tests.sh                 # Main test execution script
├── docker/                      # Test container Dockerfiles
│   ├── Dockerfile.unit-tests
│   ├── Dockerfile.integration-tests
│   ├── Dockerfile.integration-runner
│   └── Dockerfile.test-collector
├── fixtures/                    # Test data and initialization
│   ├── Dockerfile.data-init
│   └── Dockerfile.modbus-generator
├── mocks/                       # Mock external services
│   ├── Dockerfile.notification-mock
│   └── Dockerfile.scada-mock
├── integration/                 # Integration test configurations
│   ├── requirements.txt
│   └── config/
├── e2e/                        # End-to-end test scenarios
│   └── Dockerfile.e2e-tests
├── load/                       # Load testing configurations
│   ├── k6/
│   ├── artillery/
│   ├── jmeter/
│   ├── monitoring/
│   └── Dockerfile.analyzer
├── data/                       # Generated test data
└── reports/                    # Test results and reports
```

## Test Types

### 1. Unit Tests

**Purpose**: Test individual service components in isolation.

**Configuration**: `docker-compose.test.yml`

**Services**: 
- Redis with Lua Functions
- Individual service test containers
- Test data initializer

**Usage**:
```bash
./run-tests.sh --type unit
```

**Features**:
- Isolated Redis instance on port 6380
- Pre-loaded Lua Functions
- Service-specific test containers
- Parallel test execution support

### 2. Integration Tests

**Purpose**: Test cross-service communication and workflows.

**Configuration**: `docker-compose.integration.yml`

**Services**:
- Complete VoltageEMS service stack
- Multiple Modbus simulators
- Mock external services (SCADA, notifications)
- Integration test runner
- E2E test scenarios

**Usage**:
```bash
./run-tests.sh --type integration --verbose
```

**Test Scenarios**:
- **Data Flow**: Modbus → comsrv → Redis → hissrv → InfluxDB
- **Alarm Workflow**: Condition detection → alarm creation → notification
- **Rule Engine**: Rule evaluation → action execution
- **API Gateway**: Request routing and aggregation

### 3. Load Tests

**Purpose**: Evaluate performance under high load and stress conditions.

**Configuration**: `docker-compose.load.yml`

**Services**:
- Scaled VoltageEMS services (multiple replicas)
- Load testing tools (K6, Artillery, JMeter)
- Performance monitoring (Prometheus, Grafana)
- Load test analyzer

**Usage**:
```bash
./run-tests.sh --type load --no-cleanup
```

**Load Testing Tools**:

#### K6 (JavaScript-based)
- API endpoint testing
- Custom metrics and thresholds
- Ramp-up scenarios

#### Artillery.io
- Mixed workload testing
- WebSocket testing
- Advanced scenarios

#### JMeter
- Traditional load testing
- GUI and command-line execution
- Detailed reporting

**Monitoring**:
- **Grafana**: Real-time dashboards (http://localhost:3001)
- **Prometheus**: Metrics collection (http://localhost:9090)
- **Custom metrics**: Response times, error rates, throughput

## Test Data Management

### Fixtures and Initialization

The test environment automatically initializes with realistic test data:

**Redis Data**:
- 3 test channels (1001, 1002, 1003)
- Telemetry, Signal, Control, and Adjustment points
- Test models and configurations
- Sample alarms and rules

**InfluxDB Data**:
- 24 hours of historical data
- 5-minute intervals
- Multiple measurement types
- Realistic value ranges

**CSV Configurations**:
- Point definitions with proper scaling
- Protocol mappings
- Channel configurations

### Mock Services

**Notification Service**:
- REST API for notification testing
- Configurable success/failure rates
- Webhook support for alarm integration

**SCADA System**:
- Device simulation and control
- WebSocket real-time updates
- Command execution simulation
- Alarm condition generation

### Data Generators

**Modbus Data Generator**:
- Realistic telemetry patterns
- Daily cycles and trends
- Alarm condition simulation
- Multiple device simulation

## CI/CD Integration

### GitHub Actions Workflow

File: `.github/workflows/test.yml`

**Triggers**:
- Push to main/develop branches
- Pull requests
- Daily scheduled runs (2 AM UTC)

**Jobs**:
1. **Code Quality**: Formatting, linting, compilation
2. **Unit Tests**: Fast feedback on core functionality
3. **Integration Tests**: Full system testing
4. **Load Tests**: Performance validation (main branch only)
5. **Security Scans**: Vulnerability detection
6. **Test Reporting**: Results aggregation and PR comments

**Features**:
- Parallel execution for faster feedback
- Test result artifacts
- Performance regression detection
- Security vulnerability scanning
- Automatic PR comments with results

## Configuration Options

### Test Runner Options

```bash
./run-tests.sh [OPTIONS]

Options:
  -t, --type TYPE        Test type: unit|integration|load|all (default: all)
  -c, --no-cleanup      Don't cleanup containers after tests
  -v, --verbose         Verbose output
  -s, --sequential      Run tests sequentially instead of parallel
  -n, --no-save         Don't save test results
  -h, --help            Show help message
```

### Environment Variables

**Global**:
- `RUST_LOG`: Logging level (default: debug)
- `REDIS_URL`: Redis connection URL
- `INFLUXDB_URL`: InfluxDB connection URL

**Service-Specific**:
- `COMSRV_REDIS_URL`: comsrv Redis URL
- `HISSRV_INFLUXDB_TOKEN`: hissrv InfluxDB token
- `CSV_BASE_PATH`: Path to CSV configuration files

### Docker Compose Overrides

You can create `docker-compose.override.yml` files to customize configurations:

```yaml
version: '3.8'
services:
  redis-test:
    ports:
      - "6379:6379"  # Expose on standard port
  
  comsrv-test:
    environment:
      - RUST_LOG=trace  # More verbose logging
```

## Debugging and Troubleshooting

### Common Issues

**1. Port Conflicts**
```bash
# Check for conflicting services
docker ps
netstat -tlnp | grep :6379

# Use different ports in test configurations
```

**2. Redis Functions Not Loaded**
```bash
# Manually load functions
cd scripts/redis-functions
./load_functions.sh

# Verify functions
./verify_functions.sh
```

**3. Service Health Check Failures**
```bash
# Check service logs
docker logs voltageems-comsrv-test

# Manual health check
curl http://localhost:6000/health
```

**4. Test Data Issues**
```bash
# Reinitialize test data
docker-compose -f docker-compose.test.yml run --rm test-data-init

# Check Redis data
redis-cli -p 6380 keys "*"
```

### Debugging Commands

```bash
# View running containers
docker ps -a

# Check container logs
docker logs -f [container-name]

# Access container shell
docker exec -it [container-name] /bin/bash

# Monitor resource usage
docker stats

# Network inspection
docker network ls
docker network inspect voltageems-test-network
```

### Log Analysis

Test containers generate detailed logs:

**Unit Tests**: `/app/test-results/unit-test-output.json`
**Integration Tests**: `/app/results/integration_test_results.json`
**Load Tests**: `/app/results/[tool]-load-results.json`

## Performance Optimization

### Resource Allocation

```yaml
# Example resource limits
services:
  comsrv-load:
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 2G
        reservations:
          cpus: '1.0'
          memory: 1G
```

### Caching Strategy

The test environment uses Docker layer caching and Rust build caching:

```dockerfile
# Dependency caching
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release --dependencies-only

# Source code changes
COPY src ./src
RUN cargo build --release
```

### Parallel Execution

Tests are designed for parallel execution:

- **Unit tests**: Service-isolated containers
- **Integration tests**: Separate test scenarios
- **Load tests**: Multiple load generators

## Monitoring and Metrics

### Built-in Metrics

**Performance Metrics**:
- Response time percentiles (P50, P95, P99)
- Throughput (requests per second)
- Error rates and success rates
- Resource utilization (CPU, memory, network)

**Business Metrics**:
- Data flow completion rates
- Alarm detection accuracy
- Rule execution success
- API endpoint availability

### Custom Dashboards

Grafana dashboards are pre-configured for:

- **System Overview**: Resource usage and health
- **Service Performance**: Response times and throughput  
- **Error Tracking**: Error rates and patterns
- **Load Test Results**: Real-time load test monitoring

### Alerting

Prometheus alerting rules for:

- High error rates (>5%)
- Slow response times (>500ms P95)
- Resource exhaustion (>80% CPU/memory)
- Service unavailability

## Extending the Test Environment

### Adding New Test Scenarios

1. Create test container Dockerfile
2. Add service definition to appropriate docker-compose file
3. Update test runner script
4. Add CI/CD job if needed

### Custom Mock Services

```python
# Example mock service
from fastapi import FastAPI

app = FastAPI()

@app.get("/health")
async def health():
    return {"status": "healthy"}

@app.post("/webhook")
async def webhook(data: dict):
    # Process webhook
    return {"received": True}
```

### Additional Load Test Scenarios

```javascript
// K6 custom test scenario
export const options = {
  scenarios: {
    custom_scenario: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '2m', target: 100 },
        { duration: '5m', target: 100 },
        { duration: '2m', target: 0 },
      ],
    },
  },
};
```

## Best Practices

### Test Design

1. **Isolation**: Each test should be independent
2. **Repeatability**: Tests should produce consistent results
3. **Data Management**: Use fixtures and clean up test data
4. **Error Handling**: Graceful degradation and error reporting

### Performance Testing

1. **Baseline**: Establish performance baselines
2. **Realistic Load**: Use production-like traffic patterns
3. **Monitoring**: Monitor all layers of the stack
4. **Analysis**: Deep dive into performance bottlenecks

### CI/CD Integration

1. **Fast Feedback**: Prioritize fast-running tests
2. **Parallel Execution**: Maximize concurrency safely
3. **Artifact Management**: Store and analyze test results
4. **Notifications**: Alert on test failures and regressions

## Contributing

When adding new tests or modifying the test infrastructure:

1. Follow the existing patterns and conventions
2. Update documentation and README files
3. Add appropriate CI/CD integration
4. Test your changes in isolation
5. Consider the impact on existing tests

## Troubleshooting Guide

### Test Failures

**Unit Test Failures**:
1. Check Redis connection and Lua Functions
2. Verify test data initialization
3. Review service configuration files

**Integration Test Failures**:
1. Verify all services are healthy
2. Check network connectivity between containers
3. Review service logs for errors

**Load Test Failures**:
1. Check resource availability
2. Verify load balancer configuration
3. Monitor system resource usage

### Performance Issues

**Slow Tests**:
1. Optimize test data size
2. Use parallel execution
3. Implement test caching

**Resource Exhaustion**:
1. Increase Docker resource limits
2. Optimize container images
3. Use resource monitoring

For additional support, refer to the service-specific documentation in the CLAUDE.md file or create an issue in the project repository.