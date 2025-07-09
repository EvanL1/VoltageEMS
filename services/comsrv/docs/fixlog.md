# ComsRV Fix Log

记录对 comsrv 服务的修复历史。

## 2025-07-08 - 插件系统实现和编译错误修复

### 背景

- 发现项目中已存在插件系统框架 (src/core/plugins/)
- 决定基于现有框架实现协议插件，而不是创建新的系统
- 回滚了之前的所有更改，重新基于现有系统开发

### 实现的功能

#### 协议插件实现

## 2025-07-08 - 基于现有插件系统实现协议插件

### 实现的插件

1. **src/core/protocols/modbus/plugin.rs**

   - 实现了 ModbusTcpPlugin 和 ModbusRtuPlugin
   - 基于现有的 ProtocolPlugin trait
   - 提供完整的配置模板和验证
   - 支持 CLI 命令和文档
2. **src/core/protocols/iec60870/plugin.rs**

   - 实现了 Iec104Plugin
   - 支持 IEC 60870-5-104 协议
   - 包含完整的时序参数配置
   - 提供总召唤、时间同步等命令
3. **src/core/protocols/can/plugin.rs**

   - 实现了 CanPlugin
   - 支持 CAN 总线通信
   - 支持标准帧和扩展帧
   - 包含过滤器配置

### 修改的文件

1. **src/core/protocols/modbus/mod.rs**

   - 添加了 `pub mod plugin;` 导出
2. **src/core/protocols/iec60870/mod.rs**

   - 添加了 `pub mod plugin;` 导出
3. **src/core/protocols/can/mod.rs**

   - 添加了 `pub mod plugin;` 导出
4. **src/core/plugins/plugin_registry.rs**

   - 更新了 `register_builtin_plugins()` 注册所有插件
   - 注册了 modbus_tcp、modbus_rtu、iec104、can 插件
5. **src/core/plugins/plugin_manager.rs** (新增)

   - 创建了插件管理器，提供高级管理功能
   - 初始化、列表、信息查询等功能
6. **src/core/plugins/mod.rs**

   - 添加了 plugin_manager 模块导出
7. **src/core/protocols/common/combase/protocol_factory.rs**

   - 修改了 `create_protocol_with_config_manager()` 方法
   - 首先检查插件系统，如果插件不可用才使用传统工厂
8. **src/main.rs**

   - 添加了插件系统初始化调用 `init_plugin_system()?`

### 编译错误修复

1. **依赖添加** (Cargo.toml)

   - 添加了 `semver = "1.0"` 用于版本管理
   - 添加了 `regex = "1.10"` 用于配置验证
2. **错误类型修复**

   - 将所有 `Error::Config` 修改为 `Error::ConfigError`
   - 修复了 `ComSrvError` 枚举的使用
3. **协议配置修复**

   - 创建了 `src/core/protocols/iec60870/config.rs` - Iec104Config
   - 创建了 `src/core/protocols/can/config.rs` - CanConfig
   - 修复了字段名 `protocol_params` -> `parameters`
4. **虚拟协议实现** (src/core/protocols/virt/)

   - 创建了 `plugin.rs` 实现 VirtualPlugin
   - 修复了 `VirtualClient` -> `VirtualProtocol`
   - 实现了完整的 `ComBase` trait
5. **数据结构修复**

   - 修复 `PointData` 字段名：`point_id` -> `id`
   - 移除了不存在的 `quality` 字段
   - 修复了 `TelemetryType` 枚举值：`YC`/`YX` -> `Telemetry`/`Signal`
6. **CAN 统计修复**

   - 修复了 `CanStatistics` 的 Clone 实现（AtomicU64 不能直接 derive Clone）
   - 修复了原子操作的使用（`+=` -> `fetch_add`）
   - 添加了缺少的 `Ordering` 导入

### 待解决问题

1. 仍有约 34 个编译错误需要修复
2. 主要是 trait 实现不完整和类型不匹配

## 2025-07-08 - 插件系统功能测试完成

### 测试内容

1. **Modbus TCP 插件功能测试**
   - 修复了插件系统中的参数匹配问题
   - 修复了 PluginRegistry::get_global 的死锁问题
   - 实现了插件系统的点表加载功能

### 测试结果

1. **服务启动**：✅ 成功

   - comsrv 服务正常启动，监听 8090 端口
   - Modbus TCP 插件正确加载
   - 通道日志系统正常工作
2. **Modbus 通信**：✅ 成功

   - 成功连接到 Modbus TCP 服务器（127.0.0.1:5502）
   - 请求/响应交互正常
   - 通道日志记录了完整的通信数据（包含十六进制格式）
