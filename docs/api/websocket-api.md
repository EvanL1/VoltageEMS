# WebSocket API 文档

## 概述

VoltageEMS 采用 WebSocket 和 HTTP 混合通信模式，根据数据特性选择最适合的协议。

### 通信协议选择原则

**WebSocket 用于：**
- 实时数据推送 - 遥测、遥信等高频更新数据（1-10Hz）
- 告警事件通知 - 需要立即推送的告警触发/恢复事件
- 控制命令反馈 - 控制指令的实时执行状态
- 心跳保活 - 维持长连接的心跳检测

特点：低延迟（<100ms）、双向通信、服务端主动推送

**HTTP REST API 用于：**
- 配置管理 - 设备配置、点表定义、告警规则等低频数据
- 历史查询 - 历史趋势、统计分析等非实时数据
- 批量操作 - 设备管理、数据导出等批处理任务
- 用户认证 - 登录、权限验证、token刷新

特点：请求-响应模式、无状态、适合CRUD操作

### 数据传输优化策略

1. **静态配置分离** - 点位名称、单位等静态信息通过HTTP获取一次，WebSocket只传动态值
2. **按需订阅** - 客户端只订阅需要的通道和数据类型
3. **批量传输** - 相同时刻的多个数据点合并推送
4. **精简格式** - 移除冗余字段，保留必要信息

## WebSocket 报文格式

### 1. 通用报文结构

所有WebSocket报文采用JSON格式：

```json
{
  "type": "string",      // 报文类型
  "id": "string",        // 唯一标识
  "timestamp": "string", // ISO 8601时间戳
  "data": {}            // 数据载荷
}
```

### 2. 客户端发送报文

