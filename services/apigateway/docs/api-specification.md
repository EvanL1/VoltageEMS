# API 规范文档

## 概述

本文档定义了 VoltageEMS API Gateway 的接口规范，包括请求格式、响应格式、错误处理和数据类型定义。所有 API 遵循 RESTful 设计原则。

## 基础信息

### Base URL
```
生产环境: https://api.voltage-ems.com
开发环境: http://localhost:8080
```

### API 版本
当前版本: v1

### 认证方式
使用 JWT Bearer Token 认证：
```
Authorization: Bearer {access_token}
```

### 内容类型
```
Content-Type: application/json
Accept: application/json
```

### 数值精度
所有浮点数值保持 6 位小数精度，以字符串形式传输：
```json
{
  "value": "220.123456"
}
```

## 通用响应格式

### 成功响应

```json
{
  "success": true,
  "data": {
    // 响应数据
  },
  "timestamp": "2025-07-23T10:00:00.000Z"
}
```

### 错误响应

```json
{
  "success": false,
  "error": {
    "code": "INVALID_REQUEST",
    "message": "详细错误信息",
    "field": "可选：具体错误字段"
  },
  "timestamp": "2025-07-23T10:00:00.000Z"
}
```

### 分页响应

```json
{
  "success": true,
  "data": {
    "items": [...],
    "pagination": {
      "page": 1,
      "page_size": 20,
      "total": 100,
      "total_pages": 5
    }
  },
  "timestamp": "2025-07-23T10:00:00.000Z"
}
```

## 错误码定义

| 错误码 | HTTP 状态码 | 说明 |
|--------|------------|------|
| AUTH_REQUIRED | 401 | 需要认证 |
| INVALID_TOKEN | 401 | 无效的令牌 |
| TOKEN_EXPIRED | 401 | 令牌已过期 |
| PERMISSION_DENIED | 403 | 权限不足 |
| NOT_FOUND | 404 | 资源不存在 |
| INVALID_REQUEST | 400 | 请求参数错误 |
| VALIDATION_ERROR | 400 | 数据验证失败 |
| RATE_LIMIT_EXCEEDED | 429 | 请求频率超限 |
| INTERNAL_ERROR | 500 | 内部服务器错误 |
| SERVICE_UNAVAILABLE | 503 | 服务暂时不可用 |

## API 接口详细说明

### 1. 认证接口

#### 1.1 用户登录
```
POST /auth/login
```

请求：
```json
{
  "username": "admin",
  "password": "password123"
}
```

响应：
```json
{
  "success": true,
  "data": {
    "access_token": "eyJ...",
    "refresh_token": "eyJ...",
    "token_type": "Bearer",
    "expires_in": 3600,
    "user": {
      "id": "123",
      "username": "admin",
      "roles": ["admin", "operator"]
    }
  }
}
```

#### 1.2 刷新令牌
```
POST /auth/refresh
```

请求头：
```
Authorization: Bearer {refresh_token}
```

响应：
```json
{
  "success": true,
  "data": {
    "access_token": "eyJ...",
    "expires_in": 3600
  }
}
```

#### 1.3 用户登出
```
POST /auth/logout
```

请求头：
```
Authorization: Bearer {access_token}
```

响应：
```json
{
  "success": true,
  "message": "Logged out successfully"
}
```

#### 1.4 获取当前用户信息
```
GET /auth/me
```

响应：
```json
{
  "success": true,
  "data": {
    "id": "123",
    "username": "admin",
    "email": "admin@example.com",
    "roles": ["admin", "operator"],
    "permissions": [
      "channel.read",
      "channel.write",
      "alarm.acknowledge"
    ],
    "created_at": "2025-01-01T00:00:00.000Z",
    "last_login": "2025-07-23T10:00:00.000Z"
  }
}
```

### 2. 通道管理接口

#### 2.1 获取通道列表
```
GET /api/channels?page=1&page_size=20&status=active
```

查询参数：
- `page`: 页码，默认 1
- `page_size`: 每页数量，默认 20，最大 100
- `status`: 状态过滤 (active, inactive, all)
- `type`: 类型过滤 (modbus, can, iec60870)
- `search`: 搜索关键词

