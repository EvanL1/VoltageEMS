# REST API 文档

## 概述

VoltageEMS REST API 提供标准的 HTTP 接口用于系统配置、数据查询、设备管理等功能。所有 API 遵循 RESTful 设计原则。

## 基础信息

### Base URL

```
开发环境: http://localhost:6005/api/v1
生产环境: https://voltage-ems.com/api/v1
```

### 请求头

所有请求必须包含以下请求头：

```http
Authorization: Bearer {jwt_token}
Content-Type: application/json
Accept: application/json
```

### 响应格式

#### 成功响应

```json
{
  "success": true,
  "data": {},
  "message": "操作成功",
  "timestamp": "2025-08-12T10:30:00Z",
  "request_id": "req_12345"
}
```

#### 错误响应

```json
{
  "success": false,
  "error": {
    "code": "ERROR_CODE",
    "message": "错误描述",
    "details": "详细错误信息"
  },
  "timestamp": "2025-08-12T10:30:00Z",
  "request_id": "req_12345"
}
```

## 认证接口

### 用户登录

登录获取访问令牌。

**请求**

```http
POST /api/v1/auth/login
Content-Type: application/json

{
  "username": "admin",
  "password": "password"
}
```

**响应**

```json
{
  "success": true,
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIs...",
    "refresh_token": "eyJhbGciOiJIUzI1NiIs...",
    "token_type": "Bearer",
    "expires_in": 900,
    "user": {
      "user_id": "user_001",
      "username": "admin",
      "roles": ["admin"],
      "permissions": ["read", "write", "control"]
    }
  }
}
```

### 刷新令牌

使用刷新令牌获取新的访问令牌。

**请求**

```http
POST /api/v1/auth/refresh
Content-Type: application/json

{
  "refresh_token": "eyJhbGciOiJIUzI1NiIs..."
}
```

**响应**

```json
{
  "success": true,
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIs...",
    "expires_in": 900
  }
}
```

### 用户登出

注销当前会话。

**请求**

```http
POST /api/v1/auth/logout
Authorization: Bearer {token}
```

**响应**

```json
{
  "success": true,
  "message": "登出成功"
}
```

### 获取用户信息

获取当前登录用户的详细信息。

**请求**

```http
GET /api/v1/auth/profile
Authorization: Bearer {token}
```

**响应**

```json
{
  "success": true,
  "data": {
    "user_id": "user_001",
    "username": "admin",
    "email": "admin@voltage-ems.com",
    "roles": ["admin"],
    "permissions": {
      "devices": ["read", "write", "control"],
      "alarms": ["read", "acknowledge", "clear"],
      "config": ["read", "write"]
    },
    "created_at": "2025-01-01T00:00:00Z",
    "last_login": "2025-08-12T10:00:00Z"
  }
}
```

## 设备管理接口

### 获取设备列表

获取系统中所有设备的列表。

**请求**

```http
GET /api/v1/devices?page=1&size=20&status=online&area_id=north
Authorization: Bearer {token}
```

**查询参数**

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| page | integer | 否 | 页码，默认1 |
| size | integer | 否 | 每页数量，默认20 |
| status | string | 否 | 设备状态: online, offline, maintenance |
| area_id | string | 否 | 区域ID |
| device_type | string | 否 | 设备类型 |
| search | string | 否 | 搜索关键词 |

**响应**

```json
{
  "success": true,
  "data": {
    "items": [
      {
        "device_id": "PLC_001",
        "name": "1号PLC",
        "type": "plc",
        "status": "online",
        "area_id": "north",
        "channels": ["1001", "1002", "1003"],
        "protocol": "modbus_tcp",
        "address": "192.168.1.100:502",
        "last_update": "2025-08-12T10:30:00Z"
      }
    ],
    "total": 100,
    "page": 1,
    "size": 20,
    "total_pages": 5
  }
}
```

### 获取设备详情

获取指定设备的详细信息。

**请求**

```http
GET /api/v1/devices/{device_id}
Authorization: Bearer {token}
```

**响应**

