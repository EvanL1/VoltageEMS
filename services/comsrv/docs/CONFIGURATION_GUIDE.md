# ComsrvConfiguration Guide - 通信服务配置指南

## 目录

1. [概述](#概述)
2. [目录结构](#目录结构)
3. [主配置文件](#主配置文件)
4. [通道配置](#通道配置)
5. [点表配置](#点表配置)
6. [数据源表配置](#数据源表配置)
7. [配置示例](#配置示例)
8. [最佳实践](#最佳实践)
9. [故障排除](#故障排除)

## 概述

VoltageEMS通信服务(comsrv)采用基于YAML的配置文件系统，支持多协议通道、四遥点表和数据源映射。配置系统设计原则：

- **标准化路径**: 采用统一的目录结构
- **分层管理**: ComBase点表与Protocol数据源分离
- **灵活配置**: 支持默认路径和自定义路径
- **向后兼容**: 保持与旧版本配置的兼容性

## 目录结构

### 推荐的标准目录结构

```
services/comsrv/
├── config/
│   ├── comsrv.yaml                    # 主配置文件
│   ├── channels/                      # 通道配置根目录
│   │   ├── channel_100_virtual/       # 通道目录格式: channel_{id}_{name}
│   │   │   ├── combase/              # ComBase四遥点表
│   │   │   │   ├── telemetry.csv     # 遥测点表 (模拟量测量值)
│   │   │   │   ├── signaling.csv     # 遥信点表 (数字量输入)
│   │   │   │   ├── control.csv       # 遥控点表 (数字量输出)
│   │   │   │   └── setpoint.csv      # 遥调点表 (模拟量设定值)
│   │   │   └── protocol/             # 协议数据源表
│   │   │       ├── modbus_tcp_source.csv      # Modbus TCP数据源
│   │   │       ├── modbus_rtu_source.csv      # Modbus RTU数据源
│   │   │       ├── calculation_source.csv     # 计算数据源
│   │   │       └── manual_source.csv          # 手动数据源
│   │   ├── channel_101_plc_main/
│   │   │   ├── combase/
│   │   │   └── protocol/
│   │   └── channel_102_sensor_network/
│   │       ├── combase/
│   │       └── protocol/
│   └── global/                        # 全局配置(可选)
│       ├── logging.yaml
│       └── redis.yaml
├── logs/                              # 日志目录
└── src/                              # 源代码
```

### 目录命名规则

- **通道目录**: `channel_{id}_{name_lowercase}`
- **ComBase目录**: 固定为 `combase`
- **Protocol目录**: 固定为 `protocol`
- **文件名**: 使用默认文件名或在配置中自定义

## 主配置文件

### 基本结构

```yaml
# ComsrvConfiguration File
version: "2.1"

# 服务配置
service:
  name: "comsrv"
  description: "Communication Service with Redis Storage"
  
  # 日志配置
  logging:
    level: "info"                      # 日志级别: debug, info, warn, error
    file: "logs/comsrv.log"           # 日志文件路径
    max_size: 104857600               # 单个日志文件最大大小(字节)
    max_files: 5                      # 保留的日志文件数量
    console: true                     # 是否输出到控制台
  
  # API配置
  api:
    enabled: true                     # 是否启用API服务
    bind_address: "127.0.0.1:8082"    # API绑定地址
    version: "v1"                     # API版本
  
  # Redis配置
  redis:
    enabled: true                     # 是否启用Redis存储
    connection_type: "Tcp"            # 连接类型: Tcp 或 Unix
    address: "127.0.0.1:6379"         # Redis服务器地址
    db: 1                            # Redis数据库编号(0-15)
    timeout_ms: 5000                 # 连接超时时间(毫秒)
    max_connections: 20              # 连接池最大连接数
    min_connections: 5               # 连接池最小连接数

# 默认路径配置
defaults:
  channels_root: "channels"           # 通道根目录名
  combase_dir: "combase"             # ComBase点表目录名
  protocol_dir: "protocol"           # 协议数据源目录名
  filenames:                         # 默认文件名
    telemetry: "telemetry.csv"
    signaling: "signaling.csv"
    control: "control.csv"
    setpoint: "setpoint.csv"
    modbus_tcp_source: "modbus_tcp_source.csv"
    modbus_rtu_source: "modbus_rtu_source.csv"
    calculation_source: "calculation_source.csv"
    manual_source: "manual_source.csv"

# 通道配置
channels:
  # 通道配置列表
  - id: 100
    # ...详见通道配置章节
```

## 通道配置

### 通道配置基本结构

每个通道包含以下主要部分：

```yaml
channels:
  - id: 100                           # 通道唯一标识符
    name: "TestChannel1"              # 通道名称(用于目录命名)
    description: "Virtual test channel for system monitoring"  # 通道描述
    protocol: "Virtual"               # 协议类型
    parameters:                       # 协议特定参数
      poll_rate: 2000                # 轮询周期(毫秒)
      max_retries: 3                 # 最大重试次数
      timeout: 1000                  # 超时时间(毫秒)
    point_table:                      # ComBase点表配置
      # ...详见点表配置章节
    source_tables:                    # 数据源表配置
      # ...详见数据源表配置章节
```

### 支持的协议类型

#### 1. Virtual Protocol (虚拟协议)

```yaml
protocol: "Virtual"
parameters:
  poll_rate: 2000                    # 轮询周期
  max_retries: 3                     # 重试次数
  timeout: 1000                      # 超时时间
```

#### 2. Modbus TCP

```yaml
protocol: "ModbusTcp"
parameters:
  host: "192.168.1.100"              # 目标主机IP
  port: 502                          # 端口号
  timeout: 3000                      # 超时时间
  max_retries: 3                     # 最大重试次数
  poll_rate: 1000                    # 轮询周期
  slave_id: 1                        # 从站ID
```

#### 3. Modbus RTU

```yaml
protocol: "ModbusRtu"
parameters:
  port: "/dev/ttyUSB0"               # 串口设备路径
  baud_rate: 9600                    # 波特率
  data_bits: 8                       # 数据位
  parity: "None"                     # 校验位: None, Even, Odd
  stop_bits: 1                       # 停止位
  timeout: 2000                      # 超时时间
  max_retries: 3                     # 最大重试次数
  poll_rate: 2000                    # 轮询周期
  slave_id: 2                        # 从站ID
```

## 点表配置

ComBase点表配置定义四遥点位信息。

### 点表配置结构

```yaml
point_table:
  enabled: true                       # 是否启用点表
  use_defaults: true                  # 是否使用默认路径结构
  # 可选: 自定义路径配置
  # directory: "custom/path/to/combase"
  # telemetry_file: "custom_telemetry.csv"
  # signaling_file: "custom_signaling.csv"
  # control_file: "custom_control.csv"
  # setpoint_file: "custom_setpoint.csv"
  watch_changes: true                 # 是否监控文件变化
  reload_interval: 30                 # 重新加载间隔(秒)
```

### 点表CSV文件格式

#### 遥测点表 (telemetry.csv)

```csv
id,name,chinese_name,data_source_type,source_table,source_data,scale,offset,unit,description,group
1,voltage_l1,L1电压,Protocol,modbus_tcp,1001,0.1,0,V,L1相电压,电压
2,current_l1,L1电流,Protocol,modbus_tcp,1002,0.01,0,A,L1相电流,电流
3,power_total,总功率,Calculation,calculation,2001,1.0,0,kW,计算总功率,功率
```

#### 遥信点表 (signaling.csv)

```csv
id,name,chinese_name,data_source_type,source_table,source_data,description,group
1,breaker_status,断路器状态,Protocol,modbus_tcp,2001,主断路器状态,保护
2,alarm_status,报警状态,Protocol,modbus_tcp,2002,系统报警状态,报警
3,manual_mode,手动模式,Manual,manual,3001,手动操作模式,操作
```

#### 遥控点表 (control.csv)

```csv
id,name,chinese_name,data_source_type,source_table,source_data,description,group
1,breaker_open,断路器分闸,Protocol,modbus_tcp,3001,断路器分闸操作,保护
2,breaker_close,断路器合闸,Protocol,modbus_tcp,3002,断路器合闸操作,保护
3,reset_alarm,复位报警,Manual,manual,4001,报警复位操作,操作
```

#### 遥调点表 (setpoint.csv)

```csv
id,name,chinese_name,data_source_type,source_table,source_data,scale,offset,unit,description,group
1,voltage_setpoint,电压设定值,Protocol,modbus_tcp,4001,0.1,0,V,电压调节设定值,设定
2,frequency_setpoint,频率设定值,Protocol,modbus_tcp,4002,0.01,0,Hz,频率调节设定值,设定
3,power_limit,功率限制,Manual,manual,5001,1.0,0,kW,功率限制设定,限制
```

### 数据源类型说明

- **Protocol**: 协议数据源，通过通信协议获取
- **Calculation**: 计算数据源，通过计算公式得出
- **Manual**: 手动数据源，人工输入或设定

## 数据源表配置

数据源表定义具体的数据获取方式和映射关系。

### 数据源表配置结构

```yaml
source_tables:
  enabled: true                       # 是否启用数据源表
  use_defaults: true                  # 是否使用默认路径结构
  # 可选: 自定义路径和文件名
  # directory: "custom/path/to/protocol"
  # modbus_tcp_source: "custom_modbus_tcp.csv"
  # modbus_rtu_source: "custom_modbus_rtu.csv"
  # calculation_source: "custom_calculation.csv"
  # manual_source: "custom_manual.csv"
  redis_prefix: "comsrv:source_tables:channel_100"  # Redis存储前缀
```

### 数据源CSV文件格式

#### Modbus TCP数据源 (modbus_tcp_source.csv)

```csv
source_id,protocol_type,slave_id,function_code,register_address,data_type,byte_order,bit_index,scaling_factor,description
1001,modbus_tcp,1,3,100,uint16,big_endian,,0.1,L1电压原始值
1002,modbus_tcp,1,3,101,uint16,big_endian,,0.01,L1电流原始值
2001,modbus_tcp,1,1,200,bool,big_endian,0,,断路器状态位
3001,modbus_tcp,1,5,300,bool,big_endian,,,断路器分闸操作
4001,modbus_tcp,1,6,400,uint16,big_endian,,0.1,电压设定寄存器
```

#### Modbus RTU数据源 (modbus_rtu_source.csv)

```csv
source_id,protocol_type,slave_id,function_code,register_address,data_type,byte_order,bit_index,scaling_factor,description
2001,modbus_rtu,2,3,200,uint16,big_endian,,0.1,传感器温度值
2002,modbus_rtu,2,3,201,uint16,big_endian,,0.01,传感器湿度值
2003,modbus_rtu,2,1,210,bool,big_endian,0,,传感器状态位
```

#### 计算数据源 (calculation_source.csv)

```csv
source_id,calculation_type,expression,source_points,update_interval_ms,description
2001,formula,"p1*p2*1.732",1001;1002,1000,三相功率计算
2002,average,"(p1+p2+p3)/3",1001;1003;1005,5000,平均电压计算
2003,sum,"p1+p2+p3",2001;2002;2003,2000,总功率求和
```

#### 手动数据源 (manual_source.csv)

```csv
source_id,manual_type,editable,default_value,value_type,description
3001,boolean,true,false,bool,手动模式开关
4001,setpoint,true,220.0,float,手动电压设定
5001,limit,true,1000.0,float,手动功率限制
```

## 配置示例

### 完整的配置文件示例

```yaml
version: "2.1"

service:
  name: "comsrv"
  description: "Communication Service with Redis Storage"
  
  logging:
    level: "info"
    file: "logs/comsrv.log"
    max_size: 104857600
    max_files: 5
    console: true
  
  api:
    enabled: true
    bind_address: "127.0.0.1:8082"
    version: "v1"
  
  redis:
    enabled: true
    connection_type: "Tcp"
    address: "127.0.0.1:6379"
    db: 1
    timeout_ms: 5000
    max_connections: 20
    min_connections: 5

defaults:
  channels_root: "channels"
  combase_dir: "combase"
  protocol_dir: "protocol"
  filenames:
    telemetry: "telemetry.csv"
    signaling: "signaling.csv"
    control: "control.csv"
    setpoint: "setpoint.csv"
    modbus_tcp_source: "modbus_tcp_source.csv"
    modbus_rtu_source: "modbus_rtu_source.csv"
    calculation_source: "calculation_source.csv"
    manual_source: "manual_source.csv"

channels:
  # 虚拟通道示例
  - id: 100
    name: "VirtualChannel"
    description: "Virtual test channel for simulation"
    protocol: "Virtual"
    parameters:
      poll_rate: 2000
      max_retries: 3
      timeout: 1000
    point_table:
      enabled: true
      use_defaults: true
      watch_changes: true
      reload_interval: 30
    source_tables:
      enabled: true
      use_defaults: true
      redis_prefix: "comsrv:source_tables:virtual_100"

  # Modbus TCP通道示例
  - id: 101
    name: "PLC_Main"
    description: "Main PLC via Modbus TCP"
    protocol: "ModbusTcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      timeout: 3000
      max_retries: 3
      poll_rate: 1000
      slave_id: 1
    point_table:
      enabled: true
      use_defaults: true
      watch_changes: true
      reload_interval: 60
    source_tables:
      enabled: true
      use_defaults: true
      redis_prefix: "comsrv:source_tables:plc_101"

  # Modbus RTU通道示例
  - id: 102
    name: "Sensor_Network"
    description: "Sensor network via Modbus RTU"
    protocol: "ModbusRtu"
    parameters:
      port: "/dev/ttyUSB0"
      baud_rate: 9600
      data_bits: 8
      parity: "None"
      stop_bits: 1
      timeout: 2000
      max_retries: 3
      poll_rate: 2000
      slave_id: 2
    point_table:
      enabled: true
      use_defaults: true
      watch_changes: true
      reload_interval: 120
    source_tables:
      enabled: true
      use_defaults: true
      redis_prefix: "comsrv:source_tables:sensor_102"
```

### 自定义路径配置示例

```yaml
channels:
  - id: 200
    name: "CustomChannel"
    description: "Channel with custom paths"
    protocol: "ModbusTcp"
    parameters:
      host: "192.168.1.200"
      port: 502
      timeout: 3000
      max_retries: 3
      poll_rate: 1000
      slave_id: 1
    point_table:
      enabled: true
      use_defaults: false              # 使用自定义路径
      directory: "config/custom_points/energy_meter"
      telemetry_file: "energy_telemetry.csv"
      signaling_file: "energy_signaling.csv"
      control_file: "energy_control.csv"
      setpoint_file: "energy_setpoint.csv"
      watch_changes: true
      reload_interval: 60
    source_tables:
      enabled: true
      use_defaults: false              # 使用自定义路径
      directory: "config/custom_sources/energy_meter"
      modbus_tcp_source: "energy_modbus_tcp.csv"
      calculation_source: "energy_calculations.csv"
      manual_source: "energy_manual.csv"
      redis_prefix: "comsrv:source_tables:energy_200"
```

## 最佳实践

### 1. 目录组织

- 使用标准的目录结构，便于维护和理解
- 通道目录名应包含ID和描述性名称
- 保持文件命名的一致性

### 2. 配置管理

- 使用版本控制管理配置文件
- 在生产环境中使用绝对路径
- 定期备份配置文件

### 3. 性能优化

- 合理设置轮询周期，避免过于频繁的通信
- 根据网络条件调整超时时间和重试次数
- 对于大量点位，考虑分组配置

### 4. 安全考虑

- 不要在配置文件中直接存储敏感信息
- 使用环境变量或外部配置管理敏感参数
- 限制配置文件的访问权限

### 5. 监控和调试

- 启用文件变化监控，便于动态更新配置
- 设置适当的日志级别
- 使用描述性的通道和点位名称

### 6. 数据源设计

- 合理设计数据源ID，避免冲突
- 为计算点配置合适的更新间隔
- 手动点应设置合理的默认值

## 故障排除

### 常见问题

#### 1. 配置文件解析错误

**症状**: 服务启动失败，提示YAML解析错误
**解决方案**:

- 检查YAML语法，确保缩进正确
- 验证所有必需字段都已配置
- 使用YAML验证工具检查格式

#### 2. 通道连接失败

**症状**: 通道状态显示离线
**解决方案**:

- 检查网络连接和设备状态
- 验证IP地址、端口、串口配置
- 增加超时时间和重试次数

#### 3. 点表加载失败

**症状**: 点表数据为空或加载错误
**解决方案**:

- 检查CSV文件路径和权限
- 验证CSV文件格式和编码
- 查看日志文件获取详细错误信息

#### 4. 数据源映射错误

**症状**: 点表数据不更新或数据异常
**解决方案**:

- 检查数据源表中的source_id映射
- 验证寄存器地址和数据类型
- 确认缩放因子和偏移量设置

### 调试技巧

#### 1. 启用调试日志

```yaml
logging:
  level: "debug"
  console: true
```

#### 2. 使用测试配置

创建简化的测试配置，逐步添加复杂功能

#### 3. 监控Redis数据

使用Redis客户端查看存储的配置和数据

#### 4. 分步验证

- 首先验证基本连接
- 然后测试点表加载
- 最后验证数据映射

---

## 相关文档

- [API接口文档](API_REFERENCE.md)
- [协议扩展指南](PROTOCOL_EXTENSION.md)
- [性能调优指南](PERFORMANCE_TUNING.md)
- [部署运维指南](DEPLOYMENT_GUIDE.md)

---

*最后更新: 2024年12月*
*版本: 2.1*
