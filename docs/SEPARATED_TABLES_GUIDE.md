# 四遥+协议分离表架构使用指南

## 📋 概述

VoltageEMS采用**四遥+协议分离**的CSV表架构，将工程意义上的点位定义与通讯协议参数完全分离，符合工业实践。

## 🏗️ 架构特点

### ✅ 优势
- **职责分离**: 四遥点表专注工程定义，协议映射表专注通讯参数
- **工程友好**: 按遥测、遥信、遥调、遥控分类，符合工程师习惯
- **维护简单**: 独立维护，互不干扰
- **扩展方便**: 新增设备只需创建对应目录和表文件

### 📁 目录结构
```
config/
├── TankFarmModbusTCP/              # 罐区Modbus TCP
│   ├── telemetry.csv               # 遥测点表
│   ├── signal.csv                  # 遥信点表  
│   ├── adjustment.csv              # 遥调点表
│   ├── control.csv                 # 遥控点表
│   ├── mapping_telemetry.csv       # 遥测协议映射
│   ├── mapping_signal.csv          # 遥信协议映射
│   ├── mapping_adjustment.csv      # 遥调协议映射
│   └── mapping_control.csv         # 遥控协议映射
├── PumpStationModbusRTU/           # 泵站Modbus RTU
│   └── ...                         # 同上结构
└── EngineCANBus/                   # 引擎CAN总线
    └── ...                         # 同上结构
```

## 📊 表格式规范

### 1. 四遥点表格式

**字段定义**:
```csv
point_id,signal_name,chinese_name,scale,offset,unit
```

**字段说明**:
- `point_id`: 点位唯一标识符 (整数)
- `signal_name`: 信号英文名称
- `chinese_name`: 信号中文名称  
- `scale`: 缩放系数
- `offset`: 偏移量
- `unit`: 工程单位 (可选，无单位留空)

**示例**:
```csv
point_id,signal_name,chinese_name,scale,offset,unit
1001,TANK_01_LEVEL,1号罐液位,0.01,0,m
1013,PUMP_01_STATUS,1号泵状态,1,0,
```

### 2. 协议映射表格式

**字段定义**:
```csv
point_id,signal_name,address,data_type,data_format,number_of_bytes,bit_location,description
```

**字段说明**:
- `point_id`: 对应四遥点表的点位ID
- `signal_name`: 信号名称 (与四遥表一致)
- `address`: 协议地址
- `data_type`: 数据类型 (bool, uint16, float32等)
- `data_format`: 数据格式 (big_endian, little_endian)
- `number_of_bytes`: 字节数
- `bit_location`: 位偏移 (bool类型使用)
- `description`: 描述 (可选)

**示例**:
```csv
point_id,signal_name,address,data_type,data_format,number_of_bytes,bit_location,description
1001,TANK_01_LEVEL,1001,float32,big_endian,4,,1号储油罐液位测量
1013,PUMP_01_STATUS,2001,bool,big_endian,1,0,1号输送泵运行状态
```

## 🔧 配置文件

### YAML配置示例

```yaml
channels:
  - id: 1
    name: "Tank Farm Modbus TCP"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      slave_id: 1
    table_config:
      # 四遥点表配置
      four_telemetry_route: "config/TankFarmModbusTCP"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"
        signal_file: "signal.csv" 
        adjustment_file: "adjustment.csv"
        control_file: "control.csv"
      
      # 协议映射表配置
      protocol_mapping_route: "config/TankFarmModbusTCP"
      protocol_mapping_files:
        telemetry_mapping: "mapping_telemetry.csv"
        signal_mapping: "mapping_signal.csv"
        adjustment_mapping: "mapping_adjustment.csv"
        control_mapping: "mapping_control.csv"
```

## 📝 创建新设备步骤

### 1. 创建目录
```bash
mkdir config/YourDeviceProtocol
```

### 2. 创建四遥点表
```bash
# 遥测点表
touch config/YourDeviceProtocol/telemetry.csv
# 遥信点表  
touch config/YourDeviceProtocol/signal.csv
# 遥调点表
touch config/YourDeviceProtocol/adjustment.csv
# 遥控点表
touch config/YourDeviceProtocol/control.csv
```

### 3. 创建协议映射表
```bash
touch config/YourDeviceProtocol/mapping_telemetry.csv
touch config/YourDeviceProtocol/mapping_signal.csv
touch config/YourDeviceProtocol/mapping_adjustment.csv
touch config/YourDeviceProtocol/mapping_control.csv
```

### 4. 填写表内容
按照格式规范填写各表的表头和数据

### 5. 更新配置文件
在YAML配置文件的channels节添加新设备配置

## ⚠️ 注意事项

1. **point_id唯一性**: 确保point_id在整个系统中唯一
2. **ID对应关系**: 四遥点表和协议映射表的point_id必须一一对应
3. **数据类型**: 协议映射表的data_type要与实际设备匹配
4. **文件命名**: 严格按照命名规范，避免配置加载失败
5. **编码格式**: 所有CSV文件使用UTF-8编码

## 🚀 开发集成

系统会自动:
1. 加载YAML配置文件获取表文件路径
2. 解析四遥点表获取工程点位定义
3. 解析协议映射表获取通讯参数
4. 根据point_id关联两类表的数据
5. 生成完整的点位配置用于通讯

这种分离架构确保了工程配置的清晰性和维护的便利性。 