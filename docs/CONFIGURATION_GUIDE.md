# VoltageEMS 配置指南

## 目录
1. [概述](#概述)
2. [配置框架 (configframework)](#配置框架-configframework)
3. [配置加载机制](#配置加载机制)
4. [服务配置详解](#服务配置详解)
5. [SQLite 配置存储](#sqlite-配置存储)
6. [环境变量配置](#环境变量配置)
7. [配置迁移指南](#配置迁移指南)
8. [最佳实践](#最佳实践)
9. [故障排查](#故障排查)

## 概述

VoltageEMS 采用统一的配置框架 `configframework`，基于 Figment 构建，支持多源配置管理。所有服务都使用相同的配置加载机制，确保一致性和可维护性。

### 核心特性
- 🔧 **多源配置**：支持文件、数据库、环境变量等多种配置源
- 📁 **多格式支持**：YAML、TOML、JSON、SQLite
- ✅ **配置验证**：内置验证机制，确保配置正确性
- 🔄 **热更新支持**：配置变更无需重启服务（部分服务）
- 🏗️ **分层架构**：基础配置 + 服务特定配置
- 🔒 **类型安全**：Rust 类型系统保证配置安全

## 配置框架 (configframework)

### 安装使用

在 `Cargo.toml` 中添加依赖：
```toml
[dependencies]
voltage-config = { path = "../config-framework" }
```

### 基本用法

```rust
use voltage_config::prelude::*;

// 定义服务配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyServiceConfig {
    #[serde(flatten)]
    pub base: BaseServiceConfig,  // 继承基础配置
    pub api: ApiConfig,           // 服务特定配置
}

// 实现配置验证
impl Configurable for MyServiceConfig {
    fn validate(&self) -> voltage_config::Result<()> {
        // 验证逻辑
        self.base.validate()?;
        Ok(())
    }
}

// 加载配置
let config = ConfigLoaderBuilder::new()
    .add_file("config/myservice.yml")
    .add_sqlite("sqlite:data/config.db", "myservice")
    .add_env_prefix("MYSERVICE_")
    .build()?
    .load::<MyServiceConfig>()?;
```

## 配置加载机制

### 加载优先级（从高到低）

1. **命令行参数** - 最高优先级，用于临时覆盖
2. **环境变量** - 适合容器化部署
3. **SQLite 数据库** - 动态配置，支持运行时修改
4. **配置文件** - 静态配置基础
5. **默认值** - 代码中定义的默认配置

### 配置文件位置

```
VoltageEMS/
├── config/                    # 全局配置目录
│   ├── default.yml           # 默认配置
│   ├── development.yml       # 开发环境
│   ├── production.yml        # 生产环境
│   ├── comsrv.yml           # 服务特定配置
│   ├── modsrv.yml
│   ├── hissrv.yml
│   ├── netsrv.yml
│   ├── alarmsrv.yml
│   └── apigateway.yml
├── data/
│   └── config.db            # SQLite 配置数据库
└── services/
    └── {service}/
        └── config/          # 服务本地配置（可选）
```

## 服务配置详解

### 基础配置结构 (BaseServiceConfig)

所有服务都继承的基础配置：

```yaml
# 服务信息
service:
  name: "service-name"
  version: "1.0.0"
  description: "Service description"

# Redis 配置
redis:
  host: "localhost"
  port: 6379
  password: ~                  # 可选
  database: 0
  pool_size: 10
  connection_timeout: 5        # 秒
  command_timeout: 5          # 秒

# 日志配置
logging:
  level: "info"               # trace/debug/info/warn/error
  format: "json"              # json/pretty/compact
  enable_ansi: false
  enable_file: false
  file_path: "logs/service.log"
  file_max_size: 10485760     # 10MB
  file_max_age: 7             # 天
  file_max_backups: 5

# 监控配置
monitoring:
  enabled: true
  metrics_path: "/metrics"
  health_path: "/health"
  prometheus_enabled: true
```

### 各服务特定配置

#### 1. comsrv（通信服务）

```yaml
# API 配置
api:
  host: "0.0.0.0"
  port: 8091
  prefix: "/api/v1"

# 默认路径配置
default_paths:
  config_dir: "config"
  point_table_dir: "config/point_tables"

# 通道配置
channels:
  - id: 1001
    name: "ModbusTCP_Channel"
    enabled: true
    transport:
      type: "tcp"
      config:
        host: "192.168.1.100"
        port: 502
        timeout: "10s"
    protocol:
      type: "modbus_tcp"
    # CSV 表格配置（可选）
    table_config:
      use_convention: true    # 使用约定路径

# 协议特定设置
protocols:
  modbus:
    default_timeout: 1000
    max_retries: 3
    inter_frame_delay: 10
```

#### 2. modsrv（模型服务）

```yaml
# API 配置
api:
  host: "0.0.0.0"
  port: 8092

# 模型执行配置
model:
  execution_interval_ms: 1000
  max_concurrent_models: 10
  timeout_ms: 5000

# 控制操作配置
control:
  operation_timeout_ms: 5000
  max_retries: 3
  retry_delay_ms: 1000

# 存储模式
storage_mode: "hybrid"        # memory/redis/hybrid
templates_dir: "templates"
sync_interval_secs: 60
```

#### 3. hissrv（历史数据服务）

```yaml
# API 配置
api:
  host: "0.0.0.0"
  port: 8093
  cors:
    enabled: true
    allowed_origins: ["*"]

# Redis 订阅配置
redis:
  subscribe_patterns: 
    - "telemetry:*"
    - "event:*"
  scan_batch_size: 1000

# 存储后端配置
storage:
  backend: "influxdb"         # influxdb/postgresql/mongodb
  influxdb:
    url: "http://localhost:8086"
    token: "your-token"
    org: "voltage"
    bucket: "voltage_data"

# 数据处理配置
data:
  batch_size: 1000
  flush_interval_secs: 10
  filters:
    - type: "value_range"
      min: -1000
      max: 10000
```

#### 4. netsrv（网络服务）

```yaml
# 网络配置列表
networks:
  - id: "aws_iot_1"
    name: "AWS IoT Core"
    network_type: "aws_iot"
    enabled: true
    connection:
      endpoint: "xxx.iot.region.amazonaws.com"
      client_id: "voltage_device_001"
      auth:
        type: "certificate"
        cert_path: "certs/device.pem.crt"
        key_path: "certs/device.pem.key"
        ca_path: "certs/root-CA.crt"
    topics:
      telemetry: "voltage/telemetry/${device_id}"
      command: "voltage/command/${device_id}"
```

#### 5. alarmsrv（告警服务）

```yaml
# API 配置
api:
  host: "0.0.0.0"
  port: 8094

# 存储配置
storage:
  retention_days: 30
  auto_cleanup: true
  cleanup_interval_hours: 24

# 告警分类配置（从 SQLite 加载）
# classification:
#   critical_threshold: 0.8
#   warning_threshold: 0.5
```

#### 6. apigateway（API 网关）

```yaml
# 服务器配置
server:
  host: "0.0.0.0"
  port: 8080
  workers: 4

# 服务端点配置
services:
  comsrv_url: "http://localhost:8091"
  modsrv_url: "http://localhost:8092"
  hissrv_url: "http://localhost:8093"
  netsrv_url: "http://localhost:8094"
  alarmsrv_url: "http://localhost:8095"

# CORS 配置
cors:
  allowed_origins: ["http://localhost:3000", "https://app.voltage.com"]
  allowed_methods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"]
  allowed_headers: ["Content-Type", "Authorization"]
  max_age: 3600
```

## SQLite 配置存储

### 数据库结构

```sql
-- 配置主表
configs (
  id, service, key, value, type, version, 
  created_at, updated_at, is_active
)

-- 配置历史
config_history (
  id, config_id, service, key, old_value, 
  new_value, operation, changed_at
)

-- 点表数据
point_tables (
  id, channel_id, point_id, point_name, 
  point_type, data_type, unit, scale, ...
)

-- 协议映射
protocol_mappings (
  id, channel_id, point_id, protocol, 
  address, params, ...
)
```

### 使用 SQLite 存储

```rust
// 加载点表数据
let provider = SqliteProvider::new("sqlite:data/config.db", "comsrv").await?;
let points = provider.load_point_tables(channel_id).await?;

// 保存配置
provider.save_config("api.port", "8091", "number").await?;

// 删除配置
provider.delete_config("deprecated.setting").await?;
```

### 配置导入导出

```bash
# 从 CSV 导入到 SQLite
configtool import --from csv --to sqlite \
  --csv-dir config/ModbusTCP_Channel \
  --db data/config.db

# 从 SQLite 导出到 YAML
configtool export --from sqlite --to yaml \
  --service comsrv \
  --output config/comsrv_export.yml
```

## 环境变量配置

### 命名规则

环境变量使用服务名作为前缀，嵌套字段使用双下划线分隔：

```bash
# 基础配置
export COMSRV_SERVICE__NAME="comsrv"
export COMSRV_REDIS__HOST="redis.example.com"
export COMSRV_REDIS__PORT="6380"
export COMSRV_REDIS__PASSWORD="secret"

# 日志配置
export COMSRV_LOGGING__LEVEL="debug"
export COMSRV_LOGGING__ENABLE_FILE="true"
export COMSRV_LOGGING__FILE_PATH="/var/log/comsrv.log"

# API 配置
export COMSRV_API__HOST="0.0.0.0"
export COMSRV_API__PORT="9091"
```

### Docker Compose 示例

```yaml
version: '3.8'
services:
  comsrv:
    image: voltage/comsrv:latest
    environment:
      - COMSRV_REDIS__HOST=redis
      - COMSRV_REDIS__PORT=6379
      - COMSRV_API__PORT=8091
      - COMSRV_LOGGING__LEVEL=info
    volumes:
      - ./config:/app/config
      - ./data:/app/data
```

## 配置迁移指南

### 从旧版本迁移

1. **安装迁移工具**
   ```bash
   cd services/{service_name}
   cargo build --bin migrate_config
   ```

2. **运行迁移**
   ```bash
   ./target/debug/migrate_config
   ```

3. **验证配置**
   ```bash
   # 生成的配置文件位于 config/{service}.yml
   cat config/{service}.yml
   ```

4. **更新代码**
   ```rust
   // 旧代码
   use config::Config;
   let config = Config::from_file("config.toml")?;
   
   // 新代码
   use voltage_config::prelude::*;
   let config = load_config().await?;
   ```

### 迁移检查清单

- [ ] 备份现有配置
- [ ] 运行迁移工具
- [ ] 验证生成的配置文件
- [ ] 更新环境变量（如果使用）
- [ ] 测试服务启动
- [ ] 验证功能正常

## 最佳实践

### 1. 配置组织

```yaml
# ❌ 不推荐：扁平化配置
redis_host: "localhost"
redis_port: 6379
api_host: "0.0.0.0"
api_port: 8080

# ✅ 推荐：分组配置
redis:
  host: "localhost"
  port: 6379
  
api:
  host: "0.0.0.0"
  port: 8080
```

### 2. 敏感信息管理

```yaml
# ❌ 不推荐：硬编码密码
redis:
  password: "my-secret-password"

# ✅ 推荐：使用环境变量
redis:
  password: ~  # 通过环境变量 MYSERVICE_REDIS__PASSWORD 设置
```

### 3. 配置验证

```rust
impl Configurable for MyConfig {
    fn validate(&self) -> Result<()> {
        // 必填字段检查
        if self.api.port == 0 {
            return Err(ConfigError::Validation("API port cannot be 0".into()));
        }
        
        // 范围检查
        if self.timeout < 1 || self.timeout > 3600 {
            return Err(ConfigError::Validation("Timeout must be 1-3600 seconds".into()));
        }
        
        // 关联性检查
        if self.enable_tls && self.cert_path.is_none() {
            return Err(ConfigError::Validation("TLS enabled but cert_path not provided".into()));
        }
        
        Ok(())
    }
}
```

### 4. 环境特定配置

```rust
// 根据环境加载不同配置
let env = Environment::from_env(); // 从 VOLTAGE_ENV 环境变量读取

let config = ConfigLoaderBuilder::new()
    .environment(env)  // 自动加载 config/{env}.yml
    .add_file("config/service.yml")
    .build()?
    .load()?;
```

### 5. 配置文档化

```yaml
# 服务配置文件
# 本文件定义了服务的所有可配置项
# 可通过环境变量覆盖，使用前缀 MYSERVICE_

# Redis 连接配置
redis:
  host: "localhost"        # Redis 服务器地址
  port: 6379              # Redis 端口号
  password: ~             # Redis 密码（可选）
  database: 0             # 数据库索引 (0-15)
  pool_size: 10           # 连接池大小
  
# API 服务配置
api:
  host: "0.0.0.0"         # 监听地址，0.0.0.0 表示所有接口
  port: 8080              # 监听端口
  workers: 4              # 工作线程数，0 表示使用 CPU 核心数
```

## 故障排查

### 常见问题

#### 1. 配置文件找不到

**错误信息**：
```
Configuration file not found: config/myservice.yml
```

**解决方案**：
- 确认工作目录正确：`pwd`
- 创建配置文件：`touch config/myservice.yml`
- 使用绝对路径：`.add_file("/absolute/path/to/config.yml")`

#### 2. 环境变量不生效

**问题**：设置了环境变量但配置没有改变

**检查步骤**：
```bash
# 确认环境变量已设置
echo $MYSERVICE_REDIS__HOST

# 检查前缀是否正确（注意双下划线）
env | grep MYSERVICE_

# 启用调试日志查看配置加载过程
RUST_LOG=debug cargo run
```

#### 3. SQLite 连接失败

**错误信息**：
```
Failed to create SQLite provider: unable to open database file
```

**解决方案**：
```bash
# 创建数据目录
mkdir -p data

# 初始化数据库
sqlite3 data/config.db < services/config-framework/schema/sqlite_schema.sql

# 检查权限
chmod 644 data/config.db
```

#### 4. 配置验证失败

**错误信息**：
```
Configuration validation failed: API port cannot be 0
```

**调试方法**：
```rust
// 打印加载的配置
let config = load_config().await?;
println!("Loaded config: {:#?}", config);

// 单独测试验证
match config.validate() {
    Ok(_) => println!("Config is valid"),
    Err(e) => println!("Validation error: {}", e),
}
```

### 调试技巧

1. **启用详细日志**
   ```bash
   RUST_LOG=voltage_config=debug cargo run
   ```

2. **打印最终配置**
   ```rust
   let config = load_config().await?;
   println!("{}", serde_yaml::to_string(&config)?);
   ```

3. **测试配置加载**
   ```bash
   # 创建测试配置
   cat > test_config.yml << EOF
   service:
     name: "test"
   redis:
     host: "test-host"
   EOF
   
   # 测试加载
   cargo test config_loading
   ```

4. **检查配置优先级**
   ```rust
   // 逐层测试配置源
   let builder = ConfigLoaderBuilder::new();
   
   // 只加载文件
   let file_config = builder.clone()
       .add_file("config.yml")
       .build()?.load()?;
   
   // 加载文件 + 环境变量
   let env_config = builder.clone()
       .add_file("config.yml")
       .add_env_prefix("MYSERVICE_")
       .build()?.load()?;
   ```

## 附录

### 配置模板生成

```bash
# 为新服务生成配置模板
configtool generate --service myservice --output config/myservice.yml
```

### 配置校验工具

```bash
# 验证配置文件格式
configtool validate --file config/myservice.yml --schema MyServiceConfig

# 检查所有服务配置
configtool check-all --config-dir config/
```

### 性能优化建议

1. **缓存配置对象**：避免重复加载
2. **使用 SQLite 索引**：为常用查询创建索引
3. **批量加载**：一次性加载所有需要的配置
4. **异步加载**：使用 `load_async()` 避免阻塞

### 相关资源

- [Figment 文档](https://docs.rs/figment)
- [SQLx 文档](https://docs.rs/sqlx)
- [环境变量最佳实践](https://12factor.net/config)
- [YAML 规范](https://yaml.org/spec/)

---

*本文档持续更新中，最后更新：2025-07-03*