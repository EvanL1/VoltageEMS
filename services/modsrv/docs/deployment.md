# ModSrv v2.0 部署文档

## 概述

本文档详细说明了ModSrv v2.0的部署方式，包括Docker容器化部署、测试环境搭建和生产环境配置。

## 部署架构

### 系统要求

#### 硬件要求
- **CPU**: 2核心以上 (推荐4核心)
- **内存**: 4GB以上 (推荐8GB)
- **存储**: 10GB以上可用空间
- **网络**: 千兆以太网

#### 软件要求
- **操作系统**: Linux (Ubuntu 20.04+, CentOS 8+) 或 Docker支持的系统
- **Docker**: 20.10+
- **Docker Compose**: 2.0+
- **Redis**: 8.0+ (可通过Docker部署)

### 网络架构

```
┌─────────────────────────────────────────────────────────┐
│                    外部网络                             │
│              (可选的负载均衡器)                         │
└─────────────────┬───────────────────────────────────────┘
                  │ HTTP/HTTPS :8092
┌─────────────────┴───────────────────────────────────────┐
│                Docker Network                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │   ModSrv    │  │    Redis    │  │   ComsRv    │     │
│  │   :8092     │  │   :6379     │  │  (Optional) │     │
│  └─────────────┘  └─────────────┘  └─────────────┘     │
└─────────────────────────────────────────────────────────┘
```

## Docker部署

### 1. 单服务部署

#### Dockerfile

```dockerfile
# ModSrv生产环境Dockerfile
FROM rust:1.88-bullseye as builder

# 设置工作目录
WORKDIR /usr/src/voltage-ems

# 复制源代码
COPY . .

# 构建应用
RUN cargo build --release -p modsrv

# 运行时镜像
FROM debian:bullseye-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 创建用户
RUN groupadd -r modsrv && useradd -r -g modsrv modsrv

# 创建目录
RUN mkdir -p /config /logs /data && \
    chown -R modsrv:modsrv /config /logs /data

# 复制二进制文件
COPY --from=builder /usr/src/voltage-ems/target/release/modsrv /usr/local/bin/modsrv

# 设置环境变量
ENV RUST_LOG=info \
    CONFIG_FILE=/config/config.yml \
    MAPPINGS_DIR=/config/mappings

# 暴露端口
EXPOSE 8092

# 切换用户
USER modsrv

# 健康检查
HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8092/health || exit 1

# 启动命令
CMD ["modsrv", "service"]
```

#### 构建镜像

```bash
# 构建ModSrv镜像
docker build -t modsrv:v2.0 -f services/modsrv/Dockerfile .

# 验证镜像
docker images | grep modsrv
```

#### 运行容器

```bash
# 创建网络
docker network create voltage-ems-network

# 启动Redis
docker run -d \
  --name redis \
  --network voltage-ems-network \
  -v redis-data:/data \
  redis:8-alpine

# 启动ModSrv
docker run -d \
  --name modsrv \
  --network voltage-ems-network \
  -p 8092:8092 \
  -v $(pwd)/config:/config:ro \
  -v $(pwd)/logs:/logs \
  -e REDIS_URL=redis://redis:6379 \
  modsrv:v2.0
```

### 2. Docker Compose部署

#### 生产环境配置 (`docker-compose.yml`)

```yaml
version: '3.8'

services:
  # Redis数据库
  redis:
    image: redis:8-alpine
    hostname: redis
    restart: unless-stopped
    networks:
      - voltage-ems-network
    volumes:
      - redis-data:/data
      - ./config/redis/redis.conf:/usr/local/etc/redis/redis.conf:ro
    command: redis-server /usr/local/etc/redis/redis.conf
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 3
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"

  # ModSrv服务
  modsrv:
    image: modsrv:v2.0
    hostname: modsrv
    restart: unless-stopped
    depends_on:
      redis:
        condition: service_healthy
    networks:
      - voltage-ems-network
    ports:
      - "8092:8092"
    volumes:
      - ./config:/config:ro
      - ./logs/modsrv:/logs
      - ./templates:/app/templates:ro
    environment:
      - RUST_LOG=info
      - REDIS_URL=redis://redis:6379
      - CONFIG_FILE=/config/config.yml
      - MAPPINGS_DIR=/config/mappings
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8092/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 30s
    logging:
      driver: "json-file"
      options:
        max-size: "50m"
        max-file: "5"

networks:
  voltage-ems-network:
    driver: bridge

volumes:
  redis-data:
    driver: local
```

#### 启动生产环境

```bash
# 创建必要目录
mkdir -p config logs/modsrv templates

# 启动服务
docker-compose up -d

# 查看服务状态
docker-compose ps

# 查看日志
docker-compose logs -f modsrv
```

### 3. 测试环境配置

#### 测试环境配置 (`docker-compose.test.yml`)

