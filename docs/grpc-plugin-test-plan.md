# ComSrv gRPC 插件架构测试计划

## 1. 测试计划概述

### 1.1 测试目标
本测试计划旨在全面验证 ComSrv gRPC 插件架构的功能、性能、安全性和可靠性。确保：
- gRPC 接口的正确实现和交互
- 多语言插件的兼容性和稳定性
- 系统在各种负载和故障场景下的表现
- 部署和运维流程的可行性

### 1.2 测试范围
- **功能测试**：验证所有 gRPC 接口的功能正确性
- **集成测试**：验证 ComSrv Core 与插件的集成
- **性能测试**：评估系统吞吐量、延迟和资源使用
- **安全测试**：验证认证、授权和数据安全
- **容错测试**：验证故障恢复和降级机制
- **部署测试**：验证容器化部署和配置管理

### 1.3 测试策略
- 采用分层测试方法，从单元测试到端到端测试
- 优先测试核心功能和高风险区域
- 使用自动化测试提高效率和覆盖率
- 模拟真实场景进行性能和压力测试

### 1.4 测试环境
- 开发环境：本地 Docker 环境
- 测试环境：Kubernetes 集群
- 性能测试环境：专用硬件或云环境

## 2. 测试环境搭建

### 2.1 基础环境要求

```yaml
# 硬件要求
CPU: 8核以上
内存: 16GB以上
存储: SSD 100GB以上
网络: 千兆以太网

# 软件要求
操作系统: Linux (Ubuntu 20.04 LTS 或 CentOS 8)
Docker: 20.10+
Docker Compose: 2.0+
Kubernetes: 1.22+ (性能测试)
Redis: 7.0+
```

### 2.2 测试环境配置

#### 2.2.1 Docker Compose 环境

```yaml
# docker-compose.test.yml
version: '3.8'

services:
  # Redis
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    command: redis-server --save 60 1000 --loglevel debug

  # ComSrv Core (测试版本)
  comsrv:
    build:
      context: .
      dockerfile: Dockerfile.test
    environment:
      RUST_LOG: debug
      REDIS_URL: redis://redis:6379
      PLUGIN_DISCOVERY: static
      PLUGIN_ENDPOINTS: |
        modbus: modbus-plugin:50051
        iec104: iec104-plugin:50052
        can: can-plugin:50053
    volumes:
      - ./config:/config
      - ./logs:/logs
    depends_on:
      - redis
      - modbus-plugin
      - iec104-plugin
      - can-plugin

  # Modbus 插件 (Python)
  modbus-plugin:
    build:
      context: ./plugins/modbus
      dockerfile: Dockerfile
    environment:
      LOG_LEVEL: debug
      GRPC_PORT: 50051
    ports:
      - "50051:50051"
    volumes:
      - ./plugins/modbus/tests:/tests

  # IEC104 插件 (Go)
  iec104-plugin:
    build:
      context: ./plugins/iec104
      dockerfile: Dockerfile
    environment:
      LOG_LEVEL: debug
      GRPC_PORT: 50052
    ports:
      - "50052:50052"

  # CAN 插件 (Node.js)
  can-plugin:
    build:
      context: ./plugins/can
      dockerfile: Dockerfile
    environment:
      LOG_LEVEL: debug
      GRPC_PORT: 50053
    ports:
      - "50053:50053"

  # 测试工具容器
  test-runner:
    build:
      context: ./tests
      dockerfile: Dockerfile
    volumes:
      - ./tests:/tests
      - ./test-results:/results
    depends_on:
      - comsrv
    command: tail -f /dev/null

  # Prometheus (监控)
  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml

  # Grafana (可视化)
  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin

volumes:
  redis_data:
```

#### 2.2.2 测试工具配置

```dockerfile
# tests/Dockerfile
FROM rust:1.70

# 安装测试工具
RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    python3-pip \
    nodejs npm \
    golang \
    netcat \
    redis-tools \
    curl \
    jq

# 安装 Rust 测试依赖
RUN cargo install cargo-nextest

# 安装 Python 测试依赖
RUN pip3 install pytest pytest-asyncio grpcio grpcio-tools pytest-benchmark

# 安装 Node.js 测试依赖
RUN npm install -g jest @grpc/grpc-js @grpc/proto-loader

# 安装 Go 测试依赖
RUN go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest

WORKDIR /tests
```

### 2.3 模拟器配置

#### 2.3.1 Modbus 设备模拟器

```python
# tests/simulators/modbus_simulator.py
import asyncio
from pymodbus.server import StartAsyncTcpServer
from pymodbus.device import ModbusDeviceIdentification
from pymodbus.datastore import ModbusSequentialDataBlock
from pymodbus.datastore import ModbusSlaveContext, ModbusServerContext
import random
import time

class ModbusSimulator:
    def __init__(self, port=5502):
        self.port = port
        self.context = self._create_context()
        
    def _create_context(self):
        # 创建数据存储
        store = ModbusSlaveContext(
            di=ModbusSequentialDataBlock(0, [0]*100),
            co=ModbusSequentialDataBlock(0, [0]*100),
            hr=ModbusSequentialDataBlock(0, [0]*100),  # 保持寄存器
            ir=ModbusSequentialDataBlock(0, [0]*100),  # 输入寄存器
        )
        context = ModbusServerContext(slaves=store, single=True)
        return context
    
    def update_values(self):
        """定期更新模拟数据"""
        context = self.context[0]
        
        # 更新电压值 (寄存器 0-2)
        voltage_a = 220.0 + random.uniform(-5, 5)
        voltage_b = 220.0 + random.uniform(-5, 5)
        voltage_c = 220.0 + random.uniform(-5, 5)
        
        # 转换为寄存器值 (假设使用 float32)
        import struct
        context.setValues(3, 0, struct.pack('>f', voltage_a))
        context.setValues(3, 2, struct.pack('>f', voltage_b))
        context.setValues(3, 4, struct.pack('>f', voltage_c))
        
        # 更新开关状态 (线圈 0-9)
        for i in range(10):
            context.setValues(1, i, [random.randint(0, 1)])
    
    async def run(self):
        """启动模拟器"""
        # 定期更新任务
        async def updating_task():
            while True:
                self.update_values()
                await asyncio.sleep(1)
        
        # 启动更新任务
        asyncio.create_task(updating_task())
        
        # 启动服务器
        await StartAsyncTcpServer(
            context=self.context,
            address=("0.0.0.0", self.port)
        )

if __name__ == "__main__":
    simulator = ModbusSimulator()
    asyncio.run(simulator.run())
```

#### 2.3.2 IEC104 设备模拟器

```go
// tests/simulators/iec104_simulator.go
package main

import (
    "fmt"
    "log"
    "math/rand"
    "net"
    "time"
)

type IEC104Simulator struct {
    port     int
    listener net.Listener
    clients  []net.Conn
}

func NewIEC104Simulator(port int) *IEC104Simulator {
    return &IEC104Simulator{
        port:    port,
        clients: make([]net.Conn, 0),
    }
}

func (s *IEC104Simulator) Start() error {
    listener, err := net.Listen("tcp", fmt.Sprintf(":%d", s.port))
    if err != nil {
        return err
    }
    s.listener = listener
    
    // 启动数据生成器
    go s.dataGenerator()
    
    // 接受客户端连接
    for {
        conn, err := listener.Accept()
        if err != nil {
            continue
        }
        s.clients = append(s.clients, conn)
        go s.handleClient(conn)
    }
}

func (s *IEC104Simulator) dataGenerator() {
    ticker := time.NewTicker(1 * time.Second)
    defer ticker.Stop()
    
    for range ticker.C {
        // 生成模拟数据
        voltage := 220.0 + rand.Float64()*10 - 5
        current := 10.0 + rand.Float64()*2 - 1
        
        // 构造 IEC104 ASDU
        asdu := s.createMeasurementASDU(voltage, current)
        
        // 发送给所有客户端
        for _, client := range s.clients {
            client.Write(asdu)
        }
    }
}

func (s *IEC104Simulator) createMeasurementASDU(voltage, current float64) []byte {
    // 简化的 ASDU 构造
    // 实际实现需要完整的 IEC104 协议
    return []byte{} // TODO: 实现
}

func (s *IEC104Simulator) handleClient(conn net.Conn) {
    defer conn.Close()
    // 处理客户端请求
    buffer := make([]byte, 1024)
    for {
        n, err := conn.Read(buffer)
        if err != nil {
            break
        }
        // 处理接收到的数据
        s.processRequest(conn, buffer[:n])
    }
}
```

