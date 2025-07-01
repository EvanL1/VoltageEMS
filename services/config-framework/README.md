# Voltage 配置框架

VoltageEMS 微服务的统一配置管理框架，基于 Figment 构建。

## 功能特性

- **多格式支持**：支持 YAML、TOML、JSON 和环境变量
- **分层配置**：基础配置 + 环境特定配置覆盖
- **类型安全**：利用 Rust 类型系统和 serde
- **验证系统**：内置验证规则和自定义验证器
- **热重载**：监视配置文件变化并自动重载
- **异步支持**：支持同步和异步配置加载
- **可扩展**：自定义验证器和配置源

## 快速开始

### 添加依赖

在你的服务 `Cargo.toml` 中添加：

```toml
[dependencies]
voltage-config = { path = "../config-framework" }
```

### 基本使用

```rust
use serde::{Deserialize, Serialize};
use voltage_config::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
struct MyConfig {
    server: ServerConfig,
    database: DatabaseConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerConfig {
    host: String,
    port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct DatabaseConfig {
    url: String,
}

impl Configurable for MyConfig {
    fn validate(&self) -> Result<()> {
        // 自定义验证逻辑
        if self.server.port == 0 {
            return Err(ConfigError::Validation("端口不能为0".into()));
        }
        Ok(())
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let loader = ConfigLoaderBuilder::new()
        .base_path("config")
        .add_file("app.yml")
        .environment(Environment::from_env())
        .env_prefix("MYAPP")
        .build()?;
    
    let config: MyConfig = loader.load()?;
    println!("服务器: {}:{}", config.server.host, config.server.port);
    
    Ok(())
}
```

## 配置加载顺序

配置按以下顺序加载，后面的会覆盖前面的：

1. 默认值（如果提供）
2. `config/default.yml`
3. `config/{environment}.yml`（如 development.yml、production.yml）
4. 额外指定的配置文件（按添加顺序）
5. 环境变量

## 环境变量映射

环境变量会自动映射到配置键：
- `MYAPP_SERVER_HOST` → `server.host`
- `MYAPP_DATABASE_URL` → `database.url`
- `MYAPP_SERVER_PORT` → `server.port`

## 配置文件监视

```rust
// 创建配置监视器
let watcher = ConfigWatcher::new(loader, vec!["config".into()])
    .with_interval(Duration::from_secs(5));

// 启动监视
watcher.start().await?;

// 等待配置变化
while let Some(event) = watcher.wait_for_change().await {
    match event {
        WatchEvent::Modified(path) => {
            println!("配置文件已修改: {}", path.display());
            let new_config = watcher.reload::<MyConfig>().await?;
            // 处理新配置
        }
        _ => {}
    }
}
```

## 验证规则

### 内置验证规则

```rust
use voltage_config::prelude::*;

let loader = ConfigLoaderBuilder::new()
    // 正则表达式验证
    .add_validation_rule(
        "email",
        Box::new(RegexRule::new(
            "email_format",
            r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$",
            "user.email",
        )?),
    )
    // 范围验证
    .add_validation_rule(
        "port",
        Box::new(RangeRule::new(
            "port_range",
            Some(1024),
            Some(65535),
            "server.port",
        )),
    )
    .build()?;
```

### 自定义验证器

```rust
struct MyValidator;

#[async_trait::async_trait]
impl ConfigValidator for MyValidator {
    async fn validate(&self, config: &(dyn Any + Send + Sync)) -> Result<()> {
        if let Some(my_config) = config.downcast_ref::<MyConfig>() {
            // 自定义验证逻辑
            if !my_config.database.url.starts_with("postgres://") {
                return Err(ConfigError::Validation(
                    "数据库URL必须以postgres://开头".into()
                ));
            }
        }
        Ok(())
    }
    
    fn name(&self) -> &str {
        "MyValidator"
    }
}

let loader = ConfigLoaderBuilder::new()
    .add_validator(Box::new(MyValidator))
    .build()?;
```

## VoltageEMS 服务集成

### 服务配置示例

```rust
#[derive(Debug, Serialize, Deserialize)]
struct ServiceConfig {
    service: ServiceInfo,
    redis: RedisConfig,
    channels: Vec<ChannelConfig>,
    telemetry: TelemetryConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServiceInfo {
    name: String,
    version: String,
    description: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RedisConfig {
    url: String,
    prefix: String,
    pool_size: u32,
}

// ... 其他配置结构体

impl Configurable for ServiceConfig {
    fn validate(&self) -> Result<()> {
        // 服务特定的验证逻辑
        Ok(())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}
```

### 配置文件结构

```
config/
├── default.yml           # 默认配置
├── development.yml       # 开发环境配置
├── production.yml        # 生产环境配置
└── channels/            # 通道特定配置
    ├── modbus.yml
    └── iec104.yml
```

### 示例配置文件

`config/default.yml`:
```yaml
service:
  name: comsrv
  version: 1.0.0
  description: 工业通信服务

redis:
  url: redis://localhost:6379
  prefix: "voltage:"
  pool_size: 10

telemetry:
  metrics_enabled: true
  metrics_port: 9090
  log_level: info
```

`config/production.yml`:
```yaml
redis:
  url: redis://redis-cluster:6379
  pool_size: 50

telemetry:
  log_level: warn
```

## 最佳实践

1. **使用环境变量进行敏感配置**：密码、API密钥等敏感信息应通过环境变量配置
2. **验证所有配置**：在 `validate()` 方法中实现完整的配置验证
3. **提供合理的默认值**：为可选配置提供合理的默认值
4. **使用类型安全的配置**：利用 Rust 类型系统确保配置正确性
5. **监视配置变化**：在生产环境中启用配置监视以支持动态更新

## API 文档

主要类型和函数的详细文档请参考源代码注释。

## 示例

- `examples/basic_usage.rs` - 基本使用示例
- `examples/service_config.rs` - 服务配置示例
- `examples/comsrv_integration.rs` - comsrv 集成示例

## 许可证

MIT OR Apache-2.0