```json
{
  "success": true,
  "data": {
    "device_id": "PLC_001",
    "name": "1号PLC",
    "type": "plc",
    "status": "online",
    "area_id": "north",
    "protocol": "modbus_tcp",
    "address": "192.168.1.100:502",
    "configuration": {
      "timeout": 3000,
      "retry_count": 3,
      "poll_interval": 1000
    },
    "channels": [
      {
        "channel_id": "1001",
        "name": "温度传感器组",
        "data_types": ["T"],
        "point_count": 10
      }
    ],
    "statistics": {
      "uptime": "7d 12h 30m",
      "last_error": null,
      "total_points": 30,
      "active_alarms": 0
    },
    "created_at": "2025-01-01T00:00:00Z",
    "updated_at": "2025-08-12T10:30:00Z"
  }
}
```

### 创建设备

创建新设备。

**请求**

```http
POST /api/v1/devices
Authorization: Bearer {token}
Content-Type: application/json

{
  "name": "2号PLC",
  "type": "plc",
  "area_id": "south",
  "protocol": "modbus_tcp",
  "address": "192.168.1.101:502",
  "configuration": {
    "timeout": 3000,
    "retry_count": 3,
    "poll_interval": 1000
  }
}
```

**响应**

```json
{
  "success": true,
  "data": {
    "device_id": "PLC_002",
    "message": "设备创建成功"
  }
}
```

### 更新设备

更新设备配置。

**请求**

```http
PUT /api/v1/devices/{device_id}
Authorization: Bearer {token}
Content-Type: application/json

{
  "name": "1号PLC（更新）",
  "configuration": {
    "timeout": 5000,
    "poll_interval": 2000
  }
}
```

**响应**

```json
{
  "success": true,
  "message": "设备更新成功"
}
```

### 删除设备

删除指定设备。

**请求**

```http
DELETE /api/v1/devices/{device_id}
Authorization: Bearer {token}
```

**响应**

```json
{
  "success": true,
  "message": "设备删除成功"
}
```

### 获取设备状态

获取设备实时状态。

**请求**

```http
GET /api/v1/devices/{device_id}/status
Authorization: Bearer {token}
```

**响应**

```json
{
  "success": true,
  "data": {
    "device_id": "PLC_001",
    "status": "online",
    "connection": {
      "connected": true,
      "last_ping": "2025-08-12T10:30:00Z",
      "latency": 5
    },
    "metrics": {
      "cpu_usage": 45.2,
      "memory_usage": 62.8,
      "error_rate": 0.01
    },
    "active_channels": 3,
    "total_points": 30,
    "last_update": "2025-08-12T10:30:00Z"
  }
}
```

## 实时数据接口

### 获取通道实时数据

获取指定通道的当前实时数据。

**请求**

```http
GET /api/v1/realtime/channels/{channel_id}
Authorization: Bearer {token}
```

**响应**

```json
{
  "success": true,
  "data": {
    "channel_id": "1001",
    "name": "温度传感器组",
    "device_id": "PLC_001",
    "data_type": "T",
    "values": [
      {
        "point_id": 1,
        "name": "温度1",
        "value": 25.6,
        "unit": "°C",
        "quality": "good",
        "timestamp": "2025-08-12T10:30:00Z"
      },
      {
        "point_id": 2,
        "name": "温度2",
        "value": 26.1,
        "unit": "°C",
        "quality": "good",
        "timestamp": "2025-08-12T10:30:00Z"
      }
    ],
    "last_update": "2025-08-12T10:30:00Z"
  }
}
```

### 批量查询实时数据

批量查询多个通道的实时数据。

**请求**

```http
POST /api/v1/realtime/batch-query
Authorization: Bearer {token}
Content-Type: application/json

{
  "channels": ["1001", "1002", "1003"],
  "data_types": ["T", "S"],
  "include_quality": true,
  "include_timestamp": true
}
```

**响应**

```json
{
  "success": true,
  "data": {
    "results": [
      {
        "channel_id": "1001",
        "data_type": "T",
        "values": [
          {
            "point_id": 1,
            "value": 25.6,
            "quality": "good",
            "timestamp": "2025-08-12T10:30:00Z"
          }
        ]
      },
      {
        "channel_id": "1002",
        "data_type": "S",
        "values": [
          {
            "point_id": 10,
            "value": 1,
            "quality": "good",
            "timestamp": "2025-08-12T10:30:00Z"
          }
        ]
      }
    ],
    "total_channels": 2,
    "query_time": "2025-08-12T10:30:00Z"
  }
}
```