## 3. 单元测试方案

### 3.1 ComSrv Core 单元测试

#### 3.1.1 gRPC Client Adapter 测试

```rust
// tests/unit/grpc_client_adapter_test.rs
use comsrv::core::grpc_client_adapter::GrpcClientAdapter;
use comsrv::core::combase::ComBase;
use mockall::mock;

#[tokio::test]
async fn test_grpc_client_connection() {
    // 测试连接建立
    let adapter = GrpcClientAdapter::new("http://localhost:50051");
    let result = adapter.connect().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_batch_read() {
    let adapter = GrpcClientAdapter::new("http://localhost:50051");
    
    // 准备测试数据
    let point_ids = vec![10001, 10002, 10003];
    let params = HashMap::from([
        ("host", "192.168.1.100"),
        ("port", "502"),
    ]);
    
    // 执行批量读取
    let result = adapter.batch_read(&point_ids, &params).await;
    
    // 验证结果
    assert!(result.is_ok());
    let points = result.unwrap();
    assert_eq!(points.len(), 3);
}

#[tokio::test]
async fn test_encode_command() {
    let adapter = GrpcClientAdapter::new("http://localhost:50051");
    
    // 准备控制命令
    let command = ControlCommand {
        point_id: 30001,
        value: PointValue::Bool(true),
    };
    
    // 编码命令
    let result = adapter.encode_command(&command).await;
    
    // 验证结果
    assert!(result.is_ok());
    let encoded = result.unwrap();
    assert!(!encoded.is_empty());
}

#[tokio::test]
async fn test_connection_retry() {
    // 测试连接重试机制
    let adapter = GrpcClientAdapter::new("http://invalid-host:50051");
    adapter.set_retry_policy(RetryPolicy {
        max_retries: 3,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(1),
    });
    
    let start = Instant::now();
    let result = adapter.connect().await;
    let elapsed = start.elapsed();
    
    // 验证重试行为
    assert!(result.is_err());
    assert!(elapsed >= Duration::from_millis(300)); // 至少重试3次
}
```

#### 3.1.2 Plugin Manager 测试

```rust
// tests/unit/plugin_manager_test.rs
use comsrv::core::plugin_manager::PluginManager;

#[tokio::test]
async fn test_plugin_discovery_static() {
    let config = PluginConfig {
        discovery: DiscoveryMode::Static,
        endpoints: HashMap::from([
            ("modbus", "localhost:50051"),
            ("iec104", "localhost:50052"),
        ]),
    };
    
    let manager = PluginManager::new(config);
    let plugins = manager.discover_plugins().await;
    
    assert_eq!(plugins.len(), 2);
    assert!(plugins.contains_key("modbus"));
    assert!(plugins.contains_key("iec104"));
}

#[tokio::test]
async fn test_health_check() {
    let manager = PluginManager::new(test_config());
    
    // 注册插件
    manager.register_plugin("modbus", "localhost:50051").await;
    
    // 执行健康检查
    let health = manager.check_health("modbus").await;
    
    assert!(health.is_ok());
    assert!(health.unwrap().healthy);
}

#[tokio::test]
async fn test_load_balancing() {
    let manager = PluginManager::new(test_config());
    
    // 注册多个实例
    manager.register_plugin("modbus", "localhost:50051").await;
    manager.register_plugin("modbus", "localhost:50052").await;
    
    // 测试负载均衡
    let endpoints = vec![];
    for _ in 0..10 {
        let endpoint = manager.get_endpoint("modbus").await;
        endpoints.push(endpoint.unwrap());
    }
    
    // 验证请求分布
    let count_50051 = endpoints.iter().filter(|e| e.contains("50051")).count();
    let count_50052 = endpoints.iter().filter(|e| e.contains("50052")).count();
    
    assert!(count_50051 > 0);
    assert!(count_50052 > 0);
}
```

#### 3.1.3 Data Processor 测试

```rust
// tests/unit/data_processor_test.rs
use comsrv::core::data_processor::DataProcessor;

#[test]
fn test_scale_offset_conversion() {
    let processor = DataProcessor::new();
    
    // 测试 scale 和 offset 转换
    let raw_value = 1000.0;
    let scale = 0.1;
    let offset = -50.0;
    
    let converted = processor.apply_scale_offset(raw_value, scale, offset);
    assert_eq!(converted, 50.0); // (1000 * 0.1) - 50
}

#[test]
fn test_data_type_conversion() {
    let processor = DataProcessor::new();
    
    // 测试不同数据类型转换
    let test_cases = vec![
        (DataType::Float32, vec![0x41, 0x48, 0x00, 0x00], 12.5),
        (DataType::Int16, vec![0x00, 0x64], 100.0),
        (DataType::Bool, vec![0x01], 1.0),
    ];
    
    for (data_type, bytes, expected) in test_cases {
        let result = processor.convert_bytes(&bytes, data_type);
        assert_eq!(result, expected);
    }
}

#[test]
fn test_batch_processing() {
    let processor = DataProcessor::new();
    
    // 准备批量数据
    let raw_points = vec![
        RawPoint { id: 10001, value: 1000.0 },
        RawPoint { id: 10002, value: 2000.0 },
        RawPoint { id: 10003, value: 3000.0 },
    ];
    
    // 配置转换参数
    let configs = HashMap::from([
        (10001, ConversionConfig { scale: 0.1, offset: 0.0 }),
        (10002, ConversionConfig { scale: 0.01, offset: -10.0 }),
        (10003, ConversionConfig { scale: 1.0, offset: 100.0 }),
    ]);
    
    // 批量处理
    let processed = processor.batch_process(&raw_points, &configs);
    
    // 验证结果
    assert_eq!(processed[0].value, 100.0);
    assert_eq!(processed[1].value, 10.0);
    assert_eq!(processed[2].value, 3100.0);
}
```

### 3.2 插件单元测试

#### 3.2.1 Python 插件测试

