# ComSrv gRPC 协议插件架构方案

## 1. 概述

### 1.1 背景
当前 ComSrv 的协议实现与核心框架紧密耦合，所有协议都必须用 Rust 开发。为了提高系统的灵活性和可扩展性，我们计划引入 gRPC 插件架构，允许使用任意编程语言开发协议解析插件。

### 1.2 目标
- **语言无关**：支持 Python、Go、Node.js、Java 等语言开发协议插件
- **低耦合**：协议插件与 ComSrv 核心完全解耦
- **高性能**：通过批量处理和连接复用保证性能
- **易扩展**：简化新协议的接入流程
- **可维护**：插件可独立开发、测试和部署

## 2. 架构设计

### 2.1 整体架构

```
┌───────────────────────────────────────────────────────────┐
│                      ComSrv Core (Rust)                   │
│  ┌────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │  Factory   │  │   ComBase    │  │  Redis Storage   │   │
│  │            │  │ (gRPC Client)│  │                  │   │
│  └────────────┘  └──────┬───────┘  └──────────────────┘   │
└─────────────────────────┼─────────────────────────────────┘
                          │ gRPC
┌─────────────────────────┼─────────────────────────────────┐
│                    Protocol Plugins                       │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐   │
│  │ Modbus Plugin│  │ IEC104 Plugin│  │  CAN Plugin    │   │
│  │   (Python)   │  │    (Go)      │  │   (Node.js)    │   │
│  └──────────────┘  └──────────────┘  └────────────────┘   │
└───────────────────────────────────────────────────────────┘
```

### 2.2 核心组件

#### 2.2.1 ComSrv Core
- **gRPC Client Adapter**：统一的 gRPC 客户端适配器，实现 ComBase trait
- **Plugin Manager**：管理插件的生命周期（发现、健康检查、负载均衡）
- **Data Processor**：处理插件返回的原始数据（scale/offset 转换）
- **Storage Layer**：负责 Redis 存储和批量优化

#### 2.2.2 Protocol Plugin
- **gRPC Server**：提供标准化的 gRPC 服务接口
- **Protocol Parser**：特定协议的解析逻辑
- **Device Manager**：管理设备连接（可选，某些协议需要）

## 3. 接口定义

### 3.1 gRPC Service 定义

```protobuf
syntax = "proto3";

package comsrv.plugin.v1;

// 插件服务接口
service ProtocolPlugin {
  // 获取插件信息
  rpc GetInfo(Empty) returns (PluginInfo);

  // 解析原始数据
  rpc ParseData(ParseRequest) returns (ParseResponse);

  // 编码控制命令
  rpc EncodeCommand(EncodeRequest) returns (EncodeResponse);

  // 批量读取（主动轮询模式）
  rpc BatchRead(BatchReadRequest) returns (BatchReadResponse);

  // 健康检查
  rpc HealthCheck(Empty) returns (HealthStatus);
}

// 基础消息定义
message Empty {}

message PluginInfo {
  string name = 1;
  string version = 2;
  string protocol_type = 3;
  repeated string supported_features = 4;
  map<string, string> metadata = 5;
}

message ParseRequest {
  bytes raw_data = 1;
  map<string, string> context = 2;  // 通道配置等上下文信息
}

message ParseResponse {
  repeated PointData points = 1;
  string error = 2;
}

message PointData {
  uint32 point_id = 1;
  oneof value {
    double float_value = 2;
    int64 int_value = 3;
    bool bool_value = 4;
    string string_value = 5;
  }
  int64 timestamp = 6;
  uint32 quality = 7;  // 数据质量标志
}

message EncodeRequest {
  uint32 point_id = 1;
  PointData value = 2;
  map<string, string> context = 3;
}

message EncodeResponse {
  bytes encoded_data = 1;
  string error = 2;
}

message BatchReadRequest {
  map<string, string> connection_params = 1;  // 连接参数
  repeated uint32 point_ids = 2;             // 要读取的点位
  map<string, string> read_params = 3;       // 读取参数
}

message BatchReadResponse {
  repeated PointData points = 1;
  string error = 2;
}

message HealthStatus {
  bool healthy = 1;
  string message = 2;
  map<string, string> details = 3;
}
```

