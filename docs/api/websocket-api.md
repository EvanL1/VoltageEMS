# WebSocket API 文档

## 概述

VoltageEMS WebSocket API 提供实时数据推送服务，支持双向通信，适用于需要低延迟数据更新的场景。

## 连接管理

### 建立连接

#### 连接端点

```
ws://localhost/api/ws
ws://192.168.1.100/api/ws (生产环境局域网IP)
```

#### 连接参数

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| token | string | 是 | JWT 访问令牌I（开发中） |
| client_type | string | 否 | 客户端类型: web, mobile, screen |
| heartbeat | integer | 否 | 心跳间隔（秒），默认30 |

#### 连接示例

```javascript
const wsUrl = 'ws://localhost/api/ws';
const token = 'your_jwt_token';
const ws = new WebSocket(`${wsUrl}?token=${token}&client_type=web`);

ws.onopen = (event) => {
  console.log('WebSocket connected');
};

ws.onerror = (error) => {
  console.error('WebSocket error:', error);
};

ws.onclose = (event) => {
  console.log('WebSocket closed:', event.code, event.reason);
};
```

### 连接状态

WebSocket 连接状态码：

| 状态码 | 描述 | 处理建议 |
|--------|------|----------|
| 1000 | 正常关闭 | 无需处理 |
| 1001 | 端点离开 | 尝试重连 |
| 1006 | 异常关闭 | 检查网络，重连 |
| 4001 | 认证失败 | 刷新token后重连 |
| 4002 | 权限不足 | 检查用户权限 |
| 4003 | 订阅限制 | 减少订阅数量 |
| 4429 | 请求过多 | 延迟后重连 |

## 消息格式

### 基础消息结构

所有 WebSocket 消息使用 JSON 格式：

```json
{
  "type": "message_type",
  "id": "unique_message_id",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {},
  "meta": {
    "version": "1.0",
    "source": "service_name"
  }
}
```

### 消息字段说明

| 字段 | 类型 | 描述 |
|------|------|------|
| type | string | 消息类型标识 |
| id | string | 消息唯一ID |
| timestamp | string | ISO 8601 时间戳 |
| data | object | 消息数据载荷 |
| meta | object | 元数据信息 |

## 客户端消息类型

### 1. 订阅数据 (subscribe)

订阅指定通道的实时数据推送。

#### 请求消息

```json
{
  "type": "subscribe",
  "id": "sub_001",
  "data": {
    "channels": [
      {
        "channel_id": 1001,
        "data_types": ["T", "S"],
        "interval": 1000
      },
      {
        "channel_id": 1002,
        "data_types": ["T"],
        "interval": 500
      }
    ]
  }
}
```

#### 订阅参数说明

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| channel_id | number | 是 | 通道ID |
| data_types | array | 是 | 数据类型: T(遥测), S(遥信), C(遥控), A(遥调) |
| interval | integer | 否 | 推送间隔(ms)，默认1000 |

#### 响应消息

```json
{
  "type": "subscribe_ack",
  "id": "sub_001_ack",
  "data": {
    "subscribed": [1001, 1002],
    "failed": [],
    "total_subscriptions": 2
  }
}
```

### 2. 取消订阅 (unsubscribe)

取消已订阅的数据推送。

#### 请求消息

```json
{
  "type": "unsubscribe",
  "id": "unsub_001",
  "data": {
    "channels": [1001, 1002]
  }
}
```

#### 响应消息

```json
{
  "type": "unsubscribe_ack",
  "id": "unsub_001_ack",
  "data": {
    "unsubscribed": [1001, 1002],
    "remaining_subscriptions": 0
  }
}
```

### 3. 批量订阅 (subscribe_batch)

批量订阅多个数据源。

#### 请求消息

```json
{
  "type": "subscribe_batch",
  "id": "batch_001",
  "data": {
    "mode": "device",
    "device_ids": ["PLC_001", "PLC_002"],
    "data_types": ["T", "S"],
    "interval": 1000
  }
}
```

