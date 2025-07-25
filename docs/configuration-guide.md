# COMSRV 配置指南

## 概述

COMSRV使用分层配置系统，支持YAML配置文件和CSV点表。本指南详细说明了各种配置文件的格式和用法。

## 配置文件结构

### 主配置文件 (docker.yml)

使用 `table_config` 格式：

```yaml
channels:
  - id: 1001
    name: "ModbusTCP_Channel"
    protocol: "modbus_tcp"
    parameters:
      host: "modbus-sim-1"
      port: 5502
      timeout: 5
      
    table_config:
      four_telemetry_route: "ModbusTCP_Real_01"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"
        signal_file: "signal.csv"
        adjustment_file: "adjustment.csv"
        control_file: "control.csv"
      protocol_mapping_route: "ModbusTCP_Real_01/mappings"
      protocol_mapping_file:
        telemetry_mapping: "modbus_telemetry.csv"
        signal_mapping: "modbus_signal.csv"
        adjustment_mapping: "modbus_adjustment.csv"
        control_mapping: "modbus_control.csv"
```

## CSV文件格式

### 四遥点表文件

所有四遥点表文件都位于 `{four_telemetry_route}/` 目录下。**注意：点表中不包含address字段，地址信息在协议映射文件中定义。**

#### 1. 遥测点表 (telemetry.csv)

```csv
point_id,signal_name,data_type,scale,offset,unit,description
1,电压A相,uint16,0.1,0,V,A相电压
2,电流A相,uint16,0.01,0,A,A相电流
3,有功功率,uint32,0.1,0,kW,总有功功率
```

**字段说明：**
- `point_id`: 点位ID (唯一)
- `signal_name`: 信号名称
- `data_type`: 数据类型 (uint16, int16, uint32, int32, float32)
- `scale`: 缩放系数 (原始值 * scale + offset = 工程值)
- `offset`: 偏移量
- `unit`: 工程单位 (可选)
- `description`: 描述 (可选)

#### 2. 遥信点表 (signal.csv)

```csv
point_id,signal_name,data_type,reverse,description
1,开关状态1,bool,false,开关1状态
2,开关状态2,bool,true,开关2状态（反逻辑）
```

**字段说明：**
- `point_id`: 点位ID (唯一)
- `signal_name`: 信号名称
- `data_type`: 数据类型 (通常为bool)
- `reverse`: 反逻辑标志 (true表示1->0, 0->1转换)
- `description`: 描述 (可选)

#### 3. 遥控点表 (control.csv)

```csv
point_id,signal_name,data_type,reverse,description
1,开关控制1,bool,false,开关1控制
2,开关控制2,bool,true,开关2控制（反逻辑）
```

**字段说明：**
- `point_id`: 点位ID (唯一)
- `signal_name`: 信号名称  
- `data_type`: 数据类型 (通常为bool)
- `reverse`: 反逻辑标志 (true表示1->0, 0->1转换)
- `description`: 描述 (可选)

#### 4. 遥调点表 (adjustment.csv)

```csv
point_id,signal_name,data_type,scale,offset,unit,description
1,电压设定值,uint16,0.1,0,V,电压设定值
2,电流设定值,uint16,0.01,0,A,电流设定值
```

**字段说明：**
- `point_id`: 点位ID (唯一)
- `signal_name`: 信号名称
- `data_type`: 数据类型 (uint16, int16, uint32, int32, float32)
- `scale`: 缩放系数
- `offset`: 偏移量
- `unit`: 工程单位 (可选)
- `description`: 描述 (可选)

### 协议映射文件

协议映射文件位于 `{protocol_mapping_route}/` 目录下，用于将四遥点位映射到具体的协议地址。**现在是四遥分别映射，不是统一映射。**

#### Modbus映射文件格式

```csv
point_id,slave_id,function_code,register_address,bit_position,data_format,register_count,byte_order
1,1,3,40001,,uint16,1,ABCD
2,1,3,40002,,uint16,1,ABCD
3,1,3,40007,,uint32,2,ABCD
4,1,2,10001,0,bool,1,
```

**字段说明：**
- `point_id`: 对应四遥表中的点位ID
- `slave_id`: Modbus从站ID
- `function_code`: Modbus功能码 (1-读线圈, 2-读离散输入, 3-读保持寄存器, 4-读输入寄存器, 5-写单个线圈, 6-写单个寄存器)
- `register_address`: 寄存器地址
- `bit_position`: 位位置 (用于布尔值, 可选)
- `data_format`: 数据格式 (uint16, int16, uint32, int32, float32, bool)
- `register_count`: 寄存器数量 (通常为1，32位数据为2)
- `byte_order`: 字节序 (ABCD, BADC, CDAB, DCBA, 可选)

**字节序说明：**
- `ABCD`: 大端序，高字节在前 (默认)
- `BADC`: 字节交换
- `CDAB`: 字交换  
- `DCBA`: 小端序，低字节在前

## 目录结构示例

```
config/
├── docker.yml
├── ModbusTCP_Real_01/
│   ├── telemetry.csv
│   ├── signal.csv
│   ├── control.csv
│   ├── adjustment.csv
│   └── mappings/
│       ├── modbus_telemetry.csv
│       ├── modbus_signal.csv
│       ├── modbus_control.csv
│       └── modbus_adjustment.csv
└── ModbusTCP_Real_02/
    └── ... (相同结构)
```

## 配置验证

系统启动时会自动验证：
1. CSV文件格式正确性
2. 点位ID唯一性  
3. 映射文件中的point_id在四遥表中存在
4. 数据类型兼容性
5. 字节序格式正确性

## 常见问题

### Q: 为什么点表中没有address字段？
A: address属于协议特有属性，在协议映射文件中定义，保持四遥表的协议无关性。

### Q: 如何处理32位数据？
A: 设置 `register_count: 2` 并使用相应的数据类型 (uint32, int32, float32)，同时指定正确的字节序。

### Q: 布尔值如何映射？
A: 使用 `bit_position` 字段指定在寄存器中的位位置。

### Q: reverse字段的作用？
A: 用于遥信和遥控的逻辑反转，true表示读取或发送时进行1->0, 0->1转换。

### Q: 什么时候需要指定byte_order？
A: 处理多寄存器数据（如uint32, float32）时，根据设备的字节序要求指定正确的顺序。