### 3.2 插件实现要求

插件必须实现以下核心功能：
1. **GetInfo**：返回插件的基本信息
2. **ParseData**：解析原始二进制数据为结构化数据
3. **EncodeCommand**：将控制命令编码为协议格式
4. **BatchRead**：主动读取设备数据（可选）
5. **HealthCheck**：健康状态检查

## 4. 数据流程

### 4.1 数据采集流程

```
1. ComSrv Core 调度轮询任务
   ↓
2. gRPC Client Adapter 调用插件的 BatchRead
   ↓
3. Plugin 读取设备数据并解析
   ↓
4. Plugin 返回 PointData 列表
   ↓
5. ComSrv Core 执行 scale/offset 转换
   ↓
6. 批量写入 Redis Hash
```

### 4.2 控制命令流程

```
1. ComSrv 从 Redis 订阅接收控制命令
   ↓
2. 调用插件的 EncodeCommand
   ↓
3. Plugin 返回编码后的数据
   ↓
4. ComSrv 发送到设备
```

## 5. 插件开发示例

### 5.1 Python 插件示例

```python
import grpc
from concurrent import futures
import protocol_plugin_pb2
import protocol_plugin_pb2_grpc

class ModbusPlugin(protocol_plugin_pb2_grpc.ProtocolPluginServicer):
    def GetInfo(self, request, context):
        return protocol_plugin_pb2.PluginInfo(
            name="modbus-plugin",
            version="1.0.0",
            protocol_type="modbus_tcp",
            supported_features=["batch_read", "write_single"]
        )

    def BatchRead(self, request, context):
        # 连接参数
        host = request.connection_params.get("host", "localhost")
        port = int(request.connection_params.get("port", "502"))

        # 读取 Modbus 数据
        points = []
        for point_id in request.point_ids:
            # 实际的 Modbus 读取逻辑
            value = read_modbus_register(host, port, point_id)
            points.append(protocol_plugin_pb2.PointData(
                point_id=point_id,
                float_value=value,
                timestamp=int(time.time() * 1000)
            ))

        return protocol_plugin_pb2.BatchReadResponse(points=points)

def serve():
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    protocol_plugin_pb2_grpc.add_ProtocolPluginServicer_to_server(
        ModbusPlugin(), server
    )
    server.add_insecure_port('[::]:50051')
    server.start()
    server.wait_for_termination()
```

### 5.2 Go 插件示例

```go
package main

import (
    "context"
    pb "comsrv/plugin/v1"
    "google.golang.org/grpc"
)

type IEC104Plugin struct {
    pb.UnimplementedProtocolPluginServer
}

func (p *IEC104Plugin) GetInfo(ctx context.Context, req *pb.Empty) (*pb.PluginInfo, error) {
    return &pb.PluginInfo{
        Name:        "iec104-plugin",
        Version:     "1.0.0",
        ProtocolType: "iec104",
    }, nil
}

func (p *IEC104Plugin) BatchRead(ctx context.Context, req *pb.BatchReadRequest) (*pb.BatchReadResponse, error) {
    // IEC104 读取逻辑
    points := make([]*pb.PointData, 0)
    // ...
    return &pb.BatchReadResponse{Points: points}, nil
}
```

## 6. 部署方案

### 6.1 容器化部署

