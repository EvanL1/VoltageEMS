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

# 通信服务测试工具集

本工具集为VoltageEMS通信服务(comsrv)提供了一系列测试和模拟工具，帮助开发、测试和部署通信服务。

## 工具列表

- **test_api.py** - API测试脚本，用于测试通信服务的REST API接口
- **load_test.py** - 负载测试脚本，用于对通信服务进行压力测试
- **modbus_simulator.py** - Modbus协议模拟器，模拟Modbus TCP服务器
- **opcua_simulator.py** - OPC UA协议模拟器，模拟OPC UA服务器
- **generate_config.py** - 配置生成工具，用于生成通道和点位配置

## 安装依赖

在使用这些工具之前，请确保已安装所需的依赖包：

```bash
# 通用依赖
pip install requests

# Modbus模拟器依赖
pip install pymodbus

# OPC UA模拟器依赖
pip install opcua
```

## 工具使用方法

### API测试脚本 (test_api.py)

测试通信服务的REST API接口，包括健康检查、通道管理、点位管理和数据读写等功能。

```bash
python test_api.py
```

脚本会自动执行一系列API测试，并显示测试结果。

### 负载测试脚本 (load_test.py)

对通信服务进行压力测试，模拟大量并发请求。

```bash
# 基本用法
python load_test.py

# 自定义参数
python load_test.py --url http://localhost:8080/api --threads 20 --requests 2000 --read-ratio 70
```

参数说明：
- `--url` - API基础URL，默认为http://localhost:8080/api
- `--threads` - 并发线程数，默认为10
- `--requests` - 总请求数，默认为1000
- `--timeout` - 请求超时时间(秒)，默认为5秒
- `--read-ratio` - 读取操作的百分比，默认为80%

### Modbus模拟器 (modbus_simulator.py)

模拟Modbus TCP服务器，为通信服务提供测试数据源。

```bash
# 基本用法
python modbus_simulator.py

# 自定义参数
python modbus_simulator.py --host 0.0.0.0 --port 502 --slave-id 1 --update-interval 2.0
```

参数说明：
- `--host` - 监听主机地址，默认为0.0.0.0
- `--port` - 监听端口，默认为502
- `--slave-id` - 从站ID，默认为1
- `--no-auto-update` - 禁用自动更新寄存器值
- `--update-interval` - 自动更新间隔(秒)，默认为1.0秒

### OPC UA模拟器 (opcua_simulator.py)

模拟OPC UA服务器，为通信服务提供测试数据源。

```bash
# 基本用法
python opcua_simulator.py

# 自定义参数
python opcua_simulator.py --host 0.0.0.0 --port 4840 --update-interval 2.0
```

参数说明：
- `--host` - 监听主机地址，默认为0.0.0.0
- `--port` - 监听端口，默认为4840
- `--namespace` - 命名空间URI，默认为http://voltage.com/opcua/simulator
- `--no-auto-update` - 禁用自动更新节点值
- `--update-interval` - 自动更新间隔(秒)，默认为1.0秒

### 配置生成工具 (generate_config.py)

生成通信服务的通道和点位配置文件，用于测试和部署。

```bash
# 基本用法
python generate_config.py

# 自定义参数
python generate_config.py --output ./my_config --modbus 3 --opcua 2 --points 30
```

参数说明：
- `--output` - 输出目录，默认为./config
- `--modbus` - Modbus通道数量，默认为2
- `--opcua` - OPC UA通道数量，默认为2
- `--points` - 每个通道的点位数量，默认为20

## 典型测试流程

1. 使用配置生成工具生成测试配置文件：
   ```bash
   python generate_config.py --output ./test_config
   ```

2. 启动协议模拟器：
   ```bash
   # 终端1: 启动Modbus模拟器
   python modbus_simulator.py --port 502
   
   # 终端2: 启动OPC UA模拟器
   python opcua_simulator.py --port 4840
   ```

3. 启动通信服务，指定配置目录：
   ```bash
   # 终端3: 启动通信服务
   cd ../
   cargo run --bin comsrv -- --config-dir ./test_tools/test_config
   ```

4. 使用API测试脚本测试功能：
   ```bash
   # 终端4: 执行API测试
   python test_api.py
   ```

5. 执行负载测试：
   ```bash
   # 终端5: 执行负载测试
   python load_test.py --threads 20 --requests 5000
   ```

## 注意事项

- 确保通信服务已正确配置并运行，默认API端口为8080
- Modbus模拟器默认使用502端口，这在某些系统上可能需要管理员权限
- 对于真实环境中的部署，请根据实际情况调整配置参数
- 负载测试时请注意监控系统资源使用情况，避免过载