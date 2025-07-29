# ModSrv 架构文档

## 概述

ModSrv（Model Service）是VoltageEMS系统中的轻量级模型服务，专为边端设备设计。它将底层设备数据抽象为统一的设备模型，提供标准化的监视（Monitoring）和控制（Control）接口。

## 架构特点

### 1. 轻量级设计
- **无内存缓存**：直接从Redis读取数据，减少内存占用
- **简化订阅**：不再维护复杂的数据订阅逻辑
- **资源友好**：适合资源受限的边端设备

### 2. 高效同步
- **Lua脚本**：使用Redis内置Lua脚本实现原子性数据同步
- **零延迟**：数据在Redis层面直接同步，无需网络往返
- **双向同步**：支持ComsRv到ModSrv以及反向的数据流

### 3. 简洁API
- **RESTful接口**：提供标准的HTTP API
- **WebSocket推送**：支持实时数据推送
- **最小依赖**：仅依赖Redis，部署简单

## 系统架构

```
┌─────────────────────────────────────────────────────────┐
│                    前端应用 / HMI                        │
└─────────────────────┬───────────────────────────────────┘
                      │ HTTP/WebSocket
                      ▼
┌─────────────────────────────────────────────────────────┐
│                      ModSrv                             │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │ API Server  │  │ Model Manager│  │ WebSocket Mgr │  │
│  └──────┬──────┘  └──────┬───────┘  └───────┬───────┘  │
│         │                 │                   │          │
│         └─────────────────┴───────────────────┘          │
│                           │                              │
│                    ┌──────▼──────┐                      │
│                    │  EdgeRedis  │                      │
│                    └──────┬──────┘                      │
└───────────────────────────┼─────────────────────────────┘
                            │
                     ┌──────▼──────┐
                     │    Redis    │
                     │  + Lua脚本  │
                     └──────┬──────┘
                            │
┌───────────────────────────┼─────────────────────────────┐
│                        ComsRv                           │
│                   (设备通信服务)                         │
└─────────────────────────────────────────────────────────┘
```

## 核心组件

### 1. Model Manager
负责模型元数据管理：
- 加载和存储模型定义
- 管理点位映射关系
- 提供模型查询接口

### 2. EdgeRedis
轻量级Redis连接管理器：
- 管理Redis连接池
- 加载和执行Lua脚本
- 提供数据读写接口

### 3. API Server
提供HTTP REST接口：
- 模型管理API
- 数据查询API
- 控制命令API

### 4. WebSocket Manager
管理WebSocket连接：
- 客户端连接管理
- 订阅Redis更新通道
- 推送实时数据变化

## 数据流

### 1. 测量数据流
```
设备 → ComsRv → Redis(Lua脚本) → ModSrv Hash → API/WebSocket → 客户端
```

### 2. 控制命令流
```
客户端 → API → EdgeRedis(Lua脚本) → Redis Pub/Sub → ComsRv → 设备
```

### 3. 数据同步机制

#### Lua脚本功能
```lua
-- 同步测量数据
sync_measurement(channel, point, value)
  → 查找映射: mapping:c2m:{channel}:{point}
  → 更新Hash: modsrv:{model}:measurement
  → 发布更新: modsrv:{model}:update

-- 发送控制命令  
send_control(model, control, value)
  → 查找映射: mapping:m2c:{model}:{control}
  → 发布命令: cmd:{channel}:control/adjustment
```

## Redis数据结构

### 1. 模型数据存储
```
# Hash结构存储实时数据
modsrv:{model_id}:measurement
  voltage_a: "220.123456"
  current_a: "10.567890"
  power: "2205.123456"
```

### 2. 映射存储
```
# C2M映射（ComsRv到ModSrv）
mapping:c2m:{channel}:{point} → "{model}:{point_name}"

# M2C映射（ModSrv到ComsRv）  
mapping:m2c:{model}:{control} → "{channel}:{point}"
```

### 3. 更新通道
```
# 数据更新通知
modsrv:{model_id}:update → "{point}:{value}"
```

## 性能特性

### 1. 内存优化
- 不缓存数据，按需从Redis读取
- 仅保存模型元数据
- 典型内存占用 < 50MB

### 2. 延迟特性
- 数据同步延迟 < 1ms（Lua脚本执行）
- API响应时间 < 10ms
- WebSocket推送延迟 < 5ms

### 3. 并发处理
- 支持数百个WebSocket连接
- API请求并发处理
- Redis连接池复用

## 部署模式

### 1. 单机部署
适用于边端设备：
```yaml
services:
  redis:
    image: redis:7-alpine
  modsrv:
    image: voltage/modsrv
    environment:
      - REDIS_URL=redis://redis:6379
```

### 2. 高可用部署
适用于关键应用：
- Redis主从复制
- ModSrv多实例
- 负载均衡

## 扩展性

### 1. 模型扩展
- 支持动态添加模型
- 热加载映射配置
- 无需重启服务

### 2. 功能扩展
- 插件式数据处理
- 自定义API端点
- 扩展WebSocket协议

## 安全考虑

### 1. 访问控制
- API Token认证
- WebSocket连接验证
- Redis ACL权限

### 2. 数据安全
- 敏感数据加密
- 审计日志记录
- 异常检测告警

## 与其他服务集成

### 1. ComsRv集成
- 通过Redis Pub/Sub通信
- Lua脚本自动同步数据
- 支持所有协议类型

### 2. RuleSrv集成
- RuleSrv直接读取ModSrv数据
- 支持基于模型的规则定义
- 控制命令通过ModSrv下发

### 3. API Gateway集成
- Gateway可直接读取Redis数据
- ModSrv提供模型元数据
- 统一的认证和路由

## 开发指南

### 1. 添加新模型
1. 在配置文件中定义模型
2. 创建映射文件
3. 重启服务加载配置

### 2. 扩展API
1. 在api.rs中添加新路由
2. 实现处理函数
3. 更新API文档

### 3. 自定义Lua脚本
1. 修改edge_sync.lua
2. 添加新的同步逻辑
3. 重新加载脚本

## 故障处理

### 1. 常见问题
- **数据不同步**：检查Lua脚本和映射配置
- **WebSocket断开**：检查网络和心跳设置
- **内存增长**：检查是否有连接泄漏

### 2. 调试方法
- 启用DEBUG日志
- 使用Redis Monitor
- 查看Lua脚本执行日志

### 3. 性能调优
- 调整Redis连接池大小
- 优化Lua脚本逻辑
- 使用批量操作

## 版本历史

### v2.0.0（当前版本）
- 重构为轻量级服务
- 引入Lua脚本同步
- 移除内存缓存机制

### v1.0.0
- 初始版本
- 基于内存缓存
- 复杂的订阅机制