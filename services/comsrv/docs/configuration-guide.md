# Comsrv 配置指南

## 概述

Comsrv是VoltageEMS的工业协议网关服务，负责管理所有设备通信（Modbus、CAN、IEC60870等）。本文档详细说明comsrv的配置方法。

## 主要特性

- 插件架构支持协议扩展
- 发布数据到Redis Hash：`comsrv:{channelID}:{type}`
- 订阅控制命令：`cmd:{channelID}:control`
- 命令订阅在框架层处理，非协议插件层

## 配置文件结构

```
services/comsrv/
├── config/
│   ├── default.yml                    # 服务主配置
│   ├── channels.yml                   # 通道配置
│   └── {Protocol}_CH{ChannelID}/     # 通道点表配置
│       ├── measurement.csv            # YC - 遥测
│       ├── signal.csv                 # YX - 遥信
│       ├── adjustment.csv             # YT - 遥调
│       ├── control.csv                # YK - 遥控
│       └── mappings/                  # 协议映射
│           ├── modbus_measurement.csv
│           ├── modbus_signal.csv
│           ├── modbus_adjustment.csv
│           └── modbus_control.csv
```

## 服务配置 (default.yml)

```yaml
service:
  name: "comsrv"
  redis:
    url: "redis://localhost:6379"
  logging:
    level: "info"
    file: "logs/comsrv.log"
```

## 通道配置 (channels.yml)

```yaml
channels:
  - id: 1
    name: "ModbusTCP Channel 1001"
    protocol: "modbus_tcp"
    enabled: true
    table_config:
      # 四遥点表路径
      four_telemetry_route: "ModbusTCP_CH1001"
      four_telemetry_files:
        measurement_file: "measurement.csv"    # YC - 遥测
        signal_file: "signal.csv"              # YX - 遥信
        adjustment_file: "adjustment.csv"      # YT - 遥调
        control_file: "control.csv"            # YK - 遥控

      # 协议映射路径
      protocol_mapping_route: "ModbusTCP_CH1001/mappings"
      protocol_mapping_file:
        measurement_mapping: "modbus_measurement.csv"
        signal_mapping: "modbus_signal.csv"
        adjustment_mapping: "modbus_adjustment.csv"
        control_mapping: "modbus_control.csv"
```

## 四遥点表配置

### 遥测点表 (measurement.csv)

```csv
point_id,signal_name,data_type,scale,offset,unit,description
1,voltage_a,float,0.1,0,V,Phase A voltage
2,current_a,float,0.01,0,A,Phase A current
3,power_active,float,1.0,0,kW,Active power
4,power_reactive,float,1.0,0,kVar,Reactive power
```

### 遥信点表 (signal.csv)

```csv
point_id,signal_name,data_type,scale,offset,unit,description
20001,breaker_status,bool,1.0,0,,Breaker open/close status
20002,fault_alarm,bool,1.0,0,,Fault alarm signal
20003,communication_ok,bool,1.0,0,,Communication status
```

### 遥调点表 (adjustment.csv)

```csv
point_id,signal_name,data_type,scale,offset,unit,description
40001,voltage_setpoint,float,0.1,0,V,Voltage setpoint
40002,power_limit,float,1.0,0,kW,Power limit setpoint
```

### 遥控点表 (control.csv)

```csv
point_id,signal_name,data_type,scale,offset,unit,description
30001,breaker_control,bool,1.0,0,,Breaker open/close control
30002,reset_alarm,bool,1.0,0,,Reset alarm command
```

## 协议映射配置

### Modbus遥测映射 (modbus_measurement.csv)

```csv
point_id,slave_id,function_code,register_address,data_type,byte_order
1,1,3,0,float32,ABCD
2,1,3,2,float32,ABCD
3,1,3,4,float32,ABCD
4,1,3,6,float32,ABCD
```

### Modbus遥信映射 (modbus_signal.csv)

```csv
point_id,slave_id,function_code,register_address,bit_position
20001,1,2,0,0
20002,1,2,0,1
20003,1,2,0,2
```

## Redis数据架构

### Hash存储格式

```
comsrv:{channelID}:{type}   # type: m(measurement), s(signal), c(control), a(adjustment)
```

示例：
```
comsrv:1001:m → Hash{
  "10001": "220.123456",    # 6位小数精度
  "10002": "15.234567",
  ...
}
```

### Pub/Sub通道

发布格式：
```
Channel: comsrv:{channelID}:{type}
Message: "{pointID}:{value:.6f}"
```

订阅命令：
```
Channel: cmd:{channelID}:control
Channel: cmd:{channelID}:adjustment
```

## 数据标准

- **浮点精度**：6位小数 (例如："25.123456")
- **Hash访问**：O(1)查询性能
- **批量操作**：使用HGETALL、HMGET提高效率
- **无质量字段**：数据质量已从所有结构中移除

## 开发调试

### 监控Redis数据

```bash
# 查看通道数据
redis-cli hgetall "comsrv:1001:m"      # 查看通道1001的所有测量值
redis-cli hget "comsrv:1001:m" "10001" # 获取单个点位值
redis-cli hlen "comsrv:1001:m"         # 统计通道点位数

# 监控Pub/Sub
redis-cli psubscribe "comsrv:*"        # 监控所有comsrv通道
redis-cli subscribe "comsrv:1001:m"    # 监控特定通道
```

### 健康检查

```bash
curl http://localhost:8001/health
```

## 环境变量配置

支持通过环境变量覆盖默认配置：

```bash
RUST_LOG=debug cargo run                    # 调试日志
RUST_LOG=comsrv=debug cargo run            # 仅comsrv调试日志
```

## 注意事项

1. 所有浮点数使用6位小数精度
2. 命令订阅由框架层处理，协议插件不需要实现
3. 使用YAML配置服务和通道，CSV配置点表
4. Redis Hash存储提供O(1)访问性能，支持百万级点位