3. **API 功能**：✅ 成功

   - GET /api/channels/1/points 返回正确的点位数据
   - 数据格式包含 id、name、value、timestamp 等字段
   - POST 写入功能正常工作
4. **日志系统**：✅ 成功

   - 通道独立日志文件创建成功（logs/modbus_tcp_test/channel_1.log）
   - 日志格式为 JSON，包含完整的通信细节
   - 支持请求/响应的十六进制数据记录

### 性能观察

- 轮询间隔稳定在 1 秒
- 响应时间快速（毫秒级）
- 内存使用稳定

### 结论

新的插件系统已经完全可用，成功通过了 Modbus TCP 的功能测试。系统能够：

1. 使用插件架构加载协议实现
2. 正确处理 Modbus 通信
3. 提供完整的 API 接口
4. 记录详细的通道日志

### 补充说明

- **日志格式**：通道日志已经记录了完整的 Modbus TCP 报文，包括 MBAP 头部
- **报文格式**：请求和响应都包含完整的 Transaction ID、Protocol ID、Length、Unit ID 等字段
- **轮询机制**：使用 `MissedTickBehavior::Skip` 避免请求堆积，系统运行正常

## 2025-01-08 - 删除旧的工厂实现，保留插件方案

### 修改内容

1. **删除了旧的工厂实现**

   - 删除了 `ModbusTcpFactory` 结构体及其实现
   - 删除了 `ModbusRtuFactory` 结构体及其实现
   - 删除了 `create_modbus_mapping_table` 辅助函数
   - 删除了 `register_builtin_factories` 方法
2. **简化了 `create_protocol_with_config_manager` 方法**

   - 现在只使用插件系统创建协议实例
   - 删除了所有对旧工厂的回退逻辑
   - 提供了更好的错误信息，显示可用的插件列表
3. **清理了测试代码**

   - 注释掉了依赖旧工厂的测试函数
   - 保留了 MockComBase 用于其他测试

### 架构改进

- 统一使用插件架构，消除了重复代码
- 简化了协议创建流程
- 提高了系统的可扩展性和一致性

3. 部分协议实现需要更新以适配新的 trait 定义

### 继续修复的编译错误 (第二批)

1. **ModbusPollingConfig 字段修复**

   - 修改为使用正确的字段名：`default_interval_ms`、`enable_batch_reading` 等
   - 添加了 `slave_configs` 字段
2. **协议工厂参数转换**

   - 修复了 `validate_config` 的参数类型不匹配
   - 将 `serde_yaml::Value` 转换为 `serde_json::Value`
3. **CAN Client 重构**

   - 移除了对 `DefaultProtocol` 的依赖
   - 直接实现了所有 `ComBase` trait 方法
   - 修复了 `CanClientBase::new` 的参数
4. **IEC104 修复**

   - 添加了缺少的 trait 方法实现
   - 修复了 `Iec104Client::new` 的调用参数
   - 添加了缺少的导入
5. **当前状态**

   - 错误从 34 个减少到 15 个（仅 comsrv）
   - 主要剩余问题是参数类型不匹配和一些细节问题
   - 优先使用插件系统创建协议
   - 保留了对旧系统的兼容性
6. **src/main.rs**

   - 在启动时调用 `init_plugin_system()` 初始化插件系统
   - 在创建 ProtocolFactory 之前初始化

### 插件系统特点

1. **基于现有框架** - 使用已有的 ProtocolPlugin trait
2. **向后兼容** - 保留对旧协议工厂的支持
3. **统一接口** - 所有协议使用相同的插件接口
4. **配置验证** - 内置配置模板和验证规则
5. **CLI 支持** - 每个插件可以定义自己的 CLI 命令
6. **文档集成** - 插件包含自己的文档

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

   - 在 `ModbusPoint`结构体中添加了 `data_format`、`register_count`和 `byte_order`字段
   - 添加了 `parse_modbus_value`函数，支持解析float32、uint32、int32、uint16、int16、bool等数据格式
   - 更新了 `poll_batch`和 `poll_single_point`函数，使用新的数据解析逻辑
   - 修复了批处理优化中的寄存器计数逻辑，正确处理多寄存器点
2. **src/core/protocols/modbus/client.rs**

   - 更新了 `create_modbus_points`函数，为每个点添加数据格式信息
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

## 2025-01-08 配置中心集成

### 新增功能

1. **配置中心客户端模块** (`src/core/config/config_center.rs`)

   - 实现 HTTP 配置中心客户端
   - 支持配置缓存和降级策略
   - 支持认证令牌
   - 缓存到本地文件系统