```yaml
version: '3.8'

services:
  # Redis数据库
  redis:
    image: redis:8-alpine
    hostname: redis
    networks:
      - modsrv-test-network
    volumes:
      - redis-data:/data
      - ./logs/redis:/var/log/redis
    command: >
      redis-server 
      --appendonly yes 
      --appendfsync everysec
      --save 60 1000
      --loglevel notice
      --logfile /var/log/redis/redis.log
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 5s
      timeout: 3s
      retries: 5

  # ComsRv数据模拟器
  comsrv-simulator:
    build:
      context: ./docker/comsrv-simulator
      dockerfile: Dockerfile
    hostname: comsrv-simulator
    depends_on:
      redis:
        condition: service_healthy
    networks:
      - modsrv-test-network
    volumes:
      - ./logs/comsrv-simulator:/app/logs
    environment:
      - REDIS_URL=redis://redis:6379
      - LOG_LEVEL=DEBUG

  # ModSrv主服务
  modsrv:
    build:
      context: ../..
      dockerfile: services/modsrv/Dockerfile.test
    hostname: modsrv
    depends_on:
      redis:
        condition: service_healthy
      comsrv-simulator:
        condition: service_healthy
    networks:
      - modsrv-test-network
    volumes:
      - ./test-configs:/config:ro
      - ./models:/app/models:ro
      - ./templates:/app/templates:ro
      - ./logs/modsrv:/logs
      - ./test-results:/test-results
    environment:
      - RUST_LOG=modsrv=debug,redis=info
      - REDIS_URL=redis://redis:6379
      - CONFIG_FILE=/config/config.yml
      - MAPPINGS_DIR=/config/mappings

  # 测试执行器
  test-executor:
    build:
      context: ./docker/test-executor
      dockerfile: Dockerfile
    hostname: test-executor
    depends_on:
      modsrv:
        condition: service_healthy
    networks:
      - modsrv-test-network
    volumes:
      - ./tests:/app/tests:ro
      - ./test-results:/app/results
      - ./logs/test-executor:/app/logs
    environment:
      - REDIS_URL=redis://redis:6379
      - MODSRV_URL=http://modsrv:8092
      - TEST_OUTPUT=/app/results

networks:
  modsrv-test-network:
    driver: bridge
    internal: true  # 不允许外部访问

volumes:
  redis-data:
    driver: local
```

#### 运行测试环境

```bash
# 运行完整测试
./run-docker-test.sh

# 或者手动启动
docker-compose -f docker-compose.test.yml up -d

# 查看测试结果
docker-compose -f docker-compose.test.yml logs test-executor
```

## 配置管理

### 1. 目录结构

```
modsrv/
├── config/
│   ├── config.yml              # 主配置文件
│   ├── mappings/               # 映射配置目录
│   │   ├── power_meter_demo.json
│   │   └── transformer_demo.json
│   └── redis/
│       └── redis.conf          # Redis配置
├── logs/                       # 日志目录
│   ├── modsrv/
│   └── redis/
├── templates/                  # 模板目录
│   ├── devices/
│   ├── transformers/
│   └── generators/
└── docker-compose.yml         # 部署配置
```

### 2. 配置文件模板

#### 生产环境配置 (`config/config.yml`)

```yaml
service_name: "modsrv"
version: "2.0.0"

redis:
  url: "redis://redis:6379"
  key_prefix: "modsrv:"
  connection_timeout_ms: 5000
  retry_attempts: 3

log:
  level: "info"
  file_path: "/logs/modsrv.log"

api:
  host: "0.0.0.0"
  port: 8092
  timeout_seconds: 30

models:
  - id: "power_meter_01"
    name: "1号电表"
    description: "主进线电表"
    monitoring:
      voltage_a:
        description: "A相电压"
        unit: "V"
      # ... 其他监视点
    control:
      main_switch:
        description: "主开关"
      # ... 其他控制点

update_interval_ms: 1000
```

### 3. 环境变量配置

#### 开发环境 (`.env.dev`)

```bash
RUST_LOG=debug
REDIS_URL=redis://localhost:6379
CONFIG_FILE=config/config.yml
MAPPINGS_DIR=config/mappings
```

#### 生产环境 (`.env.prod`)

```bash
RUST_LOG=info
REDIS_URL=redis://redis-prod:6379
CONFIG_FILE=/config/config.yml
MAPPINGS_DIR=/config/mappings
```

## 监控和运维

### 1. 健康检查

```bash
# 检查服务状态
curl http://localhost:8092/health

# 检查Redis连接
docker exec modsrv redis-cli -h redis ping

# 检查容器状态
docker-compose ps
```

### 2. 日志管理

#### 日志配置

```yaml
# docker-compose.yml中的日志配置
logging:
  driver: "json-file"
  options:
    max-size: "50m"
    max-file: "5"
```

#### 日志查看

```bash
# 查看实时日志
docker-compose logs -f modsrv

# 查看最近的日志
docker-compose logs --tail=100 modsrv

# 查看特定时间段的日志
docker-compose logs --since="2025-01-01T00:00:00" modsrv
```

