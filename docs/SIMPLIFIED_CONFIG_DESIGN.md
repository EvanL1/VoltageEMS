# VoltageEMS 简化配置设计 - 约定优于配置

## 设计理念

基于实际使用经验，我们采用了**约定优于配置**的设计理念：

### 🎯 **核心原则**

1. **配置文件只管连接**：IP、端口、串口参数等传输层参数
2. **点表管业务数据**：点位定义、工程单位、测量范围等
3. **映射表管协议参数**：slave_id、寄存器地址、功能码、CAN ID等
4. **按约定查找文件**：统一的路径和文件名规则

### 💡 **约定规则**

#### 文件路径约定
```
config/{通道名}/
├── telemetry.csv           # 遥测点表
├── signal.csv              # 遥信点表  
├── adjustment.csv          # 遥调点表
├── control.csv             # 遥控点表
├── mapping_telemetry.csv   # 遥测映射表
├── mapping_signal.csv      # 遥信映射表
├── mapping_adjustment.csv  # 遥调映射表
└── mapping_control.csv     # 遥控映射表
```

#### 示例
- 通道名：`TankFarmModbusTCP`
- 自动查找路径：`config/TankFarmModbusTCP/`
- 文件：`telemetry.csv`, `mapping_telemetry.csv` 等

### 🚀 **配置简化效果**

#### 传统方式 vs 约定方式

**传统方式**（❌ 复杂）：
```yaml
- id: 1001
  name: "TankFarmModbusTCP" 
  transport: { ... }
  protocol: { ... }
  table_config:                    # 需要配置很多路径
    four_telemetry_route: "config/TankFarmModbusTCP"
    four_telemetry_files:
      telemetry_file: "telemetry.csv"
      signal_file: "signal.csv"
      adjustment_file: "adjustment.csv"
      control_file: "control.csv"
    protocol_mapping_route: "config/TankFarmModbusTCP"
    protocol_mapping_files:
      telemetry_mapping: "mapping_telemetry.csv"
      signal_mapping: "mapping_signal.csv"
      # ... 更多配置
```

**约定方式**（✅ 简洁）：
```yaml
- id: 1001
  name: "TankFarmModbusTCP"    # 系统自动按name查找 config/TankFarmModbusTCP/
  transport: { ... }           # 只配置连接参数
  protocol: { ... }            # 只配置通用参数
  # 不需要table_config！系统按约定自动查找文件
```

## 简化后的配置示例

### Modbus TCP通道
```yaml
- id: 1001
  name: "TankFarmModbusTCP"
  description: "油罐区Modbus TCP通信"
  enabled: true
  
  # 传输层：只配置连接参数
  transport:
    type: "tcp"
    config:
      host: "192.168.1.100"
      port: 502
      timeout: "10s"
      max_retries: 3
  
  # 协议层：只配置全局参数
  protocol:
    type: "modbus_tcp"
    config:
      transaction_id: 0x0000      # Modbus TCP事务标识符
      protocol_id: 0x0000         # Modbus TCP协议标识符（00 00）
      unit_id_from_mapping: true  # slave_id从映射表获取
```

### Modbus RTU通道
```yaml
- id: 1002
  name: "PumpStationModbusRTU"
  description: "泵站Modbus RTU通信" 
  enabled: true
  
  transport:
    type: "serial"
    config:
      port: "/dev/ttyUSB0"
      baud_rate: 9600
      data_bits: 8
      stop_bits: 1
      parity: "None"
      timeout: "5s"
  
  protocol:
    type: "modbus_rtu"
    config:
      unit_id_from_mapping: true  # 从映射表获取
```

### GPIO数字I/O通道
```yaml
- id: 1003
  name: "PumpStationDigitalIO"
  description: "泵站数字I/O控制"
  enabled: true
  
  transport:
    type: "gpio"
    config:
      device_path: "/dev/gpiochip0"
      backend: "LinuxGpioCdev"
      pins:
        - pin: 18
          mode: "DigitalInput"
          label: "Emergency Stop"
        - pin: 21
          mode: "DigitalOutput" 
          initial_value: false
          label: "Pump Start"
  
  protocol:
    type: "gpio_digital"
    config:
      mapping_from_table: true  # 映射关系在点表中
```