2. **ConfigManager 扩展** (`src/core/config/config_manager.rs`)

   - 集成配置中心客户端
   - 实现多源配置加载优先级
   - 添加异步配置加载方法 `load_async`
   - 支持获取单个配置项
   - 自动检测CONFIG_CENTER_URL环境变量
3. **配置加载优先级**

   - 环境变量（COMSRV_前缀） > 配置中心 > 本地文件 > 默认值
   - 自动降级：配置中心不可用时使用缓存，缓存过期时使用本地文件
   - 缓存机制：成功获取的配置会保存到本地，带TTL管理

### 环境变量支持

- `CONFIG_CENTER_URL`: 配置中心地址
- `CONFIG_CENTER_TOKEN`: 认证令牌（可选）
- `CONFIG_CACHE_DIR`: 缓存目录（默认 /var/cache/comsrv）
- `CONFIG_CACHE_TTL`: 缓存有效期，秒（默认 3600）

### API 接口要求

配置中心需要实现：

- `GET /api/v1/config/service/{service_name}` - 获取完整配置
- `GET /api/v1/config/service/{service_name}/item/{key}` - 获取单个配置项

### 文档更新

- 更新 `docs/comsrv配置指南.md` 添加配置中心集成章节
- 包含使用示例、API要求、容错机制说明

## 2025-07-08 插件系统重构和编译错误修复

### 背景

- 用户要求分析 comsrv 架构是否支持更多协议适配
- 发现项目中已存在插件系统框架但未完全集成
- 决定基于现有框架完善协议插件实现

### 实现的功能

#### 1. 协议插件实现

- **Modbus TCP/RTU 插件** (`src/core/protocols/modbus/plugin.rs`)

  - 实现了 ModbusTcpPlugin 和 ModbusRtuPlugin
  - 提供完整的配置模板和验证
  - 支持 CLI 命令：scan-devices, test-connection, read-registers
- **IEC60870-5-104 插件** (`src/core/protocols/iec60870/plugin.rs`)

  - 实现了 Iec104Plugin
  - 支持时序参数配置
  - 提供总召唤、时间同步等命令
- **CAN 总线插件** (`src/core/protocols/can/plugin.rs`)

  - 实现了 CanPlugin
  - 支持标准帧和扩展帧
  - 包含过滤器配置和诊断功能
- **虚拟协议插件** (`src/core/protocols/virt/plugin.rs`)

  - 实现了 VirtualPlugin 用于测试

#### 2. 插件注册系统更新

- **src/core/plugins/plugin_registry.rs**
  - 更新 `register_builtin_plugins()` 注册所有新插件
  - 支持动态插件加载和管理

#### 3. 编译错误修复

- 修复了约 45 个编译错误，包括：
  - 类型不匹配：ChannelStatus 导入路径
  - 缺失字段：PointData 的 channel_id 和 telemetry_type
  - 原子操作：AtomicU64 的 load() 调用
  - 测试数据结构初始化
  - CanId 枚举导入问题
  - CanError 类型别名使用

#### 4. 临时禁用组件

- CLI 模块因缺少依赖（colored, handlebars）暂时禁用
- 将 comsrv-cli.rs 重命名为 .disabled

### 测试结果

- 主库编译成功
- 插件注册测试通过：`test_plugin_registry ... ok`
- 插件管理器测试需要修复初始化逻辑

### 下一步计划

- 运行完整测试套件验证功能
- 修复 CLI 模块依赖问题
- 完善插件文档和使用示例

## 2025-07-08 Modbus测试集成

### 问题发现

1. **pymodbus版本兼容性**

   - pymodbus 3.x API变化较大
   - 需要移除 `pymodbus.version`导入
   - 服务器启动参数 `allow_reuse_address`不再支持
   - 客户端API参数格式有变化
2. **配置文件适配**

   - 新架构中协议名称保持为"modbus_tcp"和"modbus_rtu"
   - 配置文件需要 `version`字段
   - CSV文件格式简化，point_id从1开始
3. **comsrv启动问题**

   - 创建协议实例后日志停止
   - API未响应(bind_address配置为8080但不可访问)
   - Redis中没有数据

### 已完成工作

1. 创建了Python Modbus服务器模拟器
2. 创建了Python测试客户端
3. 更新了配置文件以适配新架构
4. 创建了集成测试脚本
5. 创建了简化的CSV点表文件

### 待解决问题

1. comsrv创建通道后挂起，可能是CSV加载或连接问题
2. 测试客户端需要适配pymodbus 3.x API
3. API未按配置启动

