# ComsRV Fix Log

记录对 comsrv 服务的修复历史。

## 2024-01-XX - 配置系统重构

### 修改的文件

1. **src/core/config/loaders/csv_loader.rs**
   - 在 `FourTelemetryRecord` 结构体中添加了 `reverse: Option<bool>` 字段
   - 将 `ModbusMappingRecord` 中的 `number_of_bytes` 字段改为可选类型 `Option<u8>`

2. **src/core/config/loaders/protocol_mapping.rs**
   - 更新 `ModbusMapping` 结构体，使 `number_of_bytes` 变为可选
   - 修改 `data_size()` 方法，优先使用 `register_count`，其次使用 `number_of_bytes`，最后根据数据格式计算默认值

3. **src/core/config/loaders/point_mapper.rs**
   - 在 `CombinedPoint` 结构体中添加 `reverse: Option<bool>` 字段
   - 将 `number_of_bytes` 字段改为 `Option<u8>` 类型
   - 更新所有测试用例以包含新字段

4. **src/core/config/unified_loader.rs**
   - 更新 `load_signal_file` 方法以正确解析包含 `data_type` 和 `reverse` 字段的CSV格式
   - 修改 `load_modbus_mappings` 方法以正确处理新的CSV列索引
   - 在 `combine_point` 方法中添加对 `reverse` 字段的处理，仅对信号和控制类型添加到 protocol_params 中

### 配置文件更新

1. **config/channel_1_power_meter/signal.csv**
   - 将列格式从 `scale,offset` 改为 `reverse`
   - 更新所有行数据以使用新格式

2. **config/channel_1_power_meter/control.csv**
   - 将列格式从 `scale,offset` 改为 `reverse`
   - 更新所有行数据以使用新格式

3. **config/channel_1_power_meter/mapping_*.csv (所有映射文件)**
   - 移除 `number_of_bytes` 列
   - 保留 `register_count` 作为主要的大小字段

