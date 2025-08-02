# VoltageEMS 架构概述

## 服务职责划分

### 1. ComSrv - 工业协议网关
**职责**：与物理设备通信，协议转换
- 支持多种工业协议（Modbus、CAN、IEC60870）
- 数据采集和命令下发
- 协议插件化架构
- 不包含业务逻辑

### 2. ModSrv - 设备影子服务
**职责**：设备数字孪生，状态映射
- 维护设备的 reported/desired/delta 状态
- 纯映射服务，零业务逻辑
- 设备注册和发现
- 状态变更通知

### 3. RuleSrv - 业务规则引擎
**职责**：业务逻辑执行，自动化控制
- DAG 规则引擎
- 条件判断和动作执行
- 复杂事件处理（CEP）
- 所有计算和控制逻辑

### 4. HisSrv - 历史数据服务
**职责**：时序数据持久化
- 从 Redis 到 InfluxDB 的数据写入
- 批量写入优化
- 数据生命周期管理
- 不提供查询功能

### 5. AlarmSrv - 告警管理服务
**职责**：告警检测和管理
- 阈值告警检测
- 告警生命周期管理
- 多级告警策略
- 告警通知分发

### 6. NetSrv - 云端网关
**职责**：数据上云转发
- 多云平台适配
- 协议转换（MQTT、HTTP）
- 断线缓存和重传
- 数据路由和过滤

### 7. API Gateway - API 网关
**职责**：统一入口，请求路由
- JWT 认证授权
- 请求代理和路由
- 直接读取 Redis 数据
- WebSocket 支持

## 数据流架构

### 上行数据流（设备→系统）
```
物理设备 → ComSrv → Redis → ModSrv（影子更新）
                         ↓
                    RuleSrv（规则判断）
                         ↓
                    动作执行/告警生成
```

### 下行控制流（系统→设备）
```
用户/规则 → ModSrv（更新 desired）→ Delta 计算
                                     ↓
                               通知 ComSrv
                                     ↓
                                物理设备
```

### 历史数据流
```
Redis 实时数据 → HisSrv → InfluxDB
                            ↓
                    API Gateway（查询代理）
```

## 关键设计原则

### 1. 单一职责
每个服务只负责一个明确的功能域：
- ComSrv：协议处理
- ModSrv：状态映射
- RuleSrv：业务逻辑
- HisSrv：数据存储

### 2. 松耦合
- 通过 Redis 进行服务间通信
- 事件驱动架构
- 标准化数据格式

### 3. 高性能
- Redis Hash 结构（O(1)访问）
- 批量操作优化
- 异步处理模式

### 4. 可扩展
- 插件化协议支持
- 水平扩展能力
- 配置化管理

## Redis 数据组织

### 实时数据
```
# 原始数据（ComSrv 写入）
comsrv:{channelID}:{type} → Hash

# 设备影子（ModSrv 维护）
modsrv:{modelName}:reported → Hash
modsrv:{modelName}:desired → Hash
modsrv:{modelName}:shadow → Hash（完整影子）

# 告警数据
alarm:{alarmID} → Hash
alarm:active → Set（活动告警）

# 规则状态
rule:{ruleID}:state → Hash
```

### 配置数据
```
# 设备映射
mapping:device:{deviceID} → modelName
mapping:channel:{channelID} → deviceID

# 规则定义
rule:{ruleID}:definition → JSON

# 告警阈值
alarm:threshold:{modelName}:{field} → value
```

## 部署架构

```
┌─────────────────────────────────────────────────┐
│                Load Balancer                    │
└─────────────────────┬───────────────────────────┘
                      │
         ┌────────────┴────────────┐
         │      API Gateway        │
         │    (多实例，无状态)      │
         └────────────┬────────────┘
                      │
    ┌─────────────────┴─────────────────┐
    │          Redis Cluster            │
    │    (主从复制 + Sentinel)          │
    └─────┬─────┬─────┬─────┬──────────┘
          │     │     │     │
     ┌────┴──┐ ┌┴───┐ ┌┴───┐ ┌────┐
     │ComSrv │ │Mod │ │Rule│ │... │
     │       │ │Srv │ │Srv │ │    │
     └───────┘ └────┘ └────┘ └────┘
```

## 服务间交互示例

### 1. 设备数据上报
```
1. 设备 → ComSrv: Modbus 数据包
2. ComSrv → Redis: HSET comsrv:1001:T field value
3. ComSrv → ModSrv: 通知数据更新
4. ModSrv: 更新 reported 状态，计算 delta
5. ModSrv → RuleSrv: 发布状态变更事件
6. RuleSrv: 评估规则，可能触发动作
```

### 2. 控制命令下发
```
1. 用户 → API Gateway: 设置温度为 25°C
2. API Gateway → ModSrv: 更新 desired.temperature = 25
3. ModSrv: 计算 delta，发现差异
4. ModSrv → ComSrv: 通知设备更新
5. ComSrv → 设备: Modbus 写命令
6. 设备 → ComSrv: 确认执行
7. ComSrv → ModSrv: 更新 reported.temperature = 25
8. ModSrv: delta 清空，同步完成
```

### 3. 告警生成流程
```
1. ModSrv: 设备状态更新
2. AlarmSrv: 订阅状态变化
3. AlarmSrv: 检测到 voltage > threshold
4. AlarmSrv → Redis: 创建告警记录
5. AlarmSrv → NetSrv: 告警上云
6. AlarmSrv → API Gateway: WebSocket 推送
```

## 最佳实践

1. **数据一致性**：使用 Redis 事务保证原子操作
2. **错误处理**：每个服务都有重试和降级机制
3. **监控告警**：完善的指标采集和告警
4. **配置管理**：支持热更新，版本控制
5. **安全防护**：JWT 认证，Redis ACL，TLS 加密