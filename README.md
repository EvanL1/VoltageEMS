# 能源管理系统 (EMS)

能源管理系统是一个用于监控、控制和优化能源系统的综合平台。该系统由多个微服务组成，每个微服务负责特定的功能。

## 服务组件

- **Comsrv**: 通信服务，负责与设备通信，采集实时数据
- **Hissrv**: 历史数据服务，负责将实时数据存储到时序数据库
- **modsrv**: 模型服务，负责执行实时模型计算和控制策略
- **netsrv**: 网络服务，负责将数据通过多种协议上送到外部系统
- **前端配置管理平台**: 基于 Vue.js 的 Web 应用，用于管理各服务的配置文件
- **API 服务**: 为前端提供配置文件读写接口
- **Grafana**: 数据可视化平台，嵌入到前端应用中

## 系统架构

系统采用微服务架构，各服务通过 Redis 进行数据交换：

```
+--------+      +--------+      +--------+      +--------+
| Comsrv | <--> |        | <--> | modsrv | <--> | netsrv |
+--------+      |        |      +--------+      +--------+
                | Redis  |
+--------+      |        |      +--------+      +--------+
| Hissrv | <--> |        | <--> |  API   | <--> |前端应用|
+--------+      +--------+      +--------+      +--------+
     |                                               |
     v                                               v
+--------+                                      +--------+
|InfluxDB|                                      | Grafana|
+--------+                                      +--------+
```

## 技术栈

- **Comsrv**: C++
- **Hissrv**: Rust
- **modsrv**: Rust
- **netsrv**: Rust
- **前端应用**: Vue.js, Element Plus
- **API 服务**: Node.js, Express
- **数据存储**: Redis, InfluxDB
- **数据可视化**: Grafana
- **容器化**: Docker, Docker Compose

## 快速开始

### 前提条件

- Docker 和 Docker Compose
- Rust 1.67 或更高版本 (开发时需要)
- C++ 编译器 (开发 Comsrv 时需要)
- Node.js 16 或更高版本 (开发前端和 API 时需要)

### 使用 Docker Compose 启动

```bash
# 启动所有服务
docker-compose up -d

# 查看日志
docker-compose logs -f

# 停止所有服务
docker-compose down
```

### 访问服务

- **前端配置管理平台**: http://localhost:8080
- **Grafana**: http://localhost:8080/grafana (或直接访问 http://localhost:3000)
- **InfluxDB 管理界面**: http://localhost:8086

### 开发环境设置

每个服务目录下都有详细的开发指南，请参考各自的 README.md 文件。

#### 前端开发

```bash
cd frontend
npm install
npm run serve
```

#### API 服务开发

```bash
cd api
npm install
npm run dev
```

## 配置

所有服务的配置文件统一存放在 `config` 目录下，按服务名称分类：

- **Comsrv**: `config/comsrv/`
- **Hissrv**: `config/hissrv/`
- **modsrv**: `config/modsrv/modsrv.toml`
- **netsrv**: `config/netsrv/netsrv.json`
- **Mosquitto**: `config/mosquitto/mosquitto.conf`
- **证书**: `config/certs/`

这种集中管理配置文件的方式使得系统配置更加清晰和易于维护。

### 配置管理平台

系统提供了一个基于 Web 的配置管理平台，可以通过浏览器直接修改各服务的配置文件。该平台具有以下特点：

1. **直观的用户界面**: 使用 Element Plus 组件库，提供美观、易用的界面
2. **实时编辑**: 可以实时编辑配置文件，并保存到服务器
3. **配置验证**: 对配置文件进行基本的格式和内容验证
4. **数据可视化**: 集成 Grafana，提供系统运行数据的可视化展示

## 许可证

[您的许可证]