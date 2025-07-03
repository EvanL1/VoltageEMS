# NetSrv Fix Log

## 2025-01-03 - 数据结构分析

### 1. 核心配置数据结构 (config_new.rs)

#### NetServiceConfig - 主配置结构
```rust
struct NetServiceConfig {
    base: BaseServiceConfig,      // 基础服务配置（flattened）
    networks: Vec<NetworkConfig>, // 网络配置列表
    data: DataConfig,            // 数据处理配置
}
```

#### DataConfig - 数据处理配置
```rust
struct DataConfig {
    redis_data_key: String,              // Redis数据键模式，默认"voltage:data:*"
    redis_polling_interval_secs: u64,    // Redis轮询间隔（秒）
    enable_buffering: bool,              // 启用数据缓冲
    buffer_size: usize,                  // 缓冲区大小
}
```

#### NetworkConfig - 网络配置（枚举）
支持三种网络类型：
1. **LegacyMqtt** - 传统MQTT配置
2. **Http** - HTTP REST API配置  
3. **CloudMqtt** - 云MQTT配置（AWS IoT、阿里云IoT等）

#### CloudMqttConfig - 云MQTT配置
```rust
struct CloudMqttConfig {
    name: String,
    provider: CloudProvider,              // 云提供商
    provider_config: ProviderConfig,      // 提供商特定配置
    auth: AuthConfig,                     // 认证配置
    topics: CloudTopicConfig,             // 主题配置
    format_type: FormatType,              // 数据格式
    tls: TlsConfig,                       // TLS配置
    aws_features: AwsIotFeatures,         // AWS IoT特性
}
```

#### ProviderConfig - 云提供商配置（枚举）
- **Aws**: endpoint, port, thing_name
- **Aliyun**: endpoint, port, product_key, device_name
- **Azure**: hostname, device_id
- **Tencent**: endpoint, port, product_id, device_name
- **Huawei**: endpoint, port, device_id
- **Custom**: broker, port, client_id

#### AuthConfig - 认证配置（枚举）
- **Certificate**: 证书认证（cert_path, key_path, ca_path）
- **DeviceSecret**: 设备密钥认证（阿里云）
- **SasToken**: SAS令牌认证（Azure）
- **UsernamePassword**: 用户名密码认证
- **Custom**: 自定义认证参数

#### AwsIotFeatures - AWS IoT特性配置
```rust
struct AwsIotFeatures {
    jobs_enabled: bool,                   // 启用AWS IoT Jobs
    device_shadow_enabled: bool,          // 启用设备影子
    fleet_provisioning_enabled: bool,     // 启用设备配置
    jobs_topic_prefix: String,            // Jobs主题前缀
    shadow_topic_prefix: String,          // Shadow主题前缀
    provisioning_template: Option<String>,// 配置模板
    auto_respond_jobs: bool,              // 自动响应Jobs
    max_concurrent_jobs: u32,             // 最大并发Jobs数
}
```

### 2. Redis数据获取 (data_fetcher_new.rs)

#### RedisDataFetcher - Redis数据获取器
```rust
struct RedisDataFetcher {
    client: redis::Client,
    config: RedisConfig,
    data_key_pattern: String,    // 数据键匹配模式
    poll_interval: Duration,     // 轮询间隔
    last_fetch_time: Instant,    // 上次获取时间
}
```

主要方法：
- `fetch_data()` - 获取匹配模式的所有Redis键值
- `start_polling()` - 开始轮询并通过channel发送数据
- `get_data_for_key()` - 获取单个键的数据（支持Hash和String）

### 3. 网络客户端 (network/)

#### NetworkClient Trait - 网络客户端接口
```rust
trait NetworkClient: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    fn is_connected(&self) -> bool;
    async fn send(&self, data: &str) -> Result<()>;
    fn name(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
}
```

#### MQTT客户端实现 (mqtt.rs)
- 支持传统MQTT和云MQTT两种模式
- 云MQTT支持多种云提供商（AWS、阿里云、Azure等）
- 实现了各云平台的认证机制：
  - AWS: X.509证书 + ALPN协议
  - 阿里云: HMAC-SHA256签名认证
  - Azure: SAS令牌
  - 自定义认证参数

#### HTTP客户端实现 (http.rs)
- 支持多种HTTP方法（GET/POST/PUT/PATCH/DELETE）
- 支持多种认证方式（Basic、Bearer、API Key）
- 可配置请求头和超时时间

### 4. 数据格式化器 (formatter/)

#### DataFormatter Trait
```rust
trait DataFormatter: Send + Sync {
    fn format(&self, data: &Value) -> Result<String>;
}
```

支持的格式：
- **JsonFormatter**: JSON格式
- **AsciiFormatter**: ASCII格式
- Binary和Protobuf（配置中定义，实现回退到JSON）

### 5. 数据流程

1. **数据获取**：
   - RedisDataFetcher按照配置的轮询间隔从Redis获取数据
   - 支持获取Hash表和String类型数据
   - 数据通过mpsc channel发送

2. **数据处理**：
   - 主循环接收channel中的数据
   - 对每个配置的网络客户端进行数据发送

3. **数据发送**：
   - 根据网络类型使用对应的格式化器
   - MQTT客户端支持主题模板变量替换
   - HTTP客户端构建完整的请求
   - 云MQTT客户端处理特定平台的要求

### 6. 错误处理

定义了统一的错误类型：
```rust
enum NetSrvError {
    Connection(String),
    Format(String),
    Config(String),
    Redis(String),
    Mqtt(String),
    Http(String),
    Io(String),
    Data(String),
}
```

### 7. 配置管理API

- 提供了REST API进行运行时配置管理
- 支持获取和更新网络配置
- 端口号：health_check_port + 1

### 8. 关键特性

1. **多协议支持**：同时支持MQTT、HTTP和多种云IoT平台
2. **灵活认证**：支持证书、密钥、令牌等多种认证方式
3. **AWS IoT优化**：特别支持AWS IoT的Jobs、Shadow等高级特性
4. **实时数据转发**：从Redis轮询数据并转发到多个目标
5. **可扩展架构**：使用trait和工厂模式便于添加新协议