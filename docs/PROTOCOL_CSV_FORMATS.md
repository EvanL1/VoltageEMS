# VoltageEMS 协议映射CSV格式规范

## 概述

VoltageEMS系统采用分离式表格架构，将工程点表和协议映射分开管理。每种协议都有自己特定的CSV格式，确保协议参数的准确性和类型安全。

## 架构设计

### 1. 分离式表格架构

```
四遥点表 (工程意义)          协议映射表 (通信参数)
├── telemetry.csv            ├── mapping_telemetry.csv
├── signal.csv               ├── mapping_signal.csv  
├── adjustment.csv           ├── mapping_adjustment.csv
└── control.csv              └── mapping_control.csv
```

### 2. 协议特定的Trait系统

- `BaseProtocolMapping` - 通用协议映射接口
- `ModbusMapping` - Modbus协议特定接口
- `CanMapping` - CAN协议特定接口
- `IecMapping` - IEC 60870协议特定接口

## Modbus协议CSV格式

### 遥测映射 (mapping_telemetry.csv)

```csv
point_id,register_address,function_code,slave_id,data_format,byte_order,register_count
1001,40001,3,1,float32,ABCD,2
1002,40003,3,1,float32,ABCD,2
1003,40005,3,1,uint16,ABCD,1
1004,40006,4,2,int16,DCBA,1
1005,40007,3,1,uint32,CDAB,2
```

**字段说明：**

- `point_id`: 点位ID，与四遥点表对应
- `register_address`: Modbus寄存器地址 (1-65535)
- `function_code`: 功能码 (3=读保持寄存器, 4=读输入寄存器)
- `slave_id`: 从站地址 (1-247)
- `data_format`: 数据格式 (uint16, int16, uint32, int32, float32, bool)
- `byte_order`: 字节序 (ABCD, DCBA, BADC, CDAB)
- `register_count`: 寄存器数量 (1=16位, 2=32位, 4=64位)
- `polling_interval`: 轮询间隔(毫秒)

### 遥信映射 (mapping_signal.csv)

```csv
point_id,register_address,function_code,slave_id,data_format,bit_position
2001,00001,1,1,bool,0
2002,00002,1,1,bool,0
2003,00003,2,1,bool,0
2004,40001,3,2,bool,0
2005,40001,3,2,bool,1
2006,40001,3,2,bool,2
```

**字段说明：**

- `point_id`: 点位ID，与四遥点表对应
- `register_address`: 寄存器地址
- `function_code`: 功能码 (1=读线圈, 2=读离散输入, 3=读保持寄存器位)
- `slave_id`: 从站地址
- `data_format`: 数据格式 (通常为bool)
- `bit_position`: 位位置 (0-15，用于寄存器位操作)
- `polling_interval`: 轮询间隔(毫秒)

### 遥调映射 (mapping_adjustment.csv)

```csv
point_id,register_address,function_code,slave_id,data_format,byte_order,register_count
3001,40001,16,1,float32,ABCD,2
3002,40003,16,1,float32,ABCD,2
3003,40005,6,1,uint16,ABCD,1
3004,40006,16,2,int32,DCBA,2
```

**字段说明：**

- `point_id`: 点位ID，与四遥点表对应
- `register_address`: 写入寄存器地址
- `function_code`: 功能码 (6=写单个寄存器, 16=写多个寄存器)
- `slave_id`: 从站地址
- `data_format`: 数据格式
- `byte_order`: 字节序
- `register_count`: 寄存器数量

### 遥控映射 (mapping_control.csv)

```csv
point_id,register_address,function_code,slave_id,data_format,bit_position
4001,00001,5,1,bool,
4002,00002,5,1,bool,
4003,00003,15,1,bool,
4004,40001,6,2,bool,0
4005,40001,6,2,bool,1
```

**字段说明：**

- `point_id`: 点位ID，与四遥点表对应
- `register_address`: 操作寄存器地址
- `function_code`: 功能码 (5=写单个线圈, 15=写多个线圈, 6=写单个寄存器)
- `slave_id`: 从站地址
- `data_format`: 数据格式 (通常为bool)
- `bit_position`: 位位置 (可选，用于寄存器位操作)

## CAN协议CSV格式

### 遥测映射 (mapping_telemetry.csv)

```csv
point_id,can_id,is_extended,start_byte,data_length,byte_order,data_type,polling_interval
5001,0x0CF00400,true,3,2,big_endian,uint16,100
5002,0x0CF00300,true,1,1,big_endian,uint8,50
5003,0x18F00503,true,0,4,little_endian,float32,200
5004,0x123,false,0,2,big_endian,int16,500
```

**字段说明：**

- `point_id`: 点位ID，与四遥点表对应
- `can_id`: CAN ID (十六进制格式，如0x18F00503)
- `is_extended`: 是否为扩展帧 (true/false)
- `start_byte`: 起始字节位置 (0-7)
- `data_length`: 数据长度 (1-8字节)
- `byte_order`: 字节序 (big_endian/little_endian)
- `data_type`: 数据类型 (uint8, int8, uint16, int16, uint32, int32, float32)
- `polling_interval`: 轮询间隔(毫秒)