### 获取数据快照

获取设备或区域的数据快照。

**请求**

```http
GET /api/v1/realtime/snapshot?device_ids=PLC_001,PLC_002&format=summary
Authorization: Bearer {token}
```

**查询参数**

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| device_ids | string | 否 | 设备ID列表，逗号分隔 |
| area_ids | string | 否 | 区域ID列表，逗号分隔 |
| format | string | 否 | 格式: full, summary |

**响应**

```json
{
  "success": true,
  "data": {
    "snapshot_time": "2025-08-12T10:30:00Z",
    "devices": [
      {
        "device_id": "PLC_001",
        "status": "online",
        "summary": {
          "total_points": 30,
          "normal_points": 29,
          "alarm_points": 1,
          "offline_points": 0
        },
        "key_metrics": {
          "avg_temperature": 25.8,
          "max_pressure": 101.5,
          "total_flow": 1234.5
        }
      }
    ]
  }
}
```

## 历史数据接口

### 查询历史数据

查询指定时间范围的历史数据。

**请求**

```http
GET /api/v1/history/channels/{channel_id}?start_time=2025-08-12T00:00:00Z&end_time=2025-08-12T23:59:59Z&interval=1h
Authorization: Bearer {token}
```

**查询参数**

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| start_time | string | 是 | 开始时间 (ISO 8601) |
| end_time | string | 是 | 结束时间 (ISO 8601) |
| interval | string | 否 | 数据间隔: 1m, 5m, 1h, 1d |
| point_ids | string | 否 | 点位ID列表，逗号分隔 |
| aggregation | string | 否 | 聚合方式: avg, max, min, sum |

**响应**

```json
{
  "success": true,
  "data": {
    "channel_id": "1001",
    "start_time": "2025-08-12T00:00:00Z",
    "end_time": "2025-08-12T23:59:59Z",
    "interval": "1h",
    "points": [
      {
        "point_id": 1,
        "name": "温度1",
        "unit": "°C",
        "data": [
          {
            "timestamp": "2025-08-12T00:00:00Z",
            "value": 25.0,
            "quality": "good"
          },
          {
            "timestamp": "2025-08-12T01:00:00Z",
            "value": 25.5,
            "quality": "good"
          }
        ]
      }
    ],
    "total_points": 24
  }
}
```

### 统计数据查询

查询历史数据的统计信息。

**请求**

```http
GET /api/v1/history/statistics/{channel_id}?period=24h&metrics=avg,max,min,std
Authorization: Bearer {token}
```

**查询参数**

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| period | string | 是 | 统计周期: 1h, 24h, 7d, 30d |
| metrics | string | 否 | 统计指标: avg, max, min, std, count |
| point_ids | string | 否 | 点位ID列表 |

**响应**

```json
{
  "success": true,
  "data": {
    "channel_id": "1001",
    "period": "24h",
    "statistics": [
      {
        "point_id": 1,
        "name": "温度1",
        "metrics": {
          "avg": 25.6,
          "max": 28.9,
          "min": 22.3,
          "std": 1.2,
          "count": 1440
        }
      }
    ],
    "calculated_at": "2025-08-12T10:30:00Z"
  }
}
```

### 聚合数据查询

执行复杂的数据聚合查询。

**请求**

```http
POST /api/v1/history/aggregate
Authorization: Bearer {token}
Content-Type: application/json

{
  "channels": ["1001", "1002"],
  "start_time": "2025-08-12T00:00:00Z",
  "end_time": "2025-08-12T23:59:59Z",
  "aggregation": {
    "interval": "1h",
    "functions": ["avg", "max", "min"]
  },
  "filters": {
    "quality": ["good"],
    "value_range": {
      "min": 0,
      "max": 100
    }
  }
}
```

**响应**

```json
{
  "success": true,
  "data": {
    "results": [
      {
        "channel_id": "1001",
        "aggregated_data": [
          {
            "timestamp": "2025-08-12T00:00:00Z",
            "avg": 25.6,
            "max": 26.8,
            "min": 24.3
          }
        ]
      }
    ],
    "query_info": {
      "total_records": 48,
      "execution_time": 125
    }
  }
}
```

