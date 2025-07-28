# 配置方案

## 1. 概述

本文档描述 ComSrv 的配置方案。采用单一配置文件加 CSV 点表的方式，保持配置简单直观。

## 2. 配置文件组织

```
config/
├── comsrv.yaml                # 主配置文件（包含所有配置）
└── channels/                   # 通道点表配置
    ├── 1001/                   # 通道 1001
    │   ├── measurement.csv     # 遥测点表
    │   ├── signal.csv          # 遥信点表
    │   ├── control.csv         # 遥控点表
    │   └── adjustment.csv      # 遥调点表
    └── 1002/                   # 通道 1002
        └── ...
```

## 3. 主配置文件结构

所有配置都在单一的 `comsrv.yaml` 文件中定义：

```yaml
# comsrv.yaml
service:
  name: "comsrv"
  version: "2.0.0"
  description: "Communication Service"
  
  # API 配置
  api:
    host: "0.0.0.0"
    port: 8001
    workers: 4  # 默认: CPU核心数
    
  # Redis 配置
  redis:
    enabled: true
    url: "redis://localhost:6379"
    pool_size: 10
    timeout_ms: 5000
    
  # 日志配置
  logging:
    level: "info"  # trace, debug, info, warn, error
    format: "pretty"  # pretty, json
    console: true
    file: "logs/comsrv.log"
    rotation:
      strategy: "daily"  # daily, size
      max_files: 7

# 通道配置列表
channels:
  # Modbus TCP 示例
  - id: 1001
    name: "南区电表"
    description: "南区配电室智能电表"
    protocol: "modbus_tcp"
    
    # 协议参数直接定义
    parameters:
      host: "192.168.1.100"
      port: 502
      slave_id: 1
      timeout_ms: 3000
      # 轮询配置
      polling_enabled: true
      polling_interval_ms: 1000
      # 批量读取配置
      batch_enabled: true
      batch_max_size: 100
      batch_max_gap: 5
      
    # 通道特定日志（可选）
    logging:
      enabled: true
      level: "debug"
      file: "logs/channel_1001.log"
      protocol_details: true
      
    # 点表配置
    table_config:
      four_remote_route: "channels/1001"
      four_remote_files:
        measurement_file: "measurement.csv"
        signal_file: "signal.csv"
        control_file: "control.csv"
        adjustment_file: "adjustment.csv"
        
  # Modbus RTU 示例
  - id: 3001
    name: "RTU设备"
    description: "串口连接的 RTU 设备"
    protocol: "modbus_rtu"
    
    parameters:
      port: "/dev/ttyUSB0"
      baud_rate: 9600
      data_bits: 8
      stop_bits: 1
      parity: "None"  # None, Even, Odd
      slave_id: 1
      timeout_ms: 1000
      
    table_config:
      four_remote_route: "channels/3001"
      # 使用默认文件名
```

## 4. CSV 点表格式

### 4.1 遥测点表 (measurement.csv)

```csv
point_id,signal_name,data_type,scale,offset,unit,description
1,voltage_a,float,0.1,0,V,A相电压
2,voltage_b,float,0.1,0,V,B相电压
3,voltage_c,float,0.1,0,V,C相电压
4,current_a,float,0.01,0,A,A相电流
5,power_active,float,1.0,0,kW,有功功率
6,power_reactive,float,1.0,0,kVar,无功功率
7,frequency,float,0.01,0,Hz,频率
8,energy_total,float,0.001,0,kWh,总电能
```

### 4.2 遥信点表 (signal.csv)

```csv
point_id,signal_name,data_type,reverse,description
1,breaker_status,bool,0,断路器状态
2,fault_alarm,bool,0,故障告警
3,door_open,bool,1,柜门状态(1=反转)
4,communication_ok,bool,0,通信状态
```

### 4.3 遥控点表 (control.csv)

```csv
point_id,signal_name,data_type,description
1,breaker_control,bool,断路器控制
2,reset_alarm,bool,复位告警
```

### 4.4 遥调点表 (adjustment.csv)

```csv
point_id,signal_name,data_type,scale,offset,unit,description
1,voltage_setpoint,float,0.1,0,V,电压设定值
2,power_limit,float,1.0,0,kW,功率限制
```

## 5. 环境变量支持

使用 `COMSRV__` 前缀覆盖配置值：

```bash
# 覆盖服务配置
COMSRV__SERVICE__NAME=comsrv-prod
COMSRV__SERVICE__API__PORT=8080
COMSRV__SERVICE__REDIS__URL=redis://redis-cluster:6379
COMSRV__SERVICE__LOGGING__LEVEL=debug

# 注意：通道配置不支持环境变量覆盖
```

## 6. 配置加载流程

```rust
1. 加载主配置文件 (comsrv.yaml)
2. 应用环境变量覆盖
3. 验证配置有效性
4. 对每个通道：
   - 加载对应的 CSV 点表文件
   - 合并形成完整的通道配置
```

## 7. 配置示例

### 7.1 最小配置

```yaml
# 最小可运行配置
service:
  name: "comsrv"

channels:
  - id: 1001
    name: "Test Channel"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      slave_id: 1
```

### 7.2 生产环境配置

```yaml
service:
  name: "comsrv-prod"
  api:
    host: "0.0.0.0"
    port: 8001
    workers: 8
  redis:
    enabled: true
    url: "redis://redis-cluster:6379"
    pool_size: 20
    timeout_ms: 5000
  logging:
    level: "info"
    format: "json"
    file: "/var/log/comsrv/comsrv.log"
    rotation:
      strategy: "size"
      max_size_mb: 100
      max_files: 10

channels:
  # 多个通道配置...
```

## 8. 配置验证

ConfigManager 在加载时会验证：

1. **必填字段**：通道ID、名称、协议类型
2. **协议参数**：根据协议类型验证必需参数
3. **点表文件**：检查配置的 CSV 文件是否存在
4. **数据类型**：验证参数类型是否正确

## 9. 注意事项

1. **路径处理**：所有相对路径都相对于配置文件所在目录
2. **协议参数**：不同协议需要不同的参数，详见协议文档
3. **点表 ID**：每种类型的点位 ID 都从 1 开始
4. **日志文件**：确保日志目录有写入权限
5. **性能考虑**：大量通道时考虑调整 Redis 连接池大小

## 10. 优势

- **简单直观**：所有配置在一个文件中
- **灵活性高**：每个通道可以有完全不同的参数
- **易于维护**：不需要在多个文件间跳转
- **点表独立**：CSV 格式便于批量编辑和版本控制