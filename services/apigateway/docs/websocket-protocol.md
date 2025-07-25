# WebSocket 协议文档

## 概述

VoltageEMS WebSocket 接口提供实时双向通信能力，支持数据推送、事件通知和实时控制。本文档定义了 WebSocket 通信协议、消息格式和交互流程。

## 连接信息

### WebSocket 端点
```
ws://localhost:8080/ws    (开发环境)
wss://api.voltage-ems.com/ws  (生产环境)
```

### 连接要求
- 支持 WebSocket 协议版本 13
- 心跳间隔: 30秒
- 最大消息大小: 1MB
- 支持文本帧，所有消息使用 JSON 格式

## 连接流程

### 1. 建立连接

```javascript
const ws = new WebSocket('ws://localhost:8080/ws');

ws.onopen = (event) => {
  console.log('WebSocket connected');
  // 发送认证消息
};

ws.onerror = (error) => {
  console.error('WebSocket error:', error);
};

ws.onclose = (event) => {
  console.log('WebSocket closed:', event.code, event.reason);
};
```

### 2. 认证

连接建立后，客户端必须在 5 秒内发送认证消息：

```json
{
  "type": "auth",
  "token": "Bearer eyJ..."
}
```

服务器响应：

成功：
```json
{
  "type": "auth_response",
  "success": true,
  "user_id": "123",
  "message": "认证成功"
}
```

失败：
```json
{
  "type": "auth_response",
  "success": false,
  "error": "Invalid token",
  "code": "AUTH_FAILED"
}
```

### 3. 心跳保持

客户端应定期发送心跳消息保持连接：

```json
{
  "type": "ping"
}
```

服务器响应：
```json
{
  "type": "pong",
  "timestamp": "2025-07-23T10:00:00.000Z"
}
```

## 消息格式

### 通用消息结构

所有消息采用 JSON 格式，包含以下基本字段：

```typescript
interface Message {
  type: string;           // 消息类型
  id?: string;           // 消息ID（用于请求响应匹配）
  timestamp?: string;     // 时间戳
  [key: string]: any;    // 其他字段
}
```

### 数值精度

所有浮点数保持 6 位小数精度，以字符串形式传输：

```json
{
  "value": "220.123456"
}
```

## 客户端消息类型

### 1. 订阅实时数据

#### 1.1 订阅通道数据
```json
{
  "type": "subscribe",
  "channels": [1001, 1002],
  "data_types": ["m", "s"],  // m=测量, s=信号, c=控制, a=调节
  "interval": 1000  // 可选：推送间隔(ms)，默认实时
}
```

#### 1.2 订阅模型数据
```json
{
  "type": "subscribe_models",
  "models": ["power_meter", "env_monitor"],
  "instances": ["meter_001", "meter_002"],  // 可选：特定实例
  "fields": ["total_power", "power_factor"]  // 可选：特定字段
}
```

#### 1.3 订阅告警
```json
{
  "type": "subscribe_alarms",
  "levels": ["critical", "major"],  // 可选：告警级别过滤
  "categories": ["power", "environmental"],  // 可选：分类过滤
  "channels": [1001, 1002]  // 可选：通道过滤
}
```

#### 1.4 订阅系统事件
```json
{
  "type": "subscribe_events",
  "events": ["service_status", "config_change", "user_action"]
}
```

### 2. 取消订阅

```json
{
  "type": "unsubscribe",
  "subscriptions": ["channels", "alarms", "models", "events"],  // 要取消的订阅类型
  "channels": [1001],  // 可选：特定通道
  "models": ["power_meter"]  // 可选：特定模型
}
```

### 3. 请求数据

#### 3.1 请求当前数据
```json
{
  "type": "get_data",
  "id": "req_123",
  "channel": 1001,
  "data_type": "m",
  "points": [10001, 10002, 10003]  // 可选：特定点位
}
```

#### 3.2 请求历史数据
```json
{
  "type": "get_history",
  "id": "req_124",
  "channel": 1001,
  "point": 10001,
  "start": "2025-07-23T00:00:00Z",
  "end": "2025-07-23T23:59:59Z",
  "interval": "5m",  // 可选：采样间隔
  "function": "mean"  // 可选：聚合函数
}
```

### 4. 控制命令

#### 4.1 发送控制命令
```json
{
  "type": "control",
  "id": "cmd_125",
  "channel": 1001,
  "point": 30001,
  "value": 1.0,
  "confirm": true  // 是否需要确认
}
```

#### 4.2 发送调节命令
```json
{
  "type": "adjustment",
  "id": "cmd_126",
  "channel": 1001,
  "point": 40001,
  "value": "220.500000"
}
```