4. **config/channel_2_plc/** (部分文件)
   - 更新 signal.csv 和 control.csv 使用 `reverse` 字段
   - 更新 mapping_telemetry.csv 移除 `number_of_bytes` 列

### 修复说明

这次修改主要是为了使点表定义更加合理：
- 遥信（YX）和遥控（YK）类型不需要缩放系数和偏移，只需要一个反转标志
- Modbus映射中 `number_of_bytes` 是可选的，因为可以从 `register_count` 或 `data_format` 推导出来
- 这样的改动使配置更加清晰和易于维护

### 测试结果

所有配置加载器测试已通过：
- test_csv_loader
- test_csv_validation  
- test_cached_csv_loader
- test_combine_points
- test_validate_combined_points
- test_group_points_by_type

## 2024-01-XX - Modbus数据读取测试

### 测试环境
- 创建了3个Modbus TCP通道配置（Power Meter、PLC Controller、Temperature Controller）
- 使用Python pymodbus库创建了多通道Modbus服务器模拟器
- 服务器在端口5020、5021、5022监听

### 发现的问题
1. **地址映射错误**: CSV配置中的寄存器地址从0开始，但服务器数据存储在地址100开始
   - 修复：更新了mapping_telemetry.csv中的地址为100、102、104等

2. **服务器数据存储位置**: Modbus服务器应该将遥测数据存储在holding registers而不是input registers
   - 修复：更新modbus_multi_server.py，将telemetry数据同步到holding_registers

3. **数据解析问题**: comsrv正在读取原始寄存器值（如56319、16513），而不是解析后的float32值
   - 状态：需要进一步调试数据解析逻辑

### 修改的文件

1. **config/channel_1_power_meter/mapping_telemetry.csv**
   - 更新寄存器地址从0开始改为100开始，与服务器数据存储位置匹配

2. **scripts/modbus_multi_server.py**
   - 修改sync_to_modbus()方法，将遥测数据写入holding_registers而非input_registers
   - 更新update_values()方法，生成更真实的模拟数据

### 当前状态
- Modbus服务器正常运行，三个通道都在监听
- comsrv成功连接到所有服务器并进行轮询
- 数据正在读取，但float32解析似乎有问题
- Redis连接正常，但数据可能因为解析问题还未写入

### 下一步
- 调试float32数据解析逻辑
- 验证数据是否正确写入Redis
- 测试信号（YX）、控制（YK）、调整（YT）类型的数据读写

## 2024-01-XX - 修复Modbus float32数据解析

### 修改的文件

1. **src/core/protocols/modbus/modbus_polling.rs**
   - 在`ModbusPoint`结构体中添加了`data_format`、`register_count`和`byte_order`字段
   - 添加了`parse_modbus_value`函数，支持解析float32、uint32、int32、uint16、int16、bool等数据格式
   - 更新了`poll_batch`和`poll_single_point`函数，使用新的数据解析逻辑
   - 修复了批处理优化中的寄存器计数逻辑，正确处理多寄存器点

2. **src/core/protocols/modbus/client.rs**
   - 更新了`create_modbus_points`函数，为每个点添加数据格式信息
   - 遥测点使用mapping中的data_format、register_count和byte_order
   - 遥信点使用固定的"bool"格式

### 修复说明

之前的问题是comsrv只是简单地将u16寄存器值转换为f64，没有考虑实际的数据格式。对于float32类型的数据，需要：
1. 读取2个连续的寄存器（共4字节）
2. 按照正确的字节序组合成float32
3. 支持不同的字节序格式（ABCD、DCBA、BADC、CDAB）

修复后，系统能够正确解析float32数据，日志中将显示实际的浮点数值（如220.5）而不是原始寄存器值（如56319）。

## 之前的修复记录...

[保留之前的内容]

## 2025-01-07 - 实现通道级别报文日志记录

### 修改的文件

1. **src/core/protocols/modbus/protocol_engine.rs**
   - 添加了 `tracing` 的 `info_span` 和 `Instrument` 导入
   - 添加了 `format_hex` 函数将字节数组格式化为十六进制字符串
   - 添加了 `format_function_code` 函数将功能码转换为可读字符串
   - 在 `send_raw_request` 方法中添加了请求和响应报文的日志记录

2. **src/core/protocols/modbus/client.rs**
   - 添加了 `info_span` 和 `Instrument` 导入
   - 在 `start` 方法中创建了通道特定的 span
   - 使用 `.instrument()` 将所有操作包装在通道 span 中

3. **src/utils/channel_logging.rs** (新文件)
   - 创建了通道特定日志记录的工具模块
   - 实现了 `create_channel_logger` 函数创建通道级别的日志层
   - 实现了 `format_modbus_packet` 函数格式化 Modbus 报文

4. **src/utils/mod.rs**
   - 添加了 `channel_logging` 模块的导出

### 功能说明

- 使用 tracing 框架而不是传统的 logger
- 每个通道的操作都在特定的 span 中执行
- 报文日志使用 `info!` 级别，包含方向（request/response）、从站ID、功能码、字节数和十六进制数据
- PDU 解析过程保持在 `debug!` 级别
- 日志以 JSON 格式输出到主日志文件，可以通过 channel_id 或 channel_name 字段过滤特定通道的日志

### 简化说明

最终采用了更简单的实现方案：
- 删除了复杂的 channel_logging 模块
- 直接在 ModbusProtocolEngine 中使用 tracing 记录报文
- 利用现有的日志配置，通过 JSON 格式便于后续分析和过滤

## 2025-01-08 - 实现通道特定日志文件记录

### 修改的文件

1. **src/main.rs**
   - 修改了 `initialize_logging` 函数，添加了基于字段的过滤器
   - 使用 `FilterFn` 过滤包含 "direction" 字段的日志（Modbus报文）
   - 主日志文件排除所有报文日志，只记录服务整体日志

2. **src/core/protocols/modbus/protocol_engine.rs**
   - 添加了 `channel_log_file` 字段存储通道特定的日志文件句柄
   - 在 `set_channel_info` 方法中创建通道日志文件
   - 添加了 `write_channel_log` 方法写入通道特定日志
   - 在 `send_raw_request` 中同时写入 tracing 日志和通道文件

### 功能说明

- 每个通道在 `logs/{channel_name}/` 目录下创建独立的日志文件
- 文件名格式为 `channel_{id}_messages.log`
- 通道日志文件只包含该通道的 Modbus 请求和响应报文
- 主日志文件不再包含报文详情，只保留服务级别的日志
- 实现了日志分离，便于独立查看和分析各通道的通信情况

## 2025-07-07 统一点位编号系统

### 问题描述
- 四遥点位的ID在不同通道之间有重叠，需要统一为从1开始的编号
- Redis键格式需要包含channel_id和telemetry_type来区分不同通道的相同点位ID

### 修改文件
1. **所有通道的CSV配置文件** - 将point_id改为从1开始
   - channel_1_power_meter/*.csv - 修改了所有四遥表中的point_id
   - channel_2_plc/*.csv - 修改了所有四遥表中的point_id
   - channel_3_temperature/*.csv - 修改了所有四遥表中的point_id

2. **src/core/protocols/common/data_types.rs**
   - PointData结构体添加telemetry_type和channel_id字段
   - TelemetryType枚举添加Copy trait
   - 更新to_point_data方法设置telemetry_type

3. **src/core/protocols/common/redis.rs**
   - RedisBatchSyncConfig添加channel_id字段
   - Redis键格式改为 `comsrv:channel:{channel_id}:{telemetry_type}:{point_id}`
   - 更新sync_with_pipeline和sync_individually方法使用新键格式

4. **src/core/protocols/modbus/client.rs**
   - 更新read_point方法设置channel_id和telemetry_type
   - 添加write_point_with_type方法支持telemetry_type参数
   - ComBase trait的read_point方法支持"YC:1"格式查询
   - Redis同步时传递channel_id

5. **src/core/protocols/modbus/modbus_polling.rs**
   - 更新PointData创建时设置telemetry_type字段
   - 添加TelemetryType转换逻辑

6. **src/core/protocols/modbus/protocol_engine.rs**
   - 修复PointData初始化时缺少的telemetry_type和channel_id字段

### 当前状态
- 编译成功，程序可以正常运行
- 通道可以成功连接到Modbus服务器
- 发现映射表加载为0，导致没有创建轮询点

### 下一步
- 调试CSV加载器，找出为什么映射表没有被正确加载
- 确保轮询引擎能够正确创建点位并开始轮询

## 2025-07-07 修复协议映射加载失败问题

### 问题描述
在 `protocol_factory.rs` 中，`create_modbus_mapping_table` 方法期望的是 `crate::core::config::types::ChannelConfig` 类型，但在 `create_client` 方法中错误地使用了 `config_mgr.get_channel(config.id)` 重新获取配置，导致无法正确传递 `combined_points` 数据。

### 修改文件
- **src/core/protocols/common/combase/protocol_factory.rs**

### 修改内容
1. 修改 `create_modbus_mapping_table` 方法的参数类型为 `&ChannelConfig`（第199行）
2. 在 `create_client` 方法中，直接使用传入的 `config` 参数而不是重新从 `ConfigManager` 获取（第415-420行）
3. 同样修改 fallback 代码路径（第464-469行）

### 结果
成功加载CSV映射，日志显示：
- Channel 1: 成功加载48个协议映射
- Channel 2: 成功加载32个协议映射  
- Channel 3: 成功加载16个协议映射

## 2025-07-08 环境变量支持CSV路径配置

### 问题描述
- CSV文件路径使用绝对路径不利于在不同环境部署
- 配置文件中有拼写错误导致CSV文件无法加载

### 修改文件
1. **src/core/config/unified_loader.rs**
   - 在 `load_channel_tables` 方法中添加环境变量支持
   - 检查 `COMSRV_CSV_BASE_PATH` 环境变量
   - 如果设置了则使用环境变量作为基础路径
   - 否则使用配置文件所在目录作为基础路径

2. **config/modbus_test.yml**
   - 修复拼写错误：`channe_1_power_meter` 改为 `channel_1_power_meter`

3. **scripts/run_with_env.sh** (新增)
   - 创建示例启动脚本，展示如何使用环境变量

4. **CLAUDE.md**
   - 在 Configuration Management 部分添加环境变量支持说明

### 使用方法
```bash
# 设置环境变量运行
export COMSRV_CSV_BASE_PATH="/path/to/csv/files"
cargo run --bin comsrv -- --config config/modbus_test.yml

# 或使用提供的脚本
./scripts/run_with_env.sh
```

### 测试结果
- 系统能够使用环境变量正确识别CSV文件路径
- 配置更加灵活，适合不同部署环境

## 2025-07-08 修复CSV映射文件point_id不匹配问题

### 问题描述
- comsrv启动时无法加载CSV文件，日志显示"Created Modbus mapping table with 0 telemetry, 0 signal, 0 adjustment, 0 control points"
- 四遥文件（telemetry.csv等）的point_id从1开始，而映射文件（mapping_*.csv）的point_id使用了通道偏移（channel_1: 1001+, channel_2: 1101+, channel_3: 1201+）
- UnifiedCsvLoader通过point_id精确匹配，导致无法建立关联

### 修改文件
1. **config/channel_1_power_meter/mapping_*.csv** (4个文件)
   - mapping_telemetry.csv: point_id 1001-1012 改为 1-12
   - mapping_signal.csv: point_id 2001-2008 改为 1-8
   - mapping_adjustment.csv: point_id 3001-3006 改为 1-6
   - mapping_control.csv: point_id 4001-4004 改为 1-4

2. **config/channel_2_plc/mapping_*.csv** (4个文件)
   - mapping_telemetry.csv: point_id 1101-1108 改为 1-8
   - mapping_signal.csv: point_id 2101-2108 改为 1-8
   - mapping_adjustment.csv: point_id 3101-3106 改为 1-6
   - mapping_control.csv: point_id 4101-4106 改为 1-6

3. **config/channel_3_temperature/mapping_*.csv** (4个文件)
   - mapping_telemetry.csv: point_id 1201-1208 改为 1-8
   - mapping_signal.csv: point_id 2201-2206 改为 1-6
   - mapping_adjustment.csv: point_id 3201-3206 改为 1-6
   - mapping_control.csv: point_id 4201-4204 改为 1-4

### 测试结果
- Channel 1 (Power Meter): 成功加载12个协议映射（4个telemetry + 2个signal + 2个adjustment + 4个control）
- Channel 2 (PLC Controller): 成功加载8个协议映射（0个telemetry + 2个signal + 0个adjustment + 6个control）
- Channel 3 (Temperature Controller): 成功加载8个协议映射（2个telemetry + 0个signal + 2个adjustment + 4个control）
- comsrv服务正常启动，可以成功连接到Modbus服务器并进行数据轮询

## 2025-07-08 实现通道独立日志文件

### 问题描述
- 用户需要将每个通道的报文日志写入独立的日志文件（channel_X.log）
- 需要支持JSON格式和每日轮转
- 不解析功能码，只显示原始十六进制报文
- 保留slave_id字段

### 修改文件
1. **src/core/protocols/modbus/protocol_engine.rs**
   - 移除了 `format_function_code` 函数
   - 修改日志格式，不再解析功能码，只显示原始十六进制数据
   - 添加了通道日志文件支持（channel_log_file字段）
   - 在 `set_channel_info` 方法中创建通道特定的日志文件
   - 添加 `write_channel_log` 方法写入JSON格式日志
   - 在 `send_raw_request` 中同时记录到tracing日志和通道文件

2. **src/main.rs**
   - 尝试修改 `initialize_logging` 函数支持通道级日志层（未成功）
   - 保留了主日志文件的过滤器，排除Modbus报文日志

### 实现细节
- 每个通道在 `logs/{channel_name}/channel_{id}.log` 目录下创建日志文件
- 日志格式为JSON，包含字段：timestamp, level, channel_id, channel_name, direction, slave_id, hex, bytes
- 使用 `RollingFileAppender` 支持每日轮转（通过手动文件管理实现）
- 主日志文件通过FilterFn过滤掉包含"direction"字段的报文日志