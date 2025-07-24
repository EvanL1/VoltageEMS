# comsrv - 工业协议网关服务

## 概述

comsrv 是 VoltageEMS 的核心通信服务，负责与各种工业设备进行数据采集和控制。它提供了统一的协议插件架构，支持多种工业通信协议，并通过 Redis Hash 结构实现高性能的实时数据存储和发布。

## 主要特性

- **插件化协议支持**: Modbus TCP/RTU、IEC60870、CAN 等工业协议
- **统一传输层**: 支持 TCP、Serial、CAN、GPIO 等多种传输方式
- **高性能存储**: 使用 Redis Hash 结构，支持百万级点位的 O(1) 访问
- **实时发布订阅**: 通过 Redis Pub/Sub 实现数据变化的实时推送
- **标准化数据格式**: 所有浮点数值强制使用 6 位小数精度

## 快速开始

### 运行服务

```bash
cd services/comsrv
cargo run
```

### 配置文件

主配置文件位于 `config/default.yml`：

```yaml
service:
  name: "comsrv"
  redis:
    url: "redis://localhost:6379"
  logging:
    level: "info"
    file: "logs/comsrv.log"

csv_base_path: "./config"
channels:
  - id: 1001
    protocol_type: "modbus_tcp"
    points_config:
      base_path: "ModbusTCP_Test_01"
```

### 点位配置

点位配置使用 CSV 文件，位于 `config/{Protocol}_Test_{ID}/` 目录：

- `telemetry.csv` - 遥测点（YC）
- `signal.csv` - 遥信点（YX）
- `control.csv` - 遥控点（YK）
- `adjustment.csv` - 遥调点（YT）

## Redis 数据结构

### Hash 存储格式

```
键: comsrv:{channelID}:{type}
字段: {pointID}
值: "{value:.6f}"

示例:
comsrv:1001:m → {
    10001: "25.123456",
    10002: "26.789012"
}
```

### 发布消息格式

```
通道: comsrv:{channelID}:{type}
消息: "{pointID}:{value:.6f}"

示例:
通道: comsrv:1001:m
消息: "10001:25.123456"
```

### 类型映射

- `m` - 测量值 (YC - Yao Ce)
- `s` - 信号值 (YX - Yao Xin)
- `c` - 控制值 (YK - Yao Kong)
- `a` - 调节值 (YT - Yao Tiao)

## 控制命令

comsrv 订阅以下 Redis 通道接收控制命令：

- `cmd:{channelID}:control` - 遥控命令
- `cmd:{channelID}:adjustment` - 遥调命令

命令格式（JSON）：
```json
{
  "point_id": 30001,
  "value": 1.0,
  "timestamp": 1642592400000
}
```

## 开发指南

### 添加新协议

1. 实现 `ProtocolPlugin` trait
2. 注册协议到插件管理器
3. 配置协议参数

详见 [协议插件开发指南](docs/protocol-plugins.md)

### 监控和调试

```bash
# 监控 Redis 数据
redis-cli monitor | grep comsrv

# 查看特定通道数据
redis-cli hgetall "comsrv:1001:m"

# 订阅数据更新
redis-cli psubscribe "comsrv:1001:*"
```

## 性能优化

- 批量写入: 使用 pipeline 批量更新 Redis
- 连接池: 维护 Redis 连接池减少连接开销
- 异步处理: 采用 tokio 异步运行时
- 内存优化: 使用 Hash 结构减少键数量

## 环境变量

- `RUST_LOG` - 日志级别 (debug/info/warn/error)
- `REDIS_URL` - Redis 连接地址
- `COMSRV_PORT` - API 服务端口（默认 8081）

## 相关文档

- [架构设计](docs/architecture.md)
- [Redis 接口](docs/redis-interface.md)
- [协议插件开发](docs/protocol-plugins.md)