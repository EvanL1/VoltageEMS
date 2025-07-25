# ModSrv - 设备模型服务

[![Docker Test](https://img.shields.io/badge/Docker%20Test-100%25%20Pass-brightgreen.svg)](./test-results/)
[![API Coverage](https://img.shields.io/badge/API%20Coverage-100%25-brightgreen.svg)](./docs/api-testing.md)
[![Redis v3.2](https://img.shields.io/badge/Redis%20v3.2-Compatible-blue.svg)](./docs/data-structures.md)

## 概述

ModSrv (Model Service) 是VoltageEMS工业物联网系统中的设备模型管理服务，负责设备模型定义、实时数据处理和控制命令执行。本版本采用简化的监视/控制二分模型，提供高性能的实时数据处理和WebSocket推送功能。

### 🚀 核心特性

- **架构**: 二分类(监视/控制)
- **映射抽象**: 逻辑名称与物理地址完全分离的映射系统
- **实时推送**: WebSocket支持的实时数据推送
- **Docker化**: 完整的容器化部署和测试环境
- **100%测试**: 全面的功能性和性能测试覆盖

## 快速开始

### 🐳 Docker部署(推荐)

```bash
# 1. 启动生产环境
docker-compose up -d

# 2. 查看服务状态
docker-compose ps

# 3. 查看日志
docker-compose logs -f modsrv

# 4. 健康检查
curl http://localhost:8092/health
```

### 🧪 测试环境

```bash
# 运行完整测试环境(内网隔离，零外部端口)
./run-docker-test.sh

# 查看测试结果
docker-compose -f docker-compose.test.yml logs test-executor

# 清理测试环境
docker-compose -f docker-compose.test.yml down
```

### 🔧 本地开发

```bash
# 1. 启动Redis
docker run -d --name redis -p 6379:6379 redis:8-alpine

# 2. 构建并运行服务
cargo check --workspace  # 先检查编译
cargo run -p modsrv       # 运行服务

# 3. 验证API
curl http://localhost:8092/health
curl http://localhost:8092/models
```

## 核心功能

### 📊 设备模型管理

- **模型定义**: 基于YAML/JSON的设备模型配置
- **映射系统**: 逻辑点位名称到物理地址的映射管理
- **批量操作**: 高效的批量数据读写和更新

### 🔄 实时数据处理

- **Redis订阅**: 实时订阅ComsRv发布的设备数据
- **数据转换**: 自动进行物理地址到逻辑名称的映射转换
- **WebSocket推送**: 实时数据变化推送到前端应用

### 🎛️ 控制命令执行

- **REST API**: 通过HTTP API接收控制命令
- **命令转发**: 将控制命令发布到Redis供ComsRv执行
- **权限验证**: 控制命令的权限验证和审计

## 架构设计

### 🏗️ 整体架构

```
┌─────────────────────────────────────────┐
│              前端应用                   │
│     Web UI | Mobile | SCADA            │
└─────────────┬───────────────────────────┘
              │ HTTP/WebSocket
┌─────────────┴───────────────────────────┐
│            ModSrv v2.0                  │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐   │
│  │API Layer│ │WebSocket│ │ Mapping │   │
│  └─────────┘ └─────────┘ └─────────┘   │
└─────────────┬───────────────────────────┘
              │ Redis Pub/Sub & KV
┌─────────────┴───────────────────────────┐
│            Redis v3.2                   │
│    Hash存储 + Pub/Sub通知 + 控制命令    │
└─────────────┬───────────────────────────┘
              │
┌─────────────┴───────────────────────────┐
│             ComsRv                      │
│      工业协议通信服务                   │
└─────────────────────────────────────────┘
```

### 📁 代码结构

```
services/modsrv/
├── src/
│   ├── main.rs           # 服务入口
│   ├── lib.rs           # 库入口
│   ├── api.rs           # REST API接口
│   ├── model.rs         # 模型管理核心
│   ├── config.rs        # 配置管理
│   └── error.rs         # 错误处理
├── config/              # 配置文件
│   ├── config.yml       # 主配置
│   └── mappings/        # 映射配置
├── docs/                # 文档目录
├── templates/           # 设备模板
└── test-*              # 测试相关文件
```

## 数据架构

### 🗄️ Redis数据结构 (v3.2规范)

```redis
# 实时数据存储 (Hash)
comsrv:{channelID}:{type} → Hash{pointID: value}
# 示例: comsrv:1001:m → {10001: "220.123456", 10002: "221.567890"}

# 数据更新通知 (Pub/Sub)
通道: comsrv:{channelID}:{type}
消息: {pointID}:{value:.6f}
# 示例: 通道comsrv:1001:m, 消息"10001:220.123456"

# 控制命令发布
通道: cmd:{channelID}:control
消息: {pointID}:{value:.6f}
```

**类型映射**:
- `m`: 测量数据 (Measurement)
- `s`: 信号数据 (Signal)
- `c`: 控制数据 (Control)
- `a`: 调节数据 (Adjustment)

### 🔗 映射系统

```json
// test-configs/mappings/power_meter_demo.json
{
  "monitoring": {
    "voltage_a": {
      "channel": 1001,
      "point": 10001,
      "type": "m"
    }
  },
  "control": {
    "main_switch": {
      "channel": 1001,
      "point": 20001,
      "type": "c"
    }
  }
}
```

## API接口

### 🌐 REST API端点

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/health` | 健康检查 |
| `GET` | `/models` | 模型列表 |
| `GET` | `/models/{id}` | 模型详情 |
| `GET` | `/models/{id}/config` | 模型配置 |
| `GET` | `/models/{id}/values` | 实时数据 |
| `POST` | `/models/{id}/control/{name}` | 控制命令 |
| `WS` | `/ws/models/{id}/values` | WebSocket推送 |

### 📡 API示例

```bash
# 获取模型列表
curl http://localhost:8092/models

# 获取模型详情
curl http://localhost:8092/models/power_meter_demo

# 执行控制命令
curl -POST http://localhost:8092/models/power_meter_demo/control/main_switch \
  -H "Content-Type: application/json" \
  -d '{"value": 1.0}'

# WebSocket连接(JavaScript)
const ws = new WebSocket('ws://localhost:8092/ws/models/power_meter_demo/values');
ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('实时数据:', data);
};
```

## 配置说明

### ⚙️ 主配置文件 (`test-configs/config.yml`)

```yaml
service_name: "modsrv"
version: "2.0.0"

redis:
  url: "redis://localhost:6379"
  key_prefix: "modsrv:"
  connection_timeout_ms: 5000
  retry_attempts: 3

api:
  host: "0.0.0.0"
  port: 8092
  timeout_seconds: 30

models:
  # 模型配置文件目录
  models_dir: "/config/models"
  # 映射配置目录
  mappings_dir: "/config/mappings"
  # 自动加载模型配置文件
  auto_load: true
  # 模型配置文件格式
  config_format: "json"

update_interval_ms: 1000
```

### 🔧 环境变量

| 变量名 | 默认值 | 说明 |
|--------|--------|------|
| `CONFIG_FILE` | `test-configs/config.yml` | 主配置文件路径 |
| `MODELS_DIR` | `test-configs/models` | 模型配置目录路径 |
| `MAPPINGS_DIR` | `test-configs/mappings` | 映射配置目录 |
| `REDIS_URL` | `redis://localhost:6379` | Redis连接URL |
| `RUST_LOG` | `info` | 日志级别 |
| `RUST_BACKTRACE` | `0` | 错误堆栈跟踪 |

## 测试与验证

### ✅ 测试覆盖 (100%通过)

```bash
# 完整测试报告
总测试数: 11
通过测试: 11
成功率: 100%

测试项目:
├── ✅ redis_connection      - Redis连接测试
├── ✅ modsrv_health        - ModSrv健康检查
├── ✅ comsrv_data          - ComsRv数据验证
├── ✅ api_comprehensive    - API功能完整测试
├── ✅ redis_format         - Redis数据格式验证
├── ✅ instance_management  - 实例管理测试
├── ✅ telemetry_retrieval  - 遥测数据获取测试
├── ✅ command_execution    - 命令执行测试
├── ✅ load_test           - 负载测试(1552请求/秒)
├── ✅ data_persistence    - 数据持续性测试
└── ✅ template_system     - 模板系统测试
```

### 📊 性能指标

- **API响应时间**: < 1ms (健康检查0.49ms, 模型列表0.46ms)
- **负载测试吞吐量**: 1552.05 请求/秒
- **并发WebSocket连接**: 支持1000+并发连接
- **内存使用**: < 50MB (含缓存)

## 部署运维

### 🚀 生产部署

```bash
# 1. 构建生产镜像
docker build -t modsrv:v2.0 .

# 2. 使用docker-compose部署
docker-compose -f docker-compose.yml up -d

# 3. 监控服务状态
docker-compose ps
docker-compose logs -f modsrv

# 4. 健康检查
curl http://localhost:8092/health
```

### 📊 监控与日志

```bash
# 查看实时日志
docker-compose logs -f modsrv

# 查看性能指标
docker stats modsrv

# 查看Redis连接状态
docker exec modsrv redis-cli -h redis ping

# 备份配置
tar -czf config-backup-$(date +%Y%m%d).tar.gz config/
```

## 开发指南

### 🛠️ 开发环境设置

```bash
# 1. 克隆代码
git clone <repository-url>
cd VoltageEMS-modsrv/services/modsrv

# 2. 安装依赖
cargo check --workspace

# 3. 启动开发环境
docker run -d --name redis-dev -p 6379:6379 redis:8-alpine
RUST_LOG=debug cargo run

# 4. 代码格式化和检查
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```

### 🔍 调试技巧

```bash
# 启用调试日志
RUST_LOG=modsrv=debug,redis=info cargo run

# 错误堆栈跟踪
RUST_BACKTRACE=1 cargo run

# 性能分析
cargo bench -p modsrv

# 单元测试
cargo test -p modsrv -- --nocapture
```

## 故障排查

### ❗ 常见问题

1. **Redis连接失败**
   ```bash
   # 检查Redis服务
   docker ps | grep redis
   # 测试连接
   redis-cli ping
   ```

2. **映射配置错误**
   ```bash
   # 检查映射文件存在
   ls -la config/mappings/
   # 验证JSON格式
   cat config/mappings/power_meter_demo.json | jq .
   ```

3. **API无响应**
   ```bash
   # 检查端口绑定
   docker port modsrv
   # 测试健康检查
   curl -v http://localhost:8092/health
   ```

### 🔧 性能调优

- **Redis连接池**: 调整`redis.connection_timeout_ms`
- **API并发**: 配置`api.timeout_seconds`
- **内存优化**: 监控`update_interval_ms`设置
- **日志级别**: 生产环境使用`info`级别

## 文档导航

### 📚 详细文档

- **[配置文档](./docs/configuration.md)** - 详细的配置项说明和最佳实践
- **[架构文档](./docs/architecture.md)** - 系统架构设计和数据流详解
- **[数据结构文档](./docs/data-structures.md)** - 数据模型和Redis格式规范
- **[部署文档](./docs/deployment.md)** - Docker部署和运维指南
- **[API测试文档](./docs/api-testing.md)** - API接口测试和示例

### 📋 修复日志

- **[修复日志 2025-07-25](./docs/fixlog/fixlog_2025-07-25.md)** - v2.0重构和测试环境完善记录

### 🔗 相关项目

- **[VoltageEMS总体架构](../../README.md)** - 整个系统的架构说明
- **[ComsRv通信服务](../comsrv/README.md)** - 工业协议通信服务
- **[Redis数据规范](../../docs/redis-spec-v3.2.md)** - Redis数据结构规范

## 版本历史

- **v2.0.0** (2025-07-25) - 架构简化，添加WebSocket支持，Docker化完整测试环境
- **v1.x.x** - 初始版本，四分类模型架构

## 许可证

本项目基于 MIT 许可证开源，详见 [LICENSE](../../LICENSE) 文件。

---

**ModSrv v2.0** - 为工业物联网而生的高性能设备模型服务 🏭⚡
