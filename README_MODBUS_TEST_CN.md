# Modbus 通信测试指南

本文档说明如何测试 VoltageEMS 的 Modbus TCP 通信功能。

## 测试工具

1. **modbus_server_simulator.py** - Modbus TCP 服务器模拟器
2. **test_modbus_client.py** - Modbus TCP 测试客户端
3. **test_modbus_integration.sh** - 集成测试脚本

## 前置要求

```bash
# 安装 Python Modbus 库
pip3 install pymodbus
```

## 快速开始

### 方法 1：使用集成测试脚本（推荐）

```bash
# 运行完整的集成测试
./test_modbus_integration.sh
```

这个脚本会：
- 启动 Modbus 服务器模拟器
- 运行所有客户端测试
- 可选运行连续测试
- 可选测试 comsrv API（如果已编译）

### 方法 2：手动测试

#### 1. 启动 Modbus 服务器模拟器

```bash
# 在默认端口 502 启动（需要 sudo）
sudo python3 modbus_server_simulator.py

# 或在自定义端口启动（无需 sudo）
python3 modbus_server_simulator.py --port 5502
```

服务器会模拟以下数据点：
- **遥测点**（保持寄存器）：
  - 地址 0-1: 电压 (Float32)
  - 地址 2-3: 电流 (Float32)
  - 地址 4: 状态 (UInt16)
  - 地址 10-11: 功率 (Float32)
  - 地址 20-21: 功率因数 (Float32)
  - 地址 30-31: 频率 (Float32)
  - 地址 40-41: 电能 (Float32)

- **遥信点**（线圈）：
  - 地址 0-7: 数字输入
  - 地址 10: 报警状态
  - 地址 20-23: 控制状态

- **遥调点**（保持寄存器 100+）：
  - 地址 100-101: 电压设定值 (Float32)
  - 地址 102-103: 电流限值 (Float32)
  - 地址 104: 控制模式 (UInt16)

#### 2. 运行测试客户端

在另一个终端运行：

```bash
# 运行所有测试
python3 test_modbus_client.py --port 5502

# 运行连续测试（60秒）
python3 test_modbus_client.py --port 5502 --continuous 60

# 连接到不同的主机
python3 test_modbus_client.py --host 192.168.1.100 --port 502
```

## 测试功能

测试客户端会执行以下操作：

1. **读保持寄存器**（功能码 03）
   - 读取遥测数据（电压、电流、功率等）
   - 解码 Float32 值

2. **读输入寄存器**（功能码 04）
   - 读取只读的模拟输入

3. **读线圈**（功能码 01）
   - 读取数字输入和状态

4. **写单个寄存器**（功能码 06）
   - 写入设定值
   - 验证写入结果

5. **写单个线圈**（功能码 05）
   - 控制开关状态
   - 验证控制结果

6. **写多个寄存器**（功能码 16）
   - 写入 Float32 值
   - 验证多寄存器写入

## 与 comsrv 集成测试

### 1. 编译 comsrv（如果尚未编译）

```bash
cd services/comsrv
cargo build
```

### 2. 配置 comsrv

确保 `config/comsrv.yaml` 中的 Modbus 配置指向测试服务器：

```yaml
channels:
  - id: 1
    name: "Modbus TCP Test"
    protocol: "modbus_tcp"
    parameters:
      host: "127.0.0.1"
      port: 5502  # 使用测试端口
      slave_id: 1
      timeout: 5000
```

### 3. 启动 comsrv

```bash
RUST_LOG=info ./target/debug/comsrv
```

### 4. 测试 API 端点

```bash
# 获取服务状态
curl http://localhost:3000/api/status | jq

# 获取健康状态
curl http://localhost:3000/api/health | jq

# 获取所有通道
curl http://localhost:3000/api/channels | jq

# 获取通道状态
curl http://localhost:3000/api/channels/1/status | jq

# 读取点位值
curl http://localhost:3000/api/channels/1/points/telemetry/voltage | jq

# 写入点位值
curl -X POST http://localhost:3000/api/channels/1/points/adjustment/voltage_setpoint \
  -H "Content-Type: application/json" \
  -d '{"value": 225.0}' | jq
```

## 故障排除

### 权限问题
如果在端口 502 上启动服务器时遇到权限错误：
```bash
# 使用 sudo
sudo python3 modbus_server_simulator.py

# 或使用高端口号（>1024）
python3 modbus_server_simulator.py --port 5502
```

### 连接被拒绝
- 检查服务器是否正在运行
- 检查防火墙设置
- 确认主机和端口配置正确

### pymodbus 未安装
```bash
pip3 install pymodbus
# 或
pip install pymodbus
```

## 扩展测试

### 压力测试
```python
# 修改 test_modbus_client.py 中的连续测试参数
client.run_continuous_test(duration=3600)  # 运行 1 小时
```

### 性能测试
监控以下指标：
- 响应时间
- 吞吐量
- 错误率
- CPU/内存使用

### 兼容性测试
测试与真实 Modbus 设备的通信：
1. 修改连接参数指向真实设备
2. 调整寄存器地址匹配设备配置
3. 运行测试验证通信

## 日志分析

服务器和客户端都会输出详细日志：
- 连接状态
- 读写操作
- 数据值
- 错误信息

可以将日志重定向到文件进行分析：
```bash
python3 modbus_server_simulator.py 2>&1 | tee server.log
python3 test_modbus_client.py 2>&1 | tee client.log
```