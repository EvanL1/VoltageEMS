# Comsrv - Rust版本高度可配置的通信服务

Comsrv是一个用Rust实现的高度可配置的通信服务，用于连接和管理各种工业设备和协议。它提供了一个统一的接口来处理不同的通信协议，如Modbus RTU、Modbus TCP等。

## 特性

- 支持多种工业通信协议
  - Modbus RTU 主/从设备
  - Modbus TCP 主/从设备
  - 可扩展以支持更多协议
- 基于配置的设备管理
- 灵活的数据轮询和处理
- 数据导出到Redis和MQTT
- 实时数据处理和监控
- 线程安全设计
- 高性能和低延迟
- Prometheus指标集成

## 架构

Comsrv采用模块化架构设计，包括以下组件：

1. **核心框架**：提供基础设施，如配置管理、日志记录等。
2. **通信接口**：定义通信协议的基本接口和抽象类。
3. **协议实现**：各种通信协议的具体实现。
4. **数据处理**：用于处理和转换数据的组件。
5. **数据导出**：用于将数据导出到外部系统，如Redis、MQTT的组件。
6. **指标**：用于监控系统性能和状态的Prometheus指标。

### 架构图

```
+------------------+     +------------------+
|  Config Manager  |     |  Metrics Manager |
+------------------+     +------------------+
        |                       |
+------------------+           |
| Protocol Factory |           |
+------------------+           |
        |                     |
        v                     v
+------------------+     +------------------+
|  ComBase Trait   | --> |  Metrics Export  |
+------------------+     +------------------+
        |                       |
        v                       v
+------------------+     +------------------+
| Data Processing  | --> |   Prometheus    |
+------------------+     +------------------+
```

## 配置

Comsrv使用YAML格式的配置文件来定义通信设备和参数。示例如下：

```yaml
version: "1.0"
service:
  name: "comsrv"
  description: "Communication Service"
  metrics:
    enabled: true
    bind_address: "0.0.0.0:9100"
  logging:
    level: "info"
    file: "/var/log/comsrv/comsrv.log"
    max_size: 10485760  # 10MB
    max_files: 5
    console: true

channels:
  - id: "pcs1"
    name: "PCS Controller 1"
    description: "Power Conversion System"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.10"
      port: 502
      timeout: 1000
      max_retries: 3
      point_tables:
        di: "points/pcs_di.csv"
        ai: "points/pcs_ai.csv"
        do: "points/pcs_do.csv"
        ao: "points/pcs_ao.csv"
      poll_rate: 1000
```

## 构建和安装

### 依赖

- Rust 1.70.0 或更高版本
- Cargo 包管理器

### 构建步骤

```bash
# 克隆代码库
git clone https://github.com/yourusername/comsrv.git
cd comsrv

# 构建项目
cargo build --release
```

### 安装

```bash
# 复制可执行文件
sudo cp target/release/comsrv /usr/local/bin/

# 创建配置目录
sudo mkdir -p /etc/comsrv
sudo cp -r config/* /etc/comsrv/
```

## 使用方法

### 运行服务

```bash
# 使用默认配置
comsrv

# 指定配置文件
comsrv /path/to/config.yaml

# 使用启动脚本
./start.sh /path/to/config.yaml
```

### 使用Docker

```bash
# 构建Docker镜像
docker build -t comsrv .

# 运行容器
docker run -v /path/to/config.yaml:/etc/comsrv/comsrv.yaml -d comsrv
```

### 监控

Comsrv在 `http://<host>:9100/metrics`暴露Prometheus指标。可用的指标包括：

- 通信指标：

  - `comsrv_bytes_total`：发送/接收的总字节数
  - `comsrv_packets_total`：发送/接收的总数据包数
  - `comsrv_packet_errors_total`：按类型统计的数据包错误数
  - `comsrv_packet_processing_duration_seconds`：数据包处理时间
- 通道指标：

  - `comsrv_channel_status`：通道连接状态
  - `comsrv_channel_response_time_seconds`：通道响应时间
  - `comsrv_channel_errors_total`：按类型统计的通道错误数
- 协议指标：

  - `comsrv_protocol_status`：协议状态
  - `comsrv_protocol_errors_total`：按类型统计的协议错误数
- 服务指标：

  - `comsrv_service_status`：服务状态
  - `comsrv_service_uptime_seconds`：服务运行时间
  - `comsrv_service_errors_total`：按类型统计的服务错误数

### 日志

Comsrv在服务和通道级别提供全面的日志记录：

- 服务日志（`/var/log/comsrv/comsrv.log`）：

  - 服务启动/关闭事件
  - 通道配置和状态变化
  - 系统级事件和错误
- 通道日志（`/var/log/comsrv/channels/<channel_id>.log`）：

  - 通道连接状态
  - 原始通信数据（INFO级别）
  - 数据解析详情（DEBUG级别）
  - 通道特定的错误和警告

## 开发

### 添加新协议

1. 创建一个新的协议结构体，实现ComBase trait
2. 实现所有required方法
3. 在ProtocolFactory中注册新协议类型

示例：

```rust
struct NewProtocol {
    base: ComBaseImpl,
    // 协议特定字段...
}

#[async_trait]
impl ComBase for NewProtocol {
    fn name(&self) -> &str {
        self.base.name()
    }
  
    fn channel_id(&self) -> &str {
        self.base.channel_id()
    }
  
    fn is_running(&self) -> bool {
        self.base.is_running()
    }
  
    async fn start(&mut self) -> Result<()> {
        // 实现启动逻辑
        Ok(())
    }
  
    async fn stop(&mut self) -> Result<()> {
        // 实现停止逻辑
        Ok(())
    }
  
    async fn status(&self) -> ChannelStatus {
        self.base.status().await
    }
}

// 在ProtocolFactory中注册
factory.register_protocol("new_protocol", |config| {
    // 创建协议实例
    let protocol = NewProtocol {
        base: ComBaseImpl::new("new_protocol", config),
        // 初始化其他字段...
    };
  
    Ok(Box::new(protocol))
}).await?;
```

### 添加新指标

1. 在Metrics结构体中定义新指标
2. 在构造函数中初始化
3. 添加方法来更新指标
4. 在协议实现中使用

示例：

```rust
// 在metrics.rs中
pub struct Metrics {
    // 现有指标...
    my_new_metric: Arc<IntCounterVec>,
}

impl Metrics {
    pub fn new(service_name: &str) -> Self {
        // 初始化其他指标...
        let my_new_metric = Arc::new(
            IntCounterVec::new(
                prometheus::opts!("comsrv_my_new_metric_total", "Description of my new metric"),
                &["service", "label"],
            )
            .expect("Failed to create my_new_metric"),
        );
      
        registry.register(Box::new(my_new_metric.clone())).expect("Failed to register my_new_metric");
      
        Metrics {
            // 其他字段...
            my_new_metric,
        }
    }
  
    // 添加方法来更新指标
    pub fn increment_my_new_metric(&self, label: &str, service_name: &str) {
        self.my_new_metric
            .with_label_values(&[service_name, label])
            .inc();
    }
}
```

## 贡献

欢迎提交Pull requests和issues。

## 许可证

MIT许可证