```python
# tests/unit/test_modbus_plugin.py
import pytest
import grpc
from concurrent import futures
import sys
sys.path.append('/plugins/modbus')

from modbus_plugin import ModbusPlugin
import protocol_plugin_pb2
import protocol_plugin_pb2_grpc

@pytest.fixture
def plugin():
    return ModbusPlugin()

@pytest.fixture
def grpc_server():
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=1))
    protocol_plugin_pb2_grpc.add_ProtocolPluginServicer_to_server(
        ModbusPlugin(), server
    )
    port = server.add_insecure_port('[::]:0')
    server.start()
    yield f'localhost:{port}'
    server.stop(grace=0)

def test_get_info(plugin):
    request = protocol_plugin_pb2.Empty()
    response = plugin.GetInfo(request, None)
    
    assert response.name == "modbus-plugin"
    assert response.version == "1.0.0"
    assert response.protocol_type == "modbus_tcp"
    assert "batch_read" in response.supported_features

def test_parse_modbus_data(plugin):
    # 模拟 Modbus 响应数据
    # 功能码03响应：地址(1) + 功能码(1) + 字节数(1) + 数据(n) + CRC(2)
    raw_data = bytes([
        0x01,  # 地址
        0x03,  # 功能码
        0x04,  # 字节数
        0x00, 0x64,  # 寄存器0: 100
        0x00, 0xC8,  # 寄存器1: 200
    ])
    
    request = protocol_plugin_pb2.ParseRequest(
        raw_data=raw_data,
        context={"point_mapping": "10001:0,10002:1"}
    )
    
    response = plugin.ParseData(request, None)
    
    assert len(response.points) == 2
    assert response.points[0].point_id == 10001
    assert response.points[0].int_value == 100
    assert response.points[1].point_id == 10002
    assert response.points[1].int_value == 200

def test_encode_write_single_coil(plugin):
    request = protocol_plugin_pb2.EncodeRequest(
        point_id=30001,
        value=protocol_plugin_pb2.PointData(bool_value=True),
        context={
            "slave_id": "1",
            "coil_address": "100"
        }
    )
    
    response = plugin.EncodeCommand(request, None)
    
    # 验证 Modbus 写单个线圈命令 (功能码 05)
    expected = bytes([
        0x01,  # 从站地址
        0x05,  # 功能码
        0x00, 0x64,  # 线圈地址 (100)
        0xFF, 0x00,  # 值 (ON)
    ])
    
    assert response.encoded_data[:6] == expected

@pytest.mark.asyncio
async def test_batch_read_holding_registers(plugin):
    request = protocol_plugin_pb2.BatchReadRequest(
        connection_params={
            "host": "localhost",
            "port": "5502",
            "slave_id": "1"
        },
        point_ids=[10001, 10002, 10003],
        read_params={
            "10001": "hr:0:float32",
            "10002": "hr:2:float32",
            "10003": "hr:4:int16"
        }
    )
    
    response = plugin.BatchRead(request, None)
    
    assert len(response.points) == 3
    assert all(p.timestamp > 0 for p in response.points)

def test_error_handling(plugin):
    # 测试无效数据处理
    request = protocol_plugin_pb2.ParseRequest(
        raw_data=b"invalid",
        context={}
    )
    
    response = plugin.ParseData(request, None)
    
    assert response.error != ""
    assert "invalid" in response.error.lower()

@pytest.mark.benchmark
def test_parse_performance(benchmark, plugin):
    # 准备大量数据
    raw_data = bytes([0x01, 0x03, 0xF0] + [0x00] * 240)  # 120个寄存器
    request = protocol_plugin_pb2.ParseRequest(
        raw_data=raw_data,
        context={"point_mapping": ",".join(f"{10000+i}:{i}" for i in range(120))}
    )
    
    # 性能测试
    result = benchmark(plugin.ParseData, request, None)
    
    assert len(result.points) == 120
```

#### 3.2.2 Go 插件测试

```go
// tests/unit/iec104_plugin_test.go
package main

import (
    "context"
    "testing"
    pb "comsrv/plugin/v1"
    "github.com/stretchr/testify/assert"
)

func TestGetInfo(t *testing.T) {
    plugin := &IEC104Plugin{}
    info, err := plugin.GetInfo(context.Background(), &pb.Empty{})
    
    assert.NoError(t, err)
    assert.Equal(t, "iec104-plugin", info.Name)
    assert.Equal(t, "iec104", info.ProtocolType)
}

func TestParseIEC104Data(t *testing.T) {
    plugin := &IEC104Plugin{}
    
    // 构造 IEC104 测量值 ASDU
    asdu := []byte{
        0x68, 0x0E,              // 开始标志和长度
        0x00, 0x00, 0x00, 0x00,  // 控制域
        0x09,                    // 类型标识 (M_ME_NA_1)
        0x01,                    // 可变结构限定词
        0x00, 0x00,              // 传送原因
        0x00, 0x00,              // ASDU地址
        0x01, 0x00, 0x00,        // 信息对象地址
        0x00, 0x00, 0xC8, 0x42,  // 测量值 (100.0)
        0x00,                    // 品质描述词
    }
    
    req := &pb.ParseRequest{
        RawData: asdu,
        Context: map[string]string{
            "point_mapping": "10001:1",
        },
    }
    
    resp, err := plugin.ParseData(context.Background(), req)
    
    assert.NoError(t, err)
    assert.Len(t, resp.Points, 1)
    assert.Equal(t, uint32(10001), resp.Points[0].PointId)
    assert.InDelta(t, 100.0, resp.Points[0].GetFloatValue(), 0.01)
}

func TestBatchRead(t *testing.T) {
    plugin := &IEC104Plugin{}
    
    req := &pb.BatchReadRequest{
        ConnectionParams: map[string]string{
            "host": "localhost",
            "port": "2404",
        },
        PointIds: []uint32{10001, 10002, 10003},
    }
    
    resp, err := plugin.BatchRead(context.Background(), req)
    
    // 这里假设有模拟器运行
    if err != nil {
        t.Skip("IEC104 simulator not running")
    }
    
    assert.Len(t, resp.Points, 3)
}

func BenchmarkParseData(b *testing.B) {
    plugin := &IEC104Plugin{}
    
    // 准备测试数据
    asdu := makeTestASDU(100) // 100个测量值
    req := &pb.ParseRequest{
        RawData: asdu,
        Context: makePointMapping(100),
    }
    
    b.ResetTimer()
    for i := 0; i < b.N; i++ {
        _, _ = plugin.ParseData(context.Background(), req)
    }
}
```

#### 3.2.3 Node.js 插件测试

```javascript
// tests/unit/can_plugin.test.js
const { CANPlugin } = require('../../plugins/can/can_plugin');
const { Empty, ParseRequest } = require('../../protos/protocol_plugin_pb');

describe('CAN Plugin', () => {
    let plugin;
    
    beforeEach(() => {
        plugin = new CANPlugin();
    });
    
    test('GetInfo returns correct information', async () => {
        const response = await plugin.GetInfo(new Empty());
        
        expect(response.name).toBe('can-plugin');
        expect(response.protocolType).toBe('can');
        expect(response.supportedFeatures).toContain('batch_read');
    });
    
    test('ParseData handles CAN frame correctly', async () => {
        // CAN 2.0A 标准帧
        const canFrame = Buffer.from([
            0x01, 0x23,     // CAN ID (0x123)
            0x08,           // DLC (数据长度)
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08  // 数据
        ]);
        
        const request = new ParseRequest({
            rawData: canFrame,
            context: {
                'frame_mapping': JSON.stringify({
                    '0x123': {
                        '10001': { byte: 0, type: 'uint8' },
                        '10002': { byte: 1, type: 'uint8' },
                        '10003': { byte: 2, type: 'uint16_be' },
                        '10004': { byte: 4, type: 'float32_be' }
                    }
                })
            }
        });
        
        const response = await plugin.ParseData(request);
        
        expect(response.points).toHaveLength(4);
        expect(response.points[0].pointId).toBe(10001);
        expect(response.points[0].intValue).toBe(1);
        expect(response.points[1].pointId).toBe(10002);
        expect(response.points[1].intValue).toBe(2);
    });
    
    test('EncodeCommand creates valid CAN frame', async () => {
        const request = {
            pointId: 30001,
            value: { intValue: 100 },
            context: {
                canId: '0x200',
                bytePosition: '0',
                dataType: 'uint8'
            }
        };
        
        const response = await plugin.EncodeCommand(request);
        
        const encoded = response.encodedData;
        expect(encoded[0]).toBe(0x02); // CAN ID high byte
        expect(encoded[1]).toBe(0x00); // CAN ID low byte
        expect(encoded[2]).toBe(0x01); // DLC
        expect(encoded[3]).toBe(100);  // 数据
    });
    
    test('Error handling for invalid data', async () => {
        const request = new ParseRequest({
            rawData: Buffer.from([0xFF]), // 无效数据
            context: {}
        });
        
        const response = await plugin.ParseData(request);
        
        expect(response.error).toBeTruthy();
        expect(response.error).toContain('invalid');
    });
});

// 性能测试
describe('CAN Plugin Performance', () => {
    let plugin;
    
    beforeEach(() => {
        plugin = new CANPlugin();
    });
    
    test('Parse 1000 CAN frames', async () => {
        const frames = [];
        for (let i = 0; i < 1000; i++) {
            frames.push(Buffer.from([
                0x01, 0x00 + i % 256,
                0x08,
                0, 1, 2, 3, 4, 5, 6, 7
            ]));
        }
        
        const start = Date.now();
        
        for (const frame of frames) {
            await plugin.ParseData(new ParseRequest({
                rawData: frame,
                context: { frame_mapping: '{}' }
            }));
        }
        
        const elapsed = Date.now() - start;
        console.log(`Parsed 1000 frames in ${elapsed}ms`);
        
        expect(elapsed).toBeLessThan(1000); // 应该在1秒内完成
    });
});
```

