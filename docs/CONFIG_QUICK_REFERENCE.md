# VoltageEMS 配置快速参考

## 🚀 快速开始

### 1. 基本配置加载
```rust
use voltage_config::prelude::*;

// 最简单的配置加载
let config = load_config().await?;

// 自定义配置加载
let config = ConfigLoaderBuilder::new()
    .add_file("config/myservice.yml")
    .add_sqlite("sqlite:data/config.db", "myservice")
    .add_env_prefix("MYSERVICE_")
    .build()?
    .load::<MyServiceConfig>()?;
```

### 2. 定义配置结构
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyServiceConfig {
    #[serde(flatten)]
    pub base: BaseServiceConfig,
    pub api: ApiConfig,
}

impl Configurable for MyServiceConfig {
    fn validate(&self) -> voltage_config::Result<()> {
        self.base.validate()?;
        // 自定义验证
        Ok(())
    }
}
```

## 📋 配置优先级（高→低）

1. **命令行参数**
2. **环境变量** (`SERVICE_` 前缀)
3. **SQLite 数据库** (`data/config.db`)
4. **配置文件** (`config/service.yml`)
5. **默认值**

## 🗂️ 文件结构

```
VoltageEMS/
├── config/
│   ├── default.yml          # 全局默认配置
│   ├── development.yml      # 开发环境配置
│   ├── production.yml       # 生产环境配置
│   └── {service}.yml        # 服务配置文件
├── data/
│   └── config.db           # SQLite 配置数据库
└── logs/                   # 日志文件目录
```

## 🔧 环境变量规则

```bash
# 格式：SERVICE_SECTION__KEY
export ALARMSRV_REDIS__HOST="localhost"
export ALARMSRV_REDIS__PORT="6379"
export ALARMSRV_API__PORT="8094"
export ALARMSRV_LOGGING__LEVEL="debug"

# 嵌套配置使用双下划线
export COMSRV_CHANNELS__0__NAME="ModbusChannel"
```

## 📝 基础配置模板

```yaml
# 所有服务共享的基础配置
service:
  name: "myservice"
  version: "1.0.0"
  description: "My Service"

redis:
  host: "localhost"
  port: 6379
  password: ~
  database: 0
  pool_size: 10

logging:
  level: "info"              # trace/debug/info/warn/error
  format: "json"             # json/pretty/compact
  enable_file: false
  file_path: "logs/service.log"

monitoring:
  enabled: true
  metrics_path: "/metrics"
  health_path: "/health"
```

## 🏷️ 各服务默认端口

| 服务 | 默认端口 | 环境变量前缀 |
|------|---------|-------------|
| comsrv | 8091 | COMSRV_ |
| modsrv | 8092 | MODSRV_ |
| hissrv | 8093 | HISSRV_ |
| alarmsrv | 8094 | ALARMSRV_ |
| netsrv | 8095 | NETSRV_ |
| apigateway | 8080 | APIGATEWAY_ |

## 💾 SQLite 配置操作

### 查看配置
```sql
-- 查看服务配置
SELECT * FROM configs WHERE service = 'comsrv' AND is_active = 1;

-- 查看配置历史
SELECT * FROM config_history WHERE service = 'comsrv' ORDER BY changed_at DESC;

-- 查看点表
SELECT * FROM v_point_full WHERE channel_id = 1001;
```

### 更新配置
```sql
-- 更新配置值
INSERT INTO configs (service, key, value, type) 
VALUES ('comsrv', 'api.port', '9091', 'number')
ON CONFLICT(service, key) DO UPDATE SET value = excluded.value;

-- 禁用配置
UPDATE configs SET is_active = 0 WHERE service = 'comsrv' AND key = 'old.setting';
```

## 🛠️ 常用命令

### 配置迁移
```bash
# 生成默认配置文件
cargo run --bin migrate_config

# 验证配置
cargo run --bin validate_config -- --config config/myservice.yml

# 导出环境变量模板
cargo run --bin export_env -- --service myservice > .env.example
```

### Docker 运行
```bash
# 使用配置文件
docker run -v $(pwd)/config:/app/config voltage/myservice

# 使用环境变量
docker run -e MYSERVICE_REDIS__HOST=redis voltage/myservice

# 使用 .env 文件
docker run --env-file .env voltage/myservice
```

## 🔍 调试技巧

### 1. 查看加载的配置
```rust
let config = load_config().await?;
info!("Loaded config: {:#?}", config);
```

### 2. 启用配置加载日志
```bash
RUST_LOG=voltage_config=debug cargo run
```

### 3. 测试配置验证
```rust
match config.validate() {
    Ok(_) => println!("✓ Config valid"),
    Err(e) => println!("✗ Config error: {}", e),
}
```

### 4. 打印最终配置
```bash
# 添加到 main.rs
println!("{}", serde_yaml::to_string(&config)?);
```

## ⚠️ 常见错误

| 错误 | 原因 | 解决方案 |
|------|------|---------|
| `Config file not found` | 文件路径错误 | 检查工作目录和文件路径 |
| `Validation failed` | 配置值无效 | 检查配置值是否符合验证规则 |
| `SQLite connection failed` | 数据库不存在 | 创建数据目录并初始化数据库 |
| `Env var not working` | 命名错误 | 使用双下划线分隔嵌套字段 |

## 📚 更多资源

- [完整配置指南](./CONFIGURATION_GUIDE.md)
- [配置框架文档](./CONFIG_FRAMEWORK.md)
- [迁移指南](./CONFIG_MIGRATION.md)

---
*快速参考 v1.0 - 2025-07-03*