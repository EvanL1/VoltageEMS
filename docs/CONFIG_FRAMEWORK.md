# VoltageEMS 配置框架

## 概述

VoltageEMS 采用微服务架构，各个服务都有独立的配置管理系统。本文档总结了所有服务的配置方式、共性特点和差异。

## 服务配置总览

| 服务 | 配置框架 | 配置格式 | 配置加载方式 | 特殊功能 |
|------|---------|---------|--------------|----------|
| comsrv | Figment | YAML/TOML/JSON | 多源配置，支持现代/遗留模式 | 多协议通道、CSV点表、分布式配置 |
| modsrv | config crate | YAML/TOML | 文件加载+默认值 | 多存储模式、模型执行配置 |
| hissrv | clap + serde | YAML | 命令行+文件，支持热重载 | 多存储后端、数据过滤 |
| netsrv | config crate | YAML | 文件加载 | 多云集成、主题模板 |
| alarmsrv | 环境变量优先 | YAML | 环境变量+文件 | 告警分类、多渠道通知 |
| apigateway | config crate | YAML | 文件+环境变量 | 统一路由、CORS配置 |

## 各服务详细配置

### 1. comsrv（通信服务）

#### 配置结构
```rust
AppConfig {
    service: ServiceConfig {      // 服务基本信息
        name: String,
        version: String,
        description: String,
    },
    api: ApiConfig {             // API 服务配置
        host: String,
        port: u16,
        workers: Option<usize>,
    },
    redis: RedisConfig {         // Redis 连接配置
        host: String,
        port: u16,
        password: Option<String>,
        db: u32,
        pool_size: usize,
    },
    logging: LoggingConfig {     // 日志配置
        level: String,
        file: Option<String>,
        max_size: u64,
        max_files: usize,
        console: bool,
    },
    default_paths: DefaultPathConfig {  // 默认路径配置
        config_dir: String,
        log_dir: String,
    },
    channels: Vec<ChannelConfig>,      // 通道配置列表
}
```

#### 特色功能
- **双配置管理器**：支持现代（ModernConfigManager）和遗留（ConfigManager）两种模式
- **多协议支持**：Modbus TCP/RTU、IEC104、CAN、GPIO
- **约定优于配置**：自动按通道名查找 CSV 点表文件
- **通道级日志**：每个通道可独立配置日志输出

#### 配置示例
```yaml
service:
  name: "comsrv"
  version: "1.0.0"
  logging:
    level: "debug"
    file: "logs/comsrv.log"
    max_size: 10485760
    console: true

channels:
  - id: 1001
    name: "TankFarmModbusTCP"
    transport:
      type: "tcp"
      config:
        host: "192.168.1.100"
        port: 502
    protocol:
      type: "modbus_tcp"
    # 系统自动查找 config/TankFarmModbusTCP/ 下的点表文件
```

### 2. modsrv（模型服务）

#### 配置结构
```rust
Config {
    redis: RedisConfig,
    logging: LoggingConfig,
    model: ModelConfig {         // 模型执行配置
        execution_interval_ms: u64,
        max_concurrent_models: usize,
        timeout_ms: u64,
    },
    control: ControlConfig {     // 控制操作配置
        operation_timeout_ms: u64,
        max_retries: u32,
        retry_delay_ms: u64,
    },
    api: ApiConfig,
    monitoring: MonitoringConfig {
        cpu_threshold: f32,
        memory_threshold_mb: u64,
    },
    templates_dir: String,
    use_redis: bool,
    storage_mode: String,        // memory/redis/hybrid
    sync_interval_secs: u64,
}
```

#### 配置示例
```yaml
redis:
  host: "localhost"
  port: 6379

model:
  execution_interval_ms: 1000
  max_concurrent_models: 10
  timeout_ms: 5000

storage_mode: "hybrid"
templates_dir: "templates"
```

### 3. hissrv（历史数据服务）

#### 配置结构
```rust
Config {
    service: ServiceConfig,
    redis: RedisConfig {
        subscribe_patterns: Vec<String>,  // 订阅模式
        scan_batch_size: u32,
    },
    storage: StorageConfig {
        backend: StorageBackend,  // influxdb/postgresql/mongodb
        influxdb: Option<InfluxDBConfig>,
        postgresql: Option<PostgreSQLConfig>,
        mongodb: Option<MongoDBConfig>,
    },
    data: DataConfig {
        filters: Vec<DataFilter>,
        transformations: Vec<DataTransformation>,
        batch_size: usize,
        flush_interval_secs: u64,
    },
    api: ApiConfig,
    monitoring: MonitoringConfig,
    logging: LoggingConfig,
    performance: PerformanceConfig,
}
```

#### 特色功能
- **多存储后端**：支持 InfluxDB、PostgreSQL、MongoDB
- **数据过滤**：基于点位、数值范围、变化率的过滤
- **配置热重载**：支持运行时重新加载配置
- **性能优化**：批处理、缓存、并行写入

### 4. netsrv（网络服务）

#### 配置结构
```rust
Config {
    redis: RedisConfig,
    networks: Vec<NetworkConfig> {
        id: String,
        name: String,
        network_type: NetworkType,  // aws_iot/aliyun_iot/azure_iot/mqtt/http
        enabled: bool,
        connection: ConnectionConfig,
        topics: TopicConfig,
        data_filtering: DataFilterConfig,
    },
    logging: LoggingConfig,
}
```

