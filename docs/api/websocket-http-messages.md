# WebSocket 和 HTTP 报文格式

## 通信协议选择原则

VoltageEMS 采用 WebSocket 和 HTTP 混合通信模式，根据数据特性选择最适合的协议：

### WebSocket 用于：
- **实时数据推送** - 遥测、遥信等高频更新数据（1-10Hz）
- **告警事件通知** - 需要立即推送的告警触发/恢复事件
- **控制命令反馈** - 控制指令的实时执行状态
- **心跳保活** - 维持长连接的心跳检测

**特点**：低延迟（<100ms）、双向通信、服务端主动推送

### HTTP REST API 用于：
- **配置管理** - 设备配置、点表定义、告警规则等低频数据
- **历史查询** - 历史趋势、统计分析等非实时数据
- **批量操作** - 设备管理、数据导出等批处理任务
- **用户认证** - 登录、权限验证、token刷新

**特点**：请求-响应模式、无状态、适合CRUD操作

### 数据传输优化策略：
1. **静态配置分离** - 点位名称、单位等静态信息通过HTTP获取一次，WebSocket只传动态值
2. **按需订阅** - 客户端只订阅需要的通道和数据类型
3. **批量传输** - 相同时刻的多个数据点合并推送
4. **精简格式** - 移除冗余字段，保留必要信息

## 1. WebSocket 报文格式

### 1.1 通用报文结构

所有WebSocket报文采用JSON格式：

```json
{
  "type": "string",      // 报文类型
  "id": "string",       // 唯一标识
  "timestamp": "string", // ISO 8601时间戳
  "data": {}            // 数据载荷
}
```

### 1.2 客户端发送报文

#### 订阅数据
```json
{
  "type": "subscribe",
  "id": "sub_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "channels": ["1001", "1002"],
    "data_types": ["T", "S"],  // T=遥测, S=遥信, C=遥控, A=遥调
    "interval": 1000           // 推送间隔(ms)
  }
}
```

#### 取消订阅
```json
{
  "type": "unsubscribe",
  "id": "unsub_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "channels": ["1001"]
  }
}
```

#### 控制命令
```json
{
  "type": "control",
  "id": "ctrl_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "channel_id": "2001",
    "point_id": 20,
    "command_type": "set_value",
    "value": 50.0,
    "operator": "user_001",
    "reason": "Production adjustment"
  }
}
```

#### 心跳
```json
{
  "type": "ping",
  "id": "ping_001",
  "timestamp": "2025-08-12T10:30:00Z"
}
```

### 1.3 服务端推送报文

#### 实时数据更新
```json
{
  "type": "data_update",
  "id": "upd_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "channel_id": "1001",
    "data_type": "T",
    "values": [
      {"point_id": 1, "value": 25.6},
      {"point_id": 2, "value": 101.3}
    ]
  }
}
```

#### 批量数据更新
```json
{
  "type": "data_batch",
  "id": "batch_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "updates": [
      {
        "channel_id": "1001",
        "data_type": "T",
        "values": [
          {"point_id": 1, "value": 25.6}
        ]
      },
      {
        "channel_id": "1002",
        "data_type": "S",
        "values": [
          {"point_id": 10, "value": 1}
        ]
      }
    ]
  }
}
```

#### 告警事件
```json
{
  "type": "alarm",
  "id": "alarm_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "alarm_id": "ALM_12345",
    "channel_id": "1001",
    "point_id": 1,
    "status": 1,  // 0=恢复, 1=触发
    "level": 2,   // 0=低, 1=中, 2=高, 3=紧急
    "value": 95.5
  }
}
```


#### 订阅确认
```json
{
  "type": "subscribe_ack",
  "id": "sub_001_ack",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "request_id": "sub_001",
    "subscribed": ["1001", "1002"],
    "failed": [],
    "total": 2
  }
}
```

#### 控制命令确认
```json
{
  "type": "control_ack",
  "id": "ctrl_001_ack",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "request_id": "ctrl_001",
    "command_id": "CMD_12345",
    "status": "executed",
    "result": {
      "success": true,
      "actual_value": 50.0
    }
  }
}
```

