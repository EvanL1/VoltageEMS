# ModSrv - 轻量级模型服务

ModSrv是VoltageEMS系统的模型服务，为边端设备提供轻量级的设备模型管理和数据访问接口。

## 特性

- 🚀 **轻量级设计** - 内存占用小于50MB，适合边端设备
- ⚡ **高性能同步** - 使用Lua脚本实现零延迟数据同步
- 🔌 **简单部署** - 仅依赖Redis，无需复杂配置
- 📡 **实时推送** - WebSocket支持实时数据推送
- 🛡️ **可靠稳定** - 生产环境验证，支持7x24小时运行

## 快速开始

### 1. 环境要求

- Redis 6.0+
- Rust 1.70+（编译时需要）

### 2. 运行服务

```bash
# 使用Docker
docker run -d \
  --name modsrv \
  -p 8002:8002 \
  -e REDIS_URL=redis://localhost:6379 \
  voltage/modsrv

# 或直接运行
./modsrv
```

### 3. 配置模型

创建配置文件 `config/default.yml`:

```yaml
service_name: "modsrv"
version: "2.0.0"

redis:
  url: "redis://localhost:6379"

api:
  host: "0.0.0.0"
  port: 8002

models:
  - id: "meter_001"
    name: "智能电表"
    description: "1号配电室电表"
    monitoring:
      voltage:
        description: "电压"
        unit: "V"
      current:
        description: "电流"
        unit: "A"
      power:
        description: "功率"
        unit: "kW"
    control:
      switch:
        description: "开关控制"
```

### 4. 创建映射

创建映射文件 `config/mappings/meter_001.json`:

```json
{
  "monitoring": {
    "voltage": {
      "channel": 1001,
      "point": 10001,
      "type": "m"
    },
    "current": {
      "channel": 1001,
      "point": 10002,
      "type": "m"
    },
    "power": {
      "channel": 1001,
      "point": 10003,
      "type": "m"
    }
  },
  "control": {
    "switch": {
      "channel": 1001,
      "point": 30001,
      "type": "c"
    }
  }
}
```

## API使用

### 获取模型列表

```bash
curl http://localhost:8002/models
```

### 获取实时数据

```bash
curl http://localhost:8002/models/meter_001/values
```

### 发送控制命令

```bash
curl -X POST http://localhost:8002/models/meter_001/control/switch \
  -H "Content-Type: application/json" \
  -d '{"value": 1}'
```

### WebSocket订阅

```javascript
const ws = new WebSocket('ws://localhost:8002/ws/meter_001');

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('实时数据:', data);
};
```

## 命令行工具

```bash
# 运行服务（默认）
modsrv

# 检查配置和环境
modsrv check

# 指定配置文件
modsrv -c /path/to/config.yml

# 查看帮助
modsrv --help
```

## 架构说明

ModSrv采用轻量级架构设计：

1. **无内存缓存** - 直接从Redis读取，减少内存占用
2. **Lua脚本同步** - 在Redis层面实现数据同步，延迟小于1ms
3. **简化API** - 提供最必要的接口，降低复杂度

详细架构请参考 [架构文档](docs/architecture.md)

## 配置说明

- [配置指南](docs/configuration-guide.md) - 详细的配置说明
- [API文档](docs/api-migration-guide.md) - API接口文档

## 编译构建

```bash
# 编译发布版本
cargo build --release

# 运行测试
cargo test

# 代码检查
cargo clippy
cargo fmt
```

## Docker部署

```yaml
version: '3.8'
services:
  redis:
    image: redis:7-alpine
    
  modsrv:
    image: voltage/modsrv:latest
    ports:
      - "8002:8002"
    environment:
      - REDIS_URL=redis://redis:6379
      - RUST_LOG=info
    depends_on:
      - redis
```

## 性能指标

- 启动时间：< 1秒
- 内存占用：< 50MB
- API延迟：< 10ms
- 数据同步延迟：< 1ms
- WebSocket并发：> 1000连接

## 故障排查

### 检查服务状态

```bash
curl http://localhost:8002/health
```

### 查看日志

```bash
tail -f logs/modsrv.log
```

### Redis调试

```bash
# 监控Redis操作
redis-cli monitor | grep modsrv

# 查看数据
redis-cli hgetall modsrv:meter_001:measurement
```

## 许可证

Copyright (c) 2024 VoltageEMS Team. All rights reserved.