#### 特色功能
- **多云支持**：AWS IoT Core、阿里云 IoT、Azure IoT Hub
- **灵活认证**：证书、设备密钥、Bearer Token、SAS Token
- **主题模板**：支持变量替换的动态主题
- **数据过滤**：发送前的数据筛选和转换

### 5. alarmsrv（告警服务）

#### 配置结构
```rust
AlarmConfig {
    redis: RedisConfig {
        connection_type: ConnectionType,  // tcp/unix
        tcp_config: Option<TcpConfig>,
        unix_config: Option<UnixConfig>,
    },
    api: ApiConfig,
    storage: StorageConfig {
        alarm_history_days: u32,
        cleanup_interval_hours: u32,
    },
}
```

#### 特色功能
- **环境变量优先**：所有配置项都可通过环境变量覆盖
- **告警分类**：critical、warning、info 三级分类
- **智能去重**：基于内容的去重和速率限制
- **多渠道通知**：email、sms、webhook

### 6. apigateway（API 网关）

#### 配置结构
```rust
Config {
    server: ServerConfig {
        host: String,
        port: u16,
        workers: usize,
    },
    redis: RedisConfig,
    services: ServicesConfig {
        comsrv_url: String,
        modsrv_url: String,
        hissrv_url: String,
        netsrv_url: String,
        alarmsrv_url: String,
    },
    cors: CorsConfig {
        allowed_origins: Vec<String>,
        allowed_methods: Vec<String>,
        allowed_headers: Vec<String>,
    },
    logging: LoggingConfig,
}
```

## 配置共性特点

### 1. 分层配置架构
所有服务都采用分层配置：
- **服务级配置**：基本信息、API端口、日志等
- **功能模块配置**：特定功能的配置项
- **连接配置**：Redis、数据库等外部连接

### 2. Redis 中心化
- 所有服务都通过 Redis 进行数据交换
- 统一的 Redis 配置结构
- 支持连接池、密码认证、数据库选择

### 3. 日志配置标准化
```yaml
logging:
  level: "debug"           # trace/debug/info/warn/error
  file: "logs/service.log" # 可选的文件输出
  max_size: 10485760      # 文件大小限制
  max_files: 5            # 保留文件数
  console: true           # 控制台输出
```

### 4. API 配置统一
```yaml
api:
  host: "0.0.0.0"
  port: 8080
  workers: 4              # 工作线程数
  cors:                   # CORS 配置
    enabled: true
    allowed_origins: ["*"]
```

### 5. 环境变量支持
- 所有服务都支持环境变量覆盖
- 统一的环境变量前缀：`SERVICE_NAME_`
- 支持嵌套配置的环境变量

## 配置加载优先级

1. **环境变量**（最高优先级）
2. **配置文件**（YAML/TOML/JSON）
3. **默认值**（代码中定义）

## 配置最佳实践

### 1. 使用 YAML 格式
虽然支持多种格式，但推荐统一使用 YAML：
- 可读性好
- 支持注释
- 层次结构清晰

### 2. 配置验证
所有服务在启动时都应验证配置：
```rust
impl Config {
    pub fn validate(&self) -> Result<(), ConfigError> {
        // 验证必填项
        // 验证取值范围
        // 验证关联性
    }
}
```

### 3. 敏感信息管理
- 密码、密钥等敏感信息使用环境变量
- 不要在配置文件中硬编码敏感信息
- 支持从密钥管理服务加载

### 4. 配置文档化
每个配置项都应有清晰的说明：
```yaml
# Redis 连接配置
redis:
  host: "localhost"      # Redis 服务器地址
  port: 6379            # Redis 端口号
  password: null        # Redis 密码（可选）
  db: 0                 # 数据库索引（0-15）
  pool_size: 10         # 连接池大小
```

### 5. 配置模板
提供配置模板文件：
- `config.example.yml`：示例配置
- `config.default.yml`：默认配置
- `config.prod.yml`：生产环境参考

## 配置迁移指南

### 从旧版本迁移
1. **备份现有配置**
2. **使用迁移工具**（如 comsrv 的配置迁移工具）
3. **验证新配置**
4. **逐步切换**

### 配置中心化
考虑使用配置中心（如 Consul、etcd）：
- comsrv 已支持分布式配置
- 其他服务可扩展支持
- 支持配置热更新

## 故障排查

### 常见配置问题
1. **配置文件找不到**
   - 检查文件路径
   - 确认工作目录

2. **配置解析失败**
   - 检查 YAML 语法
   - 验证数据类型

3. **连接失败**
   - 验证网络连通性
   - 检查端口和密码

### 调试技巧
1. **启用调试日志**：`RUST_LOG=debug`
2. **打印配置**：启动时输出解析后的配置
3. **配置验证**：使用 `--validate-config` 参数

## 未来改进方向

1. **配置中心集成**
   - 支持 Consul/etcd
   - 配置版本管理
   - 配置回滚

2. **配置热更新**
   - 更多服务支持热重载
   - 配置变更通知
   - 平滑更新

3. **配置模板引擎**
   - 支持配置模板
   - 环境变量插值
   - 条件配置

4. **配置加密**
   - 敏感信息加密存储
   - 密钥管理集成
   - 审计日志

## 总结

VoltageEMS 的配置框架体现了以下设计原则：

1. **简单性**：约定优于配置，减少配置复杂度
2. **一致性**：统一的配置结构和命名规范
3. **灵活性**：支持多种配置源和格式
4. **可维护性**：清晰的分层和职责分离
5. **安全性**：敏感信息保护和访问控制

通过统一的配置框架，VoltageEMS 实现了易用、可靠、可扩展的微服务配置管理。