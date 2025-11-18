# Rule Execution Logging

## Overview

Rule execution logging system provides detailed tracking of rule engine operations, including:
- **Rule Trigger Events**: When conditions are met and actions are executed
- **Condition Evaluation**: Whether rules pass or fail condition checks
- **Action Execution Results**: Success/failure status of each action
- **Performance Metrics**: Execution duration for each rule
- **Error Tracking**: Detailed error messages for debugging

## Architecture

```
logs/rulesrv/rules/
├── battery_protect/
│   └── battery_protect.log.2025-10-09
├── pv_optimize/
│   └── pv_optimize.log.2025-10-09
└── load_balance/
    └── load_balance.log.2025-10-09
```

Each rule has its own directory and log files, making it easy to track individual rule behavior.

## Log Formats

### 1. Rule Triggered (Conditions Met)

```
[2025-10-09 14:30:15.456] TRIGGERED | execution_id=20251009_143015_battery_protect, duration_ms=125, instance=battery_01
  Conditions: MET
  Actions executed: 2/2 success
- set_value: instance=battery_01 [SUCCESS]
- log_message: message=Battery protection activated [SUCCESS]
```

**Fields**:
- `execution_id`: Unique identifier for this execution (timestamp + rule_id)
- `duration_ms`: Total execution time in milliseconds
- **Metadata**: Extracted from action results (instance, user, etc.)
- `Conditions`: MET or NOT_MET
- `Actions executed`: Success count / Total count
- **Action details**: Each action with type, result, and status

### 2. Rule Evaluated (Conditions Not Met)

```
[2025-10-09 14:30:20.123] EVALUATED | reason=conditions_not_met, duration_ms=10
```

**Fields**:
- `reason`: Why the rule didn't trigger (usually "conditions_not_met")
- `duration_ms`: Evaluation time

### 3. Rule Execution Error

```
[2025-10-09 14:30:25.789] ERROR | error=Redis connection timeout, duration_ms=5050
```

**Fields**:
- `error`: Error message (newlines replaced with spaces)
- `duration_ms`: Time before error occurred

## Rule Configuration

Rule execution interval is configured in the main service configuration:

```yaml
# config/rulesrv/rulesrv.yaml
execution:
  interval_seconds: 5  # Execute rules every 5 seconds
```

## Log Rotation

Rule logs use the same rotation policy as other services:
- **Daily Rotation**: New log file created each day
- **Compression**: Logs older than 7 days are compressed to `.gz`
- **Deletion**: Compressed logs older than 365 days are deleted

## Monitoring and Analysis

### View Real-time Rule Logs

```bash
# Watch all rules
tail -f logs/rulesrv/rules/*/*.log.*

# Watch specific rule
tail -f logs/rulesrv/rules/battery_protect/*.log.*
```

### Search for Rule Triggers

```bash
# Find all triggers for a specific rule
grep "TRIGGERED" logs/rulesrv/rules/battery_protect/*.log.*

# Find failed actions
grep "FAILED" logs/rulesrv/rules/*/*.log.*

# Find errors
grep "ERROR" logs/rulesrv/rules/*/*.log.*
```

### Extract Performance Metrics

```bash
# Find slow rule executions (>100ms)
grep "duration_ms" logs/rulesrv/rules/*/*.log.* | awk -F'duration_ms=' '{print $2}' | awk '{if($1>100) print}'

# Count triggers per rule today
for rule in logs/rulesrv/rules/*/; do
    count=$(grep -c "TRIGGERED" $rule/*.log.$(date +%Y-%m-%d) 2>/dev/null || echo 0)
    echo "$(basename $rule): $count triggers"
done
```

## Integration with Action Executor

When actions are executed, the results are automatically logged. The `ActionResult` structure includes:

```rust
pub struct ActionResult {
    pub action_type: String,      // "set_value", "log_message", etc.
    pub result: String,            // Action-specific result message
    pub success: bool,             // Whether action succeeded
    pub error: Option<String>,     // Error message if failed
}
```

### Example Action Results

**Set Value Action**:
```
- set_value: instance=battery_01, point=10, value=1 [SUCCESS]
```

**Log Message Action**:
```
- log_message: message=System mode changed to grid [SUCCESS]
```

**Failed Action**:
```
- set_value: instance=inverter_02 [FAILED] error: Instance not found
```

## Rule History

In addition to file logs, rulesrv maintains an in-memory execution history (last 1000 executions) accessible via API:

```bash
# Get execution history
curl http://localhost:6003/api/executions

# Get rule statistics
curl http://localhost:6003/api/statistics
```

## Performance Considerations

- **Log Frequency**: Each rule execution creates one log entry if triggered or evaluated
- **Disk Space**: Typical rule logs are 100-500 bytes per execution
- **Daily Volume**: With 5-second intervals and 10 rules:
  - Maximum entries: 10 rules × (86400 / 5) = 172,800 entries/day
  - Typical size: 17-86 MB/day uncompressed, 2-10 MB compressed

## Troubleshooting

### No Logs Generated

1. Check rule execution interval: `cat config/rulesrv/rulesrv.yaml | grep interval`
2. Verify rules are enabled: `curl http://localhost:6003/api/rules/cached`
3. Check logs directory permissions: `ls -la logs/rulesrv/rules/`

### Rules Not Triggering

1. Check rule conditions are met: Review the `EVALUATED` entries
2. Verify data availability: Check Redis for required data points
3. Enable debug logging: `RUST_LOG=debug cargo run --bin rulesrv`

### Missing Action Results

1. Verify actions are properly configured in rule YAML
2. Check action executor logs in main service log
3. Look for ERROR entries in rule log files

## Example Rule Execution Flow

1. **Rule Evaluation Start** (every 5 seconds by default)
2. **Trigger Check**: Check if rule should trigger (schedule, cooldown)
3. **Condition Evaluation**: Evaluate condition group
4. **If Conditions Met**:
   - Execute all actions sequentially
   - Record results for each action
   - Log TRIGGERED entry with full details
   - Update in-memory history
5. **If Conditions Not Met**:
   - Log EVALUATED entry
6. **On Error**:
   - Log ERROR entry with error details

## API Integration

Future enhancements may include:
- REST API to query rule execution history
- WebSocket for real-time rule execution events
- Dashboard for rule performance analysis
- Alert system for rule failures
