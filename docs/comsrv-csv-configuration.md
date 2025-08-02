# Comsrv CSV 配置格式规范

## 概述

从 v0.0.1 版本开始，VoltageEMS 的 comsrv 服务采用统一的 CSV 格式来配置四遥（遥测、遥信、遥控、遥调）数据。所有四种类型都使用相同的字段结构，简化了配置和维护。

## 统一的 CSV 格式

### 字段定义

所有四遥 CSV 文件都使用以下字段：

```csv
point_id,signal_name,scale,offset,unit,reverse,data_type
```

字段说明：
- `point_id`: 点位ID（必填，从1开始的连续整数）
- `signal_name`: 信号名称（必填，描述性名称）
- `scale`: 缩放系数（必填，默认为1.0）
- `offset`: 偏移量（必填，默认为0.0）
- `unit`: 单位（可选，留空表示无单位）
- `reverse`: 是否反向（必填，true/false）
- `data_type`: 数据类型（必填，float/int/bool）

### 数据转换公式

对于数值类型（float/int）：
```
实际值 = 原始值 × scale + offset
```

对于布尔类型（bool）：
- `reverse=false`: 0=关/假，1=开/真
- `reverse=true`: 0=开/真，1=关/假

## 四遥类型配置示例

### 1. 遥测（telemetry.csv）

用于模拟量测量值，如电压、电流、温度等。

```csv
point_id,signal_name,scale,offset,unit,reverse,data_type
1,油温,1.0,0.0,℃,false,float
2,A相电压,0.1,0.0,V,false,float
3,A相电流,0.01,0.0,A,false,float
4,有功功率,0.1,0.0,kW,false,float
5,功率因数,0.001,0.0,,false,float
```

### 2. 遥信（signal.csv）

用于开关量状态，如断路器状态、告警信号等。

```csv
point_id,signal_name,scale,offset,unit,reverse,data_type
1,断路器状态,1.0,0.0,,false,bool
2,过温告警,1.0,0.0,,true,bool
3,通信状态,1.0,0.0,,false,bool
4,运行状态,1.0,0.0,,false,bool
5,远程/就地,1.0,0.0,,false,bool
```

注意：遥信的 scale 和 offset 通常设为 1.0 和 0.0，因为布尔值不需要缩放。

### 3. 遥控（control.csv）

用于远程控制命令，如断路器分合、复位等。

```csv
point_id,signal_name,scale,offset,unit,reverse,data_type
1,断路器分合,1.0,0.0,,false,bool
2,复位告警,1.0,0.0,,false,bool
3,启动设备,1.0,0.0,,false,bool
```

### 4. 遥调（adjustment.csv）

用于远程设定值调整，如温度设定、功率限值等。

```csv
point_id,signal_name,scale,offset,unit,reverse,data_type
1,温度设定值,1.0,0.0,℃,false,float
2,功率设定值,0.1,0.0,kW,false,float
3,电压设定值,0.1,0.0,V,false,float
4,频率设定值,0.01,0.0,Hz,false,float
```

## 协议映射文件

协议映射文件定义了点位与具体协议参数的对应关系。以 Modbus 为例：

### telemetry_mapping.csv
```csv
point_id,slave_id,function_code,register_address,bit_position,data_type,byte_order
1,1,3,0,,float32,ABCD
2,1,3,2,,float32,ABCD
3,1,3,4,,float32,ABCD
10,1,3,18,,int16,AB
```

### signal_mapping.csv
```csv
point_id,slave_id,function_code,register_address,bit_position,data_type,byte_order
1,1,1,0,0,bool,
2,1,1,0,1,bool,
3,1,1,0,2,bool,
```

### control_mapping.csv
```csv
point_id,slave_id,function_code,register_address,bit_position,data_type,byte_order
1,1,5,0,0,bool,
2,1,5,1,0,bool,
```

### adjustment_mapping.csv
```csv
point_id,slave_id,function_code,register_address,bit_position,data_type,byte_order
1,1,6,100,,float32,ABCD
2,1,6,102,,float32,ABCD
```

## 配置最佳实践

1. **点位ID管理**
   - 每种遥测类型的 point_id 独立编号
   - 建议预留ID空间，如遥测1-1000，遥信1001-2000
   - 保持ID连续性，便于维护

2. **数据类型选择**
   - 测量值使用 float（支持小数）
   - 计数值使用 int（整数）
   - 状态值使用 bool（开关量）

3. **缩放系数设置**
   - 根据设备实际精度设置 scale
   - 例如：设备返回 2300 表示 230.0V，则 scale=0.1
   - bool 类型固定使用 scale=1.0, offset=0.0

4. **反向标志使用**
   - 主要用于遥信，处理逻辑反向的情况
   - 例如：某些告警信号 0 表示告警，1 表示正常

5. **单位规范**
   - 使用标准单位符号：V、A、kW、℃、Hz
   - 无单位的量（如功率因数）留空
   - 保持全局一致性

## 文件组织结构

```
config/
└── comsrv/
    ├── telemetry.csv              # 遥测定义
    ├── signal.csv                 # 遥信定义
    ├── control.csv                # 遥控定义
    ├── adjustment.csv             # 遥调定义
    └── protocol/                   # 协议映射
        ├── telemetry_mapping.csv
        ├── signal_mapping.csv
        ├── control_mapping.csv
        └── adjustment_mapping.csv
```

## 版本兼容性

- v0.0.1+: 支持统一CSV格式
- 旧版本兼容：系统会自动识别旧格式并转换

## 故障排查

1. **CSV解析错误**
   - 检查字段数量是否正确（必须是7个字段）
   - 确保使用逗号分隔，不要有多余空格
   - 检查数据类型拼写（float/int/bool）

2. **数据不正确**
   - 验证 scale 和 offset 计算
   - 检查 reverse 标志设置
   - 确认协议映射的 byte_order

3. **调试方法**
   ```bash
   # 启用调试日志
   RUST_LOG=comsrv=debug cargo run -p comsrv
   
   # 监控Redis数据
   redis-cli hgetall "comsrv:1:m"
   ```