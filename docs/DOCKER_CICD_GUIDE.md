# VoltageEMS Docker CI/CD 简明指南

## 概述

本指南介绍如何使用Docker和Jenkins在边缘服务器上部署VoltageEMS系统。整个流程简单直接，适合单机或小规模集群部署。

## 快速开始

### 1. 环境准备

```bash
# 安装Docker
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker $USER

# 安装Docker Compose
sudo curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
sudo chmod +x /usr/local/bin/docker-compose

# 启动本地Docker Registry
docker run -d -p 5000:5000 --restart=always --name registry registry:2
```

### 2. 手动构建和部署

```bash
# 构建所有镜像
./scripts/build-all.sh 1.0.0

# 部署到生产环境
sudo ./scripts/deploy.sh production 1.0.0

# 查看状态
docker-compose -f /opt/voltageems/docker-compose.yml ps
```

### 3. Jenkins自动化

Jenkins会自动执行以下流程：

1. 拉取代码
2. 构建Docker镜像
3. 运行测试
4. 推送到Registry
5. 部署到服务器

## 核心脚本说明

### build-all.sh

构建所有服务的Docker镜像。

```bash
# 使用方法
./scripts/build-all.sh [版本号] [Registry地址]

# 示例
./scripts/build-all.sh 1.2.3 localhost:5000
```

### deploy.sh

部署应用到目标环境。

```bash
# 使用方法
sudo ./scripts/deploy.sh [环境] [版本号] [Registry地址]

# 示例
sudo ./scripts/deploy.sh production latest
sudo ./scripts/deploy.sh staging 1.2.3 192.168.1.100:5000
```

### rollback.sh

回滚到之前的版本。

```bash
# 使用方法
sudo ./scripts/rollback.sh

# 脚本会列出可用的备份供选择
```

## Docker Compose配置

### 生产环境配置 (docker-compose.prod.yml)

包含所有必要的服务：

- 7个业务服务（comsrv, modsrv, hissrv, netsrv, alarmsrv, apigateway, frontend）
- Redis（数据总线）
- InfluxDB（时序数据库）
- Grafana（可选，监控面板）

### 测试环境配置 (docker-compose.test.yml)

最小化配置，用于集成测试：

- Redis测试实例
- 核心服务测试实例

## 服务端口映射

| 服务        | 容器端口 | 主机端口 | 说明         |
| ----------- | -------- | -------- | ------------ |
| Frontend    | 80       | 80       | Web前端      |
| API Gateway | 8080     | 8080     | API入口      |
| Comsrv      | 8080     | 8081     | 通信服务     |
| Modsrv      | 8080     | 8082     | 模型服务     |
| Hissrv      | 8080     | 8083     | 历史数据服务 |
| Netsrv      | 8080     | 8084     | 网络服务     |
| Alarmsrv    | 8080     | 8085     | 告警服务     |
| Redis       | 6379     | 6379     | 数据总线     |
| InfluxDB    | 8086     | 8086     | 时序数据库   |
| Grafana     | 3000     | 3000     | 监控面板     |

## 常用操作

### 查看日志

```bash
# 查看所有服务日志
cd /opt/voltageems
docker-compose logs -f

# 查看特定服务日志
docker-compose logs -f comsrv

# 查看最近100行日志
docker-compose logs --tail=100 apigateway
```

### 重启服务

```bash
# 重启单个服务
cd /opt/voltageems
docker-compose restart comsrv

# 重启所有服务
docker-compose restart
```

### 更新配置

```bash
# 编辑配置文件
sudo vim /opt/voltageems/config/comsrv.yaml

# 重启服务使配置生效
cd /opt/voltageems
docker-compose restart comsrv
```

### 数据备份

```bash
# 备份Redis数据
docker exec voltageems_redis redis-cli BGSAVE

# 备份InfluxDB数据
docker exec voltageems_influxdb influx backup /backups

# 备份所有配置
sudo tar -czf voltageems-config-backup.tar.gz /opt/voltageems/config
```

## 监控和健康检查

### 健康检查端点

所有服务都提供健康检查端点：

```bash
# 检查API Gateway
curl http://localhost:8080/health

# 检查其他服务
curl http://localhost:8081/health  # comsrv
curl http://localhost:8082/health  # modsrv
# ...
```

### 使用Grafana监控

1. 访问 http://localhost:3000
2. 默认用户名/密码：admin/voltageems
3. 已预配置InfluxDB和Redis数据源

## 故障排除

### 服务无法启动

```bash
# 检查端口占用
sudo netstat -tlnp | grep :8080

# 检查Docker日志
docker-compose logs comsrv | tail -50

# 检查资源使用
docker stats
```

### 内存不足

```bash
# 清理未使用的镜像和容器
docker system prune -a

# 检查磁盘空间
df -h

# 限制服务内存使用（修改docker-compose.yml）
```

### 网络问题

```bash
# 检查Docker网络
docker network ls
docker network inspect voltageems_network

# 测试服务间连接
docker exec voltageems_comsrv ping redis
```

## 安全建议

1. **修改默认密码**

   - InfluxDB: 修改admin密码和token
   - Grafana: 修改admin密码
   - Redis: 配置密码认证
2. **网络隔离**

   - 仅暴露必要的端口
   - 使用防火墙限制访问
   - 考虑使用VPN访问
3. **数据加密**

   - 使用TLS加密传输
   - 加密敏感配置文件

## 性能优化

### Docker配置

```bash
# 增加Docker守护进程内存限制
sudo vim /etc/docker/daemon.json
{
  "log-driver": "json-file",
  "log-opts": {
    "max-size": "10m",
    "max-file": "3"
  }
}
```

### 服务优化

- 调整服务的CPU和内存限制
- 配置合适的并发连接数
- 启用日志轮转防止磁盘占满

## 维护计划

### 日常维护

- 每日检查服务状态和日志
- 监控资源使用情况
- 备份重要数据

### 定期维护

- 每周更新Docker镜像
- 每月清理旧日志和镜像
- 每季度进行性能分析和优化

## 附录：完整部署示例

```bash
# 1. 克隆代码
git clone https://github.com/voltageems/voltageems.git
cd voltageems

# 2. 构建镜像
./scripts/build-all.sh 1.0.0

# 3. 准备配置
sudo mkdir -p /opt/voltageems/config
sudo cp -r config/* /opt/voltageems/config/

# 4. 部署应用
sudo ./scripts/deploy.sh production 1.0.0

# 5. 验证部署
curl http://localhost:8080/health
curl http://localhost/

# 6. 查看日志
docker-compose -f /opt/voltageems/docker-compose.yml logs -f
```
