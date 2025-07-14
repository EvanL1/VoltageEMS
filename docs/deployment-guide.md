# VoltageEMS 部署指南

## 部署架构

### 单机部署
适用于开发测试和小规模应用。

```
┌─────────────────────────────────────┐
│         单机服务器                    │
│  ┌─────────┐  ┌─────────┐          │
│  │  Redis  │  │InfluxDB │          │
│  └─────────┘  └─────────┘          │
│  ┌─────────────────────────┐       │
│  │    VoltageEMS 服务       │       │
│  │ comsrv|modsrv|hissrv... │       │
│  └─────────────────────────┘       │
└─────────────────────────────────────┘
```

### 分布式部署
适用于生产环境和大规模应用。

```
┌────────────────┐  ┌────────────────┐  ┌────────────────┐
│   数据采集层    │  │   处理层        │  │   存储层        │
│  ┌──────────┐  │  │  ┌──────────┐  │  │  ┌──────────┐  │
│  │ comsrv-1 │  │  │  │ modsrv-1 │  │  │  │  Redis   │  │
│  │ comsrv-2 │  │  │  │ modsrv-2 │  │  │  │ Cluster  │  │
│  │ comsrv-N │  │  │  │ alarmsrv │  │  │  │          │  │
│  └──────────┘  │  │  └──────────┘  │  │  └──────────┘  │
└────────────────┘  └────────────────┘  └────────────────┘
```

## Docker 部署

### 1. 使用 Docker Compose

创建 `docker-compose.yml`：

```yaml
version: '3.8'

services:
  redis:
    image: redis:7-alpine
    container_name: voltageems-redis
    ports:
      - "6379:6379"
    volumes:
      - redis-data:/data
    command: redis-server --appendonly yes
    networks:
      - voltageems

  influxdb:
    image: influxdb:2.7
    container_name: voltageems-influxdb
    ports:
      - "8086:8086"
    volumes:
      - influxdb-data:/var/lib/influxdb2
    environment:
      - DOCKER_INFLUXDB_INIT_MODE=setup
      - DOCKER_INFLUXDB_INIT_USERNAME=admin
      - DOCKER_INFLUXDB_INIT_PASSWORD=voltageems123
      - DOCKER_INFLUXDB_INIT_ORG=voltageems
      - DOCKER_INFLUXDB_INIT_BUCKET=telemetry
    networks:
      - voltageems

  comsrv:
    image: voltageems/comsrv:latest
    container_name: voltageems-comsrv
    depends_on:
      - redis
    environment:
      - RUST_LOG=info
      - REDIS_URL=redis://redis:6379
    volumes:
      - ./config/comsrv:/app/config
      - ./logs/comsrv:/app/logs
    networks:
      - voltageems
    restart: unless-stopped

  modsrv:
    image: voltageems/modsrv:latest
    container_name: voltageems-modsrv
    depends_on:
      - redis
    environment:
      - RUST_LOG=info
      - REDIS_URL=redis://redis:6379
    volumes:
      - ./config/modsrv:/app/config
      - ./logs/modsrv:/app/logs
    networks:
      - voltageems
    restart: unless-stopped

  hissrv:
    image: voltageems/hissrv:latest
    container_name: voltageems-hissrv
    depends_on:
      - redis
      - influxdb
    environment:
      - RUST_LOG=info
      - REDIS_URL=redis://redis:6379
      - INFLUXDB_URL=http://influxdb:8086
      - INFLUXDB_TOKEN=${INFLUXDB_TOKEN}
    volumes:
      - ./logs/hissrv:/app/logs
    networks:
      - voltageems
    restart: unless-stopped

  apigateway:
    image: voltageems/apigateway:latest
    container_name: voltageems-apigateway
    depends_on:
      - redis
    ports:
      - "8080:8080"
    environment:
      - RUST_LOG=info
      - REDIS_URL=redis://redis:6379
    networks:
      - voltageems
    restart: unless-stopped

volumes:
  redis-data:
  influxdb-data:

networks:
  voltageems:
    driver: bridge
```

启动服务：

```bash
# 启动所有服务
docker-compose up -d

# 查看日志
docker-compose logs -f

# 停止服务
docker-compose down

# 清理数据
docker-compose down -v
```

### 2. 构建 Docker 镜像

创建 `Dockerfile`：

```dockerfile
# 多阶段构建
FROM rust:1.75 as builder

WORKDIR /app

# 缓存依赖
COPY Cargo.toml Cargo.lock ./
COPY services/comsrv/Cargo.toml services/comsrv/
COPY libs/voltage-common/Cargo.toml libs/voltage-common/

# 创建虚拟 main.rs 以缓存依赖
RUN mkdir -p services/comsrv/src && \
    echo "fn main() {}" > services/comsrv/src/main.rs && \
    mkdir -p libs/voltage-common/src && \
    echo "" > libs/voltage-common/src/lib.rs

# 构建依赖
RUN cargo build --release -p comsrv

# 复制源代码
COPY . .

# 重新构建
RUN touch services/comsrv/src/main.rs && \
    cargo build --release -p comsrv

# 运行时镜像
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 复制二进制文件
COPY --from=builder /app/target/release/comsrv /app/

# 复制配置文件
COPY services/comsrv/config /app/config

# 创建日志目录
RUN mkdir -p /app/logs

EXPOSE 8080

CMD ["./comsrv"]
```