### 5. 配置管理

#### 5.1 获取配置
```json
{
  "type": "get_config",
  "id": "req_127",
  "key": "cfg:channel:1001"
}
```

#### 5.2 更新配置
```json
{
  "type": "update_config",
  "id": "req_128",
  "key": "cfg:channel:1001",
  "value": {
    "name": "主变压器监测",
    "timeout": 5000
  }
}
```

## 服务器消息类型

### 1. 数据推送

#### 1.1 实时数据更新
```json
{
  "type": "data",
  "channel": 1001,
  "data_type": "m",
  "timestamp": "2025-07-23T10:00:00.000Z",
  "data": {
    "10001": {
      "value": "220.123456",
      "timestamp": "2025-07-23T10:00:00.000Z"
    },
    "10002": {
      "value": "221.234567",
      "timestamp": "2025-07-23T10:00:00.000Z"
    }
  }
}
```

#### 1.2 批量数据更新
```json
{
  "type": "batch_data",
  "timestamp": "2025-07-23T10:00:00.000Z",
  "channels": {
    "1001": {
      "m": {
        "10001": "220.123456",
        "10002": "221.234567"
      },
      "s": {
        "20001": "1.000000",
        "20002": "0.000000"
      }
    },
    "1002": {
      "m": {
        "10001": "380.123456"
      }
    }
  }
}
```

#### 1.3 模型数据更新
```json
{
  "type": "model_update",
  "model": "power_meter",
  "instance": "meter_001",
  "timestamp": "2025-07-23T10:00:00.000Z",
  "data": {
    "total_power": {
      "value": "33000.123456",
      "unit": "W"
    },
    "power_factor": {
      "value": "0.950000",
      "unit": ""
    }
  }
}
```

### 2. 告警通知

#### 2.1 新告警
```json
{
  "type": "alarm",
  "event": "created",
  "alarm": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "title": "高温告警",
    "description": "1号变压器温度超过阈值",
    "category": "environmental",
    "level": "critical",
    "source": {
      "channel": 1001,
      "point": 10001,
      "value": "85.500000"
    },
    "created_at": "2025-07-23T10:00:00.000Z"
  }
}
```

#### 2.2 告警状态更新
```json
{
  "type": "alarm_update",
  "alarm_id": "550e8400-e29b-41d4-a716-446655440000",
  "event": "acknowledged",
  "status": "acknowledged",
  "user": "operator1",
  "notes": "正在处理",
  "timestamp": "2025-07-23T10:05:00.000Z"
}
```

### 3. 系统事件

#### 3.1 服务状态变化
```json
{
  "type": "service_status",
  "service": "comsrv",
  "status": "offline",
  "reason": "Connection lost",
  "timestamp": "2025-07-23T10:00:00.000Z"
}
```

#### 3.2 配置变更通知
```json
{
  "type": "config_change",
  "key": "cfg:channel:1001",
  "action": "update",
  "user": "admin",
  "timestamp": "2025-07-23T10:00:00.000Z"
}
```

#### 3.3 用户操作通知
```json
{
  "type": "user_action",
  "action": "control_command",
  "user": "operator1",
  "details": {
    "channel": 1001,
    "point": 30001,
    "value": 1.0
  },
  "timestamp": "2025-07-23T10:00:00.000Z"
}
```

### 4. 响应消息

#### 4.1 数据请求响应
```json
{
  "type": "data_response",
  "id": "req_123",
  "success": true,
  "data": {
    "channel": 1001,
    "data_type": "m",
    "values": {
      "10001": "220.123456",
      "10002": "221.234567"
    }
  }
}
```

#### 4.2 控制命令响应
```json
{
  "type": "control_response",
  "id": "cmd_125",
  "success": true,
  "status": "executed",
  "execution_time": 150,  // ms
  "timestamp": "2025-07-23T10:00:00.150Z"
}
```

#### 4.3 错误响应
```json
{
  "type": "error",
  "id": "req_123",
  "error": {
    "code": "INVALID_CHANNEL",
    "message": "通道不存在",
    "details": {
      "channel": 9999
    }
  }
}
```

### 5. 订阅确认

```json
{
  "type": "subscription_confirmed",
  "subscriptions": {
    "channels": {
      "active": [1001, 1002],
      "data_types": ["m", "s"]
    },
    "alarms": {
      "levels": ["critical", "major"]
    },
    "models": {
      "active": ["power_meter"]
    }
  }
}
```

## 错误处理

### 错误码

