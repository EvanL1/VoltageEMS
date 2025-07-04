# VoltageEMS 更新日志

## [Unreleased]

### 2025-01-04

#### Added - 配置中心架构支持

- **新架构**：引入配置中心架构，支持微服务配置的集中管理
  - 支持多源配置加载：本地文件、配置中心、环境变量
  - 配置加载优先级：默认值 → 文件 → 配置中心 → 环境变量
  - 异步配置加载，支持超时控制

- **ModSrv 服务改造**：
  - 重构 `config.rs` 模块，添加 `ConfigLoader` 支持
  - 更新 `main.rs` 为异步入口，支持配置中心
  - 添加环境变量覆盖机制（`MODSRV_` 前缀）
  - 保持向后兼容，支持旧配置格式

- **文档**：
  - 创建 `docs/CONFIG_CENTER_ARCHITECTURE.md` - 配置中心架构说明
  - 创建 `docs/CONFIG_CENTER_SERVICE_DESIGN.md` - 配置中心服务设计
  - 更新 `CLAUDE.md` 添加配置管理说明
  - 添加配置文件示例 `config/modsrv.yaml` 和 `config/modsrv.example.yaml`

#### Changed

- **依赖更新**：
  - ModSrv 添加 `reqwest` 和 `dotenv` 依赖
  - 移除对 `voltage-config` 框架的依赖，保持微服务独立性

- **配置结构**：
  - 统一服务配置结构，添加 `service` 元数据部分
  - Redis 配置支持 URL 格式（推荐）和旧的 host/port 格式
  - 添加更多配置字段的默认值

#### Technical Details

**环境变量支持**：
- `CONFIG_CENTER_URL` - 配置中心地址
- `{SERVICE}_CONFIG_FILE` - 配置文件路径
- `{SERVICE}_REDIS_URL` - Redis 连接 URL
- `{SERVICE}_API_HOST` - API 监听地址
- `{SERVICE}_API_PORT` - API 监听端口
- `{SERVICE}_LOG_LEVEL` - 日志级别

**配置加载示例**：
```bash
# 使用本地配置
cargo run --bin modsrv

# 使用配置中心
export CONFIG_CENTER_URL=http://config-center:8080
cargo run --bin modsrv

# 环境变量覆盖
export MODSRV_REDIS_URL=redis://production:6379
export MODSRV_LOG_LEVEL=debug
cargo run --bin modsrv
```

---

## [Previous Releases]

### 2024-12-XX

- 初始版本发布
- 基础微服务架构实现
- Modbus、CAN、IEC60870 协议支持
- Redis 实时数据总线
- InfluxDB 历史数据存储
- Vue.js 前端界面