## 4. 集成测试方案

### 4.1 ComSrv Core 与插件集成测试

#### 4.1.1 端到端数据流测试

```rust
// tests/integration/e2e_data_flow_test.rs
use testcontainers::{clients, images};

#[tokio::test]
async fn test_modbus_data_flow_e2e() {
    // 启动测试容器
    let docker = clients::Cli::default();
    let redis = docker.run(images::redis::Redis::default());
    let modbus_simulator = docker.run(ModbusSimulatorImage::default());
    let modbus_plugin = docker.run(ModbusPluginImage::default());
    
    // 配置 ComSrv
    let config = ComSrvConfig {
        redis_url: format!("redis://localhost:{}", redis.get_host_port(6379)),
        channels: vec![
            ChannelConfig {
                id: 1001,
                protocol_type: "modbus_tcp",
                plugin_endpoint: format!("localhost:{}", modbus_plugin.get_host_port(50051)),
                enabled: true,
                poll_interval: Duration::from_secs(1),
            }
        ],
    };
    
    // 启动 ComSrv
    let comsrv = ComSrv::new(config).await;
    comsrv.start().await;
    
    // 等待数据流转
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // 验证 Redis 中的数据
    let redis_client = redis::Client::open(redis_url).unwrap();
    let mut conn = redis_client.get_async_connection().await.unwrap();
    
    // 检查测量值
    let value: String = conn.hget("comsrv:1001:m", "10001").await.unwrap();
    let float_value: f64 = value.parse().unwrap();
    
    assert!(float_value > 215.0 && float_value < 225.0); // 电压应该在正常范围
    
    // 检查信号值
    let signal: String = conn.hget("comsrv:1001:s", "20001").await.unwrap();
    assert!(signal == "0" || signal == "1");
}

#[tokio::test]
async fn test_control_command_flow() {
    // 设置测试环境...
    
    // 发送控制命令
    let mut conn = redis_client.get_async_connection().await.unwrap();
    conn.publish("cmd:1001:control", "30001:1").await.unwrap();
    
    // 等待命令执行
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // 验证命令已发送到设备
    let logs = modbus_simulator.logs();
    assert!(logs.contains("Write Single Coil"));
    assert!(logs.contains("Address: 100, Value: ON"));
}
```

#### 4.1.2 多插件协同测试

```rust
// tests/integration/multi_plugin_test.rs
#[tokio::test]
async fn test_multiple_plugins_concurrent() {
    // 启动多个插件
    let plugins = vec![
        ("modbus", 50051),
        ("iec104", 50052),
        ("can", 50053),
    ];
    
    // 配置多个通道
    let channels = vec![
        ChannelConfig {
            id: 1001,
            protocol_type: "modbus_tcp",
            plugin_endpoint: "localhost:50051",
        },
        ChannelConfig {
            id: 2001,
            protocol_type: "iec104",
            plugin_endpoint: "localhost:50052",
        },
        ChannelConfig {
            id: 3001,
            protocol_type: "can",
            plugin_endpoint: "localhost:50053",
        },
    ];
    
    // 启动 ComSrv
    let comsrv = ComSrv::new(config).await;
    comsrv.start().await;
    
    // 并发验证各通道数据
    let tasks = channels.iter().map(|channel| {
        let channel_id = channel.id;
        tokio::spawn(async move {
            // 验证每个通道的数据
            let value: String = conn.hget(format!("comsrv:{}:m", channel_id), "10001").await.unwrap();
            assert!(!value.is_empty());
        })
    });
    
    futures::future::join_all(tasks).await;
}
```

### 4.2 故障恢复测试

```python
# tests/integration/test_fault_recovery.py
import pytest
import docker
import time
import redis

class TestFaultRecovery:
    @pytest.fixture
    def docker_client(self):
        return docker.from_env()
    
    @pytest.fixture
    def redis_client(self):
        return redis.Redis(host='localhost', port=6379)
    
    def test_plugin_crash_recovery(self, docker_client, redis_client):
        """测试插件崩溃后的恢复"""
        # 获取插件容器
        plugin_container = docker_client.containers.get('modbus-plugin')
        
        # 记录崩溃前的数据
        before_crash = redis_client.hget('comsrv:1001:m', '10001')
        
        # 模拟插件崩溃
        plugin_container.kill()
        time.sleep(2)
        
        # 重启插件
        plugin_container.start()
        time.sleep(5)
        
        # 验证数据恢复
        after_recovery = redis_client.hget('comsrv:1001:m', '10001')
        assert after_recovery is not None
        assert float(after_recovery) != float(before_crash)  # 数据已更新
    
    def test_redis_connection_loss(self, docker_client, redis_client):
        """测试 Redis 连接丢失后的恢复"""
        redis_container = docker_client.containers.get('redis')
        
        # 停止 Redis
        redis_container.pause()
        time.sleep(2)
        
        # 检查 ComSrv 日志
        comsrv_container = docker_client.containers.get('comsrv')
        logs = comsrv_container.logs(tail=10).decode()
        assert 'Redis connection lost' in logs
        
        # 恢复 Redis
        redis_container.unpause()
        time.sleep(3)
        
        # 验证连接恢复
        new_logs = comsrv_container.logs(tail=5).decode()
        assert 'Redis connection restored' in new_logs
    
    def test_network_partition(self, docker_client):
        """测试网络分区场景"""
        # 使用 tc (traffic control) 模拟网络延迟和丢包
        plugin_container = docker_client.containers.get('modbus-plugin')
        
        # 添加网络延迟
        plugin_container.exec_run(
            'tc qdisc add dev eth0 root netem delay 500ms loss 10%'
        )
        
        time.sleep(10)
        
        # 检查健康检查状态
        comsrv_container = docker_client.containers.get('comsrv')
        health_status = comsrv_container.exec_run(
            'curl -s http://localhost:8000/health'
        ).output.decode()
        
        assert 'degraded' in health_status
        
        # 恢复网络
        plugin_container.exec_run('tc qdisc del dev eth0 root')
        time.sleep(5)
        
        # 验证恢复
        health_status = comsrv_container.exec_run(
            'curl -s http://localhost:8000/health'
        ).output.decode()
        
        assert 'healthy' in health_status
```

## 5. 性能测试方案

### 5.1 基准性能测试

```rust
// tests/performance/benchmark_test.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_grpc_calls(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("single_point_read", |b| {
        b.to_async(&runtime).iter(|| async {
            let adapter = GrpcClientAdapter::new("localhost:50051");
            let result = adapter.read_single_point(black_box(10001)).await;
            result.unwrap()
        })
    });
    
    c.bench_function("batch_100_points", |b| {
        b.to_async(&runtime).iter(|| async {
            let adapter = GrpcClientAdapter::new("localhost:50051");
            let points: Vec<u32> = (10001..10101).collect();
            let result = adapter.batch_read(black_box(&points), &HashMap::new()).await;
            result.unwrap()
        })
    });
    
    c.bench_function("batch_1000_points", |b| {
        b.to_async(&runtime).iter(|| async {
            let adapter = GrpcClientAdapter::new("localhost:50051");
            let points: Vec<u32> = (10001..11001).collect();
            let result = adapter.batch_read(black_box(&points), &HashMap::new()).await;
            result.unwrap()
        })
    });
}

fn benchmark_redis_operations(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("redis_hash_write_100", |b| {
        b.to_async(&runtime).iter(|| async {
            let mut conn = get_redis_connection().await;
            let data: Vec<(String, String)> = (0..100)
                .map(|i| (format!("{}", 10001 + i), format!("{:.6}", 220.0 + i as f64)))
                .collect();
            
            conn.hset_multiple("comsrv:1001:m", &data).await.unwrap()
        })
    });
}

criterion_group!(benches, benchmark_grpc_calls, benchmark_redis_operations);
criterion_main!(benches);
```

