# ComsRV 配置指南

本文档详细说明 ComsRV 通信服务的配置方法，包括主配置文件和点表文件的格式与含义。

## 配置文件概述

ComsRV 使用分层配置体系：
- **主配置文件**：定义服务参数和通道列表
- **点表文件**：定义设备数据点的详细信息

配置文件支持的格式：
- YAML（推荐，.yml 或 .yaml）
- JSON（.json）
- TOML（.toml）

## 主配置文件详解

### 文件结构
```yaml
service:      # 服务级配置
  ...
channels:     # 通道配置列表
  - ...
```

### 服务配置 (service)

#### 基本信息
```yaml
service:
  name: comsrv          # 服务名称，用于识别和日志记录
  version: 0.1.0        # 服务版本号
```

#### API 配置
```yaml
  api:
    enabled: true                    # 是否启用 HTTP API 服务
    bind_address: "0.0.0.0:8080"    # API 监听地址
                                    # 0.0.0.0 表示监听所有网络接口
                                    # 端口号建议使用 8080-8090
```

#### Redis 配置
```yaml
  redis:
    enabled: true                    # 是否启用 Redis 数据存储
    url: "redis://127.0.0.1:6379"   # Redis 连接地址
                                    # 格式：redis://[用户名:密码@]主机:端口/数据库号
    max_connections: 10              # 连接池最大连接数（可选）
    timeout_ms: 5000                 # 连接超时时间，毫秒（可选）
```

#### 日志配置
```yaml
  logging:
    level: info                      # 日志级别：trace, debug, info, warn, error
    file: logs/comsrv.log           # 主日志文件路径
    console: true                    # 是否同时输出到控制台
    max_size: 10485760              # 单个日志文件最大大小（字节），默认 10MB
    max_files: 5                     # 保留的日志文件数量
    retention_days: 30               # 日志保留天数（可选）
```

### 通道配置 (channels)

每个通道代表一个独立的设备连接：

#### 基本配置
```yaml
channels:
  - id: 1                           # 通道唯一标识，必须为正整数
    name: "Power Meter"             # 通道名称，便于识别
    description: "智能电表数据采集"   # 通道描述（可选）
    protocol: modbus_tcp            # 通信协议类型
```

支持的协议类型：
- `modbus_tcp`：Modbus TCP 协议
- `modbus_rtu`：Modbus RTU 协议（串口）
- `iec104`：IEC 60870-5-104 协议（计划支持）
- `canbus`：CAN 总线协议（计划支持）

#### 协议参数 (parameters)

**Modbus TCP 参数：**
```yaml
    parameters:
      host: "192.168.1.100"         # 设备 IP 地址
      port: 502                     # Modbus TCP 端口，默认 502
      timeout_ms: 1000              # 通信超时时间（毫秒）
      retry_count: 3                # 重试次数
      retry_delay_ms: 100           # 重试间隔（毫秒）
```

**Modbus RTU 参数：**
```yaml
    parameters:
      device_path: "/dev/ttyUSB0"   # 串口设备路径
      baud_rate: 9600               # 波特率：9600, 19200, 38400, 57600, 115200
      data_bits: 8                  # 数据位：7 或 8
      stop_bits: 1                  # 停止位：1 或 2
      parity: "none"                # 校验位：none, even, odd
      timeout_ms: 1000              # 超时时间
```

#### 通道日志配置
```yaml
    logging:
      enabled: true                 # 是否启用通道独立日志
      level: debug                  # 通道日志级别
      log_dir: "logs/power_meter"   # 日志目录（相对于主日志目录）
      log_messages: true            # 是否记录通信报文
      max_file_size: 5242880        # 单文件最大大小（5MB）
      max_files: 3                  # 保留文件数
```

#### 轮询配置
```yaml
    polling:
      enabled: true                 # 是否启用自动轮询
      interval_ms: 1000             # 轮询间隔（毫秒）
      batch_enabled: true           # 是否启用批量读取优化
      max_batch_size: 125           # 最大批量大小（寄存器数）
```

#### 点表配置
```yaml
    table_config:
      # 四遥定义文件路径（相对于配置文件目录）
      four_telemetry_route: "channel_1_power_meter"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"      # 遥测定义
        signal_file: "signal.csv"            # 遥信定义
        adjustment_file: "adjustment.csv"    # 遥调定义
        control_file: "control.csv"          # 遥控定义
      
      # 协议映射文件路径
      protocol_mapping_route: "channel_1_power_meter"
      protocol_mapping_files:
        telemetry_mapping: "mapping_telemetry.csv"      # 遥测映射
        signal_mapping: "mapping_signal.csv"            # 遥信映射
        adjustment_mapping: "mapping_adjustment.csv"    # 遥调映射
        control_mapping: "mapping_control.csv"          # 遥控映射
```

