# ComsrvQuick Start Guide - 快速入门指南

## 🚀 5分钟快速配置

### 第一步：创建基本目录结构

```bash
# 在 services/comsrv/ 目录下创建
mkdir -p config/channels/channel_100_virtual/{combase,protocol}
mkdir -p config/channels/channel_101_plc_main/{combase,protocol}
mkdir -p logs
```

### 第二步：创建主配置文件

创建 `config/comsrv.yaml`：

```yaml
version: "2.1"

service:
  name: "comsrv"
  description: "Communication Service"
  
  logging:
    level: "info"
    file: "logs/comsrv.log"
    max_size: 104857600
    max_files: 5
    console: true
  
  api:
    enabled: true
    bind_address: "127.0.0.1:8082"
    version: "v1"
  
  redis:
    enabled: true
    connection_type: "Tcp"
    address: "127.0.0.1:6379"
    db: 1
    timeout_ms: 5000
    max_connections: 20
    min_connections: 5

defaults:
  channels_root: "channels"
  combase_dir: "combase"
  protocol_dir: "protocol"
  filenames:
    telemetry: "telemetry.csv"
    signaling: "signaling.csv"
    control: "control.csv"
    setpoint: "setpoint.csv"
    modbus_tcp_source: "modbus_tcp_source.csv"
    modbus_rtu_source: "modbus_rtu_source.csv"
    calculation_source: "calculation_source.csv"
    manual_source: "manual_source.csv"

channels:
  # 虚拟通道 - 用于测试
  - id: 100
    name: "Virtual"
    description: "Virtual test channel"
    protocol: "Virtual"
    parameters:
      poll_rate: 2000
      max_retries: 3
      timeout: 1000
    point_table:
      enabled: true
      use_defaults: true
      watch_changes: true
      reload_interval: 30
    source_tables:
      enabled: true
      use_defaults: true
      redis_prefix: "comsrv:source_tables:virtual_100"

  # Modbus TCP通道 - 连接PLC
  - id: 101
    name: "PLC_Main"
    description: "Main PLC via Modbus TCP"
    protocol: "ModbusTcp"
    parameters:
      host: "192.168.1.100"    # 修改为你的PLC IP地址
      port: 502
      timeout: 3000
      max_retries: 3
      poll_rate: 1000
      slave_id: 1
    point_table:
      enabled: true
      use_defaults: true
      watch_changes: true
      reload_interval: 60
    source_tables:
      enabled: true
      use_defaults: true
      redis_prefix: "comsrv:source_tables:plc_101"
```

### 第三步：创建虚拟通道的点表文件

#### 遥测点表 (`config/channels/channel_100_virtual/combase/telemetry.csv`)

```csv
id,name,chinese_name,data_source_type,source_table,source_data,scale,offset,unit,description,group
1,test_voltage,测试电压,Manual,manual,1001,1.0,0,V,测试电压值,测试
2,test_current,测试电流,Manual,manual,1002,1.0,0,A,测试电流值,测试
3,calculated_power,计算功率,Calculation,calculation,2001,1.0,0,W,计算得出的功率值,计算
```

#### 遥信点表 (`config/channels/channel_100_virtual/combase/signaling.csv`)

```csv
id,name,chinese_name,data_source_type,source_table,source_data,description,group
1,test_switch,测试开关,Manual,manual,2001,测试开关状态,测试
2,alarm_status,报警状态,Manual,manual,2002,系统报警状态,报警
```

#### 遥控点表 (`config/channels/channel_100_virtual/combase/control.csv`)

```csv
id,name,chinese_name,data_source_type,source_table,source_data,description,group
1,start_command,启动命令,Manual,manual,3001,设备启动命令,控制
2,stop_command,停止命令,Manual,manual,3002,设备停止命令,控制
```

#### 遥调点表 (`config/channels/channel_100_virtual/combase/setpoint.csv`)

```csv
id,name,chinese_name,data_source_type,source_table,source_data,scale,offset,unit,description,group
1,voltage_setpoint,电压设定,Manual,manual,4001,1.0,0,V,电压设定值,设定
2,frequency_setpoint,频率设定,Manual,manual,4002,1.0,0,Hz,频率设定值,设定
```

### 第四步：创建数据源表文件

#### 手动数据源 (`config/channels/channel_100_virtual/protocol/manual_source.csv`)

```csv
source_id,manual_type,editable,default_value,value_type,description
1001,analog,true,220.0,float,测试电压默认值
1002,analog,true,10.0,float,测试电流默认值
2001,digital,true,false,bool,测试开关默认值
2002,digital,true,false,bool,报警状态默认值
3001,command,true,false,bool,启动命令
3002,command,true,false,bool,停止命令
4001,setpoint,true,220.0,float,电压设定默认值
4002,setpoint,true,50.0,float,频率设定默认值
```

