# ComSrv 配置指南

## 概述

ComSrv（通信服务）是 VoltageEMS 的核心数据采集服务，负责通过多种工业协议（Modbus TCP/RTU、Virtual 等）与现场设备通信，采集实时数据并存储到 Redis。

## 配置文件结构

ComSrv 使用分层配置系统：
- **主配置文件**: `comsrv.yaml` - 定义服务参数和通道列表
- **点表文件**: `{channel_id}/*.csv` - 定义数据点的属性
- **映射文件**: `{channel_id}/mapping/*.csv` - 定义协议特定的映射关系

```
config/
├── comsrv.yaml                    # 主配置文件
└── {channel_id}/                  # 通道配置目录
    ├── telemetry.csv              # 遥测点表
    ├── signal.csv                 # 遥信点表
    ├── control.csv                # 遥控点表
    ├── adjustment.csv             # 遥调点表
    └── mapping/                   # 协议映射
        ├── telemetry_mapping.csv
        ├── signal_mapping.csv
        ├── control_mapping.csv
        └── adjustment_mapping.csv
```

## 主配置文件 (comsrv.yaml)

### 基本结构

```yaml
# CSV 文件基础路径
csv_base_path: "/app/config"

# 通道配置列表
channels:
  - id: 1001                       # 通道唯一标识
    name: "channel_name"           # 通道名称
    protocol: "modbus_tcp"         # 协议类型
    parameters:                    # 协议参数
      host: "192.168.1.100"
      port: 502
      timeout_secs: 5
      polling_interval_ms: 1000
```

### 支持的协议

#### 1. Modbus TCP

```yaml
protocol: "modbus_tcp"
parameters:
  host: "192.168.1.100"          # Modbus 服务器地址
  port: 502                      # 端口号（默认 502）
  timeout_secs: 5                # 超时时间（秒）
  polling_interval_ms: 1000      # 轮询间隔（毫秒）
```

#### 2. Modbus RTU

```yaml
protocol: "modbus_rtu"
parameters:
  port: "/dev/ttyUSB0"           # 串口设备路径
  baud_rate: 9600                # 波特率
  data_bits: 8                   # 数据位（7 或 8）
  stop_bits: 1                   # 停止位（1 或 2）
  parity: "none"                 # 校验位（none, even, odd）
  timeout_secs: 5                # 超时时间（秒）
  polling_interval_ms: 1000      # 轮询间隔（毫秒）
```

#### 3. Virtual（虚拟协议）

```yaml
protocol: "virt"
parameters:
  polling_interval_ms: 1000      # 数据生成间隔
  # 虚拟协议用于测试，自动生成模拟数据
```

### 环境变量覆盖

支持通过环境变量覆盖配置：

```bash
# 覆盖 CSV 基础路径
CSV_BASE_PATH=/custom/path

# 覆盖 Redis 连接
REDIS_URL=redis://localhost:6379
```

## CSV 点表配置

### 遥测点表 (telemetry.csv)

定义模拟量数据点的属性：

```csv
point_id,signal_name,scale,offset,unit,reverse,data_type
1,温度,0.1,0.0,℃,false,float
2,压力,0.01,0.0,MPa,false,float
3,流量,0.1,0.0,m³/h,false,float
```

**字段说明：**
- `point_id`: 点号，在通道内唯一
- `signal_name`: 信号名称
- `scale`: 缩放系数（实际值 = 原始值 × scale + offset）
- `offset`: 偏移量
- `unit`: 工程单位
- `reverse`: 是否反向（用于控制类信号）
- `data_type`: 数据类型（float, int16, uint16, int32, uint32）

### 遥信点表 (signal.csv)

定义开关量数据点：

```csv
point_id,signal_name,normal_state,alarm_on_change,description
1,断路器状态,0,true,主断路器开关状态
2,故障信号,0,true,设备故障信号
```

### 遥控点表 (control.csv)

定义控制输出点：