### 5.2 负载测试

```python
# tests/performance/load_test.py
import asyncio
import time
import statistics
from concurrent.futures import ThreadPoolExecutor
import grpc
import protocol_plugin_pb2
import protocol_plugin_pb2_grpc

class LoadTester:
    def __init__(self, plugin_address, num_workers=10):
        self.plugin_address = plugin_address
        self.num_workers = num_workers
        self.results = []
    
    async def single_request(self):
        """执行单个请求并记录延迟"""
        start = time.time()
        
        channel = grpc.insecure_channel(self.plugin_address)
        stub = protocol_plugin_pb2_grpc.ProtocolPluginStub(channel)
        
        request = protocol_plugin_pb2.BatchReadRequest(
            connection_params={"host": "localhost", "port": "5502"},
            point_ids=list(range(10001, 10101))  # 100个点
        )
        
        try:
            response = stub.BatchRead(request)
            latency = (time.time() - start) * 1000  # 毫秒
            self.results.append({
                'latency': latency,
                'success': True,
                'points': len(response.points)
            })
        except Exception as e:
            self.results.append({
                'latency': (time.time() - start) * 1000,
                'success': False,
                'error': str(e)
            })
    
    async def run_load_test(self, duration_seconds=60, requests_per_second=100):
        """运行负载测试"""
        print(f"Starting load test: {requests_per_second} RPS for {duration_seconds}s")
        
        start_time = time.time()
        request_interval = 1.0 / requests_per_second
        
        while time.time() - start_time < duration_seconds:
            # 发起请求
            asyncio.create_task(self.single_request())
            await asyncio.sleep(request_interval)
        
        # 等待所有请求完成
        await asyncio.sleep(5)
        
        # 分析结果
        self.analyze_results()
    
    def analyze_results(self):
        """分析测试结果"""
        successful = [r for r in self.results if r['success']]
        failed = [r for r in self.results if not r['success']]
        
        if successful:
            latencies = [r['latency'] for r in successful]
            print(f"\n测试结果:")
            print(f"总请求数: {len(self.results)}")
            print(f"成功: {len(successful)}")
            print(f"失败: {len(failed)}")
            print(f"成功率: {len(successful)/len(self.results)*100:.2f}%")
            print(f"\n延迟统计 (ms):")
            print(f"最小: {min(latencies):.2f}")
            print(f"最大: {max(latencies):.2f}")
            print(f"平均: {statistics.mean(latencies):.2f}")
            print(f"中位数: {statistics.median(latencies):.2f}")
            print(f"P95: {statistics.quantiles(latencies, n=20)[18]:.2f}")
            print(f"P99: {statistics.quantiles(latencies, n=100)[98]:.2f}")

# 执行测试
async def main():
    tester = LoadTester("localhost:50051")
    
    # 逐步增加负载
    for rps in [10, 50, 100, 200, 500]:
        print(f"\n{'='*50}")
        await tester.run_load_test(duration_seconds=30, requests_per_second=rps)
        await asyncio.sleep(10)  # 冷却期

if __name__ == "__main__":
    asyncio.run(main())
```

### 5.3 压力测试

```go
// tests/performance/stress_test.go
package main

import (
    "context"
    "fmt"
    "sync"
    "sync/atomic"
    "time"
    pb "comsrv/plugin/v1"
    "google.golang.org/grpc"
)

type StressTest struct {
    endpoint        string
    concurrency     int
    duration        time.Duration
    requestCount    int64
    successCount    int64
    errorCount      int64
    totalLatency    int64
}

func (st *StressTest) Run() {
    fmt.Printf("Starting stress test: %d concurrent connections for %v\n", 
        st.concurrency, st.duration)
    
    var wg sync.WaitGroup
    start := time.Now()
    
    // 启动并发 goroutines
    for i := 0; i < st.concurrency; i++ {
        wg.Add(1)
        go st.worker(&wg, start)
    }
    
    // 等待测试完成
    wg.Wait()
    
    // 输出结果
    st.printResults()
}

func (st *StressTest) worker(wg *sync.WaitGroup, start time.Time) {
    defer wg.Done()
    
    // 建立连接
    conn, err := grpc.Dial(st.endpoint, grpc.WithInsecure())
    if err != nil {
        atomic.AddInt64(&st.errorCount, 1)
        return
    }
    defer conn.Close()
    
    client := pb.NewProtocolPluginClient(conn)
    
    // 持续发送请求
    for time.Since(start) < st.duration {
        reqStart := time.Now()
        
        // 创建批量读取请求
        req := &pb.BatchReadRequest{
            ConnectionParams: map[string]string{
                "host": "localhost",
                "port": "2404",
            },
            PointIds: generatePointIds(1000), // 1000个点
        }
        
        _, err := client.BatchRead(context.Background(), req)
        
        latency := time.Since(reqStart)
        atomic.AddInt64(&st.totalLatency, int64(latency))
        atomic.AddInt64(&st.requestCount, 1)
        
        if err != nil {
            atomic.AddInt64(&st.errorCount, 1)
        } else {
            atomic.AddInt64(&st.successCount, 1)
        }
    }
}

func (st *StressTest) printResults() {
    fmt.Println("\n压力测试结果:")
    fmt.Printf("总请求数: %d\n", st.requestCount)
    fmt.Printf("成功: %d\n", st.successCount)
    fmt.Printf("失败: %d\n", st.errorCount)
    fmt.Printf("成功率: %.2f%%\n", float64(st.successCount)/float64(st.requestCount)*100)
    
    avgLatency := time.Duration(st.totalLatency / st.requestCount)
    fmt.Printf("平均延迟: %v\n", avgLatency)
    
    rps := float64(st.requestCount) / st.duration.Seconds()
    fmt.Printf("吞吐量: %.2f RPS\n", rps)
}

func main() {
    tests := []struct {
        name        string
        concurrency int
        duration    time.Duration
    }{
        {"低并发", 10, 30 * time.Second},
        {"中并发", 50, 30 * time.Second},
        {"高并发", 100, 30 * time.Second},
        {"极限并发", 500, 30 * time.Second},
    }
    
    for _, test := range tests {
        fmt.Printf("\n%s\n%s\n", test.name, strings.Repeat("=", 50))
        
        st := &StressTest{
            endpoint:    "localhost:50052",
            concurrency: test.concurrency,
            duration:    test.duration,
        }
        
        st.Run()
        
        // 冷却期
        time.Sleep(10 * time.Second)
    }
}
```

## 6. 安全测试方案

### 6.1 认证授权测试

```python
# tests/security/test_authentication.py
import pytest
import grpc
import ssl

class TestAuthentication:
    def test_tls_connection(self):
        """测试 TLS 加密连接"""
        # 加载证书
        with open('certs/ca.crt', 'rb') as f:
            ca_cert = f.read()
        with open('certs/client.crt', 'rb') as f:
            client_cert = f.read()
        with open('certs/client.key', 'rb') as f:
            client_key = f.read()
        
        # 创建凭证
        credentials = grpc.ssl_channel_credentials(
            root_certificates=ca_cert,
            private_key=client_key,
            certificate_chain=client_cert
        )
        
        # 建立安全连接
        channel = grpc.secure_channel('localhost:50051', credentials)
        stub = protocol_plugin_pb2_grpc.ProtocolPluginStub(channel)
        
        # 测试调用
        response = stub.GetInfo(protocol_plugin_pb2.Empty())
        assert response.name == "modbus-plugin"
    
    def test_invalid_certificate(self):
        """测试无效证书"""
        # 使用错误的证书
        with open('certs/invalid.crt', 'rb') as f:
            invalid_cert = f.read()
        
        credentials = grpc.ssl_channel_credentials(
            root_certificates=invalid_cert
        )
        
        channel = grpc.secure_channel('localhost:50051', credentials)
        stub = protocol_plugin_pb2_grpc.ProtocolPluginStub(channel)
        
        # 应该连接失败
        with pytest.raises(grpc.RpcError) as exc_info:
            stub.GetInfo(protocol_plugin_pb2.Empty())
        
        assert exc_info.value.code() == grpc.StatusCode.UNAVAILABLE
    
    def test_api_key_authentication(self):
        """测试 API Key 认证"""
        metadata = [('x-api-key', 'valid-api-key')]
        
        channel = grpc.insecure_channel('localhost:50051')
        stub = protocol_plugin_pb2_grpc.ProtocolPluginStub(channel)
        
        # 有效 API Key
        response = stub.GetInfo(
            protocol_plugin_pb2.Empty(),
            metadata=metadata
        )
        assert response.name == "modbus-plugin"
        
        # 无效 API Key
        invalid_metadata = [('x-api-key', 'invalid-key')]
        with pytest.raises(grpc.RpcError) as exc_info:
            stub.GetInfo(
                protocol_plugin_pb2.Empty(),
                metadata=invalid_metadata
            )
        assert exc_info.value.code() == grpc.StatusCode.UNAUTHENTICATED
```

