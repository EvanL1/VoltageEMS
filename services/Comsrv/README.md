# Comsrv - 高度可配置的通信服务

Comsrv是一个高度可配置的通信服务，用于连接和管理各种工业设备和协议。它提供了一个统一的接口来处理不同的通信协议，如Modbus RTU、Modbus TCP等。

## 特性

- 支持多种工业通信协议
  - Modbus RTU主站/从站
  - Modbus TCP主站/从站
  - 未来可扩展更多协议
- 基于配置文件的设备管理
- 灵活的数据轮询和处理机制
- 数据导出到Redis和MQTT
- 实时数据处理和监控
- 线程安全的设计
- 高性能和低延迟

## 架构设计

Comsrv采用了模块化的架构设计，主要包括以下组件：

1. **核心框架**：提供基础设施，如配置管理、线程池、日志记录等。
2. **通信接口**：定义通信协议的基本接口和抽象类。
3. **协议实现**：各种通信协议的具体实现。
4. **数据处理**：处理和转换数据的组件。
5. **数据导出**：将数据导出到Redis、MQTT等外部系统的组件。

### 架构图

```
+------------------+
|   配置管理        |
+------------------+
        |
+------------------+
|   协议工厂        |
+------------------+
        |
        v
+------------------+     +------------------+
|   通信基类        | <-- |   协议实现        |
+------------------+     +------------------+
        |                       |
        v                       v
+------------------+     +------------------+
|   数据处理        | --> |   数据导出        |
+------------------+     +------------------+
```

## 配置文件

Comsrv使用JSON格式的配置文件来定义通信设备和参数。以下是配置文件的示例：

```json
{
    "protocols": [
        {
            "type": "modbus_rtu_master",
            "name": "RTU Master 1",
            "port": "/dev/ttyUSB0",
            "baudrate": 9600,
            "databits": 8,
            "parity": "none",
            "stopbits": 1,
            "timeout": 1000,
            "polling": [
                {
                    "slave_id": 1,
                    "address": 100,
                    "type": "holding_register",
                    "count": 10,
                    "register_type": "uint16",
                    "endian": "big_endian",
                    "interval": 1000,
                    "tag": "device1.registers"
                }
            ]
        }
    ]
}
```

## 编译和安装

### 依赖项

- C++17兼容的编译器
- CMake 3.10或更高版本
- nlohmann/json库
- 线程库

### 编译步骤

```bash
mkdir build
cd build
cmake ..
make
```

### 安装

```bash
sudo make install
```

## 使用方法

### 运行服务

```bash
comsrv /path/to/config.json
```

### 使用Docker

```bash
docker build -t comsrv .
docker run -v /path/to/config.json:/etc/comsrv/comsrv.json -d comsrv
```

## 扩展开发

### 添加新的协议

1. 创建新的协议类，继承自ComBase类
2. 实现所有必要的虚函数
3. 在ProtocolFactory中注册新的协议类型

示例：

```cpp
class NewProtocol : public ComBase {
public:
    NewProtocol(const std::string& name);
    virtual ~NewProtocol();
    
    bool start() override;
    bool stop() override;
    
    // 协议特定的方法...
};

// 在ProtocolFactory中注册
factory.registerProtocol("new_protocol", [](const std::map<std::string, ConfigManager::ConfigValue>& config) -> std::unique_ptr<ComBase> {
    // 创建新协议实例...
    return std::make_unique<NewProtocol>(name);
});
```

## 贡献

欢迎提交Pull Request或Issue。

## 许可证

MIT License 