# VoltageEMS 分层传输配置设计

## 概述

基于新的分层传输架构，VoltageEMS 采用了全新的配置文件设计。这种设计将**传输层**和**协议层**完全分离，实现了更好的代码复用、更清晰的架构分层，以及更强的可维护性。

## 设计原则

### 1. 分层分离原则
- **传输层**：负责物理通信（TCP、串口、GPIO、CAN等）
- **协议层**：负责应用逻辑（Modbus、IEC104、CAN协议等）
- **通道层**：组合传输层和协议层，形成完整的通信通道

### 2. 配置复用原则
- 同一传输配置可被多个通道使用
- 同一协议配置可应用于不同传输方式
- 减少重复配置，提高一致性

### 3. 类型安全原则
- 每种传输类型有专门的配置结构
- 编译时类型检查，避免运行时错误
- 清晰的配置验证和错误提示

### 4. 向后兼容原则
- 支持旧版本配置的自动迁移
- 渐进式升级路径
- 配置映射和转换机制

## 配置架构

```yaml
# 配置文件结构
├── service              # 服务全局配置
├── transports          # 传输层配置
│   ├── tcp_transports     # TCP传输配置
│   ├── serial_transports  # 串口传输配置
│   ├── gpio_transports    # GPIO传输配置
│   ├── can_transports     # CAN传输配置
│   └── mock_transports    # Mock传输配置
├── protocols           # 协议层配置
│   ├── modbus_protocols   # Modbus协议配置
│   ├── iec104_protocols   # IEC104协议配置
│   ├── can_protocols      # CAN协议配置
│   └── virtual_protocols  # Virtual协议配置
├── channels            # 通道配置
├── transport_factory   # 传输工厂配置
└── migration          # 迁移支持配置
```

## 传输层配置详解

### TCP传输配置

```yaml
tcp_transports:
  example_tcp:
    name: "Example TCP Transport"
    host: "192.168.1.100"          # 目标主机
    port: 502                      # 端口号
    timeout: "10s"                 # 连接超时
    max_retries: 3                 # 最大重试次数
    keep_alive: "60s"              # Keep-alive时间
    recv_buffer_size: 4096         # 接收缓冲区大小
    send_buffer_size: 4096         # 发送缓冲区大小
    no_delay: true                 # TCP_NODELAY选项
```

### 串口传输配置

```yaml
serial_transports:
  example_serial:
    name: "Example Serial Transport"
    port: "/dev/ttyUSB0"           # 串口设备路径
    baud_rate: 9600                # 波特率
    data_bits: 8                   # 数据位
    stop_bits: 1                   # 停止位
    parity: "None"                 # 校验位 (None/Even/Odd)
    flow_control: "None"           # 流控制 (None/Software/Hardware)
    timeout: "5s"                  # 连接超时
    max_retries: 3                 # 最大重试次数
    read_timeout: "2s"             # 读取超时
    write_timeout: "2s"            # 写入超时
```

### GPIO传输配置

```yaml
gpio_transports:
  example_gpio:
    name: "Example GPIO Transport"
    device_path: "/dev/gpiochip0"  # GPIO设备路径
    backend: "LinuxGpioCdev"       # GPIO后端类型
    timeout: "5s"                  # 操作超时
    max_retries: 3                 # 最大重试次数
    poll_interval: "100ms"         # 输入引脚轮询间隔
    pins:                          # 引脚配置
      - pin: 18                    # 引脚号
        mode: "DigitalInput"       # 引脚模式
        debounce_ms: 50            # 防抖时间
        label: "Emergency Stop"    # 引脚标签
      - pin: 21
        mode: "DigitalOutput"
        initial_value: false       # 输出引脚初始值
        label: "Pump Start"
```

### CAN传输配置

```yaml
can_transports:
  example_can:
    name: "Example CAN Transport"
    interface: "can0"              # CAN接口名称
    bit_rate: "Kbps500"           # 比特率
    can_fd: false                  # 是否启用CAN FD
    timeout: "5s"                  # 操作超时
    max_retries: 3                 # 最大重试次数
    recv_buffer_size: 1024         # 接收缓冲区大小
    send_buffer_size: 1024         # 发送缓冲区大小
    filters:                       # CAN过滤器
      - id: 0x100                  # 过滤器ID
        mask: 0x700                # 过滤器掩码
        extended: false            # 是否扩展帧
```

## 协议层配置详解

### Modbus协议配置

```yaml
modbus_protocols:
  example_modbus:
    name: "Example Modbus Protocol"
    slave_id: 1                    # 从站ID
    function_codes:                # 支持的功能码
      - "read_holding_registers"
      - "write_single_register"
    register_layout: "ABCD"        # 寄存器字节序
    poll_interval: "1s"            # 轮询间隔
```

### IEC60870-5-104协议配置

```yaml
iec104_protocols:
  example_iec104:
    name: "Example IEC104 Protocol"
    common_address: 1              # 公共地址
    cause_of_transmission: 3       # 传送原因
    object_address_size: 3         # 对象地址大小
    asdu_address_size: 2           # ASDU地址大小
    interrogation_interval: "60s"  # 总召唤间隔
```

## 通道配置详解

通道配置将传输层和协议层组合起来：

```yaml
channels:
  - id: 1001
    name: "ExampleChannel"
    description: "示例通道"
    transport_ref: "tcp_transports.example_tcp"    # 引用传输配置
    protocol_ref: "modbus_protocols.example_modbus" # 引用协议配置
    enabled: true                                   # 是否启用
    table_config:                                   # 点表配置
      four_telemetry_route: "config/ExampleChannel"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"
        signal_file: "signal.csv"
        adjustment_file: "adjustment.csv"
        control_file: "control.csv"
```