### 6.2 输入验证测试

```rust
// tests/security/input_validation_test.rs
#[tokio::test]
async fn test_malformed_data_handling() {
    let adapter = GrpcClientAdapter::new("localhost:50051");
    
    // 测试超大数据包
    let oversized_data = vec![0xFF; 10 * 1024 * 1024]; // 10MB
    let result = adapter.parse_data(&oversized_data).await;
    
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("size limit"));
}

#[tokio::test]
async fn test_injection_attacks() {
    let adapter = GrpcClientAdapter::new("localhost:50051");
    
    // 测试 SQL 注入尝试
    let malicious_context = HashMap::from([
        ("point_mapping", "10001:0; DROP TABLE points;"),
    ]);
    
    let result = adapter.parse_with_context(&[0x01, 0x02], malicious_context).await;
    
    // 应该正常处理，不执行恶意代码
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_resource_exhaustion() {
    let adapter = GrpcClientAdapter::new("localhost:50051");
    
    // 尝试请求大量点位
    let excessive_points: Vec<u32> = (1..1_000_000).collect();
    let result = adapter.batch_read(&excessive_points, &HashMap::new()).await;
    
    // 应该被限制
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("too many points"));
}
```

### 6.3 安全扫描

```bash
#!/bin/bash
# tests/security/security_scan.sh

echo "=== 安全扫描开始 ==="

# 1. 依赖漏洞扫描
echo "1. 扫描依赖漏洞..."
cargo audit
python -m pip_audit
npm audit

# 2. 容器镜像扫描
echo "2. 扫描容器镜像..."
trivy image voltageems/comsrv:latest
trivy image voltageems/modbus-plugin:latest

# 3. gRPC 安全测试
echo "3. gRPC 安全测试..."
grpcurl -plaintext localhost:50051 list
grpcurl -plaintext localhost:50051 describe comsrv.plugin.v1.ProtocolPlugin

# 4. 端口扫描
echo "4. 端口扫描..."
nmap -sV localhost -p 50051-50053

# 5. TLS 配置检查
echo "5. TLS 配置检查..."
testssl.sh localhost:50051

echo "=== 安全扫描完成 ==="
```

## 7. 容错测试方案

### 7.1 插件故障测试

```python
# tests/fault_tolerance/test_plugin_failures.py
import pytest
import docker
import time
import threading

class TestPluginFailures:
    @pytest.fixture
    def chaos_monkey(self):
        """混沌工程工具"""
        class ChaosMonkey:
            def __init__(self):
                self.client = docker.from_env()
            
            def kill_container(self, name):
                container = self.client.containers.get(name)
                container.kill()
            
            def pause_container(self, name):
                container = self.client.containers.get(name)
                container.pause()
            
            def unpause_container(self, name):
                container = self.client.containers.get(name)
                container.unpause()
            
            def limit_resources(self, name, cpu_limit, mem_limit):
                container = self.client.containers.get(name)
                container.update(
                    cpu_quota=cpu_limit,
                    mem_limit=mem_limit
                )
        
        return ChaosMonkey()
    
    def test_plugin_sudden_death(self, chaos_monkey):
        """测试插件突然死亡"""
        # 启动监控线程
        monitoring = []
        stop_monitoring = False
        
        def monitor_data():
            redis_client = redis.Redis()
            while not stop_monitoring:
                try:
                    value = redis_client.hget('comsrv:1001:m', '10001')
                    monitoring.append({
                        'time': time.time(),
                        'value': value,
                        'available': value is not None
                    })
                except:
                    monitoring.append({
                        'time': time.time(),
                        'available': False
                    })
                time.sleep(0.1)
        
        monitor_thread = threading.Thread(target=monitor_data)
        monitor_thread.start()
        
        # 等待稳定
        time.sleep(5)
        
        # 杀死插件
        chaos_monkey.kill_container('modbus-plugin')
        
        # 等待恢复
        time.sleep(10)
        
        # 停止监控
        stop_monitoring = True
        monitor_thread.join()
        
        # 分析结果
        unavailable_count = sum(1 for m in monitoring if not m['available'])
        recovery_time = self._calculate_recovery_time(monitoring)
        
        print(f"不可用次数: {unavailable_count}")
        print(f"恢复时间: {recovery_time:.2f}秒")
        
        # 验证自动恢复
        assert recovery_time < 30  # 应该在30秒内恢复
    
    def test_plugin_memory_leak(self, chaos_monkey):
        """测试插件内存泄漏"""
        # 限制内存
        chaos_monkey.limit_resources('modbus-plugin', 
                                    cpu_limit=50000,  # 50%
                                    mem_limit='100m')  # 100MB
        
        # 发送大量请求导致内存增长
        for i in range(1000):
            # 发送大请求
            self._send_large_request()
            time.sleep(0.01)
        
        # 检查插件状态
        container = docker.from_env().containers.get('modbus-plugin')
        stats = container.stats(stream=False)
        
        memory_usage = stats['memory_stats']['usage']
        memory_limit = stats['memory_stats']['limit']
        
        print(f"内存使用: {memory_usage/1024/1024:.2f}MB")
        print(f"内存限制: {memory_limit/1024/1024:.2f}MB")
        
        # 验证没有OOM
        assert container.status == 'running'
```

### 7.2 网络故障测试

```go
// tests/fault_tolerance/network_fault_test.go
package main

import (
    "testing"
    "os/exec"
    "time"
)

func TestNetworkLatency(t *testing.T) {
    // 添加网络延迟
    cmd := exec.Command("tc", "qdisc", "add", "dev", "eth0", 
                       "root", "netem", "delay", "200ms", "50ms")
    err := cmd.Run()
    if err != nil {
        t.Skip("需要 root 权限运行网络测试")
    }
    
    defer func() {
        // 清理
        exec.Command("tc", "qdisc", "del", "dev", "eth0", "root").Run()
    }()
    
    // 测试在高延迟下的表现
    start := time.Now()
    err = performBatchRead(100)
    elapsed := time.Since(start)
    
    t.Logf("高延迟下批量读取耗时: %v", elapsed)
    
    // 应该能完成，但会变慢
    assert.NoError(t, err)
    assert.Less(t, elapsed, 5*time.Second)
}

func TestPacketLoss(t *testing.T) {
    // 添加丢包
    cmd := exec.Command("tc", "qdisc", "add", "dev", "eth0",
                       "root", "netem", "loss", "10%")
    err := cmd.Run()
    if err != nil {
        t.Skip("需要 root 权限运行网络测试")
    }
    
    defer func() {
        exec.Command("tc", "qdisc", "del", "dev", "eth0", "root").Run()
    }()
    
    // 测试在丢包情况下的重试机制
    successCount := 0
    totalAttempts := 100
    
    for i := 0; i < totalAttempts; i++ {
        err := performSingleRead()
        if err == nil {
            successCount++
        }
    }
    
    successRate := float64(successCount) / float64(totalAttempts)
    t.Logf("10%% 丢包率下的成功率: %.2f%%", successRate*100)
    
    // 即使有10%丢包，成功率应该仍然很高（因为有重试）
    assert.Greater(t, successRate, 0.95)
}
```

