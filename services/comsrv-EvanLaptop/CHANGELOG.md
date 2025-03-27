# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [unreleased] - 2025-02-25

### Added
- **Physical Interface Type**
  - Added physical interface type to the base class(添加物理接口类型到基类)
  - Added physical interface type to the Modbus Master and Slave(添加物理接口类型到Modbus主从)

### Removed
- **Combase type**
  - Removed init parameter running_ and redisCtx_ in ComBase(删除ComBase的init参数running_和redisCtx_)

## [0.4.0] - 2025-02-25

### Added
- **Comprehensive Main Function**
  - Command-line argument processing for configuration paths and modes(命令行参数处理配置路径和模式)
  - Signal handling for graceful shutdown(优雅关闭信号处理)
  - Proper initialization of logging and configuration systems(日志和配置系统正确初始化)
  - Dynamic channel creation and management based on configuration(基于配置动态创建和管理通道)
  - Hot-reload capability for configuration changes(配置更改的热重载能力)
  - Modbus TCP and RTU support(Modbus TCP和RTU支持)
  - Modbus Master and Slave support(Modbus主从支持)
  - Redis logging and monitoring support(Redis日志和监控支持)
  - Configuration monitoring and hot-reloading(配置监控和热重载)
  - Error handling and reporting(错误处理和报告)
  - Configfile support for multiple channels(支持多个通道的配置文件)

- **Build System Enhancements**
  - Improved CMake configuration with better output organization(改进CMake配置，更好的输出组织)
  - Added shared library generation(添加共享库生成)
  - Added installation targets for headers, binaries, and configs(添加头文件、二进制文件和配置文件的安装目标)
  - Enhanced compiler warning options(增强编译器警告选项)
  - Support for testing infrastructure(支持测试基础设施)
  
- **Development Environment**
  - Enhanced Docker development environment with additional tools(增强Docker开发环境，添加更多工具)
  - Useful shell aliases for common development tasks(有用的shell别名，用于常见开发任务)  
  - Improved documentation and setup instructions(改进文档和设置说明)
  
- **Documentation Updates**
  - Expanded README with comprehensive usage instructions(扩展README，包含全面的用法说明)
  - Technical documentation for the core architecture(核心架构的技术文档)
  - Updated CHANGELOG with bilingual descriptions(更新CHANGELOG，包含双语描述)
  
### Changed
- Refactored signal handling for better reliability(更好的可靠性信号处理重构)
- Improved Redis connection handling with fallback options(改进Redis连接处理，添加回退选项)
- Enhanced error reporting and logging consistency(增强错误报告和日志一致性)

## [0.3.0] - 2025-02-18

### Added
- **Channel Configuration Restructuring** (通道配置重构)
  - Separated channel configuration from point table for better maintainability (将通道配置和点表配置分离，提高可维护性)
  - Channel configuration file (channels.json) includes: (通道配置文件包含)
    * Global configuration (logging, Redis, etc.) (全局配置：日志、Redis等)
    * Channel basic information and protocol configuration (通道基本信息和协议配置)
    * Point table file path references (点表文件路径引用)
  - Point tables use CSV format, separated by four-remote types: (点表采用CSV格式，按四遥类型分文件)
    * `*_di.csv`: Digital input point table (数字量输入点表)
    * `*_ai.csv`: Analog input point table (模拟量输入点表)
    * `*_do.csv`: Digital output point table (数字量输出点表)
    * `*_ao.csv`: Analog output point table (模拟量输出点表)

- **Modbus Communication Optimization** (Modbus通信优化)
  - Implemented segmented polling functionality: (实现分段召唤功能)
    * Added maxRead configuration item to limit single read length (添加maxRead配置项，限制单次读取长度)
    * Automatic analysis of point addresses to merge consecutive address points (自动分析点位地址，合并连续地址点位)
    * Optimized reading strategy to reduce communication frequency (优化读取策略，减少通信次数)
    * Support for calculating register counts for different data types (支持不同数据类型的寄存器数量计算)

- **Docker Environment Configuration** (Docker环境配置)
  - Added multi-stage build Dockerfile: (添加多阶段构建Dockerfile)
    * Build stage: Complete compilation environment (构建阶段：包含完整编译环境)
    * Runtime stage: Only runtime dependencies (运行阶段：仅包含运行时依赖)
    * Timezone configuration and environment variable settings (时区配置和环境变量设置)
  - Configured docker-compose.yml: (配置docker-compose.yml)
    * Integrated Redis service (集成Redis服务)
    * Configuration file and log directory mapping (配置文件和日志目录映射)
    * Serial port device access support (串口设备访问支持)
  - Added CMake build system: (添加CMake构建系统)
    * C++17 standard setup (设置C++17标准)
    * Necessary dependency configuration (配置必要的依赖库)
    * Installation path and configuration file deployment setup (设置安装路径和配置文件部署)

- **Other Improvements** (其他改进)
  - Added startup script (start.sh): (添加启动脚本)
    * Automatically create necessary directories (自动创建必要目录)
    * Set serial port device permissions (设置串口设备权限)
    * Container build and startup management (容器构建和启动管理)
  - Enhanced error handling and logging (完善错误处理和日志记录)
  - Optimized code structure and comments (优化代码结构和注释)

### Dependencies
- libmodbus: Modbus protocol support (Modbus协议支持)
- hiredis: Redis client (Redis客户端)
- jsoncpp: JSON parsing (JSON解析)
- Ubuntu 22.04 base image (Ubuntu 22.04基础镜像)

### Future Plans
- Add unit tests (添加单元测试)
- Implement configuration hot-reload (实现配置热重载)
- Add performance monitoring (添加性能监控)
- Improve documentation (完善文档)

## [0.2.0] - 2025-02-17

### Added
- Docker deployment support (Docker化部署支持)
  * Added Dockerfile and multi-stage build (添加 Dockerfile 和多阶段构建)
  * Added docker-compose.yml configuration (添加 docker-compose.yml 配置)
  * Added deployment startup script start.sh (添加部署启动脚本 start.sh)
  * Added device configuration file templates (添加设备配置文件模板)

### Changed
- Restructured Modbus communication architecture (重构 Modbus 通信架构)
  * Simplified inheritance hierarchy, removed ModbusRtuCom intermediate layer (简化继承层次，移除 ModbusRtuCom 中间层)
  * ModbusRTU now inherits directly from ModbusCom (ModbusRTU 现在直接继承自 ModbusCom)
  * Enhanced ModbusCom base class functionality (增强 ModbusCom 基类功能)
  * Improved error handling and logging mechanisms (改进错误处理和日志机制)

### Removed
- Removed multi-device management mode (移除多设备管理模式)
  * Deleted ModbusDevice related code (删除 ModbusDevice 相关代码)
  * Removed device collection management functionality (移除设备集合管理功能)
  * Simplified device connection logic (简化设备连接逻辑)

## [0.1.0] - 2025-02-13

### Added
- Initial version (初始版本)
  * Basic communication framework (基础通信框架)
  * Modbus RTU protocol support (Modbus RTU 协议支持)
  * Modbus TCP protocol support (Modbus TCP 协议支持)
  * Basic device management functionality (基本的设备管理功能)
  * Error handling mechanism (错误处理机制)
  * Logging functionality (日志记录功能)

### Fixed
- Fixed serial port communication timeout issues (修复串口通信超时问题)
- Fixed concurrent access issues with multiple devices (修复多设备并发访问问题)
- Fixed memory leak issues (修复内存泄漏问题)