#### 错误消息
```json
{
  "type": "error",
  "id": "err_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "code": "CHANNEL_NOT_FOUND",
    "message": "Channel not found",
    "details": "Channel ID '9999' not found",
    "request_id": "sub_001"
  }
}
```

#### 心跳响应
```json
{
  "type": "pong",
  "id": "pong_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "server_time": "2025-08-12T10:30:00Z",
    "latency": 5
  }
}
```

## 2. HTTP REST API 报文格式

### 2.1 请求报文

#### 请求头
```http
POST /api/v1/devices HTTP/1.1
Host: localhost:6005
Authorization: Bearer eyJhbGciOiJIUzI1NiIs...
Content-Type: application/json
Accept: application/json
X-Request-ID: req_12345
```

#### 创建设备请求
```json
POST /api/v1/devices
{
  "name": "New PLC Device",
  "type": "plc",
  "area_id": "north",
  "protocol": "modbus_tcp",
  "address": "192.168.1.100:502",
  "configuration": {
    "timeout": 3000,
    "retry_count": 3,
    "poll_interval": 1000
  }
}
```

#### 批量查询实时数据
```json
POST /api/v1/realtime/batch-query
{
  "channels": ["1001", "1002", "1003"],
  "data_types": ["T", "S"],
  "include_timestamp": true
}
```

#### 执行控制命令
```json
POST /api/v1/control/channels/{channel_id}/execute
{
  "command_type": "set_value",
  "point_id": 1,
  "value": 50.0,
  "safety_check": true,
  "reason": "生产调整"
}
```

#### 查询历史数据
```http
GET /api/v1/history/channels/1001?start_time=2025-08-12T00:00:00Z&end_time=2025-08-12T23:59:59Z&interval=1h
```

#### 确认告警
```json
POST /api/v1/alarms/{alarm_id}/acknowledge
{
  "comment": "Maintenance staff assigned",
  "estimated_fix_time": "2025-08-12T11:00:00Z"
}
```

### 2.2 响应报文

#### 成功响应（单个资源）
```json
HTTP/1.1 200 OK
{
  "success": true,
  "data": {
    "device_id": "PLC_001",
    "name": "PLC Unit 1",
    "type": "plc",
    "status": "online",
    "channels": ["1001", "1002", "1003"]
  },
  "message": "Query successful",
  "timestamp": "2025-08-12T10:30:00Z",
  "request_id": "req_12345"
}
```

#### 成功响应（列表/分页）
```json
HTTP/1.1 200 OK
{
  "success": true,
  "data": {
    "items": [
      {
        "device_id": "PLC_001",
        "name": "PLC Unit 1",
        "status": "online"
      },
      {
        "device_id": "PLC_002",
        "name": "PLC Unit 2",
        "status": "online"
      }
    ],
    "total": 50,
    "page": 1,
    "size": 20,
    "total_pages": 3
  },
  "timestamp": "2025-08-12T10:30:00Z"
}
```

#### 实时数据响应
```json
HTTP/1.1 200 OK
{
  "success": true,
  "data": {
    "channel_id": "1001",
    "device_id": "PLC_001",
    "data_type": "T",
    "values": [
      {"point_id": 1, "value": 25.6}
    ],
    "last_update": "2025-08-12T10:30:00Z"
  }
}
```

#### 历史数据响应
```json
HTTP/1.1 200 OK
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
        "data": [
          {"timestamp": "2025-08-12T00:00:00Z", "value": 25.0},
          {"timestamp": "2025-08-12T01:00:00Z", "value": 25.5}
        ]
      }
    ]
  }
}
```

#### 告警列表响应
```json
HTTP/1.1 200 OK
{
  "success": true,
  "data": {
    "items": [
      {
        "alarm_id": "ALM_12345",
        "channel_id": "1001",
        "point_id": 1,
        "status": 1,  // 0=recovered, 1=triggered
        "level": 2,   // 0=low, 1=medium, 2=high, 3=critical
        "value": 95.5,
        "triggered_at": "2025-08-12T10:25:00Z"
      }
    ],
    "total": 5
  }
}
```