### 7.3 级联故障测试

```python
# tests/fault_tolerance/test_cascading_failures.py
class TestCascadingFailures:
    def test_redis_overload_cascade(self):
        """测试 Redis 过载导致的级联故障"""
        # 1. 模拟 Redis 慢查询
        redis_client = redis.Redis()
        
        # 创建大量数据导致慢查询
        for i in range(100000):
            redis_client.hset(f"test:large:{i}", "field", "x" * 1000)
        
        # 2. 监控各组件健康状态
        health_timeline = []
        
        def monitor_health():
            components = ['comsrv', 'modbus-plugin', 'redis']
            while True:
                health = {}
                for component in components:
                    health[component] = check_component_health(component)
                health_timeline.append({
                    'time': time.time(),
                    'health': health
                })
                time.sleep(1)
        
        # 3. 执行大量并发请求
        with ThreadPoolExecutor(max_workers=50) as executor:
            futures = []
            for _ in range(1000):
                future = executor.submit(perform_heavy_operation)
                futures.append(future)
        
        # 4. 分析级联故障模式
        analyze_cascade_pattern(health_timeline)
    
    def test_plugin_chain_failure(self):
        """测试插件链式故障"""
        # 配置依赖链: Plugin A -> Plugin B -> Plugin C
        configure_plugin_dependencies()
        
        # 故障注入: Plugin B 变慢
        inject_slowness('plugin-b', delay=5000)  # 5秒延迟
        
        # 监控整个链路
        results = monitor_chain_health(duration=60)
        
        # 验证故障传播
        assert results['plugin-a']['timeout_errors'] > 0
        assert results['plugin-c']['idle_time'] > 30  # Plugin C 空闲
```

## 8. 端到端测试场景

### 8.1 完整业务流程测试

```python
# tests/e2e/test_complete_scenarios.py
class TestE2EScenarios:
    def test_power_monitoring_scenario(self):
        """电力监控完整场景测试"""
        # 1. 启动所有组件
        docker_compose_up()
        
        # 2. 配置通道和点表
        configure_channel(1001, "modbus_tcp", {
            "host": "192.168.1.100",
            "port": "502"
        })
        
        configure_points(1001, [
            {"id": 10001, "name": "voltage_a", "type": "measurement"},
            {"id": 10002, "name": "current_a", "type": "measurement"},
            {"id": 20001, "name": "breaker_status", "type": "signal"},
            {"id": 30001, "name": "breaker_control", "type": "control"},
        ])
        
        # 3. 验证数据采集
        time.sleep(5)
        data = get_redis_data("comsrv:1001:m")
        assert "10001" in data
        assert float(data["10001"]) > 200  # 电压正常
        
        # 4. 测试告警
        inject_abnormal_voltage(250)  # 过压
        time.sleep(2)
        
        alarms = get_active_alarms()
        assert any(a['type'] == 'OVER_VOLTAGE' for a in alarms)
        
        # 5. 测试控制
        send_control_command(30001, False)  # 断开断路器
        time.sleep(1)
        
        status = get_redis_data("comsrv:1001:s")
        assert status["20001"] == "0"  # 断路器已断开
        
        # 6. 验证历史数据
        history = query_influxdb(
            "SELECT * FROM voltage WHERE time > now() - 1m"
        )
        assert len(history) > 0
        
        # 7. 测试故障恢复
        kill_plugin("modbus-plugin")
        time.sleep(10)
        
        # 验证自动恢复
        new_data = get_redis_data("comsrv:1001:m")
        assert "10001" in new_data
```

### 8.2 多协议协同测试

```rust
// tests/e2e/multi_protocol_test.rs
#[tokio::test]
async fn test_multi_protocol_coordination() {
    // 场景：变电站综合自动化
    // Modbus: 电力仪表
    // IEC104: 保护装置
    // CAN: 环境监测
    
    // 1. 配置多个通道
    let channels = vec![
        Channel {
            id: 1001,
            protocol: "modbus_tcp",
            name: "电力仪表",
        },
        Channel {
            id: 2001,
            protocol: "iec104",
            name: "保护装置",
        },
        Channel {
            id: 3001,
            protocol: "can",
            name: "环境监测",
        },
    ];
    
    // 2. 启动数据采集
    let comsrv = start_comsrv_with_channels(channels).await;
    
    // 3. 模拟联动场景
    // 温度过高 -> 降低负载 -> 确认执行
    
    // 注入高温数据 (CAN)
    inject_can_data(3001, 40001, 45.0);  // 45°C
    
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // 验证告警生成
    let alarms = get_alarms().await;
    assert!(alarms.iter().any(|a| a.type == "HIGH_TEMPERATURE"));
    
    // 发送降载命令 (Modbus)
    send_modbus_command(1001, 30001, 0.8);  // 降至80%负载
    
    // 验证保护装置确认 (IEC104)
    let protection_status = get_iec104_status(2001, 20001).await;
    assert_eq!(protection_status, "LOAD_REDUCED");
    
    // 4. 验证数据一致性
    let modbus_load = get_modbus_value(1001, 10003).await;
    let iec104_load = get_iec104_value(2001, 10003).await;
    
    assert!((modbus_load - iec104_load).abs() < 0.1);
}
```

### 8.3 灾难恢复测试

```python
# tests/e2e/test_disaster_recovery.py
class TestDisasterRecovery:
    def test_complete_system_recovery(self):
        """完整系统恢复测试"""
        # 1. 备份当前状态
        backup_id = create_system_backup()
        
        # 2. 记录关键指标
        metrics_before = collect_system_metrics()
        
        # 3. 模拟灾难：删除所有容器
        docker_compose_down()
        
        # 验证系统不可用
        assert not is_system_available()
        
        # 4. 执行恢复流程
        restore_from_backup(backup_id)
        docker_compose_up()
        
        # 5. 等待系统稳定
        wait_for_system_ready(timeout=300)
        
        # 6. 验证数据完整性
        metrics_after = collect_system_metrics()
        
        # 配置应该完全恢复
        assert metrics_after['channel_count'] == metrics_before['channel_count']
        assert metrics_after['point_count'] == metrics_before['point_count']
        
        # 实时数据应该恢复
        assert metrics_after['active_connections'] > 0
        
        # 7. 验证功能恢复
        test_result = run_functional_tests()
        assert test_result['passed'] == test_result['total']
```

## 9. 测试数据准备

### 9.1 测试数据生成器

```python
# tests/data/data_generator.py
import random
import struct
import time

class TestDataGenerator:
    @staticmethod
    def generate_modbus_register_data(register_count, data_type='float32'):
        """生成 Modbus 寄存器数据"""
        data = bytearray()
        
        for i in range(register_count):
            if data_type == 'float32':
                # 生成合理的电力数据
                if i % 3 == 0:  # 电压
                    value = 220.0 + random.uniform(-5, 5)
                elif i % 3 == 1:  # 电流
                    value = 10.0 + random.uniform(-2, 2)
                else:  # 功率
                    value = 2.0 + random.uniform(-0.5, 0.5)
                
                data.extend(struct.pack('>f', value))
            elif data_type == 'int16':
                value = random.randint(0, 65535)
                data.extend(struct.pack('>H', value))
        
        return bytes(data)
    
    @staticmethod
    def generate_iec104_asdu(asdu_type, point_count):
        """生成 IEC104 ASDU"""
        asdu = bytearray()
        
        # ASDU 头部
        asdu.append(asdu_type)  # 类型标识
        asdu.append(point_count)  # 可变结构限定词
        asdu.extend([0x03, 0x00])  # 传送原因
        asdu.extend([0x01, 0x00])  # ASDU地址
        
        # 信息对象
        for i in range(point_count):
            # 信息对象地址
            asdu.extend(struct.pack('<I', 10001 + i)[:3])
            
            if asdu_type == 0x09:  # 测量值
                value = 220.0 + random.uniform(-10, 10)
                asdu.extend(struct.pack('<f', value))
                asdu.append(0x00)  # 品质描述词
            elif asdu_type == 0x01:  # 单点信息
                asdu.append(0x01 if random.random() > 0.5 else 0x00)
        
        return bytes(asdu)
    
    @staticmethod
    def generate_can_frame(can_id, data_pattern='random'):
        """生成 CAN 帧数据"""
        frame = bytearray()
        
        # CAN ID
        frame.extend(struct.pack('>H', can_id))
        
        # 数据长度
        dlc = 8
        frame.append(dlc)
        
        # 数据
        if data_pattern == 'random':
            frame.extend([random.randint(0, 255) for _ in range(dlc)])
        elif data_pattern == 'counter':
            frame.extend(range(dlc))
        elif data_pattern == 'sensor':
            # 模拟传感器数据
            temp = int((25.0 + random.uniform(-5, 5)) * 10)
            humidity = int((60.0 + random.uniform(-10, 10)) * 10)
            frame.extend(struct.pack('>HH', temp, humidity))
            frame.extend([0] * 4)
        
        return bytes(frame)
```

