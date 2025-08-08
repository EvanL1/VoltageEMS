# AlarmSrv Data Structure Design (Current Implementation)

## Overview
The alarm system uses a simple event-driven architecture with separated alarm rules and business rules to avoid key conflicts.

## 1. Alarm Rule Structure

```
Key: alarm:rule:{rule_id}
Type: Hash
Fields:
  id            -> "rule_id"                    # Rule identifier
  source_key    -> "comsrv:1:T" | "model:xxx:measurement"  # Data source
  field         -> "101" | "temperature"        # Field to monitor
  threshold     -> "85.0"                       # Trigger threshold value
  operator      -> ">" | "<" | "==" | ">=" | "<=" | "!="   # Comparison operator
  enabled       -> "true" | "false"             # Rule enable status
  alarm_level   -> "Critical" | "Major" | "Minor" | "Warning"
  alarm_title   -> "High Temperature Alarm"     # Alarm title
  created_at    -> "1754530782"                 # Creation timestamp

Example:
  alarm:rule:temp_high_1
    id -> "temp_high_1"
    source_key -> "comsrv:1:T"
    field -> "101"
    threshold -> "85"
    operator -> ">"
    enabled -> "true"
    alarm_level -> "Critical"
    alarm_title -> "Transformer 1 High Temp"
```

## 2. Alarm Instance Structure

```
Key: alarm:{rule_id}
Type: Hash
Fields:
  status        -> "active" | "cleared"         # Current alarm status
  rule_id       -> "rule_id"                    # Associated rule ID
  source_key    -> "comsrv:1:T"                 # Data source
  field         -> "101"                        # Monitored field
  trigger_value -> "86.2"                       # Value when triggered
  current_value -> "87.5"                       # Latest value (quasi-real-time)
  threshold     -> "85"                         # Threshold value
  operator      -> ">"                          # Operator used
  triggered_at  -> "1754530782"                 # Trigger timestamp
  cleared_at    -> "1754530900"                 # Clear timestamp (optional)
  updated_at    -> "1754530950"                 # Last update time
  clear_reason  -> "rule_disabled" | "condition_cleared"   # Clear reason (optional)

Example:
  alarm:temp_high_1
    status -> "active"
    rule_id -> "temp_high_1"
    source_key -> "comsrv:1:T"
    field -> "101"
    trigger_value -> "86.2"
    current_value -> "88.5"
    threshold -> "85"
    operator -> ">"
    triggered_at -> "1754530782"
```

## 3. Index Structures

```
# All alarm rules index
Key: alarm:rule:index
Type: Set
Members: [rule_id1, rule_id2, ...]

# Active alarms index
Key: idx:alarm:active
Type: Set
Members: [rule_id1, rule_id2, ...]

# Data point monitoring index (reverse index)
Key: idx:alarm:watch:{source_key}:{field}
Type: Set
Members: [rule_id1, rule_id2, ...]  # All rules monitoring this data point

# Alarm events queue
Key: alarm:events
Type: List
Elements: JSON events (last 1000 events)
```

## 4. Real-time Data Sources

```
# Communication service telemetry data
Key: comsrv:{channel_id}:T
Type: Hash
Fields:
  {point_id} -> value
  _updated_at -> timestamp

# Model service measurement data
Key: model:{model_id}:measurement
Type: String (JSON)
Value: {"field1": value1, "field2": value2, ...}
```

## 5. Event-Driven Alarm Flow

### 5.1 Data Update Triggers Alarm Check
```lua
-- In comsrv_write_telemetry or modsrv_sync_measurement
check_alarm_for_value(source_key, field, value)
```

### 5.2 Simple Alarm State Machine
```lua
function check_alarm_for_value(source_key, field, value)
    -- Find all rules watching this data point
    local rules = redis.call('SMEMBERS', 'idx:alarm:watch:' .. source_key .. ':' .. field)
    
    for each rule_id in rules:
        -- Get rule config
        local rule = redis.call('HMGET', 'alarm:rule:' .. rule_id, ...)
        
        -- Skip if disabled
        if rule.enabled == 'false' then continue
        
        -- Evaluate condition
        local condition_met = evaluate(value, rule.operator, rule.threshold)
        
        -- State transitions
        if condition_met and not active:
            trigger_alarm(rule_id, value)
        elseif not condition_met and active:
            clear_alarm(rule_id, value)
        elseif active:
            update_current_value(rule_id, value)
    end
end
```

