# Modbus 按位解析测试指南

## 概述

本指南说明如何测试COMSRV的Modbus按位解析功能。该功能允许从16位Modbus寄存器中提取单个位的值，特别适用于处理数字量信号（遥信YX）和控制点（遥控YK）。

## 功能说明

当配置文件中指定了`data_format="bool"`和`bit_position`时，系统会从读取的Modbus寄存器中提取指定位置的位值：
- bit_position范围：0-15（0是最低位LSB，15是最高位MSB）
- 返回值：0或1（整数类型）

## 测试环境准备

### 1. 启动Docker测试环境

```bash
cd services/comsrv
docker-compose -f docker-compose.test.yml up -d
```

### 2. 验证服务状态

```bash
# 检查所有容器是否正常运行
docker-compose -f docker-compose.test.yml ps

# 查看COMSRV日志
docker-compose -f docker-compose.test.yml logs -f comsrv
```

## 测试步骤

### 步骤1：运行按位解析测试脚本

```bash
# 设置环境变量
export MODBUS_HOST=localhost
export REDIS_URL=redis://localhost:6379

# 运行测试脚本
python scripts/test_bitwise_parsing.py
```

该脚本会：
1. 在Modbus模拟器中设置特定的位模式
2. 等待COMSRV读取数据
3. 验证Redis中的位值是否正确提取

### 步骤2：运行集成测试

```bash
# 在容器中运行集成测试
docker-compose -f docker-compose.test.yml exec test-runner pytest /app/tests/docker/integration_test.py::TestModbusBitwiseParsing -v
```

### 步骤3：验证Redis数据

```bash
# 运行验证脚本
./scripts/verify_bitwise_data.sh

# 或手动检查Redis
redis-cli
> GET 1001:s:1  # 查看点位1的位值
> GET 1001:s:2  # 查看点位2的位值
```

## 测试数据说明

模拟器设置了以下测试寄存器：

| 寄存器地址 | 值 (hex) | 二进制表示 | 说明 |
|-----------|----------|-----------|------|
| 1 | 0xA5 | 10100101 | 用于测试位0-7 |
| 2 | 0x5A | 01011010 | 不同的位模式 |
| 3 | 0xF00F | 1111000000001111 | 测试高位和低位 |
| 4 | 0x8001 | 1000000000000001 | 测试最高位和最低位 |
| 5 | 动态 | 变化 | 每秒变化的位模式 |

## 配置文件位置

- 信号定义：`config/test-points/signal_bitwise.csv`
- Modbus映射：`config/test-points/mappings/modbus_signal_bitwise.csv`
- Docker配置：`config/docker-test.yml`

## 预期结果

基于配置文件和模拟器设置，预期的位值为：

| 点位ID | 寄存器 | 位位置 | 预期值 |
|--------|--------|--------|--------|
| 1 | 1 | 0 | 1 |
| 2 | 1 | 1 | 0 |
| 3 | 1 | 2 | 1 |
| 4 | 1 | 3 | 0 |
| 5 | 2 | 0 | 0 |

## 故障排查

### 如果测试失败

1. **检查Modbus连接**
   ```bash
   docker-compose -f docker-compose.test.yml logs modbus-simulator
   ```

2. **检查COMSRV日志**
   ```bash
   docker-compose -f docker-compose.test.yml logs comsrv | grep -i "bit"
   ```

3. **验证配置加载**
   - 确认使用了正确的配置文件
   - 检查`bit_position`字段是否正确加载

4. **手动测试Modbus读取**
   ```bash
   # 进入test-runner容器
   docker-compose -f docker-compose.test.yml exec test-runner bash
   
   # 使用Python测试Modbus连接
   python
   >>> from pymodbus.client import ModbusTcpClient
   >>> client = ModbusTcpClient('modbus-simulator', 502)
   >>> client.connect()
   >>> result = client.read_holding_registers(1, 1, slave=1)
   >>> hex(result.registers[0])  # 应该显示'0xa5'
   ```

## 清理测试环境

```bash
# 停止并清理容器
docker-compose -f docker-compose.test.yml down

# 清理数据卷（如需要）
docker-compose -f docker-compose.test.yml down -v
```

## 性能考虑

- 按位解析的性能开销很小，位操作是CPU原生支持的
- 批量读取仍然有效，多个位可以从同一次Modbus读取中提取
- 建议将相关的位信号放在相邻的寄存器中以优化读取效率