#### 订阅模式

| 模式 | 描述 | 参数 |
|------|------|------|
| channel | 按通道订阅 | channels, data_types |
| device | 按设备订阅 | device_ids, data_types |
| area | 按区域订阅 | area_ids, priority_levels |
| alarm | 订阅告警 | severity_levels, areas |

### 4. 心跳检测 (heartbeat)

保持连接活跃。

#### 请求消息

```json
{
  "type": "ping",
  "id": "ping_001",
  "timestamp": "2025-08-12T10:30:00Z"
}
```

#### 响应消息

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

### 5. 控制命令 (control)

发送控制命令（需要相应权限）。

#### 请求消息

```json
{
  "type": "control",
  "id": "ctrl_001",
  "data": {
    "channel_id": 2001,
    "command_type": "set_value",
    "value": 100.5,
    "safety_check": true,
    "operator": "user_001",
    "reason": "生产调整"
  }
}
```

#### 响应消息

```json
{
  "type": "control_ack",
  "id": "ctrl_001_ack",
  "data": {
    "command_id": "cmd_12345",
    "status": "executed",
    "execution_time": "2025-08-12T10:30:01Z",
    "result": {
      "success": true,
      "actual_value": 100.5
    }
  }
}
```

## 服务端消息类型

### 1. 实时数据推送 (data_update)

推送订阅的实时数据。

#### 消息格式

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


### 2. 批量数据推送 (data_batch)

批量推送多个通道数据。

#### 消息格式

```json
{
  "type": "data_batch",
  "id": "batch_update_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "updates": [
      {
        "channel_id": 1001,
        "data_type": "T",
        "values": {
          "1": 25.6,
          "2": 30.2
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
    ],
    "total_points": 2,
    "compression": "none"
  }
}
```

### 3. 增量更新 (delta_update)

仅推送变化的数据。

#### 消息格式

```json
{
  "type": "delta_update",
  "id": "delta_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "channel_id": 1001,
    "changes": [
      {
        "point_id": 1,
        "field": "value",
        "old_value": 25.5,
        "new_value": 26.0,
        "timestamp": "2025-08-12T10:30:00Z"
      }
    ]
  }
}
```

### 4. 告警事件 (alarm_event)

推送告警相关事件。

#### 消息格式

```json
{
  "type": "alarm_event",
  "id": "alarm_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "event_type": "triggered",
    "alarm": {
      "alarm_id": "ALM_12345",
      "channel_id": 1001,
      "point_id": 1,
      "severity": "high",
      "message": "温度超过上限",
      "value": 95.5,
      "threshold": 90.0,
      "triggered_at": "2025-08-12T10:30:00Z"
    }
  }
}
```

#### 告警事件类型

| 事件类型 | 描述 |
|----------|------|
| triggered | 告警触发 |
| acknowledged | 告警确认 |
| cleared | 告警清除 |
| escalated | 告警升级 |

### 5. 设备状态 (device_status)

推送设备状态变化。

#### 消息格式

```json
{
  "type": "device_status",
  "id": "status_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "device_id": "PLC_001",
    "status": "online",
    "previous_status": "offline",
    "changed_at": "2025-08-12T10:30:00Z",
    "channels": [1001, 1002, 1003]
  }
}
```

### 6. 系统通知 (system_notification)

推送系统级通知。

#### 消息格式

```json
{
  "type": "system_notification",
  "id": "notify_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "level": "warning",
    "message": "系统将在10分钟后进行维护",
    "details": "维护时间: 10:40-10:50",
    "action_required": false
  }
}
```

### 7. 错误消息 (error)

推送错误信息。

#### 消息格式

```json
{
  "type": "error",
  "id": "error_001",
  "timestamp": "2025-08-12T10:30:00Z",
  "data": {
    "code": "SUBSCRIPTION_FAILED",
    "message": "订阅失败：通道不存在",
    "details": "Channel ID '9999' not found",
    "related_message_id": "sub_001"
  }
}
```