## 点表文件详解

### 四遥定义文件

#### 遥测定义 (telemetry.csv)
```csv
point_id,signal_name,chinese_name,data_type,scale,offset,unit
1,voltage_a,A相电压,FLOAT,1.0,0.0,V
2,current_a,A相电流,FLOAT,1.0,0.0,A
3,power_active,有功功率,FLOAT,0.001,0.0,kW
```

字段说明：
- `point_id`：点位唯一标识（1-65535）
- `signal_name`：英文信号名，用于程序识别
- `chinese_name`：中文名称，用于显示
- `data_type`：数据类型（FLOAT, INT, UINT）
- `scale`：缩放系数，实际值 = 原始值 × scale
- `offset`：偏移量，实际值 = (原始值 × scale) + offset
- `unit`：单位

#### 遥信定义 (signal.csv)
```csv
point_id,signal_name,chinese_name,data_type,reverse
1,breaker_status,断路器状态,bool,false
2,fault_alarm,故障报警,bool,true
```

字段说明：
- `reverse`：是否反转逻辑（true: 0=开 1=关）

#### 遥调定义 (adjustment.csv)
```csv
point_id,signal_name,chinese_name,data_type,scale,offset,unit
1,voltage_setpoint,电压设定值,FLOAT,1.0,0.0,V
2,current_limit,电流限值,FLOAT,1.0,0.0,A
```

#### 遥控定义 (control.csv)
```csv
point_id,signal_name,chinese_name,data_type,reverse
1,breaker_open,断路器分闸,bool,false
2,breaker_close,断路器合闸,bool,false
```

### 协议映射文件

#### Modbus 遥测映射 (mapping_telemetry.csv)
```csv
point_id,signal_name,slave_id,function_code,register_address,data_format,byte_order,register_count
1,voltage_a,1,3,100,float32_be,ABCD,2
2,current_a,1,3,102,float32_be,ABCD,2
3,power_active,1,3,104,int32,ABCD,2
```

字段说明：
- `slave_id`：Modbus 从站地址（1-247）
- `function_code`：功能码
  - 1: 读线圈状态
  - 2: 读离散输入
  - 3: 读保持寄存器
  - 4: 读输入寄存器
  - 5: 写单个线圈
  - 6: 写单个寄存器
  - 15: 写多个线圈
  - 16: 写多个寄存器
- `register_address`：寄存器地址（0-65535）
- `data_format`：数据格式
  - `uint16`：16位无符号整数
  - `int16`：16位有符号整数
  - `uint32`：32位无符号整数
  - `int32`：32位有符号整数
  - `float32`：32位浮点数
  - `float32_be`：32位浮点数（大端）
  - `float64`：64位浮点数
- `byte_order`：字节序
  - `ABCD`：大端序（标准）
  - `DCBA`：小端序
  - `BADC`：中间交换大端
  - `CDAB`：中间交换小端
- `register_count`：寄存器数量

#### Modbus 遥信映射 (mapping_signal.csv)
```csv
point_id,signal_name,slave_id,function_code,register_address,data_format,bit_position,register_count
1,breaker_status,1,2,0,bool,0,1
2,fault_alarm,1,2,0,bool,1,1
```

字段说明：
- `bit_position`：位位置（0-15），用于从寄存器中提取特定位

#### Modbus 遥调映射 (mapping_adjustment.csv)
```csv
point_id,signal_name,slave_id,function_code,register_address,data_format,byte_order,register_count
1,voltage_setpoint,1,6,200,float32_be,ABCD,2
2,current_limit,1,6,202,float32_be,ABCD,2
```

#### Modbus 遥控映射 (mapping_control.csv)
```csv
point_id,signal_name,slave_id,function_code,register_address,data_format,bit_position,register_count
1,breaker_open,1,5,100,bool,0,1
2,breaker_close,1,5,101,bool,0,1
```

## 环境变量配置

系统支持通过环境变量覆盖配置：

### 配置覆盖
```bash
# 覆盖服务名称
export COMSRV_SERVICE_NAME=MyComsrv

# 覆盖 Redis 地址
export COMSRV_SERVICE_REDIS_URL=redis://192.168.1.10:6379

# 覆盖日志级别
export COMSRV_SERVICE_LOGGING_LEVEL=debug
```

