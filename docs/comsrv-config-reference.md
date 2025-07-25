# COMSRV 配置参考文档

## 概述

COMSRV 使用 YAML 格式的配置文件，支持多层次的配置结构。本文档详细说明各配置项的含义和用法。

## 配置文件结构

```yaml
service:          # 服务配置
channels:         # 通道配置列表
csv_base_path:    # CSV文件基础路径
```

## 1. 服务配置 (service)

```yaml
service:
  name: "comsrv"                              # 服务名称
  description: "Communication Service"         # 服务描述
  version: "0.1.0"                            # 版本号

  api:
    enabled: true                             # 是否启用 API
    bind_address: "0.0.0.0:3000"             # API 绑定地址

  redis:
    enabled: true                             # 是否启用 Redis
    url: "redis://localhost:6379"             # Redis 连接 URL

  logging:
    level: "info"                             # 日志级别：trace/debug/info/warn/error
    console: true                             # 是否输出到控制台
    file: "logs/comsrv.log"                  # 日志文件路径
    daily_rotation: true                      # 是否按日切分
    retention_days: 7                         # 日志保留天数
```

## 2. 通道配置 (channels)

### 2.1 基本配置

```yaml
channels:
  - id: 1001                                  # 通道ID (必须唯一)
    name: "ModbusTCP_CH1001"                  # 通道名称
    protocol: "modbus_tcp"                    # 协议类型
    enabled: true                             # 是否启用
```

### 2.2 协议参数 (parameters)

#### Modbus TCP 参数

```yaml
parameters:
  # TCP 连接参数
  host: "192.168.1.100"                      # Modbus TCP 服务器地址
  port: 502                                   # Modbus TCP 端口
  timeout: 5                                  # 连接超时（秒）
  retry_count: 3                              # 重试次数
  retry_delay: 2                              # 重试延迟（秒）

  # 轮询参数
  polling:
    interval: 1                               # 轮询间隔（秒）
    batch_size: 125                           # 批量读取大小（最大125个寄存器）
```

#### Modbus RTU 参数

```yaml
parameters:
  # 串口参数
  port: "/dev/ttyUSB0"                        # 串口设备路径
  baudrate: 9600                              # 波特率
  data_bits: 8                                # 数据位（7或8）
  parity: "N"                                 # 校验位：N(无)/E(偶)/O(奇)
  stop_bits: 1                                # 停止位（1或2）
  timeout: 5                                  # 超时（秒）
  retry_count: 3                              # 重试次数
  retry_delay: 2                              # 重试延迟（秒）

  # 轮询参数
  polling:
    interval: 2                               # 轮询间隔（秒）
    batch_size: 100                           # 批量读取大小
```

### 2.3 点表配置 (table_config)

```yaml
table_config:
  # 四遥点表路径
  four_telemetry_route: "ModbusTCP_CH1001"   # 点表目录
  four_telemetry_files:
    measurement_file: "measurement.csv"           # 遥测点表
    signal_file: "signal.csv"                 # 遥信点表
    adjustment_file: "adjustment.csv"         # 遥调点表
    control_file: "control.csv"               # 遥控点表

  # 协议映射路径
  protocol_mapping_route: "ModbusTCP_CH1001/mappings"
  protocol_mapping_file:
    measurement_mapping: "modbus_measurement.csv"
    signal_mapping: "modbus_signal.csv"
    adjustment_mapping: "modbus_adjustment.csv"
    control_mapping: "modbus_control.csv"
```

## 3. CSV 文件格式

### 3.1 遥测点表 (measurement.csv)

```csv
point_id,signal_name,data_type,scale,offset,unit,description
1,Voltage_A,float,0.1,0,V,A相电压
2,Current_A,uint16,0.01,0,A,A相电流
```

- **point_id**: 点位ID（从1开始）
- **signal_name**: 信号名称
- **data_type**: 数据类型（float/uint16/int16/uint32/int32）
- **scale**: 比例系数
- **offset**: 偏移量
- **unit**: 单位
- **description**: 描述