### 3. 性能监控

#### 指标收集

```bash
# CPU和内存使用情况
docker stats modsrv

# 容器资源限制
docker update --memory=2g --cpus="1.5" modsrv

# Redis内存使用
docker exec redis redis-cli info memory
```

#### Prometheus监控

```yaml
# 添加到docker-compose.yml
  prometheus:
    image: prom/prometheus
    ports:
      - "9090:9090"
    volumes:
      - ./config/prometheus.yml:/etc/prometheus/prometheus.yml

  grafana:
    image: grafana/grafana
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
```

## 备份和恢复

### 1. 数据备份

#### Redis数据备份

```bash
# 创建Redis备份
docker exec redis redis-cli BGSAVE

# 复制备份文件
docker cp redis:/data/dump.rdb ./backups/redis-$(date +%Y%m%d).rdb
```

#### 配置文件备份

```bash
# 备份配置目录
tar -czf backups/config-$(date +%Y%m%d).tar.gz config/

# 备份日志（可选）
tar -czf backups/logs-$(date +%Y%m%d).tar.gz logs/
```

### 2. 灾难恢复

#### 服务恢复

```bash
# 停止服务
docker-compose down

# 恢复Redis数据
docker cp ./backups/redis-20250101.rdb redis:/data/dump.rdb

# 恢复配置文件
tar -xzf backups/config-20250101.tar.gz

# 重启服务
docker-compose up -d
```

## 安全配置

### 1. 网络安全

#### 防火墙配置

```bash
# 只允许必要端口
sudo ufw allow 8092/tcp
sudo ufw enable
```

#### SSL/TLS配置

```yaml
# 使用nginx作为反向代理
  nginx:
    image: nginx:alpine
    ports:
      - "443:443"
      - "80:80"
    volumes:
      - ./config/nginx.conf:/etc/nginx/nginx.conf
      - ./ssl:/etc/nginx/ssl
```

### 2. 访问控制

#### Redis安全配置

```conf
# config/redis/redis.conf
requirepass your_secure_password
bind 127.0.0.1
```

#### API访问控制

```yaml
# 环境变量中设置API密钥
environment:
  - API_KEY=your_api_key
  - ALLOWED_ORIGINS=https://your-domain.com
```

## 扩展部署

### 1. 高可用部署

#### Redis集群

```yaml
  redis-master:
    image: redis:8-alpine
    command: redis-server --appendonly yes --replica-announce-ip redis-master

  redis-replica:
    image: redis:8-alpine
    command: redis-server --appendonly yes --replicaof redis-master 6379
```

#### ModSrv多实例

```yaml
  modsrv-1:
    extends: modsrv
    environment:
      - INSTANCE_ID=1

  modsrv-2:
    extends: modsrv
    environment:
      - INSTANCE_ID=2
```

### 2. 负载均衡

```yaml
  nginx-lb:
    image: nginx:alpine
    ports:
      - "8092:8092"
    volumes:
      - ./config/nginx-lb.conf:/etc/nginx/nginx.conf
    depends_on:
      - modsrv-1
      - modsrv-2
```

## 故障排查

### 1. 常见问题

#### 服务无法启动

```bash
# 查看详细错误信息
docker-compose logs modsrv

# 检查配置文件语法
docker run --rm -v $(pwd)/config:/config modsrv:v2.0 modsrv validate-config
```

#### Redis连接失败

```bash
# 检查Redis服务
docker-compose ps redis

# 测试连接
docker exec modsrv redis-cli -h redis ping
```

#### API无响应

```bash
# 检查端口绑定
docker port modsrv

# 测试健康检查
curl -v http://localhost:8092/health
```

### 2. 调试模式

```yaml
# 开启调试模式
environment:
  - RUST_LOG=debug
  - RUST_BACKTRACE=1
```

## 升级部署

### 1. 滚动升级

```bash
# 1. 构建新版本镜像
docker build -t modsrv:v2.1 .

# 2. 更新docker-compose.yml
sed -i 's/modsrv:v2.0/modsrv:v2.1/g' docker-compose.yml

# 3. 重新部署
docker-compose up -d

# 4. 验证升级
curl http://localhost:8092/health
```

### 2. 蓝绿部署

```bash
# 1. 部署绿色环境
docker-compose -f docker-compose.green.yml up -d

# 2. 验证绿色环境
curl http://localhost:8093/health

# 3. 切换流量
# 更新负载均衡器配置

# 4. 停止蓝色环境
docker-compose down
```

## 最佳实践

1. **资源限制**: 为容器设置合适的CPU和内存限制
2. **日志轮转**: 配置日志轮转避免磁盘空间耗尽
3. **监控告警**: 设置关键指标的监控和告警
4. **定期备份**: 建立自动化的数据备份策略
5. **安全更新**: 定期更新基础镜像和依赖包
6. **文档维护**: 保持部署文档的及时更新