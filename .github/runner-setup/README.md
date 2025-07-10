# GitHub Actions Self-hosted Runner 部署指南

本目录包含部署和管理GitHub Actions self-hosted runner的所有必要文件。

## 快速开始

### 1. 安装Runner（物理机/虚拟机）

```bash
# 设置环境变量
export GITHUB_ORG=your-org
export GITHUB_REPO=VoltageEMS

# 运行安装脚本
./install-runner.sh

# 检查环境
~/actions-runner/check-env.sh
```

### 2. 配置Runner

获取注册令牌：
1. 访问 https://github.com/YOUR_ORG/VoltageEMS/settings/actions/runners
2. 点击 "New self-hosted runner"
3. 复制令牌

配置runner：
```bash
cd ~/actions-runner
./config.sh --url https://github.com/YOUR_ORG/VoltageEMS \
  --token YOUR_TOKEN \
  --name edge-device-01 \
  --labels self-hosted,linux,arm64,hw-test,gpio,can,serial \
  --work _work
```

### 3. 设置为系统服务

```bash
./setup-runner-service.sh
```

## Docker部署

### 使用Docker Compose

1. 创建 `.env` 文件：
```bash
GITHUB_ORG=your-org
GITHUB_REPO=VoltageEMS
RUNNER_TOKEN=your-runner-token
```

2. 启动runners：
```bash
docker-compose up -d
```

### 单独运行Docker容器

```bash
docker build -t github-runner -f runner.Dockerfile .

docker run -d \
  --name github-runner \
  -e GITHUB_URL=https://github.com/YOUR_ORG/VoltageEMS \
  -e GITHUB_TOKEN=YOUR_TOKEN \
  -e RUNNER_LABELS=self-hosted,linux,x64,docker \
  -v /var/run/docker.sock:/var/run/docker.sock \
  github-runner
```

## Runner标签说明

### 基础标签
- `self-hosted` - 标识为自托管runner
- `linux` - 操作系统
- `x64`/`arm64` - CPU架构

### 硬件能力标签
- `hw-test` - 支持硬件测试
- `gpio` - 支持GPIO接口
- `can` - 支持CAN总线
- `serial` - 支持串口
- `modbus` - 可访问Modbus设备

### 用途标签
- `integration` - 用于集成测试
- `performance` - 用于性能测试
- `production` - 用于生产部署

## 硬件配置

### GPIO访问
```bash
# 添加用户到gpio组
sudo usermod -aG gpio $USER
```

### 串口访问
```bash
# 添加用户到dialout组
sudo usermod -aG dialout $USER
```

### CAN总线
```bash
# 加载CAN模块
sudo modprobe can
sudo modprobe can-raw
sudo modprobe vcan

# 创建虚拟CAN接口
sudo ip link add dev vcan0 type vcan
sudo ip link set up vcan0
```

## 故障排除

### 查看日志
```bash
# 服务日志
sudo journalctl -u actions.runner.* -f

# Runner日志
cd ~/actions-runner
./view-logs.sh
```

### 常见问题

1. **Runner离线**
   - 检查网络连接
   - 检查令牌是否过期
   - 重启服务：`sudo ./svc.sh restart`

2. **权限问题**
   - 确保用户在正确的组中
   - 重新登录以应用组更改

3. **硬件访问失败**
   - 检查硬件是否可用
   - 运行检查脚本：`./scripts/hardware-tests/check-*.sh`

## 维护

### 更新Runner
```bash
cd ~/actions-runner
sudo ./svc.sh stop
# 下载新版本并解压
sudo ./svc.sh start
```

### 清理工作目录
```bash
cd ~/actions-runner
rm -rf _work/*
```

### 移除Runner
```bash
cd ~/actions-runner
sudo ./svc.sh stop
sudo ./svc.sh uninstall
./config.sh remove --token YOUR_TOKEN
```

## 安全建议

1. 仅在受信任的网络中运行self-hosted runner
2. 定期更新runner到最新版本
3. 限制runner的系统权限
4. 使用专用用户运行runner
5. 监控runner的资源使用

## 相关链接

- [GitHub Actions Self-hosted Runners文档](https://docs.github.com/en/actions/hosting-your-own-runners)
- [Runner安全建议](https://docs.github.com/en/actions/hosting-your-own-runners/about-self-hosted-runners#self-hosted-runner-security)
- [故障排除指南](https://docs.github.com/en/actions/hosting-your-own-runners/troubleshooting-self-hosted-runners)