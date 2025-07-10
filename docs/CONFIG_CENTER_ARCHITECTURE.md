# VoltageEMS 配置中心架构说明

## 概述

VoltageEMS 采用配置中心架构来管理微服务配置，实现了配置的集中管理、动态更新和多环境支持。每个微服务保持独立性的同时，支持从多个来源加载配置。

## 架构设计

### 1. 配置加载优先级

配置按以下优先级加载（后者覆盖前者）：

1. **默认配置** - 代码中的默认值
2. **本地配置文件** - YAML/JSON 格式
3. **配置中心** - HTTP API 获取
4. **环境变量** - 运行时覆盖

```
┌─────────────────┐
│   默认配置       │ (最低优先级)
└────────┬────────┘
         │
┌────────▼────────┐
│  本地配置文件     │ 
└────────┬────────┘
         │
┌────────▼────────┐
│   配置中心       │
└────────┬────────┘
         │
┌────────▼────────┐
│   环境变量       │ (最高优先级)
└─────────────────┘
```

### 2. 微服务配置结构

每个微服务的配置包含以下标准部分：

```yaml
# 服务元信息
service:
  name: "service-name"
  version: "1.0.0"
  description: "Service description"
  instance_id: "unique-instance-id"

# Redis 配置
redis:
  url: "redis://localhost:6379"
  key_prefix: "voltage:service:"
  pool_size: 10

# 日志配置
logging:
  level: "info"
  console: true
  file: "logs/service.log"

# API 配置
api:
  host: "0.0.0.0"
  port: 8080

# 监控配置
monitoring:
  enabled: true
  metrics_port: 9090

# 服务特定配置
# ...
```

### 3. 配置中心 API 设计

配置中心提供 RESTful API 来管理配置：

#### 获取服务配置

```
GET /api/v1/config/{service_name}
```

响应示例：

```json
{
  "service": {
    "name": "modsrv",
    "version": "1.0.0"
  },
  "redis": {
    "url": "redis://redis.production:6379"
  },
  // ... 其他配置
}
```

#### 更新服务配置

```
PUT /api/v1/config/{service_name}
Content-Type: application/json

{
  // 完整配置内容
}
```

#### 获取所有服务配置列表

```
GET /api/v1/config
```

## 实现方式

### 1. 服务端实现

每个微服务通过 `ConfigLoader` 加载配置：

```rust
// 创建配置加载器
let loader = ConfigLoader::new()
    .with_file("config/service.yaml")
    .with_config_center(env::var("CONFIG_CENTER_URL").ok())
    .with_env_prefix("SERVICE_");

// 加载配置
let config = loader.load().await?;
```

### 2. 环境变量命名规范

环境变量使用服务前缀，支持嵌套结构：

- `{SERVICE}_REDIS_URL` → `redis.url`
- `{SERVICE}_API_PORT` → `api.port`
- `{SERVICE}_LOG_LEVEL` → `logging.level`

示例：

```bash
export MODSRV_REDIS_URL=redis://production:6379
export MODSRV_API_PORT=8092
export MODSRV_LOG_LEVEL=debug
```

### 3. 配置文件位置

配置文件查找顺序：

1. 命令行指定：`--config /path/to/config.yaml`
2. 环境变量指定：`{SERVICE}_CONFIG_FILE`
3. 默认位置：`config/{service}.yaml`

## 部署模式

### 1. 开发环境

使用本地配置文件：

```bash
# 使用默认配置
cargo run --bin modsrv

# 指定配置文件
cargo run --bin modsrv -- --config config/modsrv.dev.yaml
```

### 2. 测试环境

结合配置中心和环境变量：

```bash
export CONFIG_CENTER_URL=http://config-center.test:8080
export MODSRV_LOG_LEVEL=debug
cargo run --bin modsrv
```

### 3. 生产环境

使用配置中心管理，敏感信息通过环境变量注入：

```bash
# Docker Compose 示例
services:
  modsrv:
    image: voltageems/modsrv:latest
    environment:
      - CONFIG_CENTER_URL=http://config-center:8080
      - MODSRV_REDIS_PASSWORD=${REDIS_PASSWORD}
      - MODSRV_API_KEY=${API_KEY}
```

## 配置中心服务设计

### 1. 存储后端

配置中心可以使用以下存储：

- **文件系统** - 简单部署，适合小规模
- **Redis** - 快速访问，支持订阅通知
- **etcd/Consul** - 分布式一致性，高可用

### 2. 配置版本管理

- 支持配置版本历史
- 支持配置回滚
- 支持配置审计

### 3. 动态更新（未来功能）

- WebSocket/SSE 推送配置更新
- 服务自动重载配置
- 灰度发布支持

## 迁移指南

### 从 config-framework 迁移

1. **移除依赖**

   ```toml
   # 删除
   voltage-config = { path = "../config-framework" }
   ```
2. **更新配置加载代码**

   ```rust
   // 旧代码
   let config = voltage_config::load_config()?;

   // 新代码
   let config = config::load_config().await?;
   ```
3. **更新配置文件结构**

   - 添加 `service` 部分
   - 调整字段名称符合新结构

### 配置文件示例

完整的配置文件示例见 `/config` 目录：

- `modsrv.yaml` - 开发环境配置
- `modsrv.example.yaml` - 生产环境示例

## 最佳实践

1. **配置分离**

   - 敏感信息使用环境变量
   - 通用配置使用配置中心
   - 默认值保证服务可启动
2. **配置验证**

   - 启动时验证必需配置
   - 提供清晰的错误信息
   - 支持配置检查命令
3. **配置文档**

   - 在配置文件中添加注释
   - 记录所有环境变量
   - 提供配置模板
4. **安全考虑**

   - 配置中心使用 HTTPS
   - 敏感配置加密存储
   - 访问控制和审计

## 故障排查

### 常见问题

1. **配置加载失败**

   ```
   Failed to load configuration: ...
   ```

   - 检查配置文件路径
   - 验证 YAML/JSON 语法
   - 确认配置中心可访问
2. **环境变量未生效**

   - 检查变量名前缀
   - 确认变量已导出
   - 查看启动日志
3. **配置中心连接超时**

   - 检查网络连接
   - 验证 CONFIG_CENTER_URL
   - 查看配置中心日志

### 调试模式

启用调试日志查看配置加载过程：

```bash
export RUST_LOG=debug
cargo run --bin modsrv
```

## 路线图

- [ ] 配置热更新支持
- [ ] 配置加密存储
- [ ] 配置模板和继承
- [ ] 多环境配置管理
- [ ] 配置变更通知
- [ ] Web UI 管理界面