### CSV 基础路径
```bash
# 设置 CSV 文件的基础目录
export COMSRV_CSV_BASE_PATH=/opt/comsrv/config
```

## 配置最佳实践

### 1. 通道配置建议
- 每个物理设备使用独立通道
- 通道 ID 使用连续编号便于管理
- 通道名称使用有意义的描述

### 2. 轮询间隔设置
- 快速变化数据：100-500ms
- 常规监测数据：1000-5000ms
- 状态数据：5000-10000ms

### 3. 超时时间设置
- 局域网设备：500-1000ms
- 广域网设备：2000-5000ms
- 无线设备：5000-10000ms

### 4. 批量读取优化
- 连续地址的点位应配置在一起
- 批量大小不超过 125 个寄存器
- 考虑设备的最大响应长度限制

### 5. 日志配置建议
- 生产环境使用 info 级别
- 调试时使用 debug 级别
- 保留足够的历史日志用于故障分析

## 配置验证

启动服务前建议检查：

1. **文件格式**：使用 YAML 验证工具检查语法
2. **路径存在**：确保所有 CSV 文件路径正确
3. **网络连通**：测试设备 IP 和端口可达
4. **权限检查**：确保有日志目录的写入权限
5. **点位唯一**：检查 point_id 在通道内不重复

## 配置中心集成

### 概述
ComsRV 支持与中心化配置管理服务集成，实现配置的集中管理和动态更新。

### 配置加载优先级
系统按以下优先级加载配置（从高到低）：
1. **环境变量**：`COMSRV_` 前缀的环境变量
2. **配置中心**：从远程配置中心获取
3. **本地文件**：YAML/JSON/TOML 配置文件
4. **默认值**：代码中的默认配置

### 启用配置中心
```bash
# 设置配置中心地址
export CONFIG_CENTER_URL=http://config-center:8080

# 设置认证令牌（可选）
export CONFIG_CENTER_TOKEN=your-auth-token

# 设置缓存目录（可选，默认 /var/cache/comsrv）
export CONFIG_CACHE_DIR=/opt/comsrv/cache

# 设置缓存有效期（秒，默认 3600）
export CONFIG_CACHE_TTL=7200

# 启动服务
./comsrv --config config/default.yaml
```

### 配置中心 API 要求
配置中心需要实现以下 API 接口：

#### 获取完整配置
```
GET /api/v1/config/service/{service_name}
Response:
{
    "version": "2.0.1",
    "checksum": "md5_hash",
    "last_modified": "2024-01-08T10:00:00Z",
    "content": {
        "service": {...},
        "channels": [...]
    }
}
```

#### 获取配置项
```
GET /api/v1/config/service/{service_name}/item/{key}
Response:
{
    "key": "channels.0.parameters.host",
    "value": "192.168.1.100",
    "type": "string"
}
```

### 容错机制
- **自动降级**：配置中心不可用时自动使用本地配置
- **缓存策略**：成功获取的配置会缓存到本地
- **版本管理**：通过 checksum 验证配置完整性

### 配置示例

#### 开发环境（仅本地文件）
```bash
./comsrv --config config/dev.yaml
```

#### 测试环境（配置中心 + 本地备份）
```bash
export CONFIG_CENTER_URL=http://config-test:8080
./comsrv --config config/test.yaml
```

#### 生产环境（配置中心 + 环境变量覆盖）
```bash
export CONFIG_CENTER_URL=http://config.prod:8080
export CONFIG_CENTER_TOKEN=prod-token
export COMSRV_SERVICE_REDIS_URL=redis://redis-cluster:6379
export COMSRV_SERVICE_LOGGING_LEVEL=warn
./comsrv --config config/prod.yaml
```

### 动态配置更新（规划中）
未来版本将支持：
- WebSocket 配置变更通知
- 热更新支持的配置项
- 配置回滚机制

## 故障排查

常见配置问题：

1. **启动失败**
   - 检查 YAML 格式是否正确
   - 查看错误日志获取详细信息

2. **通道连接失败**
   - 验证 IP 地址和端口
   - 检查防火墙设置
   - 确认设备在线

3. **点表加载失败**
   - 检查 CSV 文件编码（应为 UTF-8）
   - 验证列名是否匹配
   - 确保 point_id 匹配

4. **数据读取异常**
   - 核对寄存器地址
   - 确认数据格式匹配
   - 检查字节序设置

5. **配置中心问题**
   - 检查 CONFIG_CENTER_URL 是否正确
   - 验证网络连接和防火墙
   - 查看缓存目录权限
   - 检查认证令牌是否有效