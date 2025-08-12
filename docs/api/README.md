# VoltageEMS API 文档

## 概述

VoltageEMS 提供了完整的 API 接口用于前后端数据交互，包括 WebSocket 实时数据推送和 HTTP REST API 两种方式。

## 架构概览

```
┌─────────────┐     WebSocket      ┌──────────────┐
│   客户端     │ <----------------> │ WS Gateway   │
│             │                    │   (6100)     │
│  Web/Mobile │     HTTP/REST      │              │
│             │ <----------------> │ API Gateway  │
└─────────────┘                    │   (6005)     │
                                   └──────────────┘
                                          │
                                   ┌──────▼──────┐
                                   │   Services  │
                                   │  Mesh Layer │
                                   └──────┬──────┘
                                          │
                                 ┌────────┴────────┐
                                 │                 │
                           ┌─────▼─────┐    ┌─────▼─────┐
                           │   Redis   │    │ InfluxDB  │
                           │  实时数据  │    │   历史数据  │
                           └───────────┘    └───────────┘
```

## 快速开始

### 1. 获取访问令牌

```bash
POST /api/v1/auth/login
Content-Type: application/json

{
  "username": "admin",
  "password": "password"
}
```

响应：
```json
{
  "success": true,
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIs...",
    "refresh_token": "eyJhbGciOiJIUzI1NiIs...",
    "expires_in": 900,
    "token_type": "Bearer"
  }
}
```

### 2. WebSocket 连接

```javascript
const token = 'your_access_token';
const ws = new WebSocket(`ws://localhost:6100/ws/v1/realtime?token=${token}`);

ws.onopen = () => {
  console.log('Connected to WebSocket');

  // 订阅数据
  ws.send(JSON.stringify({
    type: 'subscribe',
    data: {
      channels: ['1001', '1002'],
      data_types: ['T', 'S']
    }
  }));
};

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  console.log('Received:', message);
};
```

### 3. HTTP API 调用

```bash
# 获取设备列表
curl -H "Authorization: Bearer ${TOKEN}" \
     http://localhost:6005/api/v1/devices

# 获取实时数据
curl -H "Authorization: Bearer ${TOKEN}" \
     http://localhost:6005/api/v1/realtime/channels/1001
```

## API 文档索引

### 核心文档

- [WebSocket API](./websocket-api.md) - 实时数据推送接口
- [REST API](./rest-api.md) - HTTP RESTful 接口
- [数据模型](./data-models.md) - 数据结构定义
- [认证授权](./authentication.md) - 身份验证和权限控制
- [错误处理](./error-handling.md) - 错误码和异常处理

### 示例代码

- [JavaScript 客户端](./examples/javascript-client.md)
- [Python 客户端](./examples/python-client.md)
- [数据订阅示例](./examples/subscription-examples.md)
- [批量操作示例](./examples/batch-operations.md)

## 认证方式

### JWT Token 认证

所有 API 请求都需要在请求头中携带有效的 JWT token：

```http
Authorization: Bearer eyJhbGciOiJIUzI1NiIs...
```

Token 有效期：
- Access Token: 15分钟
- Refresh Token: 7天

### Token 刷新

```bash
POST /api/v1/auth/refresh
Content-Type: application/json

{
  "refresh_token": "your_refresh_token"
}
```

## 请求限流

API 请求限流策略：

| 类型 | 限制 | 时间窗口 |
|------|------|---------|
| 用户级别 | 1000 | 每分钟 |
| IP级别 | 5000 | 每分钟 |
| WebSocket连接 | 10 | 每用户 |
| 控制命令 | 100 | 每分钟 |

超过限制后返回 429 状态码：

```json
{
  "success": false,
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "请求过于频繁",
    "retry_after": 30
  }
}
```

## 数据格式

### 时间格式

所有时间戳使用 ISO 8601 格式：
```
2025-08-12T10:30:00Z
```

### 数据类型

实时数据类型标识：
- `T` - Telemetry (遥测)
- `S` - Signal (遥信)
- `C` - Control (遥控)
- `A` - Adjustment (遥调)

### 响应格式

成功响应：
```json
{
  "success": true,
  "data": {},
  "message": "操作成功",
  "timestamp": "2025-08-12T10:30:00Z",
  "request_id": "req_12345"
}
```

错误响应：
```json
{
  "success": false,
  "error": {
    "code": "ERROR_CODE",
    "message": "错误描述",
    "details": "详细信息"
  },
  "timestamp": "2025-08-12T10:30:00Z",
  "request_id": "req_12345"
}
```

## 环境配置

### 开发环境

```yaml
Base URL: http://localhost
WebSocket: ws://localhost:6100
API Gateway: http://localhost:6005
```

### 生产环境

```yaml
Base URL: https://voltage-ems.com
WebSocket: wss://voltage-ems.com/ws
API Gateway: https://voltage-ems.com/api
```

## 版本控制

API 版本通过 URL 路径指定：

- 当前版本: `v1`
- WebSocket: `/ws/v1/realtime`
- REST API: `/api/v1/*`

## 性能指标

系统设计性能目标：

| 指标 | 目标值 |
|------|--------|
| WebSocket 并发连接 | 10,000+ |
| API QPS | 50,000 |
| 实时数据延迟 | <100ms |
| API 响应时间 | <200ms |
| 系统可用性 | 99.9% |

## 安全建议

1. **使用 HTTPS/WSS**: 生产环境必须使用加密传输
2. **Token 安全**: 不要在 URL 参数中传递 token（WebSocket 连接除外）
3. **CORS 配置**: 严格配置跨域访问策略
4. **输入验证**: 对所有输入进行验证和清理
5. **日志审计**: 记录所有关键操作日志

## 支持与反馈

- GitHub Issues: [https://github.com/voltageems/api-issues](https://github.com/voltageems/api-issues)
- API 状态页: [https://status.voltage-ems.com](https://status.voltage-ems.com)
- 技术支持: api-support@voltage-ems.com

## 更新日志

### v1.0.0 (2025-08-12)
- 初始版本发布
- WebSocket 实时数据推送
- REST API 基础功能
- JWT 认证系统
- 基础数据订阅模式
