# ModSrv API 迁移指南

## 概述

从ModSrv 2.1.0版本开始，实时数据读取API将迁移到API Gateway，以提供更好的性能和统一的访问入口。本指南帮助您平滑迁移到新的API架构。

## 架构变化

### 之前的架构
```
客户端 → ModSrv → Redis
```

### 新架构
```
客户端 → API Gateway → Redis (直接读取)
客户端 → API Gateway → ModSrv (配置和控制)
```

## API变更对照表

| 功能 | 旧API (ModSrv) | 新API (API Gateway) | 状态 |
|------|----------------|---------------------|------|
| 获取模型列表 | GET /models | GET /api/modsrv/models | 保留 |
| 获取模型配置 | GET /models/{id}/config | GET /api/modsrv/models/{id}/config | 保留 |
| 获取实时数据 | GET /models/{id}/values | GET /api/v2/realtime/models/{id} | **已迁移** |
| 获取完整信息 | GET /models/{id} | 分别调用config和realtime接口 | **已废弃** |
| 执行控制命令 | POST /models/{id}/control/{name} | POST /api/modsrv/models/{id}/control/{name} | 保留 |
| WebSocket订阅 | ws://modsrv:8092/ws/models/{id}/values | ws://api-gateway/ws/realtime | **已迁移** |

## 迁移步骤

### 1. 更新客户端配置

```javascript
// config.js
const config = {
  // 旧配置
  // modsrvUrl: 'http://modsrv:8092',
  
  // 新配置
  apiGatewayUrl: 'http://api-gateway',
  modsrvApiPath: '/api/modsrv',
  realtimeApiPath: '/api/v2/realtime'
};
```

### 2. 更新API调用

#### 获取实时数据

```javascript
// 旧代码
async function getModelValues(modelId) {
  const response = await fetch(`http://modsrv:8092/models/${modelId}/values`);
  return response.json();
}

// 新代码
async function getModelValues(modelId) {
  const response = await fetch(`http://api-gateway/api/v2/realtime/models/${modelId}`);
  const data = await response.json();
  
  // 注意: 返回格式略有不同
  // 旧格式: { monitoring: {...}, timestamp: ... }
  // 新格式: 直接返回Redis Hash内容
  return {
    monitoring: data,
    timestamp: Date.now()
  };
}
```

#### 批量获取实时数据

```javascript
// 新功能: 批量读取
async function getBatchValues(modelIds) {
  const response = await fetch('http://api-gateway/api/v2/realtime/batch', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ models: modelIds })
  });
  return response.json();
}
```

#### WebSocket订阅

```javascript
// 旧代码
const ws = new WebSocket(`ws://modsrv:8092/ws/models/${modelId}/values`);

// 新代码
const ws = new WebSocket('ws://api-gateway/ws/realtime');

ws.onopen = () => {
  // 发送订阅消息
  ws.send(JSON.stringify({
    type: 'subscribe',
    channels: [`models/${modelId}`]
  }));
};

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  // 处理格式:
  // {
  //   channel: 'models/power_meter',
  //   data: { voltage_a: 220.123456, ... },
  //   timestamp: 1704067200
  // }
};
```

### 3. 兼容性处理

为了平滑过渡，可以创建一个适配器：

```javascript
class ApiAdapter {
  constructor(useNewApi = true) {
    this.useNewApi = useNewApi;
    this.modsrvUrl = 'http://modsrv:8092';
    this.gatewayUrl = 'http://api-gateway';
  }
  
  async getModelValues(modelId) {
    if (this.useNewApi) {
      // 使用新API
      const response = await fetch(`${this.gatewayUrl}/api/v2/realtime/models/${modelId}`);
      const data = await response.json();
      return { monitoring: data, timestamp: Date.now() };
    } else {
      // 使用旧API
      const response = await fetch(`${this.modsrvUrl}/models/${modelId}/values`);
      return response.json();
    }
  }
  
  async executeControl(modelId, controlName, value) {
    // 控制命令始终通过ModSrv
    const url = this.useNewApi 
      ? `${this.gatewayUrl}/api/modsrv/models/${modelId}/control/${controlName}`
      : `${this.modsrvUrl}/models/${modelId}/control/${controlName}`;
      
    return fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ value })
    });
  }
}
```

## 性能对比

| 指标 | 旧架构 (通过ModSrv) | 新架构 (直接读取) |
|------|-------------------|------------------|
| 延迟 | ~20-30ms | <10ms |
| 吞吐量 | 1000 req/s | >5000 req/s |
| 网络跳数 | 2 | 1 |
| CPU使用 | 中等 | 低 |

## 注意事项

### 1. 认证和授权

新API通过API Gateway进行统一认证：

```javascript
// 添加认证头
const headers = {
  'Authorization': `Bearer ${token}`,
  'Content-Type': 'application/json'
};
```

### 2. 错误处理

```javascript
try {
  const response = await fetch(`${gatewayUrl}/api/v2/realtime/models/${modelId}`);
  
  if (response.status === 404) {
    // 模型不存在或无数据
    console.error('Model not found or no data available');
    return null;
  }
  
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }
  
  return await response.json();
} catch (error) {
  console.error('Failed to fetch model values:', error);
  // 可以回退到旧API或返回缓存数据
}
```

### 3. 监控迁移进度

建议添加监控来跟踪新旧API的使用情况：

```javascript
// 记录API调用
function logApiCall(apiType, endpoint) {
  // 发送到监控系统
  metrics.increment(`api.calls.${apiType}`, {
    endpoint: endpoint,
    timestamp: Date.now()
  });
}
```

## 时间表

- **2024年1月**: 新API上线，旧API标记为废弃
- **2024年3月**: 监控并协助主要客户迁移
- **2024年6月**: 旧API进入维护模式，仅修复严重bug
- **2024年12月**: 计划下线旧API

## FAQ

### Q: 为什么要迁移到API Gateway？

A: 主要原因包括：
- 性能提升：直接从Redis读取，减少中间层
- 统一入口：所有API通过Gateway访问，便于管理
- 更好的缓存：Gateway层可以实现智能缓存策略
- 横向扩展：Gateway可以独立扩展，不影响业务服务

### Q: 旧API会立即下线吗？

A: 不会。我们提供了充足的过渡期，旧API至少会保留到2024年底。

### Q: 新API的数据格式有变化吗？

A: 实时数据API直接返回Redis中的数据，格式更简洁。配置和控制API保持不变。

### Q: 如何处理WebSocket重连？

A: 建议使用支持自动重连的WebSocket库，如`reconnecting-websocket`：

```javascript
import ReconnectingWebSocket from 'reconnecting-websocket';

const ws = new ReconnectingWebSocket('ws://api-gateway/ws/realtime', [], {
  maxReconnectionDelay: 10000,
  minReconnectionDelay: 1000,
  reconnectionDelayGrowFactor: 1.3
});
```

### Q: 批量读取有数量限制吗？

A: 建议单次批量请求不超过100个模型，以获得最佳性能。

## 获取帮助

如果在迁移过程中遇到问题，请通过以下方式获取帮助：

1. 查看API文档：http://api-gateway/docs
2. 提交Issue：https://github.com/voltage-ems/issues
3. 联系技术支持：support@voltage-ems.com