| 错误码 | 说明 |
|-------|------|
| AUTH_FAILED | 认证失败 |
| AUTH_EXPIRED | 认证过期 |
| INVALID_MESSAGE | 消息格式错误 |
| UNKNOWN_TYPE | 未知消息类型 |
| INVALID_CHANNEL | 无效通道 |
| INVALID_POINT | 无效点位 |
| PERMISSION_DENIED | 权限不足 |
| RATE_LIMIT | 频率限制 |
| INTERNAL_ERROR | 内部错误 |

### 错误消息格式

```json
{
  "type": "error",
  "error": {
    "code": "INVALID_MESSAGE",
    "message": "消息格式错误",
    "details": {
      "field": "channels",
      "reason": "必须是数组"
    }
  },
  "timestamp": "2025-07-23T10:00:00.000Z"
}
```

## 断线重连

### 重连策略

客户端应实现自动重连机制：

```javascript
class WebSocketClient {
  constructor(url) {
    this.url = url;
    this.reconnectInterval = 1000;  // 初始重连间隔
    this.maxReconnectInterval = 30000;  // 最大重连间隔
    this.reconnectAttempts = 0;
  }
  
  connect() {
    this.ws = new WebSocket(this.url);
    
    this.ws.onopen = () => {
      console.log('Connected');
      this.reconnectInterval = 1000;
      this.reconnectAttempts = 0;
      this.authenticate();
    };
    
    this.ws.onclose = (event) => {
      console.log('Disconnected:', event.code);
      if (event.code !== 1000) {  // 非正常关闭
        this.scheduleReconnect();
      }
    };
  }
  
  scheduleReconnect() {
    this.reconnectAttempts++;
    const interval = Math.min(
      this.reconnectInterval * Math.pow(2, this.reconnectAttempts - 1),
      this.maxReconnectInterval
    );
    
    console.log(`Reconnecting in ${interval}ms...`);
    setTimeout(() => this.connect(), interval);
  }
}
```

### 断线恢复

重连后需要：
1. 重新认证
2. 恢复之前的订阅
3. 请求错过的数据（如果需要）

```json
{
  "type": "restore_subscriptions",
  "subscriptions": {
    "channels": [1001, 1002],
    "data_types": ["m", "s"],
    "alarms": {
      "levels": ["critical", "major"]
    }
  },
  "last_message_id": "msg_12345"  // 可选：最后收到的消息ID
}
```

## 性能优化

### 1. 数据压缩

对于大量数据，使用增量更新：

```json
{
  "type": "delta_update",
  "channel": 1001,
  "base_timestamp": "2025-07-23T10:00:00.000Z",
  "changes": {
    "10001": {
      "value": "220.123456",
      "delta": "+0.123456"
    }
  }
}
```

### 2. 批量订阅

使用通配符订阅多个资源：

```json
{
  "type": "subscribe_pattern",
  "patterns": [
    "channel:100*:m",  // 所有1000-1009通道的测量数据
    "model:power_*"    // 所有电力相关模型
  ]
}
```

### 3. 数据过滤

在订阅时指定过滤条件减少数据传输：

```json
{
  "type": "subscribe_filtered",
  "channel": 1001,
  "data_type": "m",
  "filters": {
    "points": [10001, 10002],  // 只要特定点位
    "change_threshold": 0.01,   // 变化超过阈值才推送
    "min_interval": 1000       // 最小推送间隔(ms)
  }
}
```

## 安全考虑

### 1. 认证和授权
- 使用 JWT token 进行认证
- Token 过期自动断开连接
- 基于角色的订阅权限控制

### 2. 消息验证
- 验证消息大小限制（1MB）
- 验证消息格式和字段类型
- 防止注入攻击

### 3. 速率限制
- 每个连接的消息发送频率限制
- 订阅数量限制
- 命令执行频率限制

## 客户端示例

### JavaScript/TypeScript