响应：
```json
{
  "success": true,
  "data": {
    "items": [
      {
        "id": 1001,
        "name": "主变压器监测",
        "type": "modbus_tcp",
        "status": "active",
        "description": "110kV主变压器监测通道",
        "config": {
          "host": "192.168.1.100",
          "port": 502,
          "slave_id": 1
        },
        "statistics": {
          "total_points": 150,
          "active_points": 145,
          "last_update": "2025-07-23T10:00:00.000Z"
        }
      }
    ],
    "pagination": {
      "page": 1,
      "page_size": 20,
      "total": 45,
      "total_pages": 3
    }
  }
}
```

#### 2.2 获取通道详情
```
GET /api/channels/{channel_id}
```

响应：
```json
{
  "success": true,
  "data": {
    "id": 1001,
    "name": "主变压器监测",
    "type": "modbus_tcp",
    "status": "active",
    "description": "110kV主变压器监测通道",
    "config": {
      "host": "192.168.1.100",
      "port": 502,
      "slave_id": 1,
      "timeout": 5000,
      "retry_count": 3
    },
    "points": {
      "telemetry": 50,
      "signals": 30,
      "controls": 20,
      "adjustments": 10
    },
    "created_at": "2025-01-01T00:00:00.000Z",
    "updated_at": "2025-07-23T10:00:00.000Z"
  }
}
```

#### 2.3 获取通道遥测数据
```
GET /api/channels/{channel_id}/telemetry?points=10001,10002,10003
```

查询参数：
- `points`: 点位ID列表，逗号分隔（可选，不指定则返回所有）

响应：
```json
{
  "success": true,
  "data": {
    "channel_id": 1001,
    "timestamp": "2025-07-23T10:00:00.000Z",
    "telemetry": {
      "10001": {
        "value": "220.123456",
        "name": "A相电压",
        "unit": "V",
        "timestamp": "2025-07-23T10:00:00.000Z"
      },
      "10002": {
        "value": "221.234567",
        "name": "B相电压",
        "unit": "V",
        "timestamp": "2025-07-23T10:00:00.000Z"
      },
      "10003": {
        "value": "219.345678",
        "name": "C相电压",
        "unit": "V",
        "timestamp": "2025-07-23T10:00:00.000Z"
      }
    }
  }
}
```

#### 2.4 获取通道信号数据
```
GET /api/channels/{channel_id}/signals
```

响应：
```json
{
  "success": true,
  "data": {
    "channel_id": 1001,
    "timestamp": "2025-07-23T10:00:00.000Z",
    "signals": {
      "20001": {
        "value": "1.000000",
        "name": "断路器状态",
        "state": "合闸",
        "timestamp": "2025-07-23T10:00:00.000Z"
      },
      "20002": {
        "value": "0.000000",
        "name": "接地刀闸",
        "state": "断开",
        "timestamp": "2025-07-23T10:00:00.000Z"
      }
    }
  }
}
```

#### 2.5 发送控制命令
```
POST /api/channels/{channel_id}/control
```

请求：
```json
{
  "point_id": 30001,
  "value": 1.0,
  "source": "web_ui",
  "reason": "手动操作",
  "operator": "admin"
}
```

响应：
```json
{
  "success": true,
  "data": {
    "command_id": "550e8400-e29b-41d4-a716-446655440000",
    "status": "sent",
    "timestamp": "2025-07-23T10:00:00.000Z"
  }
}
```

#### 2.6 发送调节命令
```
POST /api/channels/{channel_id}/adjustment
```

请求：
```json
{
  "point_id": 40001,
  "value": 220.500000,
  "source": "web_ui",
  "reason": "电压调整",
  "operator": "admin"
}
```

响应：
```json
{
  "success": true,
  "data": {
    "command_id": "550e8400-e29b-41d4-a716-446655440001",
    "status": "sent",
    "timestamp": "2025-07-23T10:00:00.000Z"
  }
}
```

#### 2.7 批量获取通道数据
```
GET /api/channels/batch?ids=1001,1002,1003&data_types=m,s
```

查询参数：
- `ids`: 通道ID列表，逗号分隔
- `data_types`: 数据类型 (m=测量, s=信号, c=控制, a=调节)

