# ModSrv Docker 测试环境

本文档描述如何在Docker环境中全面测试ModSrv服务的API功能。

## 测试套件概述

测试套件包含以下组件：

1. **测试脚本 (test-api.py)** - 全面测试ModSrv API端点，包括健康检查、规则管理、规则执行等
2. **Docker测试环境** - 通过Docker Compose配置的测试环境，包含Redis、ModSrv服务和测试容器
3. **便捷执行脚本** - 提供简单的命令行接口来执行测试

## 测试内容

测试脚本涵盖以下API功能：

- 健康检查API
- 规则管理API（列表、创建、获取、更新、删除）
- 规则执行API，包含简单规则和复杂DAG结构规则
- 模板API和控制操作API（如果可用）
- DAG规则在不同场景下的执行

## 环境要求

- Docker (20.10+)
- Docker Compose (1.29+)
- Bash shell环境

## 快速开始

1. 确保Docker和Docker Compose已安装
2. 克隆仓库并进入modsrv目录
3. 执行测试脚本：

```bash
# 赋予脚本执行权限
chmod +x run-docker-tests.sh

# 运行测试（构建镜像并在测试完成后清理环境）
./run-docker-tests.sh --build --clean
```

## 详细使用方法

### 执行脚本选项

`run-docker-tests.sh` 脚本支持以下选项：

```
用法: ./run-docker-tests.sh [选项]
选项:
  -h, --help     显示帮助信息
  -b, --build    重新构建镜像（默认使用已有镜像）
  -d, --detach   在后台运行容器
  -c, --clean    测试后清理（删除容器和网络）
  -l, --logs     显示modsrv服务的日志
  --debug        调试模式，显示更多信息
```

### 使用示例

```bash
# 重新构建镜像并在测试完成后自动清理
./run-docker-tests.sh --build --clean

# 在后台运行测试，并查看服务日志
./run-docker-tests.sh --detach --logs

# 调试模式
./run-docker-tests.sh --debug
```

### 手动执行测试

如果您想手动控制测试过程，可以直接使用Docker Compose命令：

```bash
# 构建镜像
docker-compose -f docker-compose.test.yml build

# 启动所有服务
docker-compose -f docker-compose.test.yml up -d

# 查看测试日志
docker logs -f modsrv-tester

# 查看服务日志
docker logs -f modsrv-service

# 停止并清理环境
docker-compose -f docker-compose.test.yml down -v
```

## 测试结果解读

测试完成后，测试脚本将显示详细的测试摘要，包括：

- 总测试数
- 通过的测试数
- 失败的测试数
- 跳过的测试数
- 成功率

如果所有测试都通过，脚本将以退出码0结束；否则，它将以非零退出码结束。

## 自定义测试环境

### 修改服务配置

可以通过编辑`docker-compose.test.yml`文件来修改服务配置，如Redis配置、ModSrv端口等。

### 修改测试参数

测试容器支持以下环境变量：

- `MODSRV_HOST`: ModSrv服务主机名（默认：modsrv）
- `MODSRV_PORT`: ModSrv服务端口（默认：8000）

可以在`docker-compose.test.yml`文件中修改这些环境变量。

## 在CI/CD中集成

此测试套件可以轻松集成到CI/CD流程中。示例GitHub Actions配置：

```yaml
name: ModSrv API Tests

on:
  push:
    branches: [ main, develop ]
    paths:
      - 'services/modsrv/**'
  pull_request:
    branches: [ main, develop ]
    paths:
      - 'services/modsrv/**'

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v1
      
      - name: Run ModSrv API Tests
        run: |
          cd services/modsrv
          chmod +x run-docker-tests.sh
          ./run-docker-tests.sh --build
```

## 问题排查

### 服务无法启动

如果ModSrv服务无法启动，可能是由于：

1. Redis连接问题 - 检查Redis服务是否运行正常
2. 配置问题 - 检查环境变量和配置文件
3. 端口冲突 - 确保端口8000和6379未被占用

可以通过查看服务日志来排查问题：

```bash
docker logs modsrv-service
```

### 测试失败

如果测试失败，可以通过以下方式获取更多信息：

1. 查看测试日志：`docker logs modsrv-tester`
2. 使用调试模式：`./run-docker-tests.sh --debug`
3. 修改测试脚本中的超时设置和重试次数 