```typescript
class VoltageEMSWebSocket {
  private ws: WebSocket;
  private subscriptions = new Map<string, any>();
  private messageHandlers = new Map<string, Function>();
  
  constructor(private url: string, private token: string) {
    this.setupHandlers();
  }
  
  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.ws = new WebSocket(this.url);
      
      this.ws.onopen = () => {
        this.authenticate().then(resolve).catch(reject);
      };
      
      this.ws.onmessage = (event) => {
        const message = JSON.parse(event.data);
        this.handleMessage(message);
      };
      
      this.ws.onerror = reject;
    });
  }
  
  private async authenticate(): Promise<void> {
    return this.sendMessage({
      type: 'auth',
      token: `Bearer ${this.token}`
    });
  }
  
  subscribeChannels(channels: number[], dataTypes: string[]): Promise<void> {
    return this.sendMessage({
      type: 'subscribe',
      channels,
      data_types: dataTypes
    });
  }
  
  onData(handler: (data: any) => void): void {
    this.messageHandlers.set('data', handler);
  }
  
  onAlarm(handler: (alarm: any) => void): void {
    this.messageHandlers.set('alarm', handler);
  }
  
  private handleMessage(message: any): void {
    const handler = this.messageHandlers.get(message.type);
    if (handler) {
      handler(message);
    }
  }
  
  private sendMessage(message: any): Promise<void> {
    return new Promise((resolve, reject) => {
      if (this.ws.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify(message));
        resolve();
      } else {
        reject(new Error('WebSocket not connected'));
      }
    });
  }
}

// 使用示例
const client = new VoltageEMSWebSocket('ws://localhost:8080/ws', 'your-token');

await client.connect();
await client.subscribeChannels([1001, 1002], ['m', 's']);

client.onData((data) => {
  console.log('Received data:', data);
});

client.onAlarm((alarm) => {
  console.log('New alarm:', alarm);
});
```

### Python

```python
import asyncio
import json
import websockets
from typing import Dict, List, Callable

class VoltageEMSWebSocket:
    def __init__(self, url: str, token: str):
        self.url = url
        self.token = token
        self.ws = None
        self.handlers: Dict[str, Callable] = {}
        
    async def connect(self):
        self.ws = await websockets.connect(self.url)
        await self.authenticate()
        
    async def authenticate(self):
        await self.send_message({
            'type': 'auth',
            'token': f'Bearer {self.token}'
        })
        
        response = await self.receive_message()
        if not response.get('success'):
            raise Exception('Authentication failed')
            
    async def subscribe_channels(self, channels: List[int], data_types: List[str]):
        await self.send_message({
            'type': 'subscribe',
            'channels': channels,
            'data_types': data_types
        })
        
    def on_data(self, handler: Callable):
        self.handlers['data'] = handler
        
    def on_alarm(self, handler: Callable):
        self.handlers['alarm'] = handler
        
    async def listen(self):
        async for message in self.ws:
            data = json.loads(message)
            message_type = data.get('type')
            
            if message_type in self.handlers:
                await self.handlers[message_type](data)
                
    async def send_message(self, message: dict):
        await self.ws.send(json.dumps(message))
        
    async def receive_message(self) -> dict:
        message = await self.ws.recv()
        return json.loads(message)
        
    async def close(self):
        await self.ws.close()

# 使用示例
async def main():
    client = VoltageEMSWebSocket('ws://localhost:8080/ws', 'your-token')
    await client.connect()
    
    await client.subscribe_channels([1001, 1002], ['m', 's'])
    
    def handle_data(data):
        print(f"Received data: {data}")
        
    def handle_alarm(alarm):
        print(f"New alarm: {alarm}")
        
    client.on_data(handle_data)
    client.on_alarm(handle_alarm)
    
    await client.listen()

asyncio.run(main())
```

## 调试和测试

### 测试工具

1. **wscat** - 命令行 WebSocket 客户端
```bash
wscat -c ws://localhost:8080/ws
> {"type":"auth","token":"Bearer eyJ..."}
< {"type":"auth_response","success":true}
> {"type":"subscribe","channels":[1001],"data_types":["m"]}
```

2. **Chrome DevTools**
- 打开开发者工具
- Network 标签页
- 过滤 WS 类型
- 查看 WebSocket 帧

3. **Postman**
- 支持 WebSocket 测试
- 可以保存测试集合
- 支持变量和脚本

### 日志记录

建议在客户端实现详细的日志记录：

```javascript
class WebSocketLogger {
  log(level, message, data) {
    const timestamp = new Date().toISOString();
    console.log(`[${timestamp}] [${level}] ${message}`, data);
  }
  
  logMessage(direction, message) {
    this.log('DEBUG', `${direction} message`, message);
  }
  
  logError(error) {
    this.log('ERROR', 'WebSocket error', error);
  }
}
```

## 常见问题

### 1. 连接立即断开
- 检查认证 token 是否有效
- 确认在 5 秒内发送认证消息
- 查看服务器返回的错误信息

### 2. 没有收到数据推送
- 确认订阅消息发送成功
- 检查订阅的通道和数据类型是否正确
- 验证用户权限

### 3. 消息发送失败
- 检查 WebSocket 连接状态
- 验证消息格式是否正确
- 确认消息大小不超过 1MB

### 4. 频繁断线
- 实现心跳机制
- 检查网络稳定性
- 查看服务器日志

## 版本历史

### v1.0.0 (2025-07-23)
- 初始版本
- 支持实时数据订阅
- 告警通知
- 控制命令
- 系统事件