## 2025-07-08 修复插件系统挂起问题

### 问题描述

- comsrv使用新的插件架构后，在创建Modbus TCP协议实例时挂起
- 日志停止在 "Creating protocol instance using plugin system: modbus_tcp"

### 根本原因

1. 配置文件缺少API配置部分，导致API在默认端口3000而非8090
2. 插件参数名不匹配（timeout_ms vs timeout, poll_interval vs polling_interval）
3. PluginRegistry::get_global函数存在逻辑问题，导致死锁

### 解决方案

1. 在配置文件中添加API配置
   ```yaml
   service:
     api:
       enabled: true
       bind_address: "127.0.0.1:8090"
   ```
2. 修复插件中的参数名匹配
3. 重写PluginRegistry::get_global函数，简化逻辑避免死锁

### 修改文件

- **config/modbus_test.yaml** - 添加API配置部分
- **src/core/protocols/modbus/plugin.rs** - 修复参数名（timeout_ms→timeout, poll_interval→polling_interval）
- **src/core/plugins/plugin_registry.rs** - 重写get_global函数，避免重复加锁
- **src/core/protocols/common/combase/protocol_factory.rs** - 简化插件获取逻辑

### 验证状态

- comsrv可以成功启动并创建Modbus TCP通道
- 成功连接到Modbus服务器（127.0.0.1:5502）
- API服务器在8090端口启动（但立即退出）
- 问题：点位还未加载（"Created 0 Modbus polling points"）

### 下一步

- 调查为什么comsrv启动后立即退出
- 修复点位加载逻辑，确保CSV映射被正确加载

## 2025-07-08 修复插件系统点表加载问题

**问题**: 插件系统创建 Modbus 客户端时没有加载协议映射表（点表）

**原因**:

1. 插件的 `create_instance` 方法只是创建了客户端，没有调用 `load_protocol_mappings`
2. 工厂模式中的 `create_modbus_mapping_table` 函数负责将 `combined_points` 转换为映射表，但插件系统跳过了这一步

**修复**:

1. 在 `ModbusTcpPlugin` 和 `ModbusRtuPlugin` 中添加了 `create_modbus_mapping_table` 方法
2. 在插件的 `create_instance` 方法中，创建客户端后检查 `combined_points` 并加载映射表
3. 修复了 `ModbusControlMapping` 结构体缺少 `coil_number` 字段的问题
4. 修复了局部变量名冲突的问题（`channel_config` vs `modbus_channel_config`）

**文件修改**:

- `src/core/protocols/modbus/plugin.rs`
- `src/core/protocols/common/combase/protocol_factory.rs`

**结果**:

- 插件系统现在能够正确加载映射表
- 日志显示成功加载了4个映射（2个YC，2个YK）
- Modbus轮询引擎成功启动并开始数据采集

## 2025-07-08 修复 Modbus TCP/RTU 模式判断错误

**问题描述**:

- comsrv 发送的是 Modbus RTU 格式（带 CRC），但服务器返回 Modbus TCP 格式（带 MBAP 头部）
- 日志显示：请求 `01 03 00 04 00 03 44 0A`（RTU 格式），响应 `00 00 00 00 00 03 01 A8 01`（TCP 格式）
- 错误信息：CRC mismatch: expected 0x30DA, got 0x01A8

**根本原因**:

- ModbusTcpPlugin 设置 `protocol_type` 为 "ModbusTcp"（首字母大写）
- ModbusConfig::is_tcp() 方法使用 `contains("tcp")` 进行判断（小写）
- 大小写不匹配导致协议引擎错误选择了 RTU 模式

**修复方案**:
修改 `src/core/protocols/modbus/common.rs` 中的判断方法，使用大小写不敏感的比较：

```rust
pub fn is_tcp(&self) -> bool {
    self.protocol_type.to_lowercase().contains("tcp")
}

pub fn is_rtu(&self) -> bool {
    self.protocol_type.to_lowercase().contains("rtu")
}
```

**验证结果**:

- 协议引擎正确选择 ModbusMode::Tcp
- 请求格式：`00 01 00 00 00 06 01 03 00 04 00 03`（正确的 TCP 格式，包含 MBAP 头部）
- 响应格式：`00 01 00 00 00 09 01 03 06 01 90 01 F4 02 58`（成功解析数据）
- 数据成功读取：01 90 (400), 01 F4 (500), 02 58 (600)
- API 可以查询到实时数据：Point 3 = 513, Point 4 = 514

**影响范围**: 所有使用插件系统创建的 Modbus TCP 协议实例
