# 健康检查 API 端点

## 当前可用的端点

### 1. 通道列表
- **URL**: `/api/v1/channels`
- **方法**: GET
- **描述**: 获取所有通道状态，可用作健康检查

### 2. 系统信息（需要实现）
- **URL**: `/health`
- **方法**: GET
- **描述**: 专门的健康检查端点
- **响应示例**:
```json
{
  "status": "healthy",
  "service": "comsrv",
  "timestamp": "2025-07-10T06:00:00Z",
  "channels": {
    "total": 2,
    "running": 1,
    "failed": 1
  },
  "redis": {
    "connected": true
  }
}
```

## 临时解决方案

使用 `/api/v1/channels` 作为健康检查端点，如果返回 200 状态码则认为服务健康。