#### 订阅数据
```json
{
  "type": "subscribe",
  "id": "sub_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "channels": [1001, 1002],
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
    "channels": [1001]
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
    "channel_id": 2001,
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

### 3. 服务端推送报文

#### 实时数据更新
```json
{
  "type": "data_update",
  "id": "update_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "channel_id": 1001,
    "data_type": "T",
    "values": {
      "1": 25.6,
      "2": 101.3,
      "3": 7.2
    }
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
        "channel_id": 1001,
        "data_type": "T",
        "values": {
          "1": 25.6,
          "2": 26.3
        }
      },
      {
        "channel_id": 1002,
        "data_type": "S",
        "values": {
          "10": 1,
          "11": 0
        }
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
    "channel_id": 1001,
    "point_id": 1,
    "status": 1,    // 0=恢复, 1=触发
    "level": 2,     // 0=低, 1=中, 2=高, 3=紧急
    "value": 95.5,
    "message": "Temperature exceeds threshold"
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
    "subscribed": [1001, 1002],
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
    "details": "Channel ID 9999 not found",
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

## HTTP REST API 报文格式

### 基础端点

- 开发环境: `http://localhost:6005/api`
- 生产环境: `http://192.168.1.100/api`

### 1. 通道管理

#### 获取通道列表
```http
GET /api/channels
```

响应：
```json
{
  "success": true,
  "data": [
    {
      "channel_id": 1001,
      "status": "active",
      "last_update": 1736755800,
      "active_points": 12
    }
  ],
  "message": "Found 3 channels",
  "timestamp": "2025-08-12T10:30:00Z"
}
```

#### 获取通道状态
```http
GET /api/channels/{channel_id}/status
```

响应：
```json
{
  "success": true,
  "data": {
    "channel_id": 1001,
    "status": "active",
    "last_update": 1736755800,
    "active_points": 5
  },
  "timestamp": "2025-08-12T10:30:00Z"
}
```

### 2. 实时数据查询

#### 获取通道实时数据
```http
GET /api/channels/{channel_id}/realtime?data_type=T&point_id=1&limit=100
```

查询参数：
- `data_type` - 数据类型(T/S/C/A)，可选
- `point_id` - 特定点位ID，可选
- `limit` - 最大返回数量(1-1000)，默认100

响应：
```json
{
  "success": true,
  "data": [
    {
      "channel_id": 1001,
      "data_type": "T",
      "timestamp": 1736755800,
      "values": {
        "1": 25.6,
        "2": 101.3
      }
    }
  ],
  "message": "Retrieved 1 data records",
  "timestamp": "2025-08-12T10:30:00Z"
}
```

### 3. 历史数据查询

#### 获取历史数据（需要InfluxDB集成）
```http
GET /api/channels/{channel_id}/history?start_time=1736755200&end_time=1736841600&data_type=T&point_id=1&limit=100
```

查询参数：
- `start_time` - 开始时间(Unix时间戳)
- `end_time` - 结束时间(Unix时间戳)
- `data_type` - 数据类型(T/S/C/A)，可选
- `point_id` - 特定点位ID，可选
- `limit` - 最大返回数量(1-1000)，默认100

响应：
```json
{
  "success": true,
  "data": [],
  "message": "Historical data endpoint - requires InfluxDB integration",
  "timestamp": "2025-08-12T10:30:00Z"
}
```

### 4. 健康检查

#### 基础健康检查
```http
GET /health
```

响应：
```json
{
  "success": true,
  "data": {
    "status": "healthy",
    "service": "apigateway-py"
  },
  "timestamp": "2025-08-12T10:30:00Z"
}
```

#### 详细健康检查
```http
GET /health/detailed
```

响应：
```json
{
  "success": true,
  "data": {
    "status": "healthy",
    "service": "apigateway-py",
    "timestamp": "2025-08-12T10:30:00Z",
    "dependencies": {
      "redis": {
        "status": "healthy",
        "message": "Redis connection successful"
      }
    }
  },
  "timestamp": "2025-08-12T10:30:00Z"
}
```

### 5. 错误响应格式

```json
{
  "success": false,
  "error": {
    "code": "CHANNEL_NOT_FOUND",
    "message": "Channel not found",
    "details": "Channel 9999 not found"
  },
  "timestamp": "2025-08-12T10:30:00Z"
}
```

错误码：
- `400` - 请求参数错误
- `404` - 资源未找到
- `500` - 服务器内部错误

## 完整通信示例

### WebSocket 通信流程

```javascript
// 1. 建立连接
const ws = new WebSocket('ws://localhost/api/ws');

// 2. 订阅数据
ws.send(JSON.stringify({
  "type": "subscribe",
  "id": "sub_001",
  "timestamp": new Date().toISOString(),
  "data": {
    "channels": [1001, 1002],
    "data_types": ["T"],
    "interval": 1000
  }
}));

// 3. 接收实时数据
ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  if (msg.type === 'data_update') {
    console.log('Channel:', msg.data.channel_id);
    console.log('Values:', msg.data.values);
  }
};

// 4. 心跳维持
setInterval(() => {
  ws.send(JSON.stringify({
    "type": "ping",
    "id": "ping_" + Date.now(),
    "timestamp": new Date().toISOString()
  }));
}, 30000);
```

### HTTP API 调用流程

```javascript
// 1. 获取通道列表
const response = await fetch('http://localhost:6005/api/channels');
const channels = await response.json();

// 2. 查询实时数据
const realtimeData = await fetch('http://localhost:6005/api/channels/1001/realtime?data_type=T');
const data = await realtimeData.json();

// 3. 获取通道状态
const statusResponse = await fetch('http://localhost:6005/api/channels/1001/status');
const status = await statusResponse.json();
```

## Redis 数据同步机制

### Redis Lua Functions

VoltageEMS 使用 Redis Lua Functions 实现高性能数据处理：

1. **数据写入**: comsrv/modsrv 通过 Lua Function 原子写入数据到 Redis Hash
2. **业务逻辑**: 模型计算、告警判断、规则匹配等在 Redis 内执行
3. **数据查询**: apigateway 定期轮询或通过 Lua Function 批量获取数据
4. **推送客户端**: 通过 WebSocket 推送给订阅的客户端

#### 主要 Lua Functions

- `comsrv_write_data` - 写入通道数据并触发计算
- `modsrv_calculate` - 执行模型计算
- `alarmsrv_check` - 告警条件检查
- `rulesrv_evaluate` - 规则引擎评估
- `netsrv_collect_data` - 批量数据收集
- `netsrv_forward_data` - 数据转发

这种架构的优势：
- **原子操作**: 数据写入和业务逻辑在 Redis 内原子执行
- **极低延迟**: 避免网络往返，毫秒级响应
- **高吞吐量**: 充分利用 Redis 单线程模型
- **数据一致性**: 避免并发问题

### 数据存储格式

Redis中的数据存储格式：

```
# 遥测数据
comsrv:1001:T = {
  "1": "25.6",
  "2": "101.3",
  "_timestamp": "1736755800"
}

# 遥信数据
comsrv:1001:S = {
  "10": "1",
  "11": "0",
  "_timestamp": "1736755800"
}
```

## 性能优化建议

1. **批量订阅**: 一次订阅多个通道，减少消息往返
2. **合理设置推送间隔**: 根据实际需求设置，避免过度推送
3. **使用数据过滤**: 只订阅需要的数据类型和点位
4. **连接池管理**: 复用WebSocket连接，避免频繁建立/断开
5. **缓存静态配置**: 点位定义等静态信息本地缓存

## 开发工具

### FastAPI 自动文档

访问 `http://localhost:6005/docs` 查看自动生成的 API 文档（Swagger UI）。

### WebSocket 测试工具

推荐使用：
- [wscat](https://github.com/websockets/wscat) - 命令行工具
- [Postman](https://www.postman.com/) - 支持 WebSocket 测试
- Chrome DevTools - 浏览器内置工具