响应：
```json
{
  "success": true,
  "data": {
    "1001": {
      "measurements": {
        "10001": "220.123456",
        "10002": "221.234567"
      },
      "signals": {
        "20001": "1.000000",
        "20002": "0.000000"
      }
    },
    "1002": {
      "measurements": {
        "10001": "380.123456"
      },
      "signals": {
        "20001": "1.000000"
      }
    }
  }
}
```

### 3. 设备模型接口

#### 3.1 获取模型列表
```
GET /api/device-models?type=power_meter
```

响应：
```json
{
  "success": true,
  "data": {
    "models": [
      {
        "name": "power_meter",
        "version": "1.0",
        "description": "三相电力仪表模型",
        "properties": ["rated_voltage", "rated_current"],
        "telemetry": ["voltage_a", "voltage_b", "voltage_c", "current_a"],
        "calculations": ["total_power", "power_factor"],
        "instance_count": 15
      }
    ]
  }
}
```

#### 3.2 获取模型实例
```
GET /api/device-models/{model_name}/instances
```

响应：
```json
{
  "success": true,
  "data": {
    "model": "power_meter",
    "instances": [
      {
        "id": "meter_001",
        "name": "1号变压器电表",
        "properties": {
          "rated_voltage": "380.000000",
          "rated_current": "100.000000"
        },
        "status": "online",
        "last_update": "2025-07-23T10:00:00.000Z"
      }
    ]
  }
}
```

#### 3.3 获取实例数据
```
GET /api/device-models/{model_name}/instances/{instance_id}
```

响应：
```json
{
  "success": true,
  "data": {
    "model": "power_meter",
    "instance_id": "meter_001",
    "name": "1号变压器电表",
    "properties": {
      "rated_voltage": "380.000000",
      "rated_current": "100.000000"
    },
    "telemetry": {
      "voltage_a": {
        "value": "220.123456",
        "unit": "V",
        "timestamp": "2025-07-23T10:00:00.000Z"
      },
      "current_a": {
        "value": "50.123456",
        "unit": "A",
        "timestamp": "2025-07-23T10:00:00.000Z"
      }
    },
    "calculations": {
      "total_power": {
        "value": "33000.123456",
        "unit": "W",
        "timestamp": "2025-07-23T10:00:00.000Z"
      },
      "power_factor": {
        "value": "0.950000",
        "unit": "",
        "timestamp": "2025-07-23T10:00:00.000Z"
      }
    }
  }
}
```

### 4. 历史数据接口

#### 4.1 查询历史数据
```
GET /api/historical?channel=1001&point=10001&start=2025-07-23T00:00:00Z&end=2025-07-23T23:59:59Z
```

查询参数：
- `channel`: 通道ID
- `point`: 点位ID
- `start`: 开始时间 (ISO8601)
- `end`: 结束时间 (ISO8601)
- `interval`: 采样间隔 (可选，如 5m, 1h)
- `function`: 聚合函数 (可选：mean, max, min, sum)

响应：
```json
{
  "success": true,
  "data": {
    "channel": 1001,
    "point": 10001,
    "start": "2025-07-23T00:00:00.000Z",
    "end": "2025-07-23T23:59:59.000Z",
    "count": 288,
    "values": [
      {
        "timestamp": "2025-07-23T00:00:00.000Z",
        "value": "220.123456"
      },
      {
        "timestamp": "2025-07-23T00:05:00.000Z",
        "value": "220.234567"
      }
    ]
  }
}
```

#### 4.2 聚合查询
```
GET /api/historical/aggregate?channel=1001&point=10001&window=1h&function=mean&start=2025-07-23T00:00:00Z&end=2025-07-23T23:59:59Z
```

响应：
```json
{
  "success": true,
  "data": {
    "channel": 1001,
    "point": 10001,
    "window": "1h",
    "function": "mean",
    "aggregates": [
      {
        "timestamp": "2025-07-23T00:00:00.000Z",
        "value": "220.156789",
        "count": 12
      },
      {
        "timestamp": "2025-07-23T01:00:00.000Z",
        "value": "219.987654",
        "count": 12
      }
    ]
  }
}
```

#### 4.3 批量历史查询
```
POST /api/historical/batch
```