## 重连策略

### 自动重连实现

```javascript
class WebSocketClient {
  constructor(url, token) {
    this.url = url;
    this.token = token;
    this.ws = null;
    this.reconnectAttempts = 0;
    this.maxReconnectAttempts = 5;
    this.reconnectDelay = 1000;
    this.maxReconnectDelay = 30000;
    this.reconnectDecay = 1.5;
    this.subscriptions = new Map();
  }

  connect() {
    this.ws = new WebSocket(`${this.url}?token=${this.token}`);

    this.ws.onopen = () => {
      console.log('Connected');
      this.reconnectAttempts = 0;
      this.resubscribe();
    };

    this.ws.onclose = (event) => {
      console.log('Disconnected:', event.code);
      if (event.code !== 1000) {
        this.reconnect();
      }
    };

    this.ws.onerror = (error) => {
      console.error('Error:', error);
    };

    this.ws.onmessage = (event) => {
      this.handleMessage(JSON.parse(event.data));
    };
  }

  reconnect() {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      console.error('Max reconnection attempts reached');
      return;
    }

    const delay = Math.min(
      this.reconnectDelay * Math.pow(this.reconnectDecay, this.reconnectAttempts),
      this.maxReconnectDelay
    );

    this.reconnectAttempts++;
    console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);

    setTimeout(() => {
      this.connect();
    }, delay);
  }

  resubscribe() {
    // 重新订阅之前的数据
    this.subscriptions.forEach((config, channelId) => {
      this.subscribe(channelId, config);
    });
  }

  subscribe(channelId, config) {
    this.subscriptions.set(channelId, config);

    if (this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({
        type: 'subscribe',
        data: {
          channels: [{
            channel_id: channelId,
            ...config
          }]
        }
      }));
    }
  }

  handleMessage(message) {
    switch (message.type) {
      case 'data_update':
        this.onDataUpdate(message.data);
        break;
      case 'alarm_event':
        this.onAlarmEvent(message.data);
        break;
      case 'error':
        this.onError(message.data);
        break;
      default:
        console.log('Unknown message type:', message.type);
    }
  }

  onDataUpdate(data) {
    // 处理数据更新
    console.log('Data update:', data);
  }

  onAlarmEvent(data) {
    // 处理告警事件
    console.log('Alarm event:', data);
  }

  onError(error) {
    // 处理错误
    console.error('Server error:', error);
  }
}

// 使用示例
const client = new WebSocketClient('ws://localhost/api/ws', 'your_token');
client.connect();
client.subscribe(1001, { data_types: ['T', 'S'], interval: 1000 });
```

## 性能优化

### 数据压缩

WebSocket 支持 per-message deflate 压缩：

```javascript
// 客户端请求压缩
const ws = new WebSocket(url, {
  perMessageDeflate: {
    zlibDeflateOptions: {
      level: zlib.Z_BEST_COMPRESSION,
    },
    threshold: 1024, // 1KB以上的消息才压缩
  }
});
```

### 批量处理

建议批量订阅和批量接收数据以提高性能：

```javascript
// 批量订阅
ws.send(JSON.stringify({
  type: 'subscribe_batch',
  data: {
    channels: ['1001', '1002', '1003', '1004', '1005'],
    data_types: ['T'],
    interval: 1000
  }
}));
```

### 订阅管理最佳实践

1. **合理设置推送间隔**: 根据实际需求设置，避免过于频繁
2. **使用增量模式**: 对于大量数据，使用 delta 模式减少传输量
3. **及时取消订阅**: 不需要的数据及时取消订阅
4. **批量操作**: 尽量使用批量订阅/取消订阅
5. **连接复用**: 同一客户端使用单一连接

## 错误处理

### 常见错误码