### 3.2 遥信/遥控点表 (signal.csv/control.csv)

```csv
point_id,signal_name,data_type,reverse,description
1,Breaker_Status,bool,0,断路器状态
2,Alarm_Flag,bool,1,告警标志（反向）
```

- **reverse**: 是否反向（0=正常，1=反向）

### 3.3 遥调点表 (adjustment.csv)

格式与遥测点表相同。

### 3.4 Modbus 映射文件

```csv
point_id,slave_id,function_code,register_address,bit_position,data_format,register_count,byte_order
1,1,3,40001,,float,2,ABCD
2,1,3,40003,,uint16,1,AB
3,2,1,10001,0,bool,1,
```

- **point_id**: 点位ID（与点表对应）
- **slave_id**: Modbus 从站地址（1-247）
- **function_code**: 功能码
  - 1: 读线圈状态（Read Coils）
  - 2: 读离散输入（Read Discrete Inputs）
  - 3: 读保持寄存器（Read Holding Registers）
  - 4: 读输入寄存器（Read Input Registers）
  - 5: 写单个线圈（Write Single Coil）
  - 6: 写单个寄存器（Write Single Register）
  - 15: 写多个线圈（Write Multiple Coils）
  - 16: 写多个寄存器（Write Multiple Registers）
- **register_address**: 寄存器地址
- **bit_position**: 位位置（用于布尔值，空表示使用整个寄存器）
- **data_format**: 数据格式（bool/uint16/int16/uint32/int32/float）
- **register_count**: 寄存器数量
- **byte_order**: 字节序（AB/BA/ABCD/DCBA/BADC/CDAB）

## 4. 重要说明

### 4.1 关于 slave_id 和 unit_identifier

- **slave_id**: Modbus 从站地址，应该在**点位映射文件**中指定，而不是在通道配置中
- **unit_identifier**: 这是 Modbus TCP 特有的概念，通常与 slave_id 相同，但在现代实现中很少使用

**正确做法**：
- 在通道级别的 `parameters` 中**不应该**包含 `slave_id` 或 `unit_identifier`
- 这些应该在 CSV 映射文件的每一行中指定，因为同一个通道可能访问多个不同的从站

### 4.2 批量读取优化

- Modbus 协议限制单次最多读取 125 个寄存器
- COMSRV 会自动将相邻的点位分组批量读取
- 通过 `batch_size` 参数可以调整批量大小

### 4.3 数据类型映射

| CSV 数据类型 | Modbus 格式 | 寄存器数 | 说明 |
|-------------|------------|---------|------|
| bool        | bool       | 1       | 单个位 |
| uint16      | uint16     | 1       | 无符号16位 |
| int16       | int16      | 1       | 有符号16位 |
| uint32      | uint32     | 2       | 无符号32位 |
| int32       | int32      | 2       | 有符号32位 |
| float       | float      | 2       | IEEE 754 浮点数 |

### 4.4 通道类型

- **modbus_tcp**: Modbus TCP/IP 协议
- **modbus_rtu**: Modbus RTU 串行协议
- **modbus_ascii**: Modbus ASCII 串行协议（未实现）

## 5. 配置示例

### 5.1 最小配置

```yaml
service:
  name: "comsrv"
  redis:
    url: "redis://localhost:6379"

csv_base_path: "./config"

channels:
  - id: 1001
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.100"
      port: 502
    table_config:
      four_telemetry_route: "channel_1001"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"
```

### 5.2 完整配置示例

参见 `config/docker.yml` 或 `config/massive-test.yml`

## 6. 配置验证

COMSRV 启动时会验证配置：
- 检查必需字段
- 验证参数类型
- 检查文件路径是否存在
- 验证点位ID唯一性

如果配置有误，COMSRV 会记录详细错误信息并退出。
