---
name: voltage-ems-query
description: Use this skill when the user asks about VoltageEMS live data: channels, devices, points, real-time values, historical sensor data, alarms, rules, models, instances, routing, Redis RTDB, service health, or system status. Use monarch CLI commands to answer — do NOT inspect source code, local database files, or config YAMLs to answer questions about current platform state.
---

# VoltageEMS Query Skill

Use `monarch` as the primary interface for all live VoltageEMS platform data.

## Core Rules

- Execute `monarch` commands to answer questions about current platform state.
- Do not read source code files or SQLite database files to answer live data questions.
- Do not read config YAML files to answer questions about running device data.
- Always add `--json` when you need to parse or chain results.
- When `--host` is needed, pass it as a global flag (before the subcommand).

## Command Discovery

```bash
monarch --help
monarch <subcommand> --help        # e.g. monarch alarms --help
monarch channels --help
```

## Global Flags

| Flag | Purpose |
|------|---------|
| `--json` | Machine-readable output (suppresses color/banner) |
| `--host <ip>` | Remote target (e.g. `--host 192.168.30.21`) |
| `--verbose` | Debug-level logging |

## Authentication

`monarch` requires **no login**. All commands talk directly to local services.

## Connecting to a Remote Machine

```bash
# Option A — SSH (monarch lives on the remote machine)
ssh root@192.168.30.62 'monarch --json alarms list'

# Option B — local monarch binary + --host flag
#   (requires service ports 6001/6002/6004/6006 to be reachable from localhost)
monarch --host 192.168.30.62 --json alarms list
```

Prefer Option A when monarch is already installed on the target machine and you are not sure which ports are exposed.

## Command Map

### Channels (comsrv :6001)
```bash
monarch channels list
monarch channels list --json
monarch channels status <id>
monarch channels health
monarch channels points list <channel_id>
monarch channels points get <channel_id> <point_type> <point_id>
```

### Models — Products & Instances (modsrv :6002)
```bash
monarch models products list
monarch models products get <id>
monarch models instances list
monarch models instances list --product <product_id>
monarch models instances get <id>
```

### Business Rules (modsrv :6002)
```bash
monarch rules list
monarch rules get <id>
monarch rules executions <id>           # recent execution results
```

### Alarms (alarmsrv :6007)
```bash
monarch alarms list                     # active alerts
monarch alarms list --level 3           # high-severity only
monarch alarms list --channel <id>
monarch alarms list --keyword overvolt
monarch alarms get <id>
monarch alarms rules                    # alarm rule definitions
monarch alarms rules --enabled          # only enabled rules
monarch alarms rules --channel <id>
monarch alarms rule-get <id>
monarch alarms events                   # historical trigger+recovery events
monarch alarms events --level 3
monarch alarms events --event-type trigger
monarch alarms events --rule <rule_id>
monarch alarms stats                    # alert count by level
monarch alarms monitor                  # alarm monitor loop status
```

### Historical Data (hissrv :6004)
```bash
monarch history latest <redis_key> <point_id>
# e.g. monarch history latest inst:9:M 101

monarch history query <redis_key> <point_id>
monarch history query inst:9:M 101 --from 2026-05-01T00:00:00Z --to 2026-05-12T00:00:00Z
monarch history query inst:9:M 101 --page 2 --size 50

# Batch: query multiple points in one request (max 20 series)
monarch history batch --series inst:9:M,101 --series inst:9:M,102 --from 2026-05-01T00:00:00Z
monarch history batch --series inst:9:M,101 --series inst:12:M,201 --from 2026-05-01T00:00:00Z --to 2026-05-12T00:00:00Z --limit 500

monarch history channels                # channels with recorded history
monarch history metrics                 # storage backend statistics
monarch history health                  # hissrv health check
```

### Redis RTDB (direct :6379)
```bash
monarch rtdb get <key> <field>          # HGET key field
monarch rtdb scan <pattern>             # KEYS pattern
monarch rtdb inspect <key>              # HGETALL
monarch rtdb patterns                   # list known key patterns
```

### Routing
```bash
monarch routing list                    # channel→instance routing table
monarch routing get <channel_id>
```

### Service & System Health
```bash
monarch doctor                          # full system health check
monarch services status
monarch logs list
monarch logs view <service> -n 100
monarch logs tail <service> --grep ERROR
```

## Intent → Command Mapping

| User intent | Command |
|------------|---------|
| "哪些通道在线？" | `monarch channels list --json` |
| "通道 1001 状态" | `monarch channels status 1001 --json` |
| "有哪些产品？" | `monarch models products list --json` |
| "设备实例列表" | `monarch models instances list --json` |
| "当前告警有哪些？" | `monarch alarms list --json` |
| "高级别告警" | `monarch alarms list --level 3 --json` |
| "通道 1001 的告警规则" | `monarch alarms rules --channel 1001 --json` |
| "告警事件历史" | `monarch alarms events --json` |
| "inst:9:M 点位 101 最新值" | `monarch history latest inst:9:M 101 --json` |
| "过去一天数据" | `monarch history query inst:9:M 101 --from <yesterday-iso> --json` |
| "同时查多个点" | `monarch history batch --series inst:9:M,101 --series inst:12:M,201 --from <iso> --json` |
| "Redis 实时值" | `monarch rtdb get inst:9:M 101 --json` |
| "系统健康？" | `monarch doctor --json` |
| "规则执行结果" | `monarch rules executions <id> --json` |

## Multi-step Pattern

When a user provides a name but you need an ID:

```bash
# Step 1: find ID
monarch channels list --json
# Step 2: use ID in subsequent command
monarch alarms list --channel <id> --json
```

## Output Preference

- Always add `--json` when chaining commands or parsing results programmatically.
- JSON success envelope: `{"success": true, "data": ...}`
- JSON error envelope: `{"success": false, "error": "..."}`
- When presenting results to the user, extract `data` and format meaningfully.
- For alarm warning levels: 1 = low, 2 = medium, 3 = high.
- `redis_key` format: `inst:<instance_id>:M` for measurement points, `comsrv:<channel_id>:T` for telemetry.

## Redis Key Reference

| Key pattern | Meaning |
|-------------|---------|
| `inst:<id>:M` | Instance measurement points (modsrv writes) |
| `comsrv:<ch>:T` | Channel telemetry points (comsrv writes) |
| `comsrv:<ch>:S` | Channel status points |

## Guardrails

- **Read-only**: Do not run commands that mutate state (`channels control`, `channels adjust`, `rules enable/disable`, `rtdb set`, `rtdb del`) unless the user explicitly asks to make a change.
- **No source diving**: Never open `*.rs`, `*.yaml`, or `*.db` files to answer live data questions.
- **No Redis direct**: Prefer `monarch history` for historical data over raw `monarch rtdb` inspection.
- If a service is not reachable, report the error message from `monarch` directly; do not guess at data.

## Terminology

| Term | Meaning |
|------|---------|
| channel | Communication channel (Modbus, IEC104, etc.) |
| instance | Device instance (product + configuration) |
| point / point_id | Sensor or control register within a channel or instance |
| redis_key | Hash key in Redis RTDB, e.g. `inst:9:M` |
| alarm rule | Threshold rule that triggers an alert |
| alert | Currently active alarm condition |
| alert event | Completed trigger or recovery cycle |
| hissrv | History service storing time-series data in Postgres/InfluxDB/TimescaleDB |
| comsrv | Communication service managing protocol channels |
| modsrv | Model execution service managing instances and rules |
