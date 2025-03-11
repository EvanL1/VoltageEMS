# EMS Console - 实时监控 GUI

EMS Console 是一个基于 Slint UI 的图形化监控工具，用于实时查看 VoltageEMS 系统的通道状态和数据点值。

## 功能特性

- 📊 实时显示通道数据（遥测、遥信、遥控、遥调）
- 🔄 自动刷新数据（1秒间隔）
- 📈 显示点位值、单位、时间戳
- 🎯 支持通道和四遥类型切换
- 📄 分页显示，方便浏览大量数据点

## 系统要求

### 本地运行
- Linux 系统（支持 X11 或 Wayland）
- 已安装 X11 库（libX11、libXcursor、libXrandr）
- Redis 服务正在运行
- comsrv 数据库文件存在

### SSH 远程运行
- SSH 客户端支持 X11 转发
- 服务器端启用 X11Forwarding
- 已安装 xauth 工具

## 安装

### 通过安装脚本（推荐）

```bash
# EMS Console 会随 MonarchEdge 安装包一起安装
sudo ./MonarchEdge-arm64-<version>.run

# 安装后可执行文件位于：
# - /usr/local/bin/ems-console (主程序)
# - /usr/local/bin/ems-console-launcher (启动包装脚本)
```

### 从源码编译

```bash
# 本地编译（当前平台）
cargo build --release -p ems-console

# 交叉编译 ARM64
cargo zigbuild --release --target aarch64-unknown-linux-musl -p ems-console
```

## 使用方法

### 本地运行

```bash
# 直接运行
ems-console

# 或使用启动包装脚本（带环境检查）
ems-console-launcher
```

### SSH 远程运行

#### 1. 配置 SSH 服务器

确保 SSH 服务器启用了 X11 转发：

```bash
# 编辑 /etc/ssh/sshd_config
sudo nano /etc/ssh/sshd_config

# 确保以下选项存在并启用
X11Forwarding yes
X11DisplayOffset 10
X11UseLocalhost yes

# 重启 SSH 服务
sudo systemctl restart sshd
```

#### 2. 安装必要依赖

```bash
# Debian/Ubuntu
sudo apt-get install xauth libx11-6 libxcursor1 libxrandr2

# CentOS/RHEL
sudo yum install xorg-x11-xauth libX11 libXcursor libXrandr
```

#### 3. SSH 连接并运行

```bash
# 使用 -X 选项启用 X11 转发
ssh -X user@host

# 或使用 -Y 选项（受信任的 X11 转发）
ssh -Y user@host

# 连接后运行（推荐使用启动脚本，会进行环境检查）
ems-console-launcher

# 或直接运行
ems-console
```

#### 4. SSH 配置文件（可选）

在本地 `~/.ssh/config` 中添加：

```
Host voltageems
    HostName 192.168.1.100
    User admin
    ForwardX11 yes
    ForwardX11Trusted yes
```

然后可以直接使用：
```bash
ssh voltageems
ems-console-launcher
```

## 环境变量

EMS Console 支持以下环境变量：

```bash
# Redis 连接 URL
export REDIS_URL="redis://127.0.0.1:6379"

# 统一数据库路径（所有服务共享）
export VOLTAGE_DB_PATH="data/voltage.db"

# 日志级别
export RUST_LOG="info"

# 键空间模式（生产或测试）
export KEYSPACE="production"  # 或 "test"
```

## 故障排除

### 问题：`Error: No DISPLAY environment variable set`

**原因**：没有配置 X11 转发或 DISPLAY 环境变量

**解决方案**：
1. 使用 `ssh -X` 或 `ssh -Y` 连接
2. 检查服务器端 `/etc/ssh/sshd_config` 中的 `X11Forwarding yes`
3. 确保安装了 xauth：`sudo apt-get install xauth`

### 问题：`Cannot connect to Redis`

**原因**：Redis 服务未运行或连接 URL 错误

**解决方案**：
```bash
# 检查 Redis 状态
redis-cli ping

# 或检查 Docker 容器
docker-compose ps voltage-redis

# 设置正确的 Redis URL
export REDIS_URL="redis://localhost:6379"
```

### 问题：`Database not found`

**原因**：统一数据库文件不存在或路径错误

**解决方案**：
```bash
# 初始化配置（使用统一数据库）
monarch init all
monarch sync all

# 或设置正确的数据库路径
export VOLTAGE_DB_PATH="/opt/MonarchEdge/data/voltage.db"
```

### 问题：Redis 数据为空

**原因**：通道未启动或设备连接失败

**解决方案**：
```bash
# 检查通道状态
monarch channels status

# 查看服务日志
docker-compose logs -f comsrv

# 检查 Redis 键
redis-cli KEYS "comsrv:*"
```

### 问题：显示异常或无法渲染

**原因**：缺少 X11 库或 Wayland 支持

**解决方案**：
```bash
# 安装必要的图形库
sudo apt-get install libx11-6 libxcursor1 libxrandr2 \
    libxi6 libxinerama1 libxkbcommon0

# 对于 Wayland，确保 XWayland 已安装
sudo apt-get install xwayland
```

## 开发指南

### 项目结构

```
apps/ems-console/
├── src/
│   └── main.rs          # 主程序逻辑
├── ui/
│   └── app.slint        # Slint UI 定义
├── build.rs             # 构建脚本
├── Cargo.toml           # 依赖配置
└── README.md            # 本文档
```

### 依赖说明

- **slint**: UI 框架（版本 1.14）
- **tokio**: 异步运行时
- **redis**: Redis 客户端
- **sqlx**: SQLite 数据库访问
- **voltage-config**: VoltageEMS 配置库（键空间管理）

### 构建和测试

```bash
# 开发模式运行
cd apps/ems-console
cargo run

# 发布版本构建
cargo build --release -p ems-console

# 检查代码
cargo clippy -p ems-console
cargo fmt -p ems-console
```

## 架构说明

### 数据流

```
SQLite DB (channels, points)
    ↓ (启动时加载)
Console UI (channel list, point list)
    ↓ (1秒定时器)
Redis (comsrv:channel:T/S/C/A keys)
    ↓ (HGETALL)
Console UI (更新显示值和时间戳)
```

### 关键组件

1. **通道管理**：从 SQLite 加载通道列表（`load_channels_sqlite`）
2. **点位定义**：按通道和四遥类型加载点位（`load_points_sqlite`）
3. **实时刷新**：后台任务定期从 Redis 读取数据（`refresh_values_redis`）
4. **UI 更新**：通过 Slint 的 `invoke_from_event_loop` 更新界面

### Redis 键格式

Console 使用 `voltage-config` 库的 `KeySpaceConfig` 生成标准化的 Redis 键：

```rust
let ks = KeySpaceConfig::production();
let data_key = ks.channel_key(channel_id, PointType::Telemetry);
// => "comsrv:1001:T"

let ts_key = ks.channel_ts_key(channel_id, PointType::Telemetry);
// => "comsrv:1001:T:ts"
```

## 许可证

本项目是 VoltageEMS 的一部分，遵循项目主许可证。