请求：
```json
{
  "queries": [
    {
      "channel": 1001,
      "point": 10001,
      "start": "2025-07-23T00:00:00Z",
      "end": "2025-07-23T23:59:59Z",
      "aggregation": {
        "window": "5m",
        "function": "mean"
      }
    },
    {
      "channel": 1001,
      "point": 10002,
      "start": "2025-07-23T00:00:00Z",
      "end": "2025-07-23T23:59:59Z",
      "aggregation": {
        "window": "5m",
        "function": "max"
      }
    }
  ]
}
```

响应：
```json
{
  "success": true,
  "data": {
    "results": [
      {
        "query_index": 0,
        "channel": 1001,
        "point": 10001,
        "count": 288,
        "values": [...]
      },
      {
        "query_index": 1,
        "channel": 1001,
        "point": 10002,
        "count": 288,
        "values": [...]
      }
    ]
  }
}
```

### 5. 告警管理接口

#### 5.1 获取告警列表
```
GET /api/alarms?status=active&level=critical&page=1&page_size=20
```

查询参数：
- `status`: 状态过滤 (active, acknowledged, resolved)
- `level`: 级别过滤 (critical, major, minor, warning, info)
- `category`: 分类过滤 (environmental, power, communication, system, security)
- `start_date`: 开始日期
- `end_date`: 结束日期

响应：
```json
{
  "success": true,
  "data": {
    "items": [
      {
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "title": "高温告警",
        "description": "1号变压器温度超过阈值",
        "category": "environmental",
        "level": "critical",
        "status": "active",
        "source": {
          "channel": 1001,
          "point": 10001,
          "value": "85.500000",
          "threshold": "80.000000"
        },
        "created_at": "2025-07-23T10:00:00.000Z",
        "updated_at": "2025-07-23T10:00:00.000Z",
        "acknowledged_at": null,
        "resolved_at": null
      }
    ],
    "pagination": {
      "page": 1,
      "page_size": 20,
      "total": 5,
      "total_pages": 1
    }
  }
}
```

#### 5.2 获取告警详情
```
GET /api/alarms/{alarm_id}
```

响应：
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "title": "高温告警",
    "description": "1号变压器温度超过阈值",
    "category": "environmental",
    "level": "critical",
    "status": "acknowledged",
    "source": {
      "channel": 1001,
      "point": 10001,
      "value": "85.500000",
      "threshold": "80.000000"
    },
    "history": [
      {
        "timestamp": "2025-07-23T10:00:00.000Z",
        "event": "created",
        "value": "85.500000"
      },
      {
        "timestamp": "2025-07-23T10:05:00.000Z",
        "event": "acknowledged",
        "user": "operator1",
        "notes": "正在检查"
      }
    ],
    "created_at": "2025-07-23T10:00:00.000Z",
    "updated_at": "2025-07-23T10:05:00.000Z",
    "acknowledged_at": "2025-07-23T10:05:00.000Z",
    "acknowledged_by": "operator1",
    "resolved_at": null
  }
}
```

#### 5.3 确认告警
```
POST /api/alarms/{alarm_id}/acknowledge
```

请求：
```json
{
  "notes": "已安排人员现场检查",
  "estimated_resolution": "2025-07-23T12:00:00Z"
}
```

响应：
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "status": "acknowledged",
    "acknowledged_at": "2025-07-23T10:05:00.000Z",
    "acknowledged_by": "operator1"
  }
}
```

#### 5.4 解决告警
```
POST /api/alarms/{alarm_id}/resolve
```

请求：
```json
{
  "resolution": "已更换冷却风扇，温度恢复正常",
  "root_cause": "冷却风扇故障",
  "preventive_action": "增加风扇巡检频率"
}
```

