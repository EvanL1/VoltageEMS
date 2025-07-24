# COMSRV 点表映射配置指南

## 概述

COMSRV 使用点表和映射文件来定义通道中的数据点。这种方式提供了灵活的配置能力，可以轻松适配不同的设备和协议。

## 配置结构

### 1. 通道配置

在配置文件中定义通道时，需要指定点表配置：

```yaml
channels:
  - id: 1001
    name: "ModbusTCP_Test_01"
    protocol: "modbus_tcp"
    enabled: true
    
    # 点表配置
    points_config:
      base_path: "./config/test-points"
      telemetry: "telemetry.csv"              # 遥测点表
      telemetry_mapping: "mappings/modbus_telemetry.csv"  # 遥测映射
      signal: "signal.csv"                    # 信号点表
      signal_mapping: "mappings/modbus_signal.csv"        # 信号映射
      control: "control.csv"                  # 控制点表
      control_mapping: "mappings/modbus_control.csv"      # 控制映射
      adjustment: "adjustment.csv"            # 调节点表
      adjustment_mapping: "mappings/modbus_adjustment.csv" # 调节映射
```

### 2. 点表文件格式

#### 遥测点表 (telemetry.csv)
```csv
point_id,signal_name,unit,description,scale,offset
10001,voltage_a,V,A相电压,0.1,0
10002,voltage_b,V,B相电压,0.1,0
10003,voltage_c,V,C相电压,0.1,0
```

- `point_id`: 唯一点位标识符
- `signal_name`: 信号名称
- `unit`: 单位
- `description`: 描述
- `scale`: 缩放因子
- `offset`: 偏移量

#### 信号点表 (signal.csv)
```csv
point_id,signal_name,description,normal_state
20001,breaker_status,断路器状态,0
20002,switch_position,开关位置,0
```

- `normal_state`: 正常状态值（0 或 1）

#### 控制点表 (control.csv)
```csv
point_id,signal_name,description,control_type
30001,breaker_control,断路器控制,toggle
30002,switch_control,开关控制,toggle
30003,reset_alarm,复位报警,pulse
```

- `control_type`: 控制类型 (toggle/pulse/value)

#### 调节点表 (adjustment.csv)
```csv
point_id,signal_name,unit,description,min_value,max_value,scale
40001,voltage_setpoint,V,电压设定值,200,250,0.1
40002,current_limit,A,电流限值,0,100,0.1
```

- `min_value`: 最小值
- `max_value`: 最大值

### 3. 映射文件格式

映射文件定义了点位如何映射到具体的协议地址。

#### Modbus 遥测映射 (modbus_telemetry.csv)
```csv
point_id,address,data_type,byte_order,function_code
10001,40001,float32,ABCD,3
10002,40003,float32,ABCD,3
10009,40017,uint16,,3
```

- `address`: Modbus 地址
- `data_type`: 数据类型 (uint16/int16/uint32/int32/float32/float64)
- `byte_order`: 字节序 (ABCD/DCBA/BADC/CDAB)
- `function_code`: 功能码 (3=读保持寄存器, 4=读输入寄存器)

## 数据类型说明

### Modbus 数据类型

- `uint16`: 16位无符号整数（1个寄存器）
- `int16`: 16位有符号整数（1个寄存器）
- `uint32`: 32位无符号整数（2个寄存器）
- `int32`: 32位有符号整数（2个寄存器）
- `float32`: 32位浮点数（2个寄存器）
- `float64`: 64位浮点数（4个寄存器）

### 字节序

- `ABCD`: 大端序（网络字节序）
- `DCBA`: 小端序
- `BADC`: 中间大端序
- `CDAB`: 中间小端序

## 使用示例

### 1. 创建新的通道配置

```yaml
channels:
  - id: 2001
    name: "PowerMeter_01"
    protocol: "modbus_tcp"
    enabled: true
    
    connection:
      host: "192.168.1.100"
      port: 502
      timeout: 5
    
    device:
      slave_id: 1
    
    points_config:
      base_path: "./config/power-meter"
      telemetry: "telemetry.csv"
      telemetry_mapping: "mappings/modbus_telemetry.csv"
```

### 2. 定义电力仪表的遥测点

telemetry.csv:
```csv
point_id,signal_name,unit,description,scale,offset
50001,voltage_line_ab,V,AB线电压,0.1,0
50002,voltage_line_bc,V,BC线电压,0.1,0
50003,voltage_line_ca,V,CA线电压,0.1,0
50004,total_active_power,kW,总有功功率,0.001,0
50005,total_reactive_power,kVar,总无功功率,0.001,0
```

### 3. 创建映射文件

mappings/modbus_telemetry.csv:
```csv
point_id,address,data_type,byte_order,function_code
50001,30001,uint32,ABCD,4
50002,30003,uint32,ABCD,4
50003,30005,uint32,ABCD,4
50004,30021,int32,ABCD,4
50005,30023,int32,ABCD,4
```

## Redis 数据存储

配置完成后，COMSRV 会将数据发布到 Redis：

- 遥测数据: `1001:m:10001` → `{value: 220.5, timestamp: 1234567890}`
- 信号数据: `1001:s:20001` → `{value: 1, timestamp: 1234567890}`
- 控制命令: 订阅 `cmd:1001:control`
- 调节命令: 订阅 `cmd:1001:adjustment`

## 最佳实践

1. **点位ID规划**
   - 遥测: 10000-19999
   - 信号: 20000-29999
   - 控制: 30000-39999
   - 调节: 40000-49999

2. **命名规范**
   - 使用有意义的 signal_name
   - 保持命名一致性
   - 避免特殊字符

3. **性能优化**
   - 相邻地址的点位放在一起，便于批量读取
   - 合理设置轮询间隔
   - 使用合适的 batch_size

4. **维护建议**
   - 使用版本控制管理点表文件
   - 记录点位变更历史
   - 定期验证映射正确性