## 配置迁移指南

### 从旧配置迁移到新配置

#### 旧配置示例：
```yaml
channels:
  - id: 1001
    name: "ModbusDevice"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      slave_id: 1
      timeout_ms: 5000
```

#### 新配置转换：

1. **提取传输层配置**：
```yaml
transports:
  tcp_transports:
    modbus_device_network:
      name: "Modbus Device Network"
      host: "192.168.1.100"
      port: 502
      timeout: "5s"
```

2. **提取协议层配置**：
```yaml
protocols:
  modbus_protocols:
    modbus_device_protocol:
      name: "Modbus Device Protocol"
      slave_id: 1
```

3. **重新组合通道配置**：
```yaml
channels:
  - id: 1001
    name: "ModbusDevice"
    transport_ref: "tcp_transports.modbus_device_network"
    protocol_ref: "modbus_protocols.modbus_device_protocol"
```

### 自动迁移支持

配置文件支持自动迁移功能：

```yaml
migration:
  enable_legacy_support: true
  legacy_mapping:
    modbus_tcp:
      host: "transports.tcp_transports.{channel_name}.host"
      port: "transports.tcp_transports.{channel_name}.port"
      slave_id: "protocols.modbus_protocols.{channel_name}.slave_id"
```

## 最佳实践

### 1. 传输配置命名规范

- 使用描述性名称：`tank_farm_network` 而不是 `tcp1`
- 包含位置信息：`pump_station_serial` 而不是 `serial1`
- 体现用途：`emergency_io` 而不是 `gpio1`

### 2. 配置复用策略

```yaml
# 良好实践：复用传输配置
transports:
  tcp_transports:
    plant_network:  # 一个传输配置
      host: "192.168.1.100"
      port: 502

channels:
  - name: "Tank1Modbus"
    transport_ref: "tcp_transports.plant_network"  # 复用
  - name: "Tank2Modbus"  
    transport_ref: "tcp_transports.plant_network"  # 复用
```

### 3. 配置组织结构

```yaml
# 按功能区域组织
transports:
  tcp_transports:
    # 生产区域
    production_area:
      tank_farm_network: {...}
      pump_station_network: {...}
    
    # 管理区域  
    control_room:
      hmi_network: {...}
      historian_network: {...}
```

### 4. 环境配置管理

```yaml
# 开发环境
transports:
  tcp_transports:
    test_device:
      host: "127.0.0.1"
      port: 5020

# 生产环境
transports:
  tcp_transports:
    production_device:
      host: "192.168.1.100"
      port: 502
```

### 5. GPIO配置最佳实践

```yaml
gpio_transports:
  safety_io:
    name: "Safety I/O"
    pins:
      # 使用清晰的标签
      - pin: 18
        mode: "DigitalInput"
        label: "Emergency Stop Button"
        debounce_ms: 50
      
      # 合理设置初始值
      - pin: 21
        mode: "DigitalOutput"
        initial_value: false  # 安全状态
        label: "Safety Relay"
```

### 6. CAN配置最佳实践

```yaml
can_transports:
  engine_can:
    name: "Engine CAN Bus"
    interface: "can0"
    bit_rate: "Kbps500"
    filters:
      # 只接收需要的消息
      - id: 0x100    # 发动机RPM
        mask: 0x7F0
      - id: 0x200    # 发动机温度
        mask: 0x7F0
```

## 配置验证

### 1. 语法验证
- YAML语法正确性
- 必填字段检查
- 数据类型验证

### 2. 语义验证
- 引用完整性检查
- 端口号范围验证
- 设备路径存在性检查

### 3. 运行时验证
- 网络连通性测试
- 设备可用性检查
- 权限验证

## 错误处理

### 1. 配置错误
```yaml
# 错误示例
transports:
  tcp_transports:
    bad_config:
      host: ""           # 错误：主机不能为空
      port: 70000        # 错误：端口超出范围
```

### 2. 引用错误
```yaml
channels:
  - name: "BadChannel"
    transport_ref: "tcp_transports.non_existent"  # 错误：引用不存在
```

### 3. 错误消息示例
```
配置验证失败:
- 传输配置 'tcp_transports.bad_config': 主机地址不能为空
- 通道配置 'BadChannel': 引用的传输配置 'tcp_transports.non_existent' 不存在
- GPIO配置 'pump_io': 引脚18重复配置
```

## 性能考虑

### 1. 配置加载优化
- 延迟加载未使用的传输配置
- 配置缓存机制
- 增量配置更新

### 2. 内存使用优化
- 传输配置对象复用
- 合理的缓冲区大小设置
- 及时释放未使用的连接

### 3. 网络资源优化
- 连接池管理
- 合理的超时设置
- 批量操作优化

## 监控和诊断

### 1. 配置诊断接口
```bash
# 验证配置文件
comsrv --validate-config config.yaml

# 显示配置摘要  
comsrv --config-summary config.yaml

# 测试传输连接
comsrv --test-transports config.yaml
```

### 2. 运行时监控
- 传输层连接状态
- 协议层通信统计
- 错误率和延迟监控

### 3. 配置热重载
- 支持运行时配置更新
- 增量配置变更
- 无服务中断的配置升级

这种分层配置设计为VoltageEMS提供了强大而灵活的配置管理能力，支持复杂的工业通信场景，同时保持了配置的清晰性和可维护性。 