### 数据导出

导出历史数据到文件。

**请求**

```http
POST /api/v1/history/export
Authorization: Bearer {token}
Content-Type: application/json

{
  "format": "csv",
  "channels": ["1001", "1002"],
  "start_time": "2025-08-12T00:00:00Z",
  "end_time": "2025-08-12T23:59:59Z",
  "include_headers": true,
  "compression": "gzip"
}
```

**响应**

```json
{
  "success": true,
  "data": {
    "export_id": "exp_12345",
    "download_url": "/api/v1/history/download/exp_12345",
    "file_size": 102400,
    "expires_at": "2025-08-13T10:30:00Z"
  }
}
```

## 告警管理接口

### 获取告警列表

获取当前活动告警列表。

**请求**

```http
GET /api/v1/alarms?status=active&severity=high&page=1&size=20
Authorization: Bearer {token}
```

**查询参数**

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| status | string | 否 | 告警状态: active, acknowledged, cleared |
| severity | string | 否 | 严重级别: critical, high, medium, low |
| device_id | string | 否 | 设备ID |
| area_id | string | 否 | 区域ID |
| start_time | string | 否 | 开始时间 |
| end_time | string | 否 | 结束时间 |
| page | integer | 否 | 页码 |
| size | integer | 否 | 每页数量 |

**响应**

```json
{
  "success": true,
  "data": {
    "items": [
      {
        "alarm_id": "ALM_12345",
        "channel_id": "1001",
        "point_id": 1,
        "device_id": "PLC_001",
        "severity": "high",
        "status": "active",
        "message": "温度超过上限",
        "value": 95.5,
        "threshold": 90.0,
        "triggered_at": "2025-08-12T10:25:00Z",
        "acknowledged_at": null,
        "acknowledged_by": null,
        "cleared_at": null
      }
    ],
    "total": 15,
    "page": 1,
    "size": 20,
    "total_pages": 1
  }
}
```

### 获取告警详情

获取指定告警的详细信息。

**请求**

```http
GET /api/v1/alarms/{alarm_id}
Authorization: Bearer {token}
```

**响应**

```json
{
  "success": true,
  "data": {
    "alarm_id": "ALM_12345",
    "channel_id": "1001",
    "point_id": 1,
    "device_id": "PLC_001",
    "device_name": "1号PLC",
    "point_name": "温度1",
    "severity": "high",
    "status": "active",
    "message": "温度超过上限",
    "description": "温度传感器1检测到温度异常升高",
    "value": 95.5,
    "threshold": 90.0,
    "rule_id": "RULE_001",
    "triggered_at": "2025-08-12T10:25:00Z",
    "duration": "5m 30s",
    "history": [
      {
        "timestamp": "2025-08-12T10:25:00Z",
        "event": "triggered",
        "value": 95.5
      },
      {
        "timestamp": "2025-08-12T10:26:00Z",
        "event": "escalated",
        "from": "medium",
        "to": "high"
      }
    ],
    "recommended_actions": [
      "检查冷却系统",
      "降低设备负载",
      "联系维护人员"
    ]
  }
}
```

### 确认告警

确认告警已被注意。

**请求**

```http
POST /api/v1/alarms/{alarm_id}/acknowledge
Authorization: Bearer {token}
Content-Type: application/json

{
  "comment": "已安排维护人员处理",
  "estimated_fix_time": "2025-08-12T11:00:00Z"
}
```

**响应**

```json
{
  "success": true,
  "data": {
    "alarm_id": "ALM_12345",
    "acknowledged_at": "2025-08-12T10:31:00Z",
    "acknowledged_by": "user_001"
  }
}
```

### 清除告警

清除已解决的告警。

**请求**

```http
POST /api/v1/alarms/{alarm_id}/clear
Authorization: Bearer {token}
Content-Type: application/json

{
  "resolution": "更换故障传感器",
  "root_cause": "传感器老化"
}
```

**响应**

```json
{
  "success": true,
  "data": {
    "alarm_id": "ALM_12345",
    "cleared_at": "2025-08-12T10:45:00Z",
    "cleared_by": "user_001"
  }
}
```