### 遥信映射 (mapping_signal.csv)

```csv
point_id,can_id,is_extended,start_byte,bit_position,polling_interval
6001,0x0CF00400,true,2,0,100
6002,0x0CF00400,true,2,1,100
6003,0x0CF00400,true,2,2,100
6004,0x456,false,1,7,200
```

**字段说明：**

- `point_id`: 点位ID，与四遥点表对应
- `can_id`: CAN ID
- `is_extended`: 是否为扩展帧
- `start_byte`: 起始字节位置
- `bit_position`: 位位置 (0-7)
- `polling_interval`: 轮询间隔(毫秒)

## IEC 60870协议CSV格式

### 遥测映射 (mapping_telemetry.csv)

```csv
point_id,ioa,ca,type_id,cot,qoi,polling_interval
7001,1001,100,13,3,,5000
7002,1002,100,11,3,,2000
7003,1003,100,9,3,,1000
7004,1004,101,13,3,20,5000
```

**字段说明：**

- `point_id`: 点位ID，与四遥点表对应
- `ioa`: 信息对象地址 (Information Object Address)
- `ca`: 公共地址 (Common Address)
- `type_id`: 类型标识 (9=归一化值, 11=标度值, 13=短浮点数)
- `cot`: 传输原因 (Cause of Transmission, 可选)
- `qoi`: 限定词 (Qualifier of Interrogation, 可选)
- `polling_interval`: 轮询间隔(毫秒)

### 遥信映射 (mapping_signal.csv)

```csv
point_id,ioa,ca,type_id,cot,qoi,polling_interval
8001,2001,100,1,3,,1000
8002,2002,100,1,3,,1000
8003,2003,100,3,3,,500
8004,2004,101,1,3,20,1000
```

**字段说明：**

- `point_id`: 点位ID，与四遥点表对应
- `ioa`: 信息对象地址
- `ca`: 公共地址
- `type_id`: 类型标识 (1=单点信息, 3=双点信息)
- `cot`: 传输原因 (可选)
- `qoi`: 限定词 (可选)
- `polling_interval`: 轮询间隔(毫秒)

### 遥调映射 (mapping_adjustment.csv)

```csv
point_id,ioa,ca,type_id,cot
9001,3001,100,50,6
9002,3002,100,50,6
9003,3003,100,48,6
9004,3004,101,50,6
```

**字段说明：**

- `point_id`: 点位ID，与四遥点表对应
- `ioa`: 信息对象地址
- `ca`: 公共地址
- `type_id`: 类型标识 (48=设定值命令, 50=设定值命令带时标)
- `cot`: 传输原因 (6=激活)

### 遥控映射 (mapping_control.csv)

```csv
point_id,ioa,ca,type_id,cot,select_execute
10001,4001,100,45,6,true
10002,4002,100,45,6,true
10003,4003,100,46,6,false
10004,4004,101,45,6,true
```

**字段说明：**

- `point_id`: 点位ID，与四遥点表对应
- `ioa`: 信息对象地址
- `ca`: 公共地址
- `type_id`: 类型标识 (45=单命令, 46=双命令)
- `cot`: 传输原因 (6=激活)
- `select_execute`: 是否使用选择-执行模式 (true/false)

## 字节序格式详解

### Modbus字节序格式

以32位浮点数 `0x41200000` (十进制10.0) 为例：

| 格式 | 寄存器排列       | 字节顺序 | 适用设备          | 说明           |
| ---- | ---------------- | -------- | ----------------- | -------------- |
| ABCD | [0x4120, 0x0000] | AB CD    | 施耐德、西门子PLC | 标准大端序     |
| DCBA | [0x0000, 0x2041] | DC BA    | 部分嵌入式设备    | 完全颠倒       |
| BADC | [0x2041, 0x0000] | BA DC    | 某些变频器        | 字节对调换位置 |
| CDAB | [0x0000, 0x4120] | CD AB    | 部分工控设备      | 小端序字交换   |

### CAN字节序格式

- `big_endian`: 高字节在前，符合网络字节序
- `little_endian`: 低字节在前，符合Intel x86架构

## 数据类型支持

### Modbus数据类型

| 类型    | 字节数 | 寄存器数 | 范围                   | 说明           |
| ------- | ------ | -------- | ---------------------- | -------------- |
| bool    | 1      | 1        | 0/1                    | 布尔值         |
| uint16  | 2      | 1        | 0-65535                | 无符号16位整数 |
| int16   | 2      | 1        | -32768~32767           | 有符号16位整数 |
| uint32  | 4      | 2        | 0-4294967295           | 无符号32位整数 |
| int32   | 4      | 2        | -2147483648~2147483647 | 有符号32位整数 |
| float32 | 4      | 2        | IEEE 754               | 32位浮点数     |

