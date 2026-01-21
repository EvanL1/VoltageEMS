# Monarch CLI 使用指南

Monarch 是 VoltageEMS 的统一管理工具，提供配置同步、服务管理和运维操作的一站式解决方案。

## 快速开始

```bash
# 首次使用：初始化 + 同步配置
monarch init
monarch sync

# 启动服务
monarch services start

# 验证系统状态
monarch doctor
```

---

## 目录

- [全局选项](#全局选项)
- [配置管理命令](#配置管理命令)
  - [sync - 同步配置](#sync---同步配置)
  - [status - 查看状态](#status---查看状态)
  - [init - 初始化数据库](#init---初始化数据库)
  - [export - 导出配置](#export---导出配置)
- [服务管理命令](#服务管理命令)
  - [channels - 通道管理](#channels---通道管理)
  - [models - 模型管理](#models---模型管理)
  - [rules - 规则管理](#rules---规则管理)
  - [services - Docker 服务](#services---docker-服务)
  - [logs - 日志管理](#logs---日志管理)
- [调试工具](#调试工具)
  - [rtdb - Redis 操作](#rtdb---redis-操作)
  - [shm - 共享内存](#shm---共享内存)
  - [doctor - 系统诊断](#doctor---系统诊断)
- [环境变量](#环境变量)
- [常见场景](#常见场景)

---

## 全局选项

所有命令都支持以下全局选项：

| 选项 | 短选项 | 说明 |
|------|--------|------|
| `--verbose` | `-v` | 启用详细日志输出 |
| `--no-color` | | 禁用彩色输出（用于脚本或日志记录） |
| `--config-path <PATH>` | `-c` | 配置文件目录（默认自动检测） |
| `--db-path <PATH>` | | 数据库文件目录（默认自动检测） |
| `--offline` | `-o` | 强制离线模式（使用本地库 API） |
| `--online` | | 强制在线模式（仅使用 HTTP API） |

### 路径自动检测

Monarch 按以下优先级检测路径：

1. **环境变量**：`VOLTAGE_CONFIG_PATH` / `VOLTAGE_DATA_PATH`
2. **生产路径**：`/opt/MonarchEdge/config` / `/opt/MonarchEdge/data`
3. **开发路径**：`./config` / `./data`

### 离线 vs 在线模式

- **离线模式** (`--offline`)：直接调用本地库，无需服务运行，响应更快
- **在线模式** (`--online`)：通过 HTTP API 调用运行中的服务
- **自动模式**（默认）：优先使用离线模式，失败时回退到在线

---

## 配置管理命令

### sync - 同步配置

将 YAML/CSV 配置文件同步到 SQLite 数据库。

```bash
# 同步所有配置
monarch sync

# 验证配置（不实际写入数据库）
monarch sync --dry-run

# 显示详细进度
monarch sync --detailed

# 强制同步（跳过验证）
monarch sync --force

# 同步后检查数据库一致性
monarch sync --check
```

**选项：**

| 选项 | 短选项 | 说明 |
|------|--------|------|
| `--dry-run` | `-n` | 仅验证，不写入数据库 |
| `--force` | `-f` | 跳过验证强制同步 |
| `--detailed` | `-d` | 显示每个项目的同步进度 |
| `--check` | | 同步后检查重复 ID 和引用完整性 |

**同步顺序：** global → comsrv → modsrv

### status - 查看状态

显示当前配置状态和数据库信息。

```bash
# 基本状态
monarch status

# 详细状态（包含同步时间和项目数量）
monarch status --detailed
```

### init - 初始化数据库

创建或升级数据库 schema。

```bash
# 初始化数据库（安全升级，不删除数据）
monarch init

# 注意：--force 选项已禁用，防止意外数据丢失
```

**安全机制：**
- 使用 `CREATE TABLE IF NOT EXISTS` 确保安全升级
- 不会删除已有数据
- 如需重置数据库，需手动删除 `data/voltage.db`

### export - 导出配置

从数据库导出配置到 YAML/CSV 文件。

```bash
# 导出到默认目录（config/）
monarch export

# 导出到指定目录
monarch export --output /path/to/backup/

# 显示详细导出进度
monarch export --detailed
```

---

## 服务管理命令

### channels - 通道管理

管理通信通道（协议、设备连接）。

```bash
# 列出所有通道
monarch channels list

# 查看通道状态
monarch channels status <channel_id>

# 发送控制命令（0/1）- 需要离线模式
monarch channels control <channel_id> <point_id> <value>

# 发送调节值 - 需要离线模式
monarch channels adjust <channel_id> <point_id> <value>

> **注意**：`control` 和 `adjust` 命令的在线模式（`--online`）暂未正确实现，请使用默认的离线模式。

# 重新加载通道配置
monarch channels reload

# 检查服务健康状态
monarch channels health
```

**示例：**

```bash
# 查看通道 1 状态
monarch channels status 1

# 发送控制命令：通道 1，点位 10，值 1（开）
monarch channels control 1 10 1

# 发送调节值：通道 2，点位 20，值 50.5
monarch channels adjust 2 20 50.5
```

### models - 模型管理

管理产品模板和设备实例。

#### 产品管理

```bash
# 列出所有内置产品
monarch models products list

# 查看可用产品定义（开发用）
monarch models products available

# 获取产品详情
monarch models products get <product_name>
```

**示例：**

```bash
monarch models products list
monarch models products get PCS
monarch models products get Battery
```

> **注意**：产品名称区分大小写，必须与 `monarch models products list` 返回的 `product_name` 完全匹配。

#### 实例管理

```bash
# 列出所有实例
monarch models instances list

# 按产品类型筛选
monarch models instances list --product PCS

# 创建新实例
monarch models instances create <product> <name> [--props key=value...]

# 获取实例详情
monarch models instances get <name>

# 更新实例属性
monarch models instances update <name> --props key=value...

# 删除实例
monarch models instances delete <name>
monarch models instances delete <name> --force  # 跳过确认

# 获取实例运行时数据
monarch models instances data <name>
monarch models instances data <name> --point-type M  # 仅测量点
monarch models instances data <name> --point-type A  # 仅动作点
```

**示例：**

```bash
# 创建 PCS 实例
monarch models instances create PCS pcs_01 \
  --props rated_power=500.0 \
  --props manufacturer=Sungrow

# 更新实例属性
monarch models instances update pcs_01 --props rated_power=600.0

# 查看实例运行时数据
monarch models instances data pcs_01
```

### rules - 规则管理

管理业务规则（条件触发、定时任务）。

```bash
# 列出所有规则
monarch rules list

# 仅显示已启用的规则
monarch rules list --enabled

# 获取规则详情
monarch rules get <rule_id>

# 启用/禁用规则
monarch rules enable <rule_id>
monarch rules disable <rule_id>

# 执行规则
monarch rules execute <rule_id>
monarch rules execute <rule_id> --force  # 强制执行（忽略条件）

# 测试规则条件（仅评估，不执行动作）- 计划中
# monarch rules test <rule_id>

# 查看执行历史 - 计划中
# monarch rules executions [rule_id] [--limit 20]
```

> **注意**：`test` 和 `executions` 命令的后端 API 尚未实现。

**示例：**

```bash
# 列出所有已启用规则
monarch rules list --enabled

# 启用规则 1001
monarch rules enable 1001

# 手动执行规则
monarch rules execute 1001
```

### services - Docker 服务

管理 VoltageEMS Docker 容器。

```bash
# 启动所有服务
monarch services start

# 启动指定服务
monarch services start comsrv modsrv

# 停止服务
monarch services stop
monarch services stop comsrv

# 重启服务
monarch services restart
monarch services restart modsrv

# 查看服务状态
monarch services status

# 查看服务日志
monarch services logs <service>
monarch services logs comsrv --follow     # 实时跟踪
monarch services logs modsrv --tail 200   # 显示最后 200 行

# 重新加载配置（热加载）
monarch services reload

# 构建 Docker 镜像
monarch services build
monarch services build comsrv modsrv

# 拉取最新镜像
monarch services pull

# 清理 Docker 资源
monarch services clean
monarch services clean --volumes  # 同时清理卷

# 刷新服务（重建容器）
monarch services refresh
monarch services refresh --pull   # 先拉取最新镜像
monarch services refresh --smart  # 智能模式（仅更新变化的镜像，保护 Redis）
```

**特殊命令：**

```bash
# 通过 M2C 路由执行动作
monarch services set-action <instance_name> <point_id> <value>
monarch services set-action pcs_01 1 100.0 --detailed

# 查看路由表
monarch services routing-show
monarch services routing-show --route-type c2m      # 仅上行路由
monarch services routing-show --route-type m2c      # 仅下行路由
monarch services routing-show --prefix "2:T:"       # 按前缀筛选
monarch services routing-show --limit 50 --detailed
```

### logs - 日志管理

动态调整运行中服务的日志级别。

```bash
# 设置日志级别
monarch logs level <service> <level>

# 获取当前日志级别
monarch logs get <service>
```

**服务名称：** `comsrv`, `modsrv`, `all`

**日志级别：** `trace`, `debug`, `info`, `warn`, `error`

**示例：**

```bash
# 切换所有服务到 debug 模式
monarch logs level all debug

# 设置 comsrv 为 trace 级别
monarch logs level comsrv trace

# 使用过滤器语法
monarch logs level comsrv "info,comsrv::protocol=debug"

# 查看所有服务当前日志级别
monarch logs get all
```

---

## 调试工具

### rtdb - Redis 操作

直接操作 Redis 实时数据库（需要 `--offline` 模式）。

```bash
# 获取键值
monarch --offline rtdb get <key>
monarch --offline rtdb get <key> --field <field>  # Hash 字段

# 设置键值
monarch --offline rtdb set <key> <value>
monarch --offline rtdb set <key> <value> --field <field>

# 扫描键
monarch --offline rtdb scan <pattern> [--limit 100]

# 删除键
monarch --offline rtdb del <key1> [key2...]
monarch --offline rtdb del <key> --force

# 检查键类型和内容
monarch --offline rtdb inspect <key>
monarch --offline rtdb inspect <key> --full

# 显示常用键模式
monarch --offline rtdb patterns
```

**常用键模式：**

| 模式 | 说明 |
|------|------|
| `inst:<id>:M` | 实例测量点 Hash |
| `inst:<id>:A` | 实例动作点 Hash |
| `comsrv:<ch_id>:T` | 通道遥测点 Hash |
| `comsrv:<ch_id>:S` | 通道信号点 Hash |
| `route:c2m` | 上行路由表 |
| `route:m2c` | 下行路由表 |

### shm - 共享内存

零延迟共享内存 CLI（类似 mysql-cli）。

```bash
# 一次性查询
monarch shm get <key>

# 查看共享内存信息
monarch shm info

# 实时监控键值变化
monarch shm watch <key> [--interval-ms 500]

# TUI 实时仪表板（类似 htop）
monarch shm top
```

**键格式：**

| 格式 | 说明 | 示例 |
|------|------|------|
| `inst:<id>:M:<point_id>` | 实例测量点 | `inst:1:M:10` |
| `inst:<id>:A:<point_id>` | 实例动作点 | `inst:1:A:5` |
| `ch:<id>:T:<point_id>` | 通道遥测点 | `ch:1001:T:1` |
| `ch:<id>:S:<point_id>` | 通道信号点 | `ch:1001:S:1` |
| `ch:<id>:C:<point_id>` | 通道控制点 | `ch:1001:C:1` |
| `ch:<id>:A:<point_id>` | 通道调节点 | `ch:1001:A:1` |

**示例：**

```bash
# 查询实例 1 的测量点 10
monarch shm get inst:1:M:10

# 实时监控通道 1001 遥测点 1
monarch shm watch ch:1001:T:1 --interval-ms 200

# 打开实时仪表板
monarch shm top
```

### doctor - 系统诊断

检查系统健康状态并诊断问题。

```bash
# 基本健康检查
monarch doctor

# 详细输出（包含响应时间）
monarch doctor --verbose

# JSON 格式输出（用于脚本）
monarch doctor --json
```

**检查项目：**

| 检查项 | 说明 |
|--------|------|
| Docker Engine | Docker 是否运行 |
| Redis | Redis 容器状态和连接 |
| comsrv | 通信服务健康状态 |
| modsrv | 模型服务健康状态 |
| Database | SQLite 数据库状态 |
| Config Files | 配置文件完整性 |
| Shared Memory | 共享内存可用性 |

**输出示例：**

```
✓ Docker Engine    Running (v24.0.7)
✓ Redis            Connected (ping: 1ms)
✓ comsrv           Healthy (port 6001)
✓ modsrv           Healthy (port 6002)
✓ Database         OK (last sync: 2024-01-15 10:30)
✓ Config Files     All present
✓ Shared Memory    Available
```

---

## 环境变量

Monarch 支持通过环境变量配置，所有变量使用 `VOLTAGE_` 前缀：

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `VOLTAGE_CONFIG_PATH` | 配置文件目录 | 自动检测 |
| `VOLTAGE_DATA_PATH` | 数据文件目录 | 自动检测 |
| `VOLTAGE_REDIS_URL` | Redis 连接 URL | `redis://localhost:6379` |
| `VOLTAGE_COMSRV_URL` | Comsrv 服务 URL | `http://localhost:6001` |
| `VOLTAGE_MODSRV_URL` | Modsrv 服务 URL | `http://localhost:6002` |

---

## 常见场景

### 场景 1：首次部署

```bash
# 1. 初始化数据库
monarch init

# 2. 同步配置（验证模式）
monarch sync --dry-run

# 3. 正式同步
monarch sync

# 4. 启动服务
monarch services start

# 5. 验证系统
monarch doctor
```

### 场景 2：更新配置

```bash
# 1. 编辑配置文件
vim config/comsrv/comsrv.yaml

# 2. 验证更改
monarch sync --dry-run --detailed

# 3. 同步到数据库
monarch sync

# 4. 热加载服务
monarch services reload
```

### 场景 3：调试问题

```bash
# 1. 检查系统状态
monarch doctor --verbose

# 2. 查看服务日志
monarch services logs comsrv --follow

# 3. 切换到 debug 模式
monarch logs level all debug

# 4. 检查 Redis 数据
monarch --offline rtdb scan "inst:*"

# 5. 监控实时数据
monarch shm top
```

### 场景 4：更新服务镜像

```bash
# 智能刷新（推荐）
monarch services refresh --smart

# 或手动流程
monarch services pull
monarch services refresh
```

### 场景 5：备份和恢复

```bash
# 导出配置备份
monarch export --output /backup/config-$(date +%Y%m%d)/

# 恢复配置
cp -r /backup/config-20240115/* config/
monarch sync
```

---

## 故障排除

### 问题：monarch 命令未找到

```bash
# 确保 monarch 在 PATH 中
export PATH="$PATH:/opt/MonarchEdge/bin"

# 或使用完整路径
/opt/MonarchEdge/bin/monarch doctor
```

### 问题：数据库连接失败

```bash
# 检查数据库文件
ls -la data/voltage.db

# 重新初始化（如果损坏）
rm data/voltage.db
monarch init
monarch sync
```

### 问题：服务无法启动

```bash
# 检查 Docker 状态
docker ps -a

# 查看服务日志
monarch services logs comsrv --tail 200

# 检查端口占用
lsof -i :6001
```

### 问题：离线模式不工作

```bash
# 确保 Redis 可访问
redis-cli ping

# 检查环境变量
echo $VOLTAGE_REDIS_URL

# 显式使用离线模式
monarch --offline --verbose channels list
```