### 批量确认告警

批量确认多个告警。

**请求**

```http
POST /api/v1/alarms/batch-acknowledge
Authorization: Bearer {token}
Content-Type: application/json

{
  "alarm_ids": ["ALM_12345", "ALM_12346", "ALM_12347"],
  "comment": "批量处理"
}
```

**响应**

```json
{
  "success": true,
  "data": {
    "acknowledged": ["ALM_12345", "ALM_12346"],
    "failed": [
      {
        "alarm_id": "ALM_12347",
        "reason": "告警已清除"
      }
    ],
    "total_acknowledged": 2
  }
}
```

### 获取告警统计

获取告警统计信息。

**请求**

```http
GET /api/v1/alarms/statistics?period=24h&group_by=severity
Authorization: Bearer {token}
```

**查询参数**

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| period | string | 否 | 统计周期: 1h, 24h, 7d, 30d |
| group_by | string | 否 | 分组方式: severity, device, area |

**响应**

```json
{
  "success": true,
  "data": {
    "period": "24h",
    "total_alarms": 150,
    "active_alarms": 15,
    "by_severity": {
      "critical": 2,
      "high": 5,
      "medium": 8,
      "low": 0
    },
    "by_status": {
      "active": 15,
      "acknowledged": 30,
      "cleared": 105
    },
    "trend": {
      "current_hour": 3,
      "previous_hour": 5,
      "change_percent": -40
    },
    "top_devices": [
      {
        "device_id": "PLC_001",
        "alarm_count": 25
      }
    ]
  }
}
```

## 控制命令接口

### 执行控制命令

发送控制命令到设备。

**请求**

```http
POST /api/v1/control/channels/{channel_id}/execute
Authorization: Bearer {token}
Content-Type: application/json

{
  "command_type": "set_value",
  "point_id": 1,
  "value": 50.0,
  "safety_check": true,
  "reason": "生产调整",
  "expire_time": "2025-08-12T11:00:00Z"
}
```

**响应**

```json
{
  "success": true,
  "data": {
    "command_id": "CMD_12345",
    "status": "pending",
    "created_at": "2025-08-12T10:30:00Z",
    "estimated_execution": "2025-08-12T10:30:05Z"
  }
}
```

### 查询控制命令状态

查询控制命令的执行状态。

**请求**

```http
GET /api/v1/control/commands/{command_id}/status
Authorization: Bearer {token}
```

**响应**

```json
{
  "success": true,
  "data": {
    "command_id": "CMD_12345",
    "channel_id": "2001",
    "status": "executed",
    "result": {
      "success": true,
      "actual_value": 50.0,
      "execution_time": 125
    },
    "created_at": "2025-08-12T10:30:00Z",
    "executed_at": "2025-08-12T10:30:05Z",
    "executed_by": "system"
  }
}
```

### 批量控制命令

批量执行多个控制命令。

**请求**

```http
POST /api/v1/control/batch-execute
Authorization: Bearer {token}
Content-Type: application/json

{
  "commands": [
    {
      "channel_id": "2001",
      "point_id": 1,
      "command_type": "set_value",
      "value": 50.0
    },
    {
      "channel_id": "2002",
      "point_id": 2,
      "command_type": "set_value",
      "value": 75.0
    }
  ],
  "execution_mode": "sequential",
  "stop_on_error": true
}
```

**响应**

```json
{
  "success": true,
  "data": {
    "batch_id": "BATCH_12345",
    "commands": [
      {
        "command_id": "CMD_12345",
        "status": "pending"
      },
      {
        "command_id": "CMD_12346",
        "status": "pending"
      }
    ],
    "total_commands": 2
  }
}
```

### 获取控制历史

获取通道的控制命令历史。

**请求**

```http
GET /api/v1/control/channels/{channel_id}/history?start_time=2025-08-12T00:00:00Z&page=1&size=20
Authorization: Bearer {token}
```

**响应**

```json
{
  "success": true,
  "data": {
    "items": [
      {
        "command_id": "CMD_12345",
        "command_type": "set_value",
        "point_id": 1,
        "value": 50.0,
        "status": "executed",
        "operator": "user_001",
        "reason": "生产调整",
        "created_at": "2025-08-12T10:30:00Z",
        "executed_at": "2025-08-12T10:30:05Z"
      }
    ],
    "total": 50,
    "page": 1,
    "size": 20
  }
}
```

