# 能源管理系统 (EMS)

能源管理系统是一个用于监控、控制和优化能源系统的综合平台。该系统由多个微服务组成，每个微服务负责特定的功能。

## 服务组件

- **Comsrv**: 通信服务，负责与设备通信，采集实时数据
- **Hissrv**: 历史数据服务，负责将实时数据存储到时序数据库
- **modsrv**: 模型服务，负责执行实时模型计算和控制策略
- **netsrv**: 网络服务，负责将数据通过多种协议上送到外部系统

## 系统架构

系统采用微服务架构，各服务通过 Redis 进行数据交换：

```
+--------+      +--------+      +--------+      +--------+
| Comsrv | <--> |        | <--> | modsrv | <--> | netsrv |
+--------+      |        |      +--------+      +--------+
                | Redis  |
+--------+      |        |      +--------+
| Hissrv | <--> |        | <--> |  ...   |
+--------+      +--------+      +--------+
     |
     v
+--------+
|InfluxDB|
+--------+
```

## 技术栈

- **Comsrv**: C++
- **Hissrv**: Rust
- **modsrv**: Rust
- **netsrv**: Rust
- **数据存储**: Redis, InfluxDB
- **容器化**: Docker, Docker Compose

## 快速开始

### 前提条件

- Docker 和 Docker Compose
- Rust 1.67 或更高版本 (开发时需要)
- C++ 编译器 (开发 Comsrv 时需要)

### 使用 Docker Compose 启动

```bash
# 启动所有服务
docker-compose up -d

# 查看日志
docker-compose logs -f

# 停止所有服务
docker-compose down
```

### 开发环境设置

每个服务目录下都有详细的开发指南，请参考各自的 README.md 文件。

## 配置

- **Comsrv**: `Comsrv/config/`
- **modsrv**: `modsrv/modsrv.toml`
- **netsrv**: `netsrv/netsrv.json`

## 许可证

[您的许可证] 