## 6. API Operations

### 6.1 Rule Management
```bash
# Create rule
FCALL alarmsrv_create_rule 0 "rule_id" '{"source_key":"...", "field":"...", ...}'

# Enable/Disable rule
FCALL alarmsrv_enable_rule 0 "rule_id"
FCALL alarmsrv_disable_rule 0 "rule_id"

# Delete rule
FCALL alarmsrv_delete_rule 0 "rule_id"

# List rules
FCALL alarmsrv_list_rules 0
```

### 6.2 Alarm Queries
```bash
# List active alarms
FCALL alarmsrv_list_active_alarms 0

# Get alarm details
FCALL alarmsrv_get_alarm 0 "rule_id"

# Get statistics
FCALL alarmsrv_get_statistics 0
```

### 6.3 Quick Status Check
```bash
# Check if any alarms are active
SCARD idx:alarm:active

# Get active alarm IDs
SMEMBERS idx:alarm:active

# Check specific rule status
HGET alarm:rule:rule_id enabled
HGET alarm:rule_id status
```

## 7. Key Design Principles

1. **Event-Driven**: Alarms are checked when data updates, not by polling
2. **Simple State Machine**: Only two states - active/cleared
3. **Index-Based**: O(1) rule lookup using reverse indexes
4. **Separated Namespaces**: `alarm:*` for alarms, `rule:*` for business rules
5. **No Data Duplication**: Real-time values stay in source, only snapshots in alarms
6. **Queue Limits**: Event queues limited to prevent memory overflow

## 8. Performance Optimizations

- Use indexes instead of SCAN/KEYS
- Hash structures for O(1) field access
- Reverse indexes for efficient rule lookup
- Limited queue length (LTRIM)
- Skip disabled rules early in check

## 9. Configuration Example

```json
{
  "source_key": "comsrv:1:T",
  "field": "101",
  "threshold": 85,
  "operator": ">",
  "alarm_level": "Critical",
  "alarm_title": "High Temperature",
  "enabled": true
}
```

## 10. Complete Usage Flow

### Step 1: Create Alarm Rule
```bash
redis-cli FCALL alarmsrv_create_rule 0 "temp_high_1" '{
  "source_key": "comsrv:1:T",
  "field": "101",
  "threshold": 85,
  "operator": ">",
  "alarm_level": "Critical",
  "alarm_title": "High Temperature Alert",
  "enabled": true
}'
```

### Step 2: Write Data (Triggers Alarm Check)
```bash
# This will automatically check all rules watching comsrv:1:T field 101
redis-cli FCALL comsrv_write_telemetry 1 "comsrv:1:T" '{"101": 90}'
```

### Step 3: Check Active Alarms
```bash
# Quick check
redis-cli SCARD idx:alarm:active

# Get details
redis-cli FCALL alarmsrv_list_active_alarms 0
```

### Step 4: Manage Rules
```bash
# Disable rule (clears active alarm if any)
redis-cli FCALL alarmsrv_disable_rule 0 "temp_high_1"

# Re-enable rule
redis-cli FCALL alarmsrv_enable_rule 0 "temp_high_1"

# Delete rule
redis-cli FCALL alarmsrv_delete_rule 0 "temp_high_1"
```

## 11. Differences from Old Design

| Old Design | Current Implementation |
|------------|------------------------|
| `rule:{rule_id}` keys | `alarm:rule:{rule_id}` to avoid conflicts |
| Complex delay/count triggers | Simple immediate trigger |
| Separate clear threshold | Same threshold for trigger/clear |
| Acknowledgment workflow | Not implemented (can be added) |
| Multiple alarm states | Only active/cleared states |
| Polling-based checks | Event-driven on data updates |