```yaml
version: '3.8'

services:
  # ComSrv 核心服务
  comsrv:
    image: voltageems/comsrv:latest
    environment:
      PLUGIN_DISCOVERY: "dns"  # 使用 DNS 发现插件
      PLUGIN_ENDPOINTS: |
        modbus: modbus-plugin:50051
        iec104: iec104-plugin:50052
        can: can-plugin:50053
    depends_on:
      - redis
      - modbus-plugin
      - iec104-plugin

  # Modbus 插件（Python）
  modbus-plugin:
    image: voltageems/modbus-plugin:latest
    build:
      context: ./plugins/modbus
      dockerfile: Dockerfile
    ports:
      - "50051:50051"
    environment:
      LOG_LEVEL: "info"

  # IEC104 插件（Go）
  iec104-plugin:
    image: voltageems/iec104-plugin:latest
    build:
      context: ./plugins/iec104
      dockerfile: Dockerfile
    ports:
      - "50052:50052"

  # Redis
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
```

### 6.2 插件管理

#### 6.2.1 服务发现
- **静态配置**：在配置文件中指定插件地址
- **DNS 发现**：通过 DNS SRV 记录发现插件
- **服务注册**：插件主动注册到 ComSrv

#### 6.2.2 健康检查
- 定期调用 HealthCheck 接口
- 失败重试和熔断机制
- 自动故障转移（如果有多个插件实例）

#### 6.2.3 负载均衡
- 多个插件实例时的请求分发
- 基于响应时间的动态权重
- 连接池管理

## 7. 性能优化

### 7.1 批量处理
- 批量读取多个点位，减少 RPC 调用次数
- 批量写入 Redis，提高吞吐量

### 7.2 连接复用
- gRPC 连接池，避免频繁建立连接
- HTTP/2 多路复用，单连接处理多个请求

### 7.3 缓存策略
- 插件信息缓存，减少 GetInfo 调用
- 连接参数缓存，加速重连

### 7.4 异步处理
- ComSrv 使用异步 gRPC 客户端
- 支持流式 RPC 以提高实时性

## 8. 安全考虑

### 8.1 认证授权
- TLS 加密通信
- 基于证书的双向认证
- API Key 或 JWT 令牌验证

### 8.2 资源限制
- 请求速率限制
- 消息大小限制
- 超时控制

### 8.3 隔离性
- 插件运行在独立进程/容器
- 资源配额限制（CPU、内存）
- 网络隔离

## 9. 监控和可观测性

### 9.1 指标
- RPC 调用延迟
- 成功率和错误率
- 插件资源使用情况

### 9.2 日志
- 结构化日志
- 分布式追踪（OpenTelemetry）
- 错误聚合和告警

### 9.3 调试
- gRPC 反射支持
- 请求/响应日志
- 性能分析工具

## 10. 迁移计划

### 10.1 第一阶段：原型验证
1. 实现 gRPC 客户端适配器
2. 开发 Modbus Python 插件原型
3. 性能测试和对比

### 10.2 第二阶段：逐步迁移
1. 保留现有 Rust 插件
2. 新协议优先使用 gRPC 插件
3. 逐步迁移现有协议

### 10.3 第三阶段：完全切换
1. 所有协议使用 gRPC 插件
2. 移除旧的插件系统
3. 优化和调优

## 11. 风险和挑战

### 11.1 性能影响
- **风险**：gRPC 调用增加延迟
- **缓解**：批量处理、连接复用、本地部署

### 11.2 复杂度增加
- **风险**：部署和运维复杂度上升
- **缓解**：容器化、自动化部署、完善监控

### 11.3 兼容性
- **风险**：与现有系统的兼容性
- **缓解**：渐进式迁移、充分测试

## 12. 总结

gRPC 插件架构将为 ComSrv 带来以下优势：
1. **灵活性**：支持多语言开发，降低协议接入门槛
2. **可扩展性**：插件独立部署，便于横向扩展
3. **可维护性**：协议逻辑与核心系统解耦
4. **生态友好**：可复用现有协议库

通过合理的设计和实施，我们可以在保持高性能的同时，大幅提升系统的灵活性和可扩展性。
