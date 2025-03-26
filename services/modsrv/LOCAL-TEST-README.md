# ModSrv 本地测试环境

本文档描述如何使用Docker运行ModSrv服务并在本地Python环境中执行API测试。

## 测试架构

测试采用以下架构：

1. 使用Docker和标准`docker-compose.yml`文件启动ModSrv服务和Redis
2. 在本地Python环境中运行测试脚本`test-api.py`
3. 通过网络连接测试本地Docker容器中的ModSrv API服务

## 环境要求

- Docker (20.10+)
- Docker Compose (1.29+)
- Python 3.6+ (本地安装)
- pip (Python包管理器)
- bash或兼容shell环境

## 快速开始

1. 确保Docker、Docker Compose和Python已安装
2. 克隆仓库并进入modsrv目录
3. 运行测试脚本：

```bash
# 赋予脚本执行权限
chmod +x run-local-tests.sh

# 运行测试（构建镜像并在测试完成后清理环境）
./run-local-tests.sh --build --clean
```

## 脚本功能

`run-local-tests.sh`脚本自动化以下步骤：

1. 检查所需的软件依赖（Docker、Python、pip）
2. 安装测试所需的Python包（requests、pytest等）
3. 启动Docker环境中的ModSrv服务和Redis
4. 等待服务可用并验证健康状态
5. 在本地Python环境中运行测试脚本
6. 汇总测试结果并可选择地清理环境

## 详细使用方法

### 脚本选项

`run-local-tests.sh`脚本支持以下选项：

```
用法: ./run-local-tests.sh [选项]
选项:
  -h, --help     显示帮助信息
  -b, --build    重新构建镜像（默认使用已有镜像）
  -c, --clean    测试后清理（删除容器和网络）
  -l, --logs     显示modsrv服务的日志
  --debug        调试模式，显示更多信息
```

### 使用示例

```bash
# 重新构建镜像并在测试完成后自动清理
./run-local-tests.sh --build --clean

# 运行测试并查看服务日志
./run-local-tests.sh --logs

# 调试模式，显示更多信息
./run-local-tests.sh --debug
```

### 手动执行测试

如果您想手动控制测试过程，可以分步执行：

```bash
# 1. 启动Docker环境
docker-compose up -d

# 2. 等待服务启动
# 可以通过以下命令检查服务健康状态
curl http://localhost:8000/api/health

# 3. 检查服务日志
docker-compose logs -f modsrv

# 4. 安装所需Python包
pip install requests pytest pytest-timeout python-dotenv

# 5. 运行测试脚本
python3 test-api.py

# 6. 测试结束后清理环境
docker-compose down -v
```

## 测试内容

测试脚本`test-api.py`涵盖以下API功能：

- 健康检查API
- 规则管理API（列表、创建、获取、更新、删除）
- 规则执行API，包括简单规则和复杂DAG结构规则
- 模板API和控制操作API（如果可用）
- DAG规则在不同场景下的执行

## 测试结果解读

测试完成后，测试脚本将显示详细的测试摘要，包括：

- 总测试数
- 通过的测试数
- 失败的测试数
- 跳过的测试数
- 成功率

如果所有测试都通过，脚本将以退出码0结束；否则，它将以非零退出码结束。

## 自定义测试

### 修改Docker服务配置

可以通过编辑`docker-compose.yml`文件来修改服务配置，例如：

- 端口映射
- 环境变量
- 数据卷挂载

### 修改测试场景

如果需要调整测试场景或添加新的测试用例，可以直接编辑`test-api.py`文件：

1. 添加新的测试函数
2. 修改现有测试参数
3. 调整重试机制或超时配置

## 问题排查

### 服务无法启动

如果ModSrv服务无法启动，可能是由于：

1. 端口冲突 - 确保端口8000和6379未被占用
2. Docker配置问题 - 检查Docker守护进程是否正常运行
3. 构建失败 - 查看构建日志寻找错误信息

```bash
# 查看端口占用情况
netstat -tuln | grep 8000

# 查看服务日志
docker-compose logs modsrv
```

### 测试失败

如果测试失败，可以尝试以下方法：

1. 增加调试输出：使用`--debug`选项运行脚本
2. 检查API响应：使用curl或Postman手动测试API
3. 调整超时参数：编辑`test-api.py`文件中的超时设置

```bash
# 手动测试健康检查API
curl http://localhost:8000/api/health

# 查看测试脚本中的设置
grep "MAX_RETRIES\|RETRY_INTERVAL\|REQUEST_TIMEOUT" test-api.py
```

### Python依赖问题

如果遇到Python依赖问题，可以尝试：

```bash
# 手动安装依赖
pip install -U requests pytest pytest-timeout python-dotenv

# 检查Python版本
python3 --version
``` 