## 配置管理接口

### 获取系统配置

获取系统全局配置。

**请求**

```http
GET /api/v1/config/system
Authorization: Bearer {token}
```

**响应**

```json
{
  "success": true,
  "data": {
    "system": {
      "name": "VoltageEMS",
      "version": "1.0.0",
      "timezone": "Asia/Shanghai"
    },
    "data_retention": {
      "realtime": "7d",
      "history": "365d",
      "alarms": "90d"
    },
    "limits": {
      "max_websocket_connections": 10000,
      "max_api_rate": 1000,
      "max_subscriptions_per_client": 100
    },
    "features": {
      "websocket_enabled": true,
      "history_enabled": true,
      "control_enabled": true
    }
  }
}
```

### 更新系统配置

更新系统配置（需要管理员权限）。

**请求**

```http
PUT /api/v1/config/system
Authorization: Bearer {token}
Content-Type: application/json

{
  "data_retention": {
    "realtime": "14d",
    "history": "730d"
  },
  "limits": {
    "max_api_rate": 2000
  }
}
```

**响应**

```json
{
  "success": true,
  "message": "配置更新成功",
  "data": {
    "updated_fields": ["data_retention", "limits"],
    "restart_required": false
  }
}
```

### 导入通道配置

批量导入通道配置。

**请求**

```http
POST /api/v1/config/channels/import
Authorization: Bearer {token}
Content-Type: multipart/form-data

file: channels.csv
format: csv
mode: merge
```

**响应**

```json
{
  "success": true,
  "data": {
    "imported": 100,
    "updated": 20,
    "failed": 2,
    "errors": [
      {
        "line": 45,
        "error": "Invalid data type"
      }
    ]
  }
}
```

### 导出通道配置

导出通道配置到文件。

**请求**

```http
GET /api/v1/config/channels/export?format=csv&device_id=PLC_001
Authorization: Bearer {token}
```

**响应**

文件下载响应或：

```json
{
  "success": true,
  "data": {
    "download_url": "/api/v1/config/download/cfg_12345",
    "file_size": 51200,
    "expires_at": "2025-08-13T10:30:00Z"
  }
}
```

## 用户管理接口

### 获取用户列表

获取系统用户列表（需要管理员权限）。

**请求**

```http
GET /api/v1/users?role=operator&status=active&page=1&size=20
Authorization: Bearer {token}
```

**响应**

```json
{
  "success": true,
  "data": {
    "items": [
      {
        "user_id": "user_001",
        "username": "admin",
        "email": "admin@voltage-ems.com",
        "roles": ["admin"],
        "status": "active",
        "created_at": "2025-01-01T00:00:00Z",
        "last_login": "2025-08-12T10:00:00Z"
      }
    ],
    "total": 50,
    "page": 1,
    "size": 20
  }
}
```

### 创建用户

创建新用户（需要管理员权限）。

**请求**

```http
POST /api/v1/users
Authorization: Bearer {token}
Content-Type: application/json

{
  "username": "operator1",
  "email": "operator1@voltage-ems.com",
  "password": "secure_password",
  "roles": ["operator"],
  "permissions": {
    "devices": ["read"],
    "alarms": ["read", "acknowledge"]
  }
}
```

**响应**

```json
{
  "success": true,
  "data": {
    "user_id": "user_002",
    "message": "用户创建成功"
  }
}
```

### 更新用户

更新用户信息。

**请求**

```http
PUT /api/v1/users/{user_id}
Authorization: Bearer {token}
Content-Type: application/json

{
  "email": "newemail@voltage-ems.com",
  "roles": ["operator", "viewer"]
}
```

**响应**

```json
{
  "success": true,
  "message": "用户更新成功"
}
```

### 删除用户

删除用户（需要管理员权限）。

**请求**

```http
DELETE /api/v1/users/{user_id}
Authorization: Bearer {token}
```

**响应**

```json
{
  "success": true,
  "message": "用户删除成功"
}
```

## 错误码参考

### HTTP 状态码

