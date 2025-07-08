# ComsRV 启动流程说明

## 系统概述

ComsRV 是 VoltageEMS 的通信管理服务，负责与现场设备进行数据通信。系统采用分层架构设计：

**服务 → 通道 → 协议映射 → 设备点位**

这种设计使得系统具有良好的扩展性，可以同时管理多个不同类型的设备通信。

## 启动方式

### 基本启动命令

```bash
# 生产环境启动
./comsrv --config config/production.yml

# 测试环境启动
./comsrv --config config/test.yml

# 开发环境启动（使用cargo）
cargo run --release --bin comsrv -- --config config/modbus_test.yml
```

### 命令行参数

- `-c, --config`: 指定配置文件路径（默认：config/comsrv.yaml）

## 系统启动流程

### 第一阶段：系统初始化

1. **读取启动参数**

   - 解析命令行参数，获取配置文件路径
   - 如果未指定，使用默认配置文件
2. **加载环境配置**

   - 读取系统环境变量
   - 加载 .env 文件（如果存在）
3. **加载主配置文件**

   - 系统自动识别配置文件格式（支持 YAML、JSON、TOML）
   - 解析服务级配置：服务名称、版本、API设置、Redis连接等
   - 解析通道配置：每个通道代表一个设备连接

### 第二阶段：配置文件处理

#### 主配置文件结构

配置文件包含两个主要部分：

**服务配置**

- 服务基本信息（名称、版本）
- API服务设置（是否启用、监听地址）
- Redis数据库配置（连接地址、认证信息）
- 日志系统配置（日志级别、输出方式）

**通道配置**

- 通道ID和名称
- 通信协议类型（如 modbus_tcp）
- 连接参数（IP地址、端口、超时时间等）
- 点表文件路径配置

#### 环境变量支持

系统支持通过环境变量覆盖配置项：

- 环境变量前缀：`COMSRV_`
- 例如：`COMSRV_SERVICE_NAME=MyService` 会覆盖配置文件中的服务名称

### 第三阶段：点表加载

点表是设备数据点的定义文件，使用CSV格式便于维护。系统会为每个通道加载两组文件：

#### 1. 四遥定义文件

定义设备的数据点基本信息：

- **遥测文件 (telemetry.csv)**: 数值型测量数据，如电压、电流、温度
- **遥信文件 (signal.csv)**: 开关状态数据，如断路器状态、故障信号
- **遥调文件 (adjustment.csv)**: 可调节的参数，如阈值设定
- **遥控文件 (control.csv)**: 控制命令点，如启动/停止命令

#### 2. 协议映射文件

定义如何从设备读取这些数据点：

- **mapping_telemetry.csv**: 遥测点的读取地址和数据格式
- **mapping_signal.csv**: 遥信点的读取地址和位定义
- **mapping_adjustment.csv**: 遥调点的写入地址和格式
- **mapping_control.csv**: 遥控点的控制地址

#### 点表加载过程

1. 根据配置文件中的路径找到CSV文件
2. 读取四遥定义，获取点位的基本信息
3. 读取协议映射，获取通信参数
4. 将两者按照点位ID进行匹配合并
5. 生成完整的通信点表供运行时使用

### 第四阶段：服务组件启动

1. **日志系统初始化**

   - 创建主日志文件（JSON格式，支持日志分析）
   - 为每个通道创建独立的通信日志
   - 设置日志轮转策略（每日轮转，保留历史）
2. **数据存储初始化**

   - 如果配置了Redis，建立连接池
   - 测试连接可用性
   - 如果连接失败，系统仍可使用内存存储继续运行
3. **通道启动**

   - 按照配置创建各个通信通道
   - 每个通道独立运行，互不影响
   - 建立与设备的网络连接
   - 启动数据轮询任务
4. **API服务启动**

   - 如果启用，在指定端口启动HTTP服务
   - 提供系统状态查询接口
   - 提供数据读写接口
5. **后台任务启动**

   - 启动连接状态监控
   - 启动异常清理任务
   - 启动性能统计任务

## 运行时行为

### 数据采集流程

1. 每个通道按照配置的轮询周期定时读取设备数据
2. 根据点表配置解析设备响应
3. 将数据写入Redis供其他服务使用
4. 记录通信日志用于故障诊断

### 错误处理机制