构建镜像：

```bash
# 构建单个服务
docker build -f services/comsrv/Dockerfile -t voltageems/comsrv:latest .

# 构建所有服务
./scripts/build-all-images.sh
```

## Kubernetes 部署

### 1. 创建命名空间

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: voltageems
```

### 2. Redis 部署

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: redis
  namespace: voltageems
spec:
  replicas: 1
  selector:
    matchLabels:
      app: redis
  template:
    metadata:
      labels:
        app: redis
    spec:
      containers:
      - name: redis
        image: redis:7-alpine
        ports:
        - containerPort: 6379
        volumeMounts:
        - name: redis-data
          mountPath: /data
      volumes:
      - name: redis-data
        persistentVolumeClaim:
          claimName: redis-pvc
---
apiVersion: v1
kind: Service
metadata:
  name: redis
  namespace: voltageems
spec:
  selector:
    app: redis
  ports:
  - port: 6379
    targetPort: 6379
```

### 3. comsrv 部署

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: comsrv
  namespace: voltageems
spec:
  replicas: 3
  selector:
    matchLabels:
      app: comsrv
  template:
    metadata:
      labels:
        app: comsrv
    spec:
      containers:
      - name: comsrv
        image: voltageems/comsrv:latest
        env:
        - name: RUST_LOG
          value: "info"
        - name: REDIS_URL
          value: "redis://redis:6379"
        volumeMounts:
        - name: config
          mountPath: /app/config
        - name: logs
          mountPath: /app/logs
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"
      volumes:
      - name: config
        configMap:
          name: comsrv-config
      - name: logs
        emptyDir: {}
```

### 4. ConfigMap 配置

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: comsrv-config
  namespace: voltageems
data:
  default.yml: |
    service:
      name: comsrv
      id: "${HOSTNAME}"
    
    redis:
      url: "redis://redis:6379"
      key_prefix: "voltage"
    
    channels:
      - id: 1001
        name: "主站通道"
        protocol_type: "modbus_tcp"
        transport:
          type: "tcp"
          host: "192.168.1.100"
          port: 502
```

### 5. Ingress 配置

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: voltageems-ingress
  namespace: voltageems
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /
spec:
  rules:
  - host: api.voltageems.local
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: apigateway
            port:
              number: 8080
```

### 6. 部署脚本

```bash
# 应用所有配置
kubectl apply -f k8s/

# 检查部署状态
kubectl get pods -n voltageems

# 查看日志
kubectl logs -f deployment/comsrv -n voltageems

# 扩容
kubectl scale deployment/comsrv --replicas=5 -n voltageems
```

## systemd 部署

### 1. 创建服务文件

创建 `/etc/systemd/system/voltageems-comsrv.service`：

```ini
[Unit]
Description=VoltageEMS Communication Service
After=network.target redis.service
Requires=redis.service

[Service]
Type=simple
User=voltageems
Group=voltageems
WorkingDirectory=/opt/voltageems/comsrv
Environment="RUST_LOG=info"
Environment="REDIS_URL=redis://localhost:6379"
ExecStart=/opt/voltageems/comsrv/comsrv
Restart=always
RestartSec=10

# 安全设置
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/voltageems/comsrv/logs /opt/voltageems/comsrv/data

# 资源限制
LimitNOFILE=65535
LimitNPROC=4096

[Install]
WantedBy=multi-user.target
```

### 2. 安装脚本

```bash
#!/bin/bash
# install.sh

# 创建用户
sudo useradd -r -s /bin/false voltageems

# 创建目录
sudo mkdir -p /opt/voltageems/{comsrv,modsrv,hissrv}/{bin,config,logs,data}
sudo chown -R voltageems:voltageems /opt/voltageems