#### 错误响应
```json
HTTP/1.1 400 Bad Request
{
  "success": false,
  "error": {
    "code": "INVALID_PARAMETER",
    "message": "Invalid parameter",
    "details": "Channel ID must be provided"
  },
  "timestamp": "2025-08-12T10:30:00Z",
  "request_id": "req_12345"
}
```

```json
HTTP/1.1 404 Not Found
{
  "success": false,
  "error": {
    "code": "DEVICE_NOT_FOUND",
    "message": "Device not found",
    "details": "Device with ID 'PLC_999' not found"
  }
}
```

```json
HTTP/1.1 401 Unauthorized
{
  "success": false,
  "error": {
    "code": "TOKEN_EXPIRED",
    "message": "Authentication token expired",
    "details": "Please refresh your token"
  }
}
```

```json
HTTP/1.1 403 Forbidden
{
  "success": false,
  "error": {
    "code": "PERMISSION_DENIED",
    "message": "Insufficient permissions",
    "details": "You don't have permission to control this device"
  }
}
```

```json
HTTP/1.1 429 Too Many Requests
{
  "success": false,
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Too many requests",
    "retry_after": 30
  }
}
```

```json
HTTP/1.1 500 Internal Server Error
{
  "success": false,
  "error": {
    "code": "INTERNAL_ERROR",
    "message": "Internal server error",
    "details": "An unexpected error occurred"
  }
}
```

## 3. 静态配置获取

### 点位配置（启动时获取一次）
```json
GET /api/v1/config/points
{
  "success": true,
  "data": {
    "channels": {
      "1001": {
        "name": "Device 1 Temperature Channel",
        "device_id": "PLC_001",
        "points": {
          "1": {"name": "Temperature 1", "unit": "°C", "min": 0, "max": 100},
          "2": {"name": "Pressure 1", "unit": "kPa", "min": 0, "max": 200}
        }
      },
      "1002": {
        "name": "Device 1 Status Channel",
        "device_id": "PLC_001",
        "points": {
          "10": {"name": "Running Status", "states": ["Stopped", "Running"]},
          "11": {"name": "Fault Status", "states": ["Normal", "Fault"]}
        }
      }
    }
  }
}
```

### 告警定义（启动时获取一次）
```json
GET /api/v1/config/alarms
{
  "success": true,
  "data": {
    "definitions": {
      "ALM_12345": {
        "name": "High Temperature Alarm",
        "message": "Temperature exceeds upper threshold",
        "threshold": 90.0
      },
      "ALM_12346": {
        "name": "Pressure Anomaly Alarm",
        "message": "Pressure out of normal range",
        "threshold": 180.0
      }
    }
  }
}
```

## 4. 完整通信示例

### WebSocket通信流程
```javascript
// 1. 建立连接
ws://localhost:6005/ws/v1/realtime?token=xxx

// 2. 订阅数据
→ {"type":"subscribe","id":"sub_001","data":{"channels":["1001"],"data_types":["T"]}}
← {"type":"subscribe_ack","data":{"subscribed":["1001"],"total":1}}

// 3. 接收实时数据
← {"type":"data_update","data":{"channel_id":"1001","values":[{"point_id":1,"value":25.6}]}}

// 4. 发送控制
→ {"type":"control","id":"ctrl_001","data":{"channel_id":"2001","point_id":20,"value":50}}
← {"type":"control_ack","data":{"command_id":"CMD_123","status":"executed"}}

// 5. 心跳
→ {"type":"ping","id":"ping_001"}
← {"type":"pong","id":"pong_001","data":{"latency":5}}
```

### HTTP API调用流程
```bash
# 1. 登录获取token
POST /api/v1/auth/login
{"username":"admin","password":"password"}
→ {"access_token":"xxx","refresh_token":"yyy"}

# 2. 查询设备
GET /api/v1/devices
Authorization: Bearer xxx
→ {"success":true,"data":{"items":[...]}}

# 3. 获取实时数据
GET /api/v1/realtime/channels/1001
→ {"success":true,"data":{"values":[...]}}

# 4. 执行控制
POST /api/v1/control/channels/2001/execute
{"command_type":"set_value","value":50}
→ {"success":true,"data":{"command_id":"CMD_123"}}
```