响应：
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "status": "resolved",
    "resolved_at": "2025-07-23T11:30:00.000Z",
    "resolved_by": "engineer1"
  }
}
```

#### 5.5 获取告警统计
```
GET /api/alarms/stats?period=today
```

查询参数：
- `period`: 统计周期 (today, week, month, custom)
- `start_date`: 自定义开始日期
- `end_date`: 自定义结束日期

响应：
```json
{
  "success": true,
  "data": {
    "period": "today",
    "total": 150,
    "by_status": {
      "active": 10,
      "acknowledged": 5,
      "resolved": 135
    },
    "by_level": {
      "critical": 2,
      "major": 3,
      "minor": 5,
      "warning": 20,
      "info": 120
    },
    "by_category": {
      "environmental": 45,
      "power": 60,
      "communication": 20,
      "system": 15,
      "security": 10
    },
    "trend": [
      {
        "hour": "00:00",
        "count": 5
      },
      {
        "hour": "01:00",
        "count": 3
      }
    ]
  }
}
```

### 6. 规则管理接口

#### 6.1 获取规则列表
```
GET /api/rules?enabled=true&type=threshold
```

响应：
```json
{
  "success": true,
  "data": {
    "rules": [
      {
        "id": "power_optimization",
        "name": "功率优化控制",
        "description": "基于功率计算结果的优化控制",
        "type": "dag",
        "enabled": true,
        "triggers": ["data_change"],
        "conditions": 3,
        "actions": 2,
        "last_triggered": "2025-07-23T09:30:00.000Z",
        "created_at": "2025-01-01T00:00:00.000Z"
      }
    ]
  }
}
```

#### 6.2 创建规则
```
POST /api/rules
```

请求：
```json
{
  "name": "温度监控",
  "description": "监控设备温度并触发告警",
  "type": "threshold",
  "config": {
    "source": "comsrv:1001:m:10001",
    "operator": ">",
    "value": 80.0,
    "duration": 60,
    "actions": [
      {
        "type": "alarm",
        "level": "major",
        "title": "温度过高"
      }
    ]
  }
}
```

响应：
```json
{
  "success": true,
  "data": {
    "id": "temp_monitor_001",
    "name": "温度监控",
    "enabled": true,
    "created_at": "2025-07-23T10:00:00.000Z"
  }
}
```

#### 6.3 更新规则
```
PUT /api/rules/{rule_id}
```

请求：
```json
{
  "enabled": false,
  "config": {
    "value": 85.0
  }
}
```

#### 6.4 删除规则
```
DELETE /api/rules/{rule_id}
```

#### 6.5 手动执行规则
```
POST /api/rules/{rule_id}/execute
```

请求：
```json
{
  "context": {
    "force": true,
    "reason": "手动测试"
  }
}
```

响应：
```json
{
  "success": true,
  "data": {
    "execution_id": "550e8400-e29b-41d4",
    "status": "completed",
    "results": {
      "conditions_met": true,
      "actions_executed": 2,
      "duration_ms": 150
    }
  }
}
```

### 7. 配置管理接口

#### 7.1 获取配置
```
GET /api/configs/{key}
```

响应：
```json
{
  "success": true,
  "data": {
    "key": "cfg:channel:1001",
    "value": {
      "name": "主变压器监测",
      "type": "modbus_tcp",
      "config": {
        "host": "192.168.1.100",
        "port": 502
      }
    },
    "version": 3,
    "updated_at": "2025-07-23T10:00:00.000Z",
    "updated_by": "admin"
  }
}
```

#### 7.2 更新配置
```
PUT /api/configs/{key}
```

请求：
```json
{
  "value": {
    "name": "主变压器监测",
    "type": "modbus_tcp",
    "config": {
      "host": "192.168.1.101",
      "port": 502
    }
  },
  "reason": "IP地址变更"
}
```

#### 7.3 删除配置
```
DELETE /api/configs/{key}
```

#### 7.4 同步配置
```
POST /api/configs/sync/{service}
```

响应：
```json
{
  "success": true,
  "data": {
    "service": "comsrv",
    "status": "syncing",
    "job_id": "sync_123456"
  }
}
```

#### 7.5 清理缓存
```
POST /api/configs/cache/clear
```

请求：
```json
{
  "patterns": ["cfg:channel:*", "cfg:model:*"]
}
```

### 8. 系统信息接口

#### 8.1 获取系统信息
```
GET /api/system/info
```

响应：
```json
{
  "success": true,
  "data": {
    "version": "1.0.0",
    "build": "2025.07.23.001",
    "uptime": 864000,
    "start_time": "2025-07-13T10:00:00.000Z",
    "environment": "production",
    "features": {
      "websocket": true,
      "influxdb": true,
      "cache": true
    }
  }
}
```

#### 8.2 获取服务状态
```
GET /api/system/services
```

响应：
```json
{
  "success": true,
  "data": {
    "services": {
      "redis": {
        "status": "healthy",
        "latency": 2,
        "version": "7.0.12"
      },
      "influxdb": {
        "status": "healthy",
        "latency": 15,
        "version": "2.7.0"
      },
      "comsrv": {
        "status": "healthy",
        "latency": 5,
        "endpoints": 6
      },
      "modsrv": {
        "status": "healthy",
        "latency": 8,
        "models": 15
      }
    },
    "overall_status": "healthy"
  }
}
```

#### 8.3 获取性能指标
```
GET /api/system/metrics
```

响应：
```json
{
  "success": true,
  "data": {
    "requests": {
      "total": 1000000,
      "rate_per_second": 150,
      "average_latency_ms": 25
    },
    "connections": {
      "http_active": 120,
      "websocket_active": 45,
      "redis_pool": {
        "active": 5,
          "idle": 5,
        "max": 10
      }
    },
    "cache": {
      "hits": 850000,
      "misses": 150000,
      "hit_rate": 0.85,
      "size": 980,
      "max_size": 1000
    },
    "memory": {
      "used_mb": 256,
      "rss_mb": 280,
      "heap_mb": 200
    }
  }
}
```

## 健康检查接口

### 简单健康检查
```
GET /health
```

响应：
```
OK
```

### 详细健康检查
```
GET /health/detailed
```

响应：
```json
{
  "status": "healthy",
  "checks": {
    "redis": {
      "status": "healthy",
      "latency_ms": 2
    },
    "influxdb": {
      "status": "healthy",
      "latency_ms": 15
    },
    "backend_services": {
      "status": "healthy",
      "services": {
        "comsrv": "healthy",
        "modsrv": "healthy",
        "hissrv": "healthy",
        "alarmsrv": "healthy",
        "rulesrv": "healthy",
        "netsrv": "healthy"
      }
    }
  },
  "timestamp": "2025-07-23T10:00:00.000Z"
}
```

## 速率限制

API 实施以下速率限制：

| 端点类型 | 限制 | 时间窗口 |
|---------|------|---------|
| 认证接口 | 10 | 1分钟 |
| 数据查询 | 1000 | 1分钟 |
| 控制命令 | 100 | 1分钟 |
| 配置修改 | 50 | 1分钟 |
| 批量操作 | 10 | 1分钟 |

超出限制时返回：
```json
{
  "success": false,
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "请求频率超限",
    "retry_after": 30
  }
}
```

## 数据类型定义

### 时间格式
所有时间使用 ISO 8601 格式：
```
2025-07-23T10:00:00.000Z
```

### 数值格式
所有浮点数保持 6 位小数精度，以字符串传输：
```json
{
  "value": "220.123456"
}
```

### 枚举值

#### 通道状态
- `active` - 活跃
- `inactive` - 非活跃
- `error` - 错误
- `maintenance` - 维护中

#### 告警级别
- `critical` - 严重
- `major` - 主要
- `minor` - 次要
- `warning` - 警告
- `info` - 信息

#### 告警状态
- `active` - 活跃
- `acknowledged` - 已确认
- `resolved` - 已解决
- `archived` - 已归档

#### 告警分类
- `environmental` - 环境
- `power` - 电力
- `communication` - 通信
- `system` - 系统
- `security` - 安全

## 最佳实践

### 1. 使用合适的 HTTP 方法
- GET: 读取数据
- POST: 创建资源或执行操作
- PUT: 更新整个资源
- PATCH: 部分更新资源
- DELETE: 删除资源

### 2. 批量操作
对于大量数据操作，优先使用批量接口：
```
GET /api/channels/batch?ids=1001,1002,1003
POST /api/historical/batch
```

### 3. 分页查询
对于列表查询，使用分页参数避免一次返回过多数据：
```
GET /api/alarms?page=1&page_size=20
```

### 4. 字段过滤
使用字段参数只返回需要的数据：
```
GET /api/channels/1001?fields=id,name,status
```

### 5. 错误处理
始终检查响应的 `success` 字段，处理可能的错误情况。

### 6. 缓存利用
对于不经常变化的数据，客户端应该实现适当的缓存策略。

## 版本控制

API 版本通过 URL 路径控制：
```
/api/v1/channels
/api/v2/channels  (未来版本)
```

当前版本: v1

## 变更日志

### v1.0.0 (2025-07-23)
- 初始版本发布
- 支持通道、告警、规则、配置管理
- WebSocket 实时数据推送
- JWT 认证机制