# 复制文件
sudo cp target/release/comsrv /opt/voltageems/comsrv/bin/
sudo cp -r services/comsrv/config/* /opt/voltageems/comsrv/config/

# 安装服务
sudo cp scripts/systemd/*.service /etc/systemd/system/
sudo systemctl daemon-reload

# 启动服务
sudo systemctl enable voltageems-comsrv
sudo systemctl start voltageems-comsrv

# 检查状态
sudo systemctl status voltageems-comsrv
```

## 监控配置

### 1. Prometheus 指标

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'voltageems'
    static_configs:
      - targets:
        - 'comsrv:9090'
        - 'modsrv:9091'
        - 'hissrv:9092'
```

### 2. Grafana 仪表板

导入预配置的仪表板：

```bash
# 导入仪表板
curl -X POST http://admin:admin@localhost:3000/api/dashboards/db \
  -H 'Content-Type: application/json' \
  -d @grafana/dashboards/voltageems.json
```

### 3. 日志聚合

使用 Loki 收集日志：

```yaml
# loki-config.yml
positions:
  filename: /tmp/positions.yaml

clients:
  - url: http://loki:3100/loki/api/v1/push

scrape_configs:
  - job_name: voltageems
    static_configs:
      - targets:
          - localhost
        labels:
          job: voltageems
          __path__: /opt/voltageems/*/logs/*.log
```

## 性能优化

### 1. Redis 优化

```bash
# /etc/redis/redis.conf
maxmemory 4gb
maxmemory-policy allkeys-lru
save ""
appendonly yes
appendfsync everysec
```

### 2. 系统优化

```bash
# /etc/sysctl.conf
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 65535
net.ipv4.ip_local_port_range = 1024 65535
net.ipv4.tcp_tw_reuse = 1
fs.file-max = 1000000
```

### 3. 服务配置

```yaml
# 优化配置
performance:
  worker_threads: 8
  max_connections: 10000
  buffer_size: 65536
  
redis:
  pool_size: 32
  connection_timeout: 5s
  
cache:
  max_entries: 100000
  ttl: 300s
```

## 备份与恢复

### 1. 数据备份

```bash
#!/bin/bash
# backup.sh

BACKUP_DIR="/backup/voltageems/$(date +%Y%m%d)"
mkdir -p $BACKUP_DIR

# 备份 Redis
redis-cli --rdb $BACKUP_DIR/redis.rdb

# 备份 InfluxDB
influx backup $BACKUP_DIR/influxdb

# 备份配置
tar -czf $BACKUP_DIR/config.tar.gz /opt/voltageems/*/config

# 清理旧备份
find /backup/voltageems -type d -mtime +30 -exec rm -rf {} \;
```

### 2. 恢复流程

```bash
#!/bin/bash
# restore.sh

BACKUP_DIR=$1

# 停止服务
systemctl stop voltageems-*

# 恢复 Redis
redis-cli --pipe < $BACKUP_DIR/redis.rdb

# 恢复 InfluxDB
influx restore $BACKUP_DIR/influxdb

# 恢复配置
tar -xzf $BACKUP_DIR/config.tar.gz -C /

# 启动服务
systemctl start voltageems-*
```

## 故障排查

### 1. 健康检查

```bash
# 检查服务状态
curl http://localhost:8080/health

# 检查 Redis 连接
redis-cli ping

# 检查日志
journalctl -u voltageems-comsrv -f
```

### 2. 常见问题

#### Redis 连接失败
```bash
# 检查 Redis 服务
systemctl status redis

# 检查防火墙
sudo firewall-cmd --list-ports

# 测试连接
redis-cli -h localhost -p 6379 ping
```

#### 内存不足
```bash
# 查看内存使用
free -h

# 查看进程内存
ps aux | grep voltageems

# 调整内存限制
systemctl edit voltageems-comsrv
# 添加 MemoryLimit=2G
```

## 安全加固

### 1. 网络隔离

```bash
# 创建专用网络
docker network create --driver bridge \
  --subnet=172.20.0.0/16 \
  --opt com.docker.network.bridge.name=voltageems0 \
  voltageems-net
```

### 2. TLS 配置

```yaml
# TLS 配置
tls:
  enabled: true
  cert_file: /etc/voltageems/certs/server.crt
  key_file: /etc/voltageems/certs/server.key
  ca_file: /etc/voltageems/certs/ca.crt
```

### 3. 访问控制

```yaml
# API 访问控制
auth:
  enabled: true
  jwt_secret: ${JWT_SECRET}
  token_expiry: 24h
```

## 升级流程

### 1. 滚动升级

```bash
# Kubernetes 滚动升级
kubectl set image deployment/comsrv comsrv=voltageems/comsrv:v1.2.0 -n voltageems

# 检查升级状态
kubectl rollout status deployment/comsrv -n voltageems
```

### 2. 蓝绿部署

```bash
# 部署新版本（绿）
docker-compose -f docker-compose.green.yml up -d

# 切换流量
./scripts/switch-to-green.sh

# 验证后删除旧版本（蓝）
docker-compose -f docker-compose.blue.yml down
```

## 监控告警

### 告警规则示例

```yaml
# prometheus-alerts.yml
groups:
  - name: voltageems
    rules:
      - alert: ServiceDown
        expr: up{job="voltageems"} == 0
        for: 5m
        annotations:
          summary: "服务 {{ $labels.instance }} 已下线"
          
      - alert: HighMemoryUsage
        expr: process_resident_memory_bytes / 1024 / 1024 > 1000
        for: 10m
        annotations:
          summary: "服务 {{ $labels.instance }} 内存使用超过 1GB"
```