```csv
point_id,signal_name,control_type,min_value,max_value,description
1,断路器控制,binary,0,1,控制主断路器开合
```

### 遥调点表 (adjustment.csv)

定义模拟量输出点：

```csv
point_id,signal_name,min_value,max_value,unit,description
1,功率设定,0,100,MW,设定输出功率
```

## 协议映射文件

### Modbus 映射 (telemetry_mapping.csv)

定义 Modbus 寄存器与点号的映射关系：

```csv
point_id,slave_id,function_code,register_address,data_type,byte_order
1,1,3,0,float32,ABCD
2,1,3,2,float32,ABCD
3,1,3,4,uint16,AB
```

**字段说明：**
- `point_id`: 对应点表中的点号
- `slave_id`: Modbus 从站地址
- `function_code`: 功能码（3=读保持寄存器，4=读输入寄存器）
- `register_address`: 寄存器起始地址
- `data_type`: Modbus 数据类型
  - `uint16`: 单寄存器无符号整数
  - `int16`: 单寄存器有符号整数
  - `uint32`: 双寄存器无符号整数
  - `int32`: 双寄存器有符号整数
  - `float32`: 双寄存器浮点数
- `byte_order`: 字节序
  - 16位: `AB` 或 `BA`
  - 32位: `ABCD`, `DCBA`, `BADC`, `CDAB`

## Redis 数据存储

采集的数据自动存储到 Redis，使用以下键格式：

```
comsrv:{channel_id}:T    # 遥测数据 Hash
comsrv:{channel_id}:S    # 遥信数据 Hash
comsrv:{channel_id}:C    # 遥控数据 Hash
comsrv:{channel_id}:A    # 遥调数据 Hash
```

每个 Hash 的字段为 point_id，值为实际数据。

## 高级配置

### 日志配置

通过命令行参数控制日志级别：

```bash
# 启动时指定日志级别
./comsrv -l debug

# 可选级别: trace, debug, info, warn, error
```

### 性能优化

```yaml
# 在 channel parameters 中配置
parameters:
  polling_interval_ms: 100      # 降低轮询间隔提高实时性
  batch_size: 50                # 批量读取寄存器数量
  max_retries: 3                # 最大重试次数
  retry_delay_ms: 1000          # 重试延迟
```

### 多从站配置

同一通道可以配置多个从站设备，在映射文件中使用不同的 `slave_id`：

```csv
# telemetry_mapping.csv
point_id,slave_id,function_code,register_address,data_type,byte_order
1,1,3,0,float32,ABCD    # 从站1的数据
2,2,3,0,float32,ABCD    # 从站2的数据
3,3,3,0,float32,ABCD    # 从站3的数据
```

## 配置验证

使用验证脚本检查配置：

```bash
./scripts/validate-comsrv-config.sh config/comsrv
```

验证内容包括：
- YAML 语法正确性
- CSV 文件格式
- 点号唯一性
- 映射关系完整性

## 故障排除

### 常见问题

1. **CSV 文件未加载**
   - 检查 `csv_base_path` 路径是否正确
   - 确认文件权限可读
   - 查看日志中的错误信息

2. **Modbus 连接失败**
   - 确认网络连通性
   - 检查防火墙设置
   - 验证从站地址和端口

3. **数据未写入 Redis**
   - 检查 Redis 连接状态
   - 确认 Redis 服务运行正常
   - 查看 Redis 日志

### 调试模式

启用调试模式获取详细信息：

```bash
# 启动调试模式
./comsrv -d -l trace

# 或设置环境变量
RUST_LOG=debug,comsrv=trace ./comsrv
```

## 最佳实践

1. **合理设置轮询间隔**：根据数据变化频率调整，避免过度轮询
2. **使用批量读取**：相邻寄存器尽量连续排列，提高读取效率
3. **配置超时重试**：网络不稳定时适当增加重试次数
4. **监控通道状态**：通过 API 端点监控各通道运行状态
5. **定期备份配置**：保存 CSV 和 YAML 文件的备份