| 状态码 | 描述 |
|--------|------|
| 200 | 请求成功 |
| 201 | 创建成功 |
| 204 | 删除成功 |
| 400 | 请求参数错误 |
| 401 | 未认证 |
| 403 | 权限不足 |
| 404 | 资源不存在 |
| 429 | 请求过多 |
| 500 | 服务器错误 |
| 503 | 服务不可用 |

### 业务错误码

| 错误码 | 描述 | HTTP状态码 |
|--------|------|-----------|
| AUTH_FAILED | 认证失败 | 401 |
| TOKEN_EXPIRED | Token过期 | 401 |
| PERMISSION_DENIED | 权限不足 | 403 |
| DEVICE_NOT_FOUND | 设备不存在 | 404 |
| CHANNEL_NOT_FOUND | 通道不存在 | 404 |
| INVALID_PARAMETER | 参数无效 | 400 |
| DEVICE_OFFLINE | 设备离线 | 503 |
| CONTROL_FAILED | 控制失败 | 500 |
| DATA_QUALITY_POOR | 数据质量差 | 200 |
| RATE_LIMIT_EXCEEDED | 超过速率限制 | 429 |

## 分页规范

所有返回列表的接口都支持分页：

### 请求参数

| 参数 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| page | integer | 1 | 页码，从1开始 |
| size | integer | 20 | 每页数量，最大100 |
| sort | string | - | 排序字段 |
| order | string | asc | 排序方向: asc, desc |

### 响应格式

```json
{
  "success": true,
  "data": {
    "items": [],
    "total": 1000,
    "page": 1,
    "size": 20,
    "total_pages": 50,
    "has_previous": false,
    "has_next": true
  }
}
```

## 过滤与搜索

支持多种过滤和搜索方式：

### 精确匹配

```
GET /api/v1/devices?status=online&type=plc
```

### 范围查询

```
GET /api/v1/history?value_min=0&value_max=100
```

### 时间范围

```
GET /api/v1/alarms?start_time=2025-08-12T00:00:00Z&end_time=2025-08-12T23:59:59Z
```

### 关键词搜索

```
GET /api/v1/devices?search=温度
```

### IN 查询

```
GET /api/v1/devices?device_ids=PLC_001,PLC_002,PLC_003
```

## 批量操作

支持批量操作的接口：

### 批量请求格式

```json
{
  "operations": [
    {
      "method": "PUT",
      "path": "/devices/PLC_001",
      "body": {}
    },
    {
      "method": "DELETE",
      "path": "/devices/PLC_002"
    }
  ]
}
```

### 批量响应格式

```json
{
  "success": true,
  "data": {
    "results": [
      {
        "index": 0,
        "success": true,
        "data": {}
      },
      {
        "index": 1,
        "success": false,
        "error": {}
      }
    ],
    "total_success": 1,
    "total_failed": 1
  }
}
```

## 最佳实践

### 1. 使用合适的HTTP方法

- GET: 查询数据
- POST: 创建资源
- PUT: 完整更新
- PATCH: 部分更新
- DELETE: 删除资源

### 2. 合理使用缓存

利用 HTTP 缓存头：

```http
Cache-Control: max-age=60
ETag: "123456"
Last-Modified: Wed, 12 Aug 2025 10:30:00 GMT
```

### 3. 批量操作优化

对于大量数据操作，使用批量接口而不是多次单独请求。

### 4. 错误处理

始终检查响应的 `success` 字段，处理可能的错误情况。

### 5. 分页处理

对于大数据集，始终使用分页参数，避免一次性加载过多数据。

### 6. 异步操作

对于耗时操作，使用异步模式：

```json
{
  "success": true,
  "data": {
    "task_id": "task_12345",
    "status_url": "/api/v1/tasks/task_12345/status"
  }
}
```

## API 版本管理

### 版本策略

- 当前版本: v1
- 版本通过URL路径指定: `/api/v1/`
- 向后兼容承诺: 主版本内保持向后兼容

### 版本废弃通知

废弃的API会在响应头中包含警告：

```http
Sunset: Wed, 12 Aug 2026 10:30:00 GMT
Deprecation: true
Link: </api/v2/devices>; rel="successor-version"
```