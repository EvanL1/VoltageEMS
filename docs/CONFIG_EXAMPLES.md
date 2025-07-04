# VoltageEMS 配置示例集

本文档提供各种场景下的配置示例，帮助快速上手。

## 目录
- [开发环境配置](#开发环境配置)
- [生产环境配置](#生产环境配置)
- [Docker 部署配置](#docker-部署配置)
- [高可用配置](#高可用配置)
- [安全加固配置](#安全加固配置)
- [性能优化配置](#性能优化配置)

## 开发环境配置

### 单机开发环境

`config/development.yml`:
```yaml
# 开发环境通用配置
redis:
  host: "localhost"
  port: 6379
  database: 0

logging:
  level: "debug"
  format: "pretty"
  enable_ansi: true
  enable_file: true
  file_path: "logs/dev.log"

monitoring:
  enabled: false  # 开发环境关闭监控
```

### VSCode 调试配置

`.vscode/launch.json`:
```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug alarmsrv",
      "cargo": {
        "args": ["build", "--bin=alarmsrv", "--package=alarmsrv"],
        "filter": {
          "name": "alarmsrv",
          "kind": "bin"
        }
      },
      "args": [],
      "cwd": "${workspaceFolder}",
      "env": {
        "RUST_LOG": "debug",
        "ALARMSRV_REDIS__HOST": "localhost",
        "ALARMSRV_API__PORT": "8094"
      }
    }
  ]
}
```

## 生产环境配置

### 基础生产配置

`config/production.yml`:
```yaml
# 生产环境通用配置
redis:
  host: "redis-cluster.internal"
  port: 6379
  password: "${REDIS_PASSWORD}"  # 从环境变量读取
  database: 0
  pool_size: 50
  connection_timeout: 10
  command_timeout: 10

logging:
  level: "info"
  format: "json"
  enable_ansi: false
  enable_file: true
  file_path: "/var/log/voltage/${SERVICE_NAME}.log"
  file_max_size: 104857600  # 100MB
  file_max_age: 30          # 30天
  file_max_backups: 10

monitoring:
  enabled: true
  metrics_path: "/metrics"
  health_path: "/health"
  prometheus_enabled: true
```

### 完整服务配置示例

`config/comsrv.yml`:
```yaml
# 通信服务生产配置
service:
  name: "comsrv"
  version: "1.0.0"
  description: "Industrial Communication Service"

api:
  host: "0.0.0.0"
  port: 8091
  prefix: "/api/v1"
  max_connections: 10000
  timeout: 30

channels:
  # Modbus TCP 通道
  - id: 1001
    name: "Plant_A_ModbusTCP"
    enabled: true
    transport:
      type: "tcp"
      config:
        host: "192.168.100.10"
        port: 502
        timeout: "5s"
        max_retries: 3
        retry_delay: "1s"
    protocol:
      type: "modbus_tcp"
      config:
        unit_id: 1
        max_concurrent_requests: 10
    logging:
      enabled: true
      level: "info"
      log_dir: "logs/plant_a"
      max_file_size: 52428800  # 50MB
      retention_days: 7

  # Modbus RTU 通道
  - id: 2001
    name: "Plant_B_ModbusRTU"
    enabled: true
    transport:
      type: "serial"
      config:
        port: "/dev/ttyUSB0"
        baud_rate: 9600
        data_bits: 8
        stop_bits: 1
        parity: "None"
        timeout: "3s"
    protocol:
      type: "modbus_rtu"
      config:
        inter_frame_delay: 10  # ms

  # IEC104 通道
  - id: 3001
    name: "Substation_IEC104"
    enabled: true
    transport:
      type: "tcp"
      config:
        host: "10.0.1.100"
        port: 2404
        timeout: "10s"
    protocol:
      type: "iec104"
      config:
        t1_timeout: 15
        t2_timeout: 10
        t3_timeout: 20
        k_value: 12
        w_value: 8
```

## Docker 部署配置

### Docker Compose 配置

`docker-compose.yml`:
```yaml
version: '3.8'

services:
  redis:
    image: redis:7-alpine
    command: redis-server --requirepass ${REDIS_PASSWORD}
    volumes:
      - redis_data:/data
    networks:
      - voltage_net

  influxdb:
    image: influxdb:2.7
    environment:
      - DOCKER_INFLUXDB_INIT_MODE=setup
      - DOCKER_INFLUXDB_INIT_USERNAME=admin
      - DOCKER_INFLUXDB_INIT_PASSWORD=${INFLUXDB_PASSWORD}
      - DOCKER_INFLUXDB_INIT_ORG=voltage
      - DOCKER_INFLUXDB_INIT_BUCKET=voltage_data
      - DOCKER_INFLUXDB_INIT_ADMIN_TOKEN=${INFLUXDB_TOKEN}
    volumes:
      - influxdb_data:/var/lib/influxdb2
    networks:
      - voltage_net

  comsrv:
    image: voltage/comsrv:${VERSION:-latest}
    environment:
      - RUST_LOG=info
      - COMSRV_REDIS__HOST=redis
      - COMSRV_REDIS__PASSWORD=${REDIS_PASSWORD}
      - COMSRV_API__PORT=8091
    volumes:
      - ./config:/app/config:ro
      - ./data:/app/data
      - comsrv_logs:/app/logs
    ports:
      - "8091:8091"
    depends_on:
      - redis
    networks:
      - voltage_net
    restart: unless-stopped

  hissrv:
    image: voltage/hissrv:${VERSION:-latest}
    environment:
      - RUST_LOG=info
      - HISSRV_REDIS__HOST=redis
      - HISSRV_REDIS__PASSWORD=${REDIS_PASSWORD}
      - HISSRV_STORAGE__INFLUXDB__URL=http://influxdb:8086
      - HISSRV_STORAGE__INFLUXDB__TOKEN=${INFLUXDB_TOKEN}
      - HISSRV_API__PORT=8093
    volumes:
      - ./config:/app/config:ro
      - hissrv_logs:/app/logs
    ports:
      - "8093:8093"
    depends_on:
      - redis
      - influxdb
    networks:
      - voltage_net
    restart: unless-stopped

  apigateway:
    image: voltage/apigateway:${VERSION:-latest}
    environment:
      - RUST_LOG=info
      - APIGATEWAY_REDIS__HOST=redis
      - APIGATEWAY_REDIS__PASSWORD=${REDIS_PASSWORD}
      - APIGATEWAY_SERVICES__COMSRV_URL=http://comsrv:8091
      - APIGATEWAY_SERVICES__HISSRV_URL=http://hissrv:8093
    ports:
      - "80:8080"
    depends_on:
      - redis
      - comsrv
      - hissrv
    networks:
      - voltage_net
    restart: unless-stopped

volumes:
  redis_data:
  influxdb_data:
  comsrv_logs:
  hissrv_logs:

networks:
  voltage_net:
    driver: bridge
```

### 环境变量文件

`.env`:
```bash
# 版本控制
VERSION=1.0.0

# 数据库密码
REDIS_PASSWORD=your_redis_password_here
INFLUXDB_PASSWORD=your_influxdb_password_here
INFLUXDB_TOKEN=your_influxdb_token_here

# 服务配置
RUST_LOG=info

# Redis 配置
REDIS_HOST=redis
REDIS_PORT=6379

# API 网关配置
APIGATEWAY_CORS__ALLOWED_ORIGINS=https://app.voltage.com,https://admin.voltage.com
```

## 高可用配置

### Redis Sentinel 配置

`config/redis-ha.yml`:
```yaml
redis:
  sentinel:
    enabled: true
    master_name: "voltage-master"
    sentinels:
      - host: "sentinel1.voltage.internal"
        port: 26379
      - host: "sentinel2.voltage.internal"
        port: 26379
      - host: "sentinel3.voltage.internal"
        port: 26379
  password: "${REDIS_PASSWORD}"
  pool_size: 100
  connection_timeout: 15
  command_timeout: 15
  retry_policy:
    max_retries: 5
    base_delay_ms: 100
    max_delay_ms: 5000
```

### 多实例负载均衡

`config/apigateway-ha.yml`:
```yaml
server:
  host: "0.0.0.0"
  port: 8080
  workers: 16  # CPU 核心数 * 2

services:
  # 使用内部负载均衡器
  comsrv_url: "http://comsrv-lb.internal:8091"
  modsrv_url: "http://modsrv-lb.internal:8092"
  hissrv_url: "http://hissrv-lb.internal:8093"
  
  # 超时和重试配置
  timeout_ms: 30000
  retry_policy:
    max_retries: 3
    retry_on: [502, 503, 504]
    backoff:
      type: "exponential"
      base_ms: 100
      max_ms: 10000

# 健康检查配置
health_check:
  interval: 10s
  timeout: 5s
  unhealthy_threshold: 3
  healthy_threshold: 2
```

## 安全加固配置

### TLS/SSL 配置

`config/secure.yml`:
```yaml
# API TLS 配置
api:
  tls:
    enabled: true
    cert_path: "/etc/voltage/certs/server.crt"
    key_path: "/etc/voltage/certs/server.key"
    ca_path: "/etc/voltage/certs/ca.crt"
    client_auth: "request"  # none/request/require
    min_version: "1.2"
    ciphers:
      - "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384"
      - "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256"

# Redis TLS
redis:
  tls:
    enabled: true
    cert_path: "/etc/voltage/certs/redis-client.crt"
    key_path: "/etc/voltage/certs/redis-client.key"
    ca_path: "/etc/voltage/certs/redis-ca.crt"
    insecure_skip_verify: false
```

### 认证授权配置

`config/auth.yml`:
```yaml
# JWT 认证配置
auth:
  jwt:
    enabled: true
    secret: "${JWT_SECRET}"  # 从环境变量读取
    issuer: "voltage-ems"
    audience: "voltage-api"
    expiration_hours: 24
    refresh_enabled: true
    refresh_expiration_days: 30
    
  # API Key 认证
  api_key:
    enabled: true
    header_name: "X-API-Key"
    keys:
      - name: "monitoring"
        key: "${MONITORING_API_KEY}"
        permissions: ["read:metrics", "read:health"]
      - name: "admin"
        key: "${ADMIN_API_KEY}"
        permissions: ["*"]
```

## 性能优化配置

### 高性能数据采集

`config/comsrv-performance.yml`:
```yaml
# 性能优化配置
performance:
  # 异步 I/O 配置
  async_io:
    worker_threads: 8
    max_concurrent_requests: 1000
    queue_size: 10000
    
  # 批处理配置
  batching:
    enabled: true
    max_batch_size: 500
    max_wait_time_ms: 100
    
  # 缓存配置
  cache:
    enabled: true
    type: "memory"  # memory/redis
    ttl_seconds: 60
    max_entries: 100000
    eviction_policy: "lru"
    
  # 连接池优化
  connection_pool:
    min_idle: 10
    max_size: 100
    connection_timeout_ms: 5000
    idle_timeout_ms: 600000  # 10分钟
    max_lifetime_ms: 1800000  # 30分钟
```

### 数据压缩传输

`config/compression.yml`:
```yaml
# 数据压缩配置
compression:
  enabled: true
  algorithm: "zstd"  # none/gzip/zstd/lz4
  level: 3  # 1-9, 3 是平衡选择
  min_size_bytes: 1024  # 小于此大小不压缩
  
  # API 响应压缩
  api:
    enabled: true
    mime_types:
      - "application/json"
      - "text/plain"
      - "text/csv"
      
  # Redis 数据压缩
  redis:
    enabled: true
    threshold_bytes: 512
    
  # 网络传输压缩
  network:
    enabled: true
    protocols: ["mqtt", "http"]
```

### 批量写入优化

`config/hissrv-batch.yml`:
```yaml
# 历史数据批量写入
storage:
  batch_writer:
    enabled: true
    batch_size: 5000
    flush_interval_ms: 1000
    max_pending_batches: 10
    
    # 并发写入
    concurrent_writers: 4
    write_timeout_ms: 30000
    
    # 失败重试
    retry:
      max_attempts: 3
      backoff_ms: 1000
      max_backoff_ms: 60000
      
    # 写入缓冲
    buffer:
      type: "memory"  # memory/disk
      max_size_mb: 1024
      overflow_policy: "drop_oldest"  # drop_oldest/block/spill_to_disk
```

## SQLite 配置示例

### 动态配置存储

```sql
-- 插入动态配置
INSERT INTO configs (service, key, value, type, description) VALUES
  ('comsrv', 'channels.1001.enabled', 'true', 'boolean', '启用/禁用通道'),
  ('comsrv', 'channels.1001.poll_interval_ms', '1000', 'number', '轮询间隔'),
  ('alarmsrv', 'classification.thresholds.critical', '0.9', 'number', '严重告警阈值'),
  ('alarmsrv', 'classification.thresholds.warning', '0.7', 'number', '警告告警阈值'),
  ('hissrv', 'filters.outlier_detection.enabled', 'true', 'boolean', '异常值检测'),
  ('hissrv', 'filters.outlier_detection.zscore', '3.5', 'number', 'Z分数阈值');

-- 创建配置模板
INSERT INTO config_templates (name, service, description, template_data) VALUES
  ('modbus_channel', 'comsrv', 'Modbus通道模板', '{
    "transport": {
      "type": "tcp",
      "config": {
        "timeout": "5s",
        "max_retries": 3
      }
    },
    "protocol": {
      "type": "modbus_tcp"
    }
  }');

-- 配置验证规则
INSERT INTO config_validators (service, key, rule_type, rule_data, error_message) VALUES
  ('comsrv', 'api.port', 'range', '{"min": 1024, "max": 65535}', '端口必须在1024-65535之间'),
  ('alarmsrv', 'storage.retention_days', 'range', '{"min": 1, "max": 365}', '保留天数必须在1-365之间'),
  ('hissrv', 'storage.backend', 'enum', '["influxdb", "postgresql", "mongodb"]', '不支持的存储后端');
```

## 配置测试示例

### 单元测试配置

`tests/config/test.yml`:
```yaml
# 测试环境配置
service:
  name: "test-service"
  version: "0.0.1"

redis:
  host: "localhost"
  port: 6379
  database: 15  # 使用独立的测试数据库

logging:
  level: "debug"
  format: "pretty"
  enable_file: false

# 测试专用配置
test:
  mock_external_services: true
  timeout_ms: 5000
  parallel_tests: 4
```

### 集成测试配置

```rust
#[cfg(test)]
mod tests {
    use voltage_config::prelude::*;
    
    #[tokio::test]
    async fn test_config_loading() {
        // 创建测试配置
        let config = ConfigLoaderBuilder::new()
            .add_file("tests/config/test.yml")
            .add_env_prefix("TEST_")
            .defaults(TestConfig::default())
            .unwrap()
            .build()
            .unwrap()
            .load::<TestConfig>()
            .unwrap();
            
        assert_eq!(config.base.service.name, "test-service");
        assert_eq!(config.base.redis.database, 15);
    }
}
```

---

这些示例涵盖了 VoltageEMS 的主要配置场景。根据实际需求选择合适的配置模板进行调整。