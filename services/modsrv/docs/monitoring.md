# Rule Execution Monitoring and Observability

This document describes the monitoring and observability features of the Model Service. These features help track rule execution metrics, maintain execution history, and monitor system health.

## Overview

The monitoring system offers the following capabilities:

1. **Rule Execution Metrics**: Collect and analyze metrics for rule executions including execution counts, success rates, and timing information.
2. **Execution History**: Track historical rule executions with detailed information about inputs, outputs, and errors.
3. **Health Monitoring**: Monitor the overall system health with detailed checks and automatic recovery.
4. **Structured Logging**: Enhanced logging capabilities for better observability.

## API Endpoints

### Rule Metrics

#### Get metrics for all rules

```
GET /api/metrics
```

Returns a map of rule IDs to metric objects containing:

- Total executions count
- Success and failure counts
- Success rate
- Execution timing statistics (min, max, avg)
- Last execution timestamp

#### Get metrics for a specific rule

```
GET /api/rules/{ruleId}/metrics
```

Returns detailed metrics for a specific rule.

### Rule Execution History

```
GET /api/rules/{ruleId}/history?limit=10
```

Returns the execution history for a specific rule, with an optional limit parameter to control the number of entries returned.

Each history entry includes:

- Execution timestamp
- Duration
- Success status
- Input context
- Output result
- Error message (if failed)

### Health Monitoring

#### Basic health check

```
GET /api/health
```

Returns a simple status code indicating if the service is running.

#### Detailed health check

```
GET /api/health/detailed
```

Returns detailed health information:

- Overall health status (Healthy, Degraded, Unhealthy)
- System uptime
- Memory usage
- Number of rules loaded
- Redis connection status
- Individual component health checks

## Using Monitoring Features

### Tracking Rule Performance

The monitoring system automatically tracks metrics for all rule executions. These metrics can be used to:

1. Identify slow-running rules
2. Monitor success rates for specific rules
3. Track execution patterns over time

Example usage:

```javascript
// Get metrics for all rules
fetch('/api/metrics')
  .then(response => response.json())
  .then(metrics => {
    // Find rules with high failure rates
    const problematicRules = Object.entries(metrics)
      .filter(([_, m]) => m.success_rate < 0.9)
      .map(([id, _]) => id);
  
    console.log('Rules with high failure rates:', problematicRules);
  });
```

### Debugging Rule Execution

When a rule fails, you can use the history API to investigate:

```javascript
// Get recent execution history for a rule
fetch('/api/rules/my-rule-id/history?limit=5')
  .then(response => response.json())
  .then(history => {
    // Check the most recent execution
    const lastExecution = history[0];
    console.log('Last execution status:', lastExecution.success ? 'Success' : 'Failed');
    if (!lastExecution.success) {
      console.log('Error:', lastExecution.error);
      console.log('Input context:', lastExecution.context);
    }
  });
```

### Monitoring System Health

You can integrate the health check endpoint into your monitoring system:

```javascript
// Check system health
fetch('/api/health/detailed')
  .then(response => response.json())
  .then(health => {
    if (health.status !== 'Healthy') {
      console.warn('System health is degraded:', health.status);
    
      // Check specific components
      const unhealthyChecks = Object.entries(health.checks)
        .filter(([_, check]) => check.status !== 'Healthy')
        .map(([id, check]) => ({ id, details: check.details }));
    
      console.warn('Unhealthy components:', unhealthyChecks);
    }
  });
```

## Automatic Recovery

The monitoring system includes automatic recovery mechanisms for certain failure scenarios:

1. **Redis Connection Issues**: The system will attempt to reconnect to Redis if the connection is lost.
2. **Rule Execution Failures**: Individual rule failures are isolated and won't affect the execution of other rules.

## Retention Policies

By default, the system maintains:

- The most recent 1000 execution history entries across all rules
- All metrics, which are persisted to Redis for durability

These limits can be configured in the application settings.

## Integration with External Systems

The metrics and health endpoints are designed to be compatible with common monitoring systems such as Prometheus, Grafana, and ELK stack.

For Prometheus integration, consider using the Prometheus Redis exporter to expose Redis metrics, including the rule execution metrics stored in Redis.

## Log Levels

The application uses structured logging with the following levels:

- **ERROR**: Critical issues that require immediate attention
- **WARN**: Potentially problematic situations that might require attention
- **INFO**: Important events and status updates
- **DEBUG**: Detailed information for debugging purposes

The log level can be configured in the `docker-compose.yml` file:

```yaml
environment:
  - RUST_LOG=debug  # Set to info, debug, or trace as needed
```