#### 计算数据源 (`config/channels/channel_100_virtual/protocol/calculation_source.csv`)

```csv
source_id,calculation_type,expression,source_points,update_interval_ms,description
2001,formula,"p1*p2","1001;1002",1000,功率计算: 电压×电流
```

### 第五步：为PLC通道创建基本点表

#### PLC遥测点表 (`config/channels/channel_101_plc_main/combase/telemetry.csv`)

```csv
id,name,chinese_name,data_source_type,source_table,source_data,scale,offset,unit,description,group
1,plc_voltage_l1,PLC L1电压,Protocol,modbus_tcp,1001,0.1,0,V,L1相电压,电压
2,plc_current_l1,PLC L1电流,Protocol,modbus_tcp,1002,0.01,0,A,L1相电流,电流
3,plc_frequency,PLC频率,Protocol,modbus_tcp,1003,0.01,0,Hz,系统频率,频率
```

#### PLC遥信点表 (`config/channels/channel_101_plc_main/combase/signaling.csv`)

```csv
id,name,chinese_name,data_source_type,source_table,source_data,description,group
1,plc_running,PLC运行状态,Protocol,modbus_tcp,2001,PLC运行状态,状态
2,plc_alarm,PLC报警,Protocol,modbus_tcp,2002,PLC报警状态,报警
```

#### PLC Modbus TCP数据源 (`config/channels/channel_101_plc_main/protocol/modbus_tcp_source.csv`)

```csv
source_id,protocol_type,slave_id,function_code,register_address,data_type,byte_order,bit_index,scaling_factor,description
1001,modbus_tcp,1,3,100,uint16,big_endian,,0.1,L1电压寄存器
1002,modbus_tcp,1,3,101,uint16,big_endian,,0.01,L1电流寄存器
1003,modbus_tcp,1,3,102,uint16,big_endian,,0.01,频率寄存器
2001,modbus_tcp,1,1,200,bool,big_endian,0,,运行状态位
2002,modbus_tcp,1,1,201,bool,big_endian,0,,报警状态位
```

### 第六步：启动服务并测试

```bash
# 确保Redis服务正在运行
redis-server

# 启动comsrv服务
cd services/comsrv
cargo run
```

### 第七步：验证配置

#### 检查API接口

```bash
# 检查服务状态
curl http://localhost:8082/v1/status

# 获取通道列表
curl http://localhost:8082/v1/channels

# 获取虚拟通道的点表数据
curl http://localhost:8082/v1/channels/100/points/telemetry
```

#### 检查Redis数据

```bash
# 连接Redis并查看数据
redis-cli
127.0.0.1:6379> SELECT 1
127.0.0.1:6379[1]> KEYS "comsrv:*"
```

## 📋 配置检查清单

### ✅ 文件结构检查

- [ ] `config/comsrv.yaml` 主配置文件已创建
- [ ] `config/channels/channel_100_virtual/combase/` 目录已创建
- [ ] `config/channels/channel_100_virtual/protocol/` 目录已创建
- [ ] 四遥CSV文件已创建（telemetry.csv, signaling.csv, control.csv, setpoint.csv）
- [ ] 数据源CSV文件已创建（manual_source.csv, calculation_source.csv）

### ✅ 配置验证检查

- [ ] YAML文件语法正确（无缩进错误）
- [ ] 通道ID唯一且不重复
- [ ] 所有必需的协议参数已配置
- [ ] CSV文件格式正确（表头匹配）
- [ ] 数据源ID映射正确

### ✅ 服务运行检查

- [ ] Redis服务正在运行
- [ ] comsrv服务启动无错误
- [ ] 日志文件正常输出
- [ ] API接口响应正常

## 🔧 常见问题快速解决

### 问题1: YAML解析错误

**解决方案**: 检查缩进，确保使用空格而非Tab

### 问题2: 找不到CSV文件

**解决方案**: 检查文件路径，确保目录结构正确

### 问题3: Redis连接失败

**解决方案**: 确保Redis服务运行，检查连接配置

### 问题4: 通道连接失败

**解决方案**: 检查网络连接，验证设备IP和端口

## 📖 下一步

- 阅读完整的[配置指南](CONFIGURATION_GUIDE.md)
- 学习[API接口文档](API_REFERENCE.md)
- 了解[性能调优](PERFORMANCE_TUNING.md)
- 查看[故障排除指南](TROUBLESHOOTING.md)

---

*需要帮助？请查看详细的[配置指南](CONFIGURATION_GUIDE.md)或联系技术支持。*