- **配置错误**：启动失败，输出详细错误信息
- **点表加载失败**：记录警告，该通道可能无法正常工作
- **设备连接失败**：自动重试，不影响其他通道
- **Redis连接失败**：降级到内存模式，保证基本功能

### 系统监控

- 每个通道的连接状态
- 数据采集成功率
- 通信响应时间
- 系统资源使用情况

## 停止服务

### 正常停止

- 使用 Ctrl+C 或发送 SIGTERM 信号
- 系统会优雅关闭：
  1. 停止新的数据采集
  2. 完成当前正在进行的通信
  3. 关闭所有网络连接
  4. 保存必要的状态信息
  5. 释放系统资源

### 强制停止

- 使用 kill -9 强制终止（不推荐）
- 可能导致数据丢失或状态不一致

## 配置文件示例

```yaml
# 服务配置
service:
  name: comsrv                      # 服务名称
  version: 0.1.0                    # 版本号
  api:
    enabled: true                   # 启用API服务
    bind_address: "0.0.0.0:8080"    # API监听地址
  redis:
    enabled: true                   # 启用Redis
    url: "redis://127.0.0.1:6379"   # Redis连接地址
  logging:
    level: info                     # 日志级别
    file: logs/comsrv.log          # 日志文件路径
    console: true                   # 同时输出到控制台
    max_files: 5                    # 保留日志文件数

# 通道配置
channels:
  - id: 1                           # 通道ID
    name: "Power Meter"             # 通道名称
    protocol: modbus_tcp            # 通信协议
    parameters:
      host: "127.0.0.1"            # 设备IP地址
      port: 5020                    # 端口号
      timeout_ms: 1000              # 超时时间（毫秒）
    logging:
      enabled: true                 # 启用通道日志
      level: debug                  # 通道日志级别
      log_dir: "logs/power_meter"   # 日志目录
    table_config:
      # 四遥定义文件路径
      four_telemetry_route: "channel_1_power_meter"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"
        signal_file: "signal.csv"
        adjustment_file: "adjustment.csv"
        control_file: "control.csv"
      # 协议映射文件路径
      protocol_mapping_route: "channel_1_power_meter"
      protocol_mapping_files:
        telemetry_mapping: "mapping_telemetry.csv"
        signal_mapping: "mapping_signal.csv"
        adjustment_mapping: "mapping_adjustment.csv"
        control_mapping: "mapping_control.csv"
```

## 运维建议

### 故障排查

1. 检查主日志文件了解系统状态
2. 查看通道日志分析通信问题
3. 使用API接口查询实时状态
4. 检查网络连接和防火墙设置
5. 验证设备端配置是否匹配

### 性能优化

1. 合理设置轮询周期，避免过于频繁
2. 启用批量读取减少通信次数
3. 调整并发连接数优化资源使用
4. 定期清理历史日志释放磁盘空间

## 启动日志示例

系统启动时会输出详细的日志信息，便于确认各组件状态：

```
2025-07-08T09:29:09.149510Z INFO Starting Communication Service v0.1.0
2025-07-08T09:29:09.149673Z INFO Configuration loaded successfully:
2025-07-08T09:29:09.149682Z INFO   - Service name: comsrv
2025-07-08T09:29:09.149688Z INFO   - Channels configured: 3
2025-07-08T09:29:09.149693Z INFO   - API enabled: true
2025-07-08T09:29:09.149699Z INFO   - Redis enabled: true
2025-07-08T09:29:09.153152Z INFO Redis connection manager created successfully
2025-07-08T09:29:09.154098Z INFO Created channel log file: "logs/Power Meter/channel_1.log"
2025-07-08T09:29:09.154115Z INFO Creating Modbus client: Power Meter
2025-07-08T09:29:09.154134Z INFO Created Modbus mapping table with 4 telemetry, 2 signal, 2 adjustment, 4 control points
2025-07-08T09:29:09.158123Z INFO Channel 1 started successfully
2025-07-08T09:29:09.162541Z INFO API server started on 0.0.0.0:8080
2025-07-08T09:29:09.162789Z INFO Communication service started successfully
```

日志级别说明：

- **INFO**: 正常运行信息
- **WARN**: 警告信息，系统仍可运行
- **ERROR**: 错误信息，某些功能可能受影响
- **DEBUG**: 调试信息，用于故障排查