| 错误码 | 描述 | 处理建议 |
|--------|------|----------|
| WS_AUTH_FAILED | 认证失败 | 刷新token重连 |
| WS_SUBSCRIPTION_LIMIT | 订阅数量超限 | 减少订阅数量 |
| WS_INVALID_MESSAGE | 消息格式错误 | 检查消息格式 |
| WS_CHANNEL_NOT_FOUND | 通道不存在 | 检查通道ID |
| WS_PERMISSION_DENIED | 权限不足 | 检查用户权限 |
| WS_RATE_LIMIT | 请求频率过高 | 降低请求频率 |

### 错误处理示例

```javascript
ws.onmessage = (event) => {
  const message = JSON.parse(event.data);

  if (message.type === 'error') {
    switch (message.data.code) {
      case 'WS_AUTH_FAILED':
        // 刷新token
        refreshToken().then(newToken => {
          reconnectWithNewToken(newToken);
        });
        break;

      case 'WS_SUBSCRIPTION_LIMIT':
        // 清理不必要的订阅
        cleanupSubscriptions();
        break;

      case 'WS_RATE_LIMIT':
        // 延迟重试
        setTimeout(() => {
          retryLastAction();
        }, message.data.retry_after * 1000);
        break;

      default:
        console.error('Unhandled error:', message.data);
    }
  }
};
```

## 安全注意事项

1. **Token 安全**:
   - 不要在日志中记录 token
   - 定期刷新 token
   - 使用 HTTPS/WSS 传输

2. **输入验证**:
   - 验证所有客户端消息
   - 限制消息大小
   - 防止注入攻击

3. **连接管理**:
   - 限制每用户连接数
   - 实施连接超时
   - 监控异常连接模式

4. **数据权限**:
   - 验证订阅权限
   - 数据级别访问控制
   - 审计关键操作

## 调试技巧

### Chrome DevTools

1. 打开 Chrome DevTools
2. 切换到 Network 标签
3. 筛选 WS 类型
4. 点击 WebSocket 连接查看消息

### 日志记录

```javascript
// 详细日志记录
class DebugWebSocketClient extends WebSocketClient {
  send(message) {
    console.log('>>> Sending:', message);
    super.send(message);
  }

  handleMessage(message) {
    console.log('<<< Received:', message);
    super.handleMessage(message);
  }
}
```

### 测试工具

推荐使用 wscat 进行命令行测试：

```bash
# 安装
npm install -g wscat

# 连接测试
wscat -c "ws://localhost/api/ws?token=your_token"

# 发送消息
> {"type":"subscribe","data":{"channels":[1001],"data_types":["T"]}}
```

## Redis Pub/Sub 机制

VoltageEMS WebSocket 实现基于 Redis Pub/Sub 进行实时数据推送：

### 数据流程

1. **数据写入**: comsrv 将数据写入 Redis Hash (如 `comsrv:1001:T`)
2. **发布通知**: comsrv 向 Redis 发布数据更新通知
3. **订阅监听**: apigateway 订阅相关 Redis 频道
4. **WebSocket 推送**: apigateway 通过 WebSocket 推送给客户端

### Redis 频道命名

```
voltageems:data:1001:T     # 通道1001遥测数据更新
voltageems:data:1001:S     # 通道1001遥信数据更新
voltageems:data:1001:C     # 通道1001遥控数据更新
voltageems:data:1001:A     # 通道1001遥调数据更新
voltageems:alarm:*         # 告警事件通知
voltageems:device:*        # 设备状态变化
```

### 数据同步

- **实时性**: 基于 Redis Pub/Sub，延迟通常 < 10ms
- **一致性**: 客户端订阅后立即获取 Redis 中的最新数据
- **可靠性**: 连接断开重连后自动重新获取数据状态

### 订阅管理

apigateway 为每个 WebSocket 连接维护：
- 活跃订阅列表
- Redis 订阅频道映射
- 客户端推送队列
