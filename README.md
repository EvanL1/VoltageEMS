# VoltageEMS

<div align="center">

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![Redis](https://img.shields.io/badge/redis-7.0%2B-red.svg)](https://redis.io/)
[![Docker](https://img.shields.io/badge/docker-ready-blue.svg)](https://www.docker.com/)

**高性能工业物联网能源管理系统**

[架构文档](docs/architecture/system-architecture.md) | [开发指南](docs/development-guide.md) | [部署指南](docs/deployment-guide.md) | [API 文档](docs/api-reference.md)

</div>

## 概述

VoltageEMS 是一个基于 Rust 构建的分布式工业物联网平台，专注于能源管理和实时数据采集。系统采用微服务架构，通过 Redis 作为中央消息总线，实现高性能、高可靠的工业数据处理。

### 核心特性

- 🚀 **高性能**: 基于 Rust 的零成本抽象，支持百万级点位实时处理
- 🔌 **多协议支持**: Modbus、IEC 60870、CAN 等工业协议插件化支持
- 📊 **实时计算**: DAG 计算引擎，支持复杂的实时数据处理
- 🏭 **物模型抽象**: 完整的设备建模和实例管理系统
- ☁️ **云端集成**: 支持 AWS IoT、阿里云等主流云平台
- 📈 **时序存储**: InfluxDB 集成，支持历史数据查询和分析
- 🚨 **智能告警**: 灵活的规则引擎和多渠道通知
- 🔒 **安全可靠**: TLS 加密、JWT 认证、完善的权限管理

## 系统架构

```
┌─────────────────────────────────────────────────────────────┐
│                      Web Application                        │
│            Web UI | Mobile App │ HMI/SCADA                  │
└─────────────────────────┬───────────────────────────────────┘
                          │
                   ┌──────┴──────┐
                   │ API Gateway │
                   └──────┬──────┘
                          │
┌─────────────────────────┴───────────────────────────────────┐
│                    Redis Message                            │
│              Pub/Sub | Key-Value | Streams                  │
└──┬──────────┬────────┬─────────┬──────────┬──────────┬──────┘
   │          │        │         │          │          │
┌──┴───┐  ┌───┴──┐  ┌──┴───┐  ┌──┴───┐  ┌───┴────┐  ┌──┴──┐
│comsrv│  │modsrv│  │hissrv│  │netsrv│  │alarmsrv│  │ ... │
└──┬───┘  └──────┘  └──────┘  └──────┘  └────────┘  └─────┘
   │
┌──┴──────────────────────────────┐
│            Devices              │
│   Modbus | IEC60870 | CAN | ... │
└─────────────────────────────────┘
```

## 快速开始

### 环境要求

- Rust 1.70+
- Redis 7.0+
- Docker 20.10+ (可选)
- Git 2.30+

### 安装

```bash
# 克隆仓库
git clone https://github.com/VoltageEMS/VoltageEMS.git
cd VoltageEMS

# 安装依赖
cargo build --workspace

# 启动 Redis
docker run -d --name redis -p 6379:6379 redis:7-alpine

# 运行服务
cargo run -p comsrv
```

### Docker 部署

```bash
# 使用 Docker Compose 一键部署
docker-compose up -d

# 查看服务状态
docker-compose ps

# 查看日志
docker-compose logs -f
```

详细部署说明请参考 [部署指南](docs/deployment-guide.md)。

## 核心服务

### comsrv - 通信服务

负责工业协议数据采集，支持插件化协议扩展。

- 支持 Modbus TCP/RTU、IEC 60870、CAN 等协议
- 统一的传输层抽象（TCP、Serial、CAN、GPIO）
- 高性能批量数据处理
- 实时命令订阅和执行

### modsrv - 计算服务

提供实时数据计算和物模型管理。

- 设备物模型抽象
- DAG 计算引擎
- 规则引擎集成
- 高性能缓存层

### hissrv - 历史服务

时序数据存储和查询服务。

- Redis 到 InfluxDB 数据桥接
- 自动降采样和数据压缩
- 灵活的保留策略
- 高性能查询接口

### netsrv - 云网关服务

多云平台数据同步网关。

- 支持 AWS IoT、阿里云 IoT 等
- MQTT、HTTP/HTTPS 协议适配
- 断线缓存和自动重连
- 数据过滤和聚合

### alarmsrv - 告警服务

实时告警检测和通知管理。

- 灵活的告警规则配置
- 智能告警抑制
- 多渠道通知（邮件、短信、Webhook）
- 完整的告警生命周期管理

### apigateway - API 网关

统一的外部访问入口。

- RESTful API
- WebSocket 实时推送
- JWT 认证授权
- 请求路由和负载均衡

## 数据流

### Redis 扁平化存储

系统采用高性能的扁平化键值存储设计：

```
键格式: {channel_id}:{type}:{point_id}
值格式: {value}:{timestamp}

示例:
1001:m:10001 -> "380.5:1704956400"    # 通道1001的测量点10001，值为380.5
1001:s:20001 -> "1:1704956400"        # 通道1001的信号点20001，值为1
```

类型映射：

- `m` (Measurement): 遥测/模拟量
- `s` (Signal): 遥信/数字量
- `c` (Control): 遥控/控制命令
- `a` (Adjustment): 遥调/设定值

## 配置示例

### 通道配置 (comsrv)

```yaml
channels:
  - id: 1001
    name: "主变电站"
    protocol_type: "modbus_tcp"
    transport:
      type: "tcp"
      host: "192.168.1.100"
      port: 502
    protocol_params:
      slave_id: 1
      timeout_ms: 1000
    points_config:
      base_path: "config/ModbusTCP_Test_01"
```

### 设备模型 (modsrv)

```yaml
id: power_meter_v1
name: 智能电表
device_type: energy

telemetry:
  - identifier: voltage_a
    name: A相电压
    mapping:
      channel_id: 1001
      point_type: m
      point_id: 10001
    unit: V

calculations:
  - identifier: total_power
    inputs: [power_a, power_b, power_c]
    expression:
      built_in:
        function: sum
```

## 性能指标

- **数据采集**: < 100ms 延迟，10,000+ points/s
- **实时计算**: < 50ms P99 延迟
- **存储写入**: 支持 100,000+ points/s 批量写入
- **查询响应**: < 200ms P95

## 开发

### 项目结构

```
VoltageEMS/
├── services/           # 微服务
│   ├── comsrv/        # 通信服务
│   ├── modsrv/        # 计算服务
│   ├── hissrv/        # 历史服务
│   └── ...
├── libs/              # 共享库
│   └── voltage-common/
├── docs/              # 文档
├── config/            # 配置文件
└── scripts/           # 脚本工具
```

### 开发环境

```bash
# 安装开发工具
cargo install cargo-watch cargo-nextest

# 运行测试
cargo test --workspace

# 代码检查
cargo fmt --all
cargo clippy --all-targets --all-features
```

详细开发说明请参考 [开发指南](docs/development-guide.md)。

## 文档

- [系统架构](docs/architecture/system-architecture.md)
- [Redis 存储架构](docs/architecture/redis-storage-architecture.md)
- [数据流架构](docs/architecture/data-flow-architecture.md)
- [开发指南](docs/development-guide.md)
- [部署指南](docs/deployment-guide.md)
- [API 参考](docs/api-reference.md)

完整文档请访问 [文档中心](docs/README.md)。

## 贡献

欢迎贡献代码、文档或提出建议！

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'feat: add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

请确保遵循我们的[贡献指南](CONTRIBUTING.md)和[行为准则](CODE_OF_CONDUCT.md)。

## 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。

## 致谢

- [Rust](https://www.rust-lang.org/) - 系统编程语言
- [Redis](https://redis.io/) - 内存数据库
- [InfluxDB](https://www.influxdata.com/) - 时序数据库
- [Tokio](https://tokio.rs/) - 异步运行时

## 联系我们

- 项目主页: [https://github.com/VoltageEMS/VoltageEMS](https://github.com/VoltageEMS/VoltageEMS)
- Issue 追踪: [GitHub Issues](https://github.com/VoltageEMS/VoltageEMS/issues)
- 讨论社区: [Discussions](https://github.com/VoltageEMS/VoltageEMS/discussions)

---

<div align="center">
Made with ❤️ by the VoltageEMS Team
</div>