### CAN数据类型

| 类型    | 字节数 | 范围                   | 说明           |
| ------- | ------ | ---------------------- | -------------- |
| uint8   | 1      | 0-255                  | 无符号8位整数  |
| int8    | 1      | -128~127               | 有符号8位整数  |
| uint16  | 2      | 0-65535                | 无符号16位整数 |
| int16   | 2      | -32768~32767           | 有符号16位整数 |
| uint32  | 4      | 0-4294967295           | 无符号32位整数 |
| int32   | 4      | -2147483648~2147483647 | 有符号32位整数 |
| float32 | 4      | IEEE 754               | 32位浮点数     |

### IEC 60870数据类型

| Type ID | 类型名称         | 说明          | 适用 |
| ------- | ---------------- | ------------- | ---- |
| 1       | 单点信息         | 布尔值        | 遥信 |
| 3       | 双点信息         | 三态值        | 遥信 |
| 9       | 归一化值         | -1.0~+1.0     | 遥测 |
| 11      | 标度值           | 整数值        | 遥测 |
| 13      | 短浮点数         | IEEE 754      | 遥测 |
| 45      | 单命令           | 布尔命令      | 遥控 |
| 46      | 双命令           | 三态命令      | 遥控 |
| 48      | 设定值命令       | 归一化值      | 遥调 |
| 50      | 设定值命令带时标 | 归一化值+时间 | 遥调 |

## 配置验证规则

### 通用验证规则

1. `point_id` 必须 > 0 且在通道内唯一
2. 所有必填字段不能为空
3. 数值字段必须在有效范围内
4. 协议特定参数必须符合协议规范

### Modbus验证规则

1. `register_address`: 1-65535
2. `function_code`: 1,2,3,4,5,6,15,16
3. `slave_id`: 1-247
4. `register_count`: 1-4 (根据数据类型)
5. `bit_position`: 0-15 (位操作时)

### CAN验证规则

1. `can_id`: 0x1-0x7FF (标准帧) 或 0x1-0x1FFFFFFF (扩展帧)
2. `start_byte`: 0-7
3. `data_length`: 1-8
4. `start_byte + data_length <= 8`
5. `bit_position`: 0-7 (位操作时)

### IEC 60870验证规则

1. `ioa`: 1-16777215 (24位地址)
2. `ca`: 1-65535
3. `type_id`: 符合IEC 60870-5标准
4. `cot`: 1-63 (6位传输原因)

## 使用示例

### 加载协议映射

```rust
use crate::core::config::protocol_mapping_traits::*;

// 创建协议映射管理器
let manager = ProtocolMappingManager::new();

// 加载Modbus遥测映射
let modbus_mappings = manager.load_mappings(
    "modbus", 
    "telemetry", 
    "config/ModbusTCP/mapping_telemetry.csv"
).await?;

// 加载CAN遥测映射
let can_mappings = manager.load_mappings(
    "can", 
    "telemetry", 
    "config/CanBus/mapping_telemetry.csv"
).await?;
```

### 验证CSV格式

```rust
// 验证Modbus映射文件格式
manager.validate_mapping_file(
    "modbus", 
    "telemetry", 
    "config/ModbusTCP/mapping_telemetry.csv"
).await?;

// 验证IEC 60870映射文件格式
manager.validate_mapping_file(
    "iec60870", 
    "signal", 
    "config/IEC104/mapping_signal.csv"
).await?;
```

## 扩展新协议

### 1. 定义协议特定trait

```rust
pub trait NewProtocolMapping: BaseProtocolMapping {
    // 协议特定方法
    fn protocol_specific_param(&self) -> u32;
}
```

### 2. 实现具体映射结构体

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProtocolTelemetryMapping {
    pub point_id: u32,
    pub protocol_address: u32,
    // 其他协议特定字段
}
```

### 3. 实现CSV加载器

```rust
pub struct NewProtocolCsvLoader;

#[async_trait]
impl ProtocolCsvLoader<NewProtocolTelemetryMapping> for NewProtocolCsvLoader {
    // 实现加载逻辑
}
```

### 4. 注册到管理器

```rust
impl ProtocolMappingManager {
    pub async fn load_mappings(&self, protocol: &str, telemetry_type: &str, file_path: &str) -> Result<Vec<Box<dyn BaseProtocolMapping>>> {
        match (protocol.to_lowercase().as_str(), telemetry_type.to_lowercase().as_str()) {
            // 现有协议...
            ("new_protocol", "telemetry") => {
                // 新协议加载逻辑
            },
            _ => Err(ComSrvError::ConfigError("不支持的协议".to_string()))
        }
    }
}
```

这样的设计确保了：

- **类型安全**：每个协议有自己的类型定义
- **扩展性**：容易添加新协议
- **维护性**：协议特定逻辑分离
- **一致性**：统一的接口和验证规则
