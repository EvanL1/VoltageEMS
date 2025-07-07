# ComsRv 配置指南

## 目录
- [概述](#概述)
- [快速开始](#快速开始)
- [配置文件结构](#配置文件结构)
- [多源配置加载](#多源配置加载)
- [服务配置](#服务配置)
- [通道配置](#通道配置)
- [点表配置（CSV）](#点表配置csv)
- [环境变量](#环境变量)
- [配置中心集成](#配置中心集成)
- [实践示例](#实践示例)
- [故障排除](#故障排除)
- [从旧版本迁移](#从旧版本迁移)

## 概述

ComsRv（通信服务）是VoltageEMS系统的核心组件，负责与各种工业设备和协议进行通信。本文档详细介绍了ComsRv的配置系统，包括：

- **多源配置加载**：支持本地文件、配置中心、环境变量等多种配置源
- **灵活的协议支持**：Modbus TCP/RTU、IEC 60870-5-104、CAN总线等
- **CSV点表管理**：通过CSV文件管理测点配置
- **分层日志系统**：服务级和通道级的独立日志配置

## 快速开始

### 最小配置示例

创建 `config/comsrv.yaml`：

```yaml
version: "2.0"

service:
  name: "comsrv"
  api:
    enabled: true
    bind_address: "127.0.0.1:3000"
  redis:
    url: "redis://127.0.0.1:6379"

channels:
  - id: 1
    name: "modbus_channel"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.100"
      port: 502
```

启动服务：

```bash
./comsrv --config config/comsrv.yaml
```

## 配置文件结构

### 完整配置模板

```yaml
# 配置版本号
version: "2.0"

# 服务配置部分
service:
  # 服务名称
  name: "comsrv"
  
  # 服务描述
  description: "Industrial Communication Service"
  
  # API服务器配置
  api:
    enabled: true                    # 是否启用API服务
    bind_address: "127.0.0.1:3000"   # API监听地址
    version: "v1"                    # API版本
  
  # Redis配置
  redis:
    enabled: true                    # 是否启用Redis
    url: "redis://127.0.0.1:6379"    # Redis连接URL
    db: 0                           # Redis数据库编号
    key_prefix: "voltageems:"       # 键前缀
    timeout_ms: 5000                # 连接超时（毫秒）
    retry_attempts: 3               # 重试次数
    retry_delay_ms: 100            # 重试延迟（毫秒）
    pool_size: 10                  # 连接池大小
    
  # 日志配置
  logging:
    level: "info"                   # 日志级别: trace/debug/info/warn/error
    file: "logs/comsrv.log"        # 日志文件路径
    max_size: 10485760             # 单个日志文件最大大小（字节）
    max_files: 5                   # 保留的日志文件数量
    console: true                  # 是否输出到控制台

# 默认路径配置
defaults:
  channels_root: "channels"        # 通道配置根目录
  combase_dir: "combase"          # ComBase配置目录
  protocol_dir: "protocol"        # 协议配置目录

# 通信通道配置
channels:
  # 通道配置数组，每个元素代表一个通信通道
  - id: 1                         # 通道ID（必须唯一）
    name: "channel_name"          # 通道名称
    description: "Channel desc"   # 通道描述
    protocol: "modbus_tcp"        # 协议类型
    parameters: {}                # 协议特定参数
    logging: {}                   # 通道级日志配置
    table_config: {}             # 点表配置
```

## 多源配置加载

### 配置加载优先级（从高到低）

1. **环境变量**（最高优先级）
2. **配置中心**（如果启用）
3. **本地配置文件**
4. **默认值**（最低优先级）

### 配置文件搜索路径

系统按以下顺序搜索配置文件：

1. 命令行指定：`--config /path/to/config.yaml`
2. `./config/comsrv.yaml`
3. `./comsrv.yaml`
4. `./config/default.yaml`
5. `/etc/comsrv/config.yaml`

### 支持的文件格式

- YAML (`.yaml`, `.yml`)
- TOML (`.toml`)
- JSON (`.json`)

## 服务配置

### API配置

```yaml
service:
  api:
    enabled: true                  # 启用/禁用API服务
    bind_address: "0.0.0.0:3000"   # 监听地址和端口
    version: "v1"                  # API版本前缀
    # 高级选项（可选）
    max_connections: 1000          # 最大并发连接数
    timeout_ms: 30000             # 请求超时时间
    cors_enabled: true            # 启用CORS
    cors_origins: ["*"]           # CORS允许的来源
```

### Redis配置

```yaml
service:
  redis:
    enabled: true
    # 连接配置
    url: "redis://127.0.0.1:6379"  # 支持单机模式
    # url: "redis://node1:6379,node2:6379,node3:6379"  # 集群模式
    
    # 连接参数
    db: 0                         # 数据库编号（单机模式）
    password: "secret"            # 密码（可选）
    username: "default"           # 用户名（Redis 6.0+）
    
    # 键管理
    key_prefix: "voltageems:"     # 所有键的前缀
    key_ttl: 3600                # 默认TTL（秒）
    
    # 连接池
    pool_size: 10                # 连接池大小
    pool_min_idle: 5             # 最小空闲连接数
    pool_max_lifetime: 1800      # 连接最大生命周期（秒）
    
    # 超时和重试
    timeout_ms: 5000             # 操作超时
    connect_timeout_ms: 10000    # 连接超时
    retry_attempts: 3            # 重试次数
    retry_delay_ms: 100         # 重试间隔
    retry_max_delay_ms: 5000    # 最大重试间隔
```

### 日志配置

#### 服务级日志

```yaml
service:
  logging:
    # 基本配置
    level: "info"                # 日志级别
    console: true               # 控制台输出
    
    # 文件输出配置
    file: "logs/comsrv.log"     # 日志文件路径
    max_size: 10485760          # 10MB - 单文件最大大小
    max_files: 5                # 保留文件数量
    
    # 高级选项
    format: "json"              # 输出格式：text/json
    timestamp_format: "rfc3339" # 时间戳格式
    include_location: true      # 包含代码位置
    include_thread: true        # 包含线程信息
```

#### 通道级日志

```yaml
channels:
  - id: 1
    logging:
      enabled: true             # 启用通道日志
      level: "debug"           # 通道日志级别
      
      # 文件配置
      log_dir: "logs/channel_1" # 日志目录
      max_file_size: 5242880   # 5MB
      max_files: 3             # 保留3个文件
      retention_days: 7        # 保留7天
      
      # 输出选项
      console_output: true     # 同时输出到控制台
      log_messages: true       # 记录协议消息
      log_hex_dump: true       # 十六进制转储
      
      # 性能选项
      buffer_size: 8192        # 日志缓冲区大小
      flush_interval: 1000     # 刷新间隔（毫秒）
```

## 通道配置

### Modbus TCP配置

```yaml
channels:
  - id: 1
    name: "modbus_tcp_master"
    protocol: "modbus_tcp"
    parameters:
      # 连接参数
      host: "192.168.1.100"
      port: 502
      
      # 超时和重试
      timeout_ms: 3000
      connect_timeout_ms: 5000
      max_retries: 3
      retry_delay_ms: 1000
      
      # 轮询配置
      polling_interval_ms: 1000
      polling_mode: "continuous"  # continuous/on_demand
      
      # 批量优化
      enable_batch_reading: true
      max_batch_size: 50         # 最大批量大小
      max_batch_gap: 10          # 寄存器最大间隔
      
      # 高级选项
      unit_id: 1                 # 默认从站ID
      max_concurrent_requests: 5 # 最大并发请求
      request_queue_size: 100    # 请求队列大小
```

### Modbus RTU配置

```yaml
channels:
  - id: 2
    name: "modbus_rtu_master"
    protocol: "modbus_rtu"
    parameters:
      # 串口参数
      port: "/dev/ttyUSB0"       # Linux
      # port: "COM3"              # Windows
      baud_rate: 9600
      data_bits: 8
      parity: "none"             # none/even/odd
      stop_bits: 1               # 1/2
      
      # 时序参数
      timeout_ms: 3000
      inter_frame_delay_us: 3500 # 帧间延迟（微秒）
      
      # RTU特定参数
      rts_control: "auto"        # auto/manual/none
      rts_delay_ms: 0           # RTS延迟
      
      # 其他参数（同Modbus TCP）
      polling_interval_ms: 1000
      enable_batch_reading: true
```

### IEC 60870-5-104配置

```yaml
channels:
  - id: 3
    name: "iec104_client"
    protocol: "iec60870_104"
    parameters:
      # 连接参数
      host: "192.168.1.200"
      port: 2404
      
      # IEC104协议参数
      k: 12                     # 未确认I帧最大数量
      w: 8                      # 接收序号确认阈值
      t0: 30                    # 连接建立超时（秒）
      t1: 15                    # 发送或测试APDU超时（秒）
      t2: 10                    # 无数据报文确认超时（秒）
      t3: 20                    # 测试帧发送间隔（秒）
      
      # ASDU参数
      common_address: 1         # 公共地址
      cause_of_transmission: 2  # 传输原因字节数
      asdu_address: 2          # ASDU地址字节数
      ioa_address: 3           # 信息对象地址字节数
      
      # 功能配置
      enable_time_sync: true    # 启用时钟同步
      enable_test_frame: true   # 启用测试帧
      enable_commands: true     # 启用控制命令
```

### CAN总线配置

```yaml
channels:
  - id: 4
    name: "can_bus"
    protocol: "can"
    parameters:
      # 接口配置
      interface: "can0"         # Linux SocketCAN
      # interface: "PCAN-USB"   # Windows PCAN
      
      # 总线参数
      bitrate: 500000          # 波特率
      sample_point: 0.875      # 采样点
      sjw: 1                   # 同步跳转宽度
      
      # 过滤器配置
      filters:
        - id: 0x100
          mask: 0x7FF
          type: "standard"     # standard/extended
        - id: 0x18FF0000
          mask: 0x1FFFFFFF
          type: "extended"
      
      # 缓冲区配置
      rx_buffer_size: 1000
      tx_buffer_size: 100
      
      # 错误处理
      error_passive_threshold: 127
      error_busoff_threshold: 255
      auto_recovery: true
```

### GPIO配置

```yaml
channels:
  - id: 5
    name: "gpio_io"
    protocol: "gpio"
    parameters:
      # GPIO芯片
      chip: "/dev/gpiochip0"    # Linux
      
      # 输入配置
      inputs:
        - pin: 17
          name: "DI_1"
          pull: "up"           # up/down/none
          debounce_ms: 10
        - pin: 27
          name: "DI_2"
          pull: "down"
          edge: "both"         # rising/falling/both
      
      # 输出配置
      outputs:
        - pin: 22
          name: "DO_1"
          initial: false       # 初始状态
          active_low: false    # 低电平有效
        - pin: 23
          name: "DO_2"
          initial: true
          pwm_enabled: true    # 启用PWM
          pwm_frequency: 1000  # PWM频率(Hz)
```

## 点表配置（CSV）

### 点表文件结构

ComsRv使用CSV文件管理四遥（遥测、遥信、遥控、遥调）点表：

#### 遥测点表 (telemetry.csv)

```csv
point_id,name,description,unit,data_type,scale,offset
1001,电压A相,A相线电压,V,float,1.0,0
1002,电流A相,A相线电流,A,float,1.0,0
1003,有功功率,总有功功率,kW,float,0.001,0
1004,无功功率,总无功功率,kVar,float,0.001,0
1005,功率因数,总功率因数,,float,0.01,0
1006,频率,电网频率,Hz,float,0.01,0
1007,温度1#,变压器温度,°C,float,0.1,-40
```

字段说明：
- `point_id`: 点号（唯一标识）
- `name`: 信号名称（英文标识）
- `description`: 中文描述
- `unit`: 工程单位
- `data_type`: 数据类型（float/int/uint）
- `scale`: 缩放系数
- `offset`: 偏移量

#### 遥信点表 (signal.csv)

```csv
point_id,name,description,normal_state,alarm_level
2001,断路器状态,10kV进线断路器,0,2
2002,接地刀闸,接地刀闸位置,0,1
2003,过流保护,过流保护动作,0,3
2004,温度报警,变压器温度报警,0,2
2005,通信状态,设备通信状态,1,1
```

字段说明：
- `normal_state`: 正常状态值（0/1）
- `alarm_level`: 报警级别（1-5）

#### 遥控点表 (control.csv)

```csv
point_id,name,description,control_type,pulse_duration
3001,断路器控制,10kV断路器分合,select_execute,0
3002,复归信号,故障复归,direct,500
3003,试验按钮,试验控制,select_execute,0
```

字段说明：
- `control_type`: 控制类型（direct/select_execute）
- `pulse_duration`: 脉冲持续时间（毫秒）

#### 遥调点表 (adjustment.csv)

```csv
point_id,name,description,unit,min_value,max_value,step
4001,有功设定,有功功率设定值,kW,0,1000,0.1
4002,无功设定,无功功率设定值,kVar,-500,500,0.1
4003,电压设定,电压设定值,V,9500,10500,10
```

字段说明：
- `min_value`: 最小值
- `max_value`: 最大值
- `step`: 步进值

### 协议映射配置

每种点类型都有对应的协议映射文件：

#### Modbus映射 (mapping_telemetry.csv)

```csv
point_id,register_address,function_code,slave_id,data_format,byte_order,register_count
1001,1000,3,1,float32,ABCD,2
1002,1002,3,1,float32,CDAB,2
1003,1004,3,1,uint16,AB,1
1004,1005,3,1,int16,AB,1
1005,2000,4,1,float32,BADC,2
```

字段说明：
- `register_address`: 寄存器地址
- `function_code`: 功能码（1/2/3/4）
- `slave_id`: 从站ID
- `data_format`: 数据格式
  - 整数类型：uint16/int16/uint32/int32
  - 浮点类型：float32/float64
  - 位类型：bit
- `byte_order`: 字节序
  - 16位：AB/BA
  - 32位：ABCD/DCBA/BADC/CDAB
- `register_count`: 寄存器数量

### 点表配置示例

```yaml
channels:
  - id: 1
    table_config:
      # 四遥文件路径
      four_telemetry_route: "config/points/station_1"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"
        signal_file: "signal.csv"
        adjustment_file: "adjustment.csv"
        control_file: "control.csv"
      
      # 协议映射文件路径
      protocol_mapping_route: "config/points/station_1"
      protocol_mapping_files:
        telemetry_mapping: "mapping_telemetry.csv"
        signal_mapping: "mapping_signal.csv"
        adjustment_mapping: "mapping_adjustment.csv"
        control_mapping: "mapping_control.csv"
```

## 环境变量

### 环境变量命名规则

所有环境变量使用 `COMSRV_` 前缀，嵌套配置使用下划线分隔：

```bash
COMSRV_SERVICE_NAME=my-comsrv
COMSRV_SERVICE_API_BIND_ADDRESS=0.0.0.0:8080
COMSRV_SERVICE_REDIS_URL=redis://redis:6379
```

### 完整环境变量列表

#### 服务配置

```bash
# 基本信息
COMSRV_VERSION=2.0
COMSRV_SERVICE_NAME=comsrv
COMSRV_SERVICE_DESCRIPTION="Industrial Communication Service"

# API配置
COMSRV_SERVICE_API_ENABLED=true
COMSRV_SERVICE_API_BIND_ADDRESS=0.0.0.0:3000
COMSRV_SERVICE_API_VERSION=v1
COMSRV_SERVICE_API_MAX_CONNECTIONS=1000
COMSRV_SERVICE_API_TIMEOUT_MS=30000

# Redis配置
COMSRV_SERVICE_REDIS_ENABLED=true
COMSRV_SERVICE_REDIS_URL=redis://127.0.0.1:6379
COMSRV_SERVICE_REDIS_DB=0
COMSRV_SERVICE_REDIS_KEY_PREFIX=voltageems:
COMSRV_SERVICE_REDIS_PASSWORD=secret
COMSRV_SERVICE_REDIS_TIMEOUT_MS=5000
COMSRV_SERVICE_REDIS_RETRY_ATTEMPTS=3
COMSRV_SERVICE_REDIS_POOL_SIZE=10

# 日志配置
COMSRV_SERVICE_LOGGING_LEVEL=info
COMSRV_SERVICE_LOGGING_FILE=/var/log/comsrv/app.log
COMSRV_SERVICE_LOGGING_MAX_SIZE=10485760
COMSRV_SERVICE_LOGGING_MAX_FILES=5
COMSRV_SERVICE_LOGGING_CONSOLE=true
COMSRV_SERVICE_LOGGING_FORMAT=json
```

#### 配置中心

```bash
# 配置中心URL
CONFIG_CENTER_URL=http://config-center:8080

# 配置中心认证（可选）
CONFIG_CENTER_TOKEN=your-auth-token
CONFIG_CENTER_TIMEOUT=10000
```

#### 运行时配置

```bash
# 指定配置文件
COMSRV_CONFIG_FILE=/etc/comsrv/custom.yaml

# 调试选项
RUST_LOG=comsrv=debug
RUST_BACKTRACE=1
```

## 配置中心集成

### 启用配置中心

```bash
export CONFIG_CENTER_URL=http://config-center:8080
```

### 配置中心API

配置中心应提供以下端点：

```
GET /api/v1/config/service/comsrv
```

### 响应格式

```json
{
  "status": "success",
  "message": null,
  "version": "2024-01-15-001",
  "updated_at": "2024-01-15T10:30:00Z",
  "data": {
    "version": "2.0",
    "service": {
      "name": "comsrv",
      "api": {
        "enabled": true,
        "bind_address": "0.0.0.0:3000"
      },
      "redis": {
        "url": "redis://redis-cluster:6379"
      }
    },
    "channels": [
      {
        "id": 1,
        "name": "modbus_channel",
        "protocol": "modbus_tcp",
        "parameters": {
          "host": "192.168.1.100",
          "port": 502
        }
      }
    ]
  }
}
```

### 配置热更新（规划中）

未来版本将支持配置热更新：

```yaml
service:
  config_sync:
    enabled: true
    interval_seconds: 60
    websocket_enabled: true
    websocket_url: "ws://config-center:8080/ws"
```

## 实践示例

### 示例1：开发环境配置

```yaml
# config/dev.yaml
version: "2.0"

service:
  name: "comsrv-dev"
  api:
    enabled: true
    bind_address: "127.0.0.1:3000"
  redis:
    url: "redis://localhost:6379"
  logging:
    level: "debug"
    console: true

channels:
  - id: 1
    name: "modbus_simulator"
    protocol: "modbus_tcp"
    parameters:
      host: "127.0.0.1"
      port: 5020
      timeout_ms: 1000
    logging:
      enabled: true
      level: "trace"
      console_output: true
      log_messages: true
```

### 示例2：生产环境配置

```yaml
# config/prod.yaml
version: "2.0"

service:
  name: "comsrv-prod"
  api:
    enabled: true
    bind_address: "0.0.0.0:3000"
    max_connections: 5000
  redis:
    url: "redis://redis-cluster:6379"
    pool_size: 50
    key_prefix: "prod:voltageems:"
  logging:
    level: "warn"
    file: "/var/log/comsrv/app.log"
    max_size: 104857600  # 100MB
    max_files: 10
    format: "json"

channels:
  - id: 1
    name: "substation_1"
    protocol: "modbus_tcp"
    parameters:
      host: "10.0.1.100"
      port: 502
      timeout_ms: 5000
      enable_batch_reading: true
      max_batch_size: 100
    logging:
      enabled: true
      level: "info"
      log_dir: "/var/log/comsrv/channels/substation_1"
      retention_days: 30
      
  - id: 2
    name: "rtu_devices"
    protocol: "modbus_rtu"
    parameters:
      port: "/dev/ttyS0"
      baud_rate: 19200
      data_bits: 8
      parity: "even"
      stop_bits: 1
```

### 示例3：多协议配置

```yaml
version: "2.0"

service:
  name: "comsrv-multi"
  api:
    enabled: true
  redis:
    url: "redis://127.0.0.1:6379"

channels:
  # Modbus TCP通道
  - id: 1
    name: "modbus_tcp_devices"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      enable_batch_reading: true
    table_config:
      four_telemetry_route: "config/points/modbus"
      
  # Modbus RTU通道
  - id: 2
    name: "modbus_rtu_devices"
    protocol: "modbus_rtu"
    parameters:
      port: "/dev/ttyUSB0"
      baud_rate: 9600
    table_config:
      four_telemetry_route: "config/points/rtu"
      
  # IEC 104通道
  - id: 3
    name: "iec104_gateway"
    protocol: "iec60870_104"
    parameters:
      host: "192.168.1.200"
      port: 2404
      common_address: 1
    table_config:
      four_telemetry_route: "config/points/iec104"
      
  # CAN总线通道
  - id: 4
    name: "can_devices"
    protocol: "can"
    parameters:
      interface: "can0"
      bitrate: 500000
    table_config:
      four_telemetry_route: "config/points/can"
```

### 示例4：批量优化配置

```yaml
channels:
  - id: 1
    name: "optimized_modbus"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      
      # 启用批量读取优化
      enable_batch_reading: true
      
      # 批量参数
      max_batch_size: 50        # 单批最多读取50个寄存器
      max_batch_gap: 10         # 寄存器间隔超过10则分批
      
      # 智能合并策略
      merge_strategy: "aggressive"  # conservative/moderate/aggressive
      
      # 寄存器分组
      register_groups:
        - name: "power_metrics"
          start: 1000
          count: 20
          priority: "high"
        - name: "status_flags"
          start: 2000
          count: 10
          priority: "medium"
          
      # 轮询优化
      polling_optimization:
        enabled: true
        adaptive_interval: true  # 根据变化率自动调整
        min_interval_ms: 100
        max_interval_ms: 5000
```

## 故障排除

### 常见问题

#### 1. 配置文件未找到

错误信息：
```
Failed to load configuration: Configuration file not found
```

解决方案：
- 检查配置文件路径是否正确
- 使用 `--config` 参数指定配置文件
- 确保文件有读取权限

#### 2. Redis连接失败

错误信息：
```
Failed to connect to Redis: Connection refused
```

解决方案：
```bash
# 检查Redis是否运行
redis-cli ping

# 检查连接URL
COMSRV_SERVICE_REDIS_URL=redis://localhost:6379

# 检查防火墙设置
sudo ufw allow 6379
```

#### 3. 端口已被占用

错误信息：
```
Failed to bind to address: Address already in use
```

解决方案：
```bash
# 查找占用端口的进程
lsof -i :3000

# 更改绑定地址
COMSRV_SERVICE_API_BIND_ADDRESS=127.0.0.1:3001
```

#### 4. CSV文件解析错误

错误信息：
```
Failed to parse CSV: Invalid field count
```

解决方案：
- 检查CSV文件格式
- 确保没有多余的逗号或空行
- 使用UTF-8编码保存文件

### 调试技巧

#### 启用详细日志

```bash
# 环境变量方式
export RUST_LOG=comsrv=trace
export COMSRV_SERVICE_LOGGING_LEVEL=trace

# 查看配置加载过程
export RUST_LOG=comsrv::core::config=debug
```

#### 配置验证

```bash
# 验证配置文件语法
./comsrv --config config/comsrv.yaml --validate

# 打印最终配置（合并所有源后）
./comsrv --config config/comsrv.yaml --print-config
```

#### 性能分析

```yaml
service:
  metrics:
    enabled: true
    prometheus_enabled: true
    prometheus_bind: "0.0.0.0:9090"
```

## 从旧版本迁移

### 版本1.0到2.0迁移

#### 主要变化

1. **配置结构变化**
   - 旧版：扁平结构
   - 新版：分层结构（service/channels/defaults）

2. **环境变量前缀**
   - 旧版：`REDIS_URL`
   - 新版：`COMSRV_SERVICE_REDIS_URL`

3. **点表配置**
   - 旧版：内嵌在通道配置中
   - 新版：独立的CSV文件

#### 迁移步骤

1. **备份现有配置**
   ```bash
   cp config/comsrv.yaml config/comsrv.yaml.v1.backup
   ```

2. **转换配置格式**
   ```bash
   # 使用迁移工具（如果可用）
   ./comsrv-migrate --input config/comsrv.yaml.v1 --output config/comsrv.yaml
   ```

3. **更新环境变量**
   ```bash
   # 旧版
   export REDIS_URL=redis://localhost:6379
   
   # 新版
   export COMSRV_SERVICE_REDIS_URL=redis://localhost:6379
   ```

4. **迁移点表**
   - 将内嵌的点配置导出为CSV文件
   - 更新table_config指向新的CSV文件

5. **测试配置**
   ```bash
   ./comsrv --config config/comsrv.yaml --validate
   ```

### 配置兼容性

新版本保持了对旧配置格式的部分兼容性：

```yaml
# 旧版本配置仍可工作，但建议迁移到新格式
redis:
  host: "localhost"
  port: 6379
  
# 新版本推荐格式
service:
  redis:
    url: "redis://localhost:6379"
```

## 附录

### 配置模式参考

完整的配置模式定义可在以下位置找到：
- JSON Schema: `schemas/comsrv-config.schema.json`
- TypeScript定义: `types/comsrv-config.d.ts`

### 相关文档

- [Modbus协议实现指南](./MODBUS_USER_GUIDE.md)
- [传输层架构](./TRANSPORT_LAYER_ARCHITECTURE.md)
- [配置中心API文档](./config-center-api.md)
- [性能优化指南](./MODBUS_OPTIMIZATION.md)

### 配置示例仓库

更多配置示例可在以下位置找到：
- `examples/configs/` - 各种场景的配置示例
- `tests/configs/` - 测试用配置文件

---

最后更新：2024-01-15  
版本：2.0