### CAN总线通道
```yaml
- id: 1004
  name: "EngineCANBus"
  description: "发动机CAN总线通信"
  enabled: true
  
  transport:
    type: "can"
    config:
      interface: "can0"
      bit_rate: "Kbps500"
      filters:
        - id: 0x100
          mask: 0x700
          extended: false
  
  protocol:
    type: "can_j1939"
    config:
      mapping_from_table: true  # CAN ID和解析规则在映射表中
```

## 点表和映射表设计

### 四遥点表 (telemetry.csv)
```csv
point_id,point_name,point_type,unit,description,min_value,max_value
T001,Tank1_Level,YC,L,1号罐液位,0,10000
S001,Tank1_HighAlarm,YX,,1号罐高液位报警,,
C001,Pump1_Start,YK,,1号泵启动,,
A001,Pump1_Speed,YT,rpm,1号泵转速调节,0,3000
```

### 协议映射表

#### Modbus映射表 (mapping_telemetry.csv)
```csv
point_id,slave_id,function_code,register_address,data_type,register_count,byte_order
T001,1,03,40001,float32,2,ABCD
S001,2,02,10001,bool,1,
C001,1,05,00001,bool,1,
A001,1,06,40010,uint16,1,AB
```

#### GPIO映射表 (mapping_signal.csv)
```csv
point_id,pin_number,pin_type,active_level,debounce_ms
S001,18,input,low,50
C001,21,output,high,0
```

#### CAN映射表 (mapping_telemetry.csv)
```csv
point_id,can_id,start_bit,bit_length,scale,offset,byte_order,signal_type
T001,0x100,0,16,0.1,0,big_endian,signed
S001,0x200,0,1,1,0,big_endian,unsigned
```

## 设计优势

### 1. **配置极简**
- ✅ 删除了所有 `table_config` 配置块
- ✅ 一个通道只需几行配置
- ✅ 按通道名自动查找文件

### 2. **约定清晰**  
- ✅ 统一的文件路径：`config/{通道名}/`
- ✅ 标准的文件名：`telemetry.csv`, `mapping_telemetry.csv`
- ✅ 减少配置错误和维护成本

### 3. **职责分离**
```
配置文件 ← 运维人员
├── 网络连接参数
├── 串口参数  
└── 设备路径

点表文件 ← 业务人员
├── 点位定义
├── 工程单位
└── 测量范围

映射表文件 ← 工程师
├── 协议地址
├── 数据类型
└── 寄存器配置
```

### 4. **维护简单**
- ✅ 新增设备：只需在对应目录添加映射表
- ✅ 网络变更：只需修改配置文件的连接参数
- ✅ 业务调整：只需修改点表文件

### 5. **错误减少**
- ✅ 不需要手动配置文件路径
- ✅ 标准化的文件名避免拼写错误
- ✅ 配置文件结构简单，减少配置错误

## 实际使用场景

### 场景1：新增Modbus设备
1. 创建目录：`config/NewDevice/`
2. 添加标准文件：`telemetry.csv`, `mapping_telemetry.csv`
3. 在配置文件中添加通道：`name: "NewDevice"`
4. 完成！系统自动按约定查找文件

### 场景2：网络IP变更
1. 只需修改配置文件中的 `host: "192.168.1.100"`
2. 不需要修改任何点表文件
3. 重启服务即可

### 场景3：修改点位定义
1. 只需修改对应的 CSV 文件
2. 不需要修改配置文件
3. 支持热重载（如果实现）

## 与传统方案对比

| 方面 | 传统配置 | 约定配置 |
|------|----------|----------|
| 配置文件行数 | 多（包含大量路径配置） | 少（只有核心参数） |
| 文件路径管理 | 手动配置，容易出错 | 自动按约定查找 |
| 新增设备复杂度 | 高（需配置多个路径） | 低（按约定创建目录） |
| 维护成本 | 高 | 低 |
| 配置一致性 | 需要人工保证 | 约定保证 |
| 学习成本 | 高（需了解所有配置项） | 低（只需了解约定） |

## 总结

通过**约定优于配置**的设计理念，我们实现了：

1. **配置文件极简化**：删除了冗长的 `table_config` 配置
2. **标准化文件管理**：统一的路径和文件名约定
3. **维护成本降低**：按约定查找，减少配置错误
4. **职责清晰分离**：配置、点表、映射各司其职

这种设计让VoltageEMS的配置变得简单、清晰、易维护，特别适合工业现场的实际使用需求。 