### 9.2 测试配置生成

```yaml
# tests/data/test_configs/stress_test_config.yml
test_scenarios:
  - name: "高频采集测试"
    channels:
      - id: 1001
        protocol: "modbus_tcp"
        poll_interval: 100ms
        points: 1000
        simulators:
          - address: "localhost:5502"
            device_count: 10
            register_count: 100
      
  - name: "大规模点位测试"
    channels:
      - id: 2001
        protocol: "iec104"
        poll_interval: 1s
        points: 10000
        simulators:
          - address: "localhost:2404"
            asdu_count: 100
            points_per_asdu: 100
      
  - name: "混合协议测试"
    channels:
      - id: 3001
        protocol: "modbus_tcp"
        poll_interval: 500ms
        points: 500
      - id: 3002
        protocol: "iec104"
        poll_interval: 1s
        points: 500
      - id: 3003
        protocol: "can"
        poll_interval: 200ms
        points: 200
```

### 9.3 测试数据验证器

```python
# tests/data/data_validator.py
class DataValidator:
    @staticmethod
    def validate_measurement_data(data, expected_range):
        """验证测量数据"""
        errors = []
        
        for point_id, value in data.items():
            try:
                float_value = float(value)
                
                # 检查范围
                if not (expected_range['min'] <= float_value <= expected_range['max']):
                    errors.append(f"Point {point_id}: value {float_value} out of range")
                
                # 检查精度
                decimal_places = len(value.split('.')[-1])
                if decimal_places != 6:
                    errors.append(f"Point {point_id}: incorrect precision {decimal_places}")
                
            except ValueError:
                errors.append(f"Point {point_id}: invalid float value {value}")
        
        return errors
    
    @staticmethod
    def validate_signal_data(data):
        """验证信号数据"""
        errors = []
        
        for point_id, value in data.items():
            if value not in ["0", "1"]:
                errors.append(f"Point {point_id}: invalid signal value {value}")
        
        return errors
    
    @staticmethod
    def validate_data_continuity(time_series_data, max_gap_seconds=5):
        """验证数据连续性"""
        gaps = []
        
        for i in range(1, len(time_series_data)):
            time_diff = time_series_data[i]['timestamp'] - time_series_data[i-1]['timestamp']
            if time_diff > max_gap_seconds:
                gaps.append({
                    'start': time_series_data[i-1]['timestamp'],
                    'end': time_series_data[i]['timestamp'],
                    'duration': time_diff
                })
        
        return gaps
```

## 10. 测试工具和框架选择

### 10.1 单元测试框架

| 语言 | 框架 | 用途 |
|------|------|------|
| Rust | `cargo test` + `mockall` | ComSrv Core 单元测试 |
| Python | `pytest` + `pytest-asyncio` | Python 插件测试 |
| Go | `go test` + `testify` | Go 插件测试 |
| Node.js | `jest` | Node.js 插件测试 |

### 10.2 集成测试工具

| 工具 | 用途 |
|------|------|
| Docker Compose | 环境编排和管理 |
| Testcontainers | 动态容器管理 |
| grpcurl | gRPC 接口测试 |
| Redis CLI | Redis 数据验证 |

### 10.3 性能测试工具

| 工具 | 用途 |
|------|------|
| Criterion (Rust) | 微基准测试 |
| Locust | 负载测试 |
| Grafana K6 | 场景化性能测试 |
| pprof | 性能分析 |

### 10.4 监控和可观测性

| 工具 | 用途 |
|------|------|
| Prometheus | 指标收集 |
| Grafana | 可视化 |
| Jaeger | 分布式追踪 |
| ELK Stack | 日志分析 |

### 10.5 CI/CD 集成

```yaml
# .github/workflows/test.yml
name: gRPC Plugin Tests

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Run Rust Tests
        run: |
          cargo test --all
          cargo test --doc
      
      - name: Run Python Tests
        run: |
          cd plugins/modbus
          pip install -r requirements-test.txt
          pytest tests/ --cov=. --cov-report=xml
      
      - name: Run Go Tests
        run: |
          cd plugins/iec104
          go test -v ./... -coverprofile=coverage.out
      
      - name: Run Node.js Tests
        run: |
          cd plugins/can
          npm install
          npm test -- --coverage

  integration-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Start Test Environment
        run: |
          docker-compose -f docker-compose.test.yml up -d
          ./scripts/wait-for-ready.sh
      
      - name: Run Integration Tests
        run: |
          docker-compose -f docker-compose.test.yml exec test-runner \
            cargo test --test integration_tests
      
      - name: Collect Logs
        if: failure()
        run: |
          docker-compose -f docker-compose.test.yml logs > logs.txt
          
      - name: Upload Logs
        if: failure()
        uses: actions/upload-artifact@v3
        with:
          name: test-logs
          path: logs.txt

  performance-tests:
    runs-on: ubuntu-latest
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v3
      
      - name: Run Performance Tests
        run: |
          docker-compose -f docker-compose.perf.yml up -d
          ./scripts/run-perf-tests.sh
      
      - name: Upload Results
        uses: actions/upload-artifact@v3
        with:
          name: performance-results
          path: perf-results/
```

## 11. 测试执行计划

### 11.1 测试阶段

1. **开发阶段**
   - 单元测试（每次提交）
   - 本地集成测试

2. **持续集成**
   - 自动化测试套件
   - 代码覆盖率检查

3. **预发布测试**
   - 完整集成测试
   - 性能基准测试
   - 安全扫描

4. **发布前测试**
   - 端到端场景测试
   - 压力测试
   - 兼容性测试

### 11.2 测试报告

测试完成后生成详细报告，包括：
- 测试覆盖率统计
- 性能指标对比
- 发现的问题和风险
- 改进建议

## 12. 常见问题和解决方案

### 12.1 测试环境问题

**问题**: Docker 容器无法通信
**解决**: 确保所有容器在同一网络中

```bash
docker network create test-network
docker-compose -f docker-compose.test.yml up --force-recreate
```

**问题**: 端口冲突
**解决**: 使用动态端口分配或修改配置

### 12.2 测试数据问题

**问题**: 测试数据不稳定
**解决**: 使用固定种子的随机数生成器

```python
random.seed(42)  # 固定随机种子
```

### 12.3 性能测试问题

**问题**: 性能测试结果波动大
**解决**: 
- 使用专用测试环境
- 多次运行取平均值
- 关闭其他应用程序

## 总结

本测试计划提供了全面的测试策略和实施方案，覆盖了 gRPC 插件架构的各个方面。通过系统化的测试，可以确保：

1. **功能正确性**：所有接口按预期工作
2. **性能达标**：满足吞吐量和延迟要求
3. **高可用性**：故障恢复机制有效
4. **安全可靠**：防范各类安全威胁
5. **易于维护**：问题可快速定位和修复

测试团队应根据实际情况调整测试策略，持续改进测试流程，确保系统质量。