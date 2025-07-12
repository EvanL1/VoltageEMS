# VoltageEMS 系统集成测试计划

## 测试目标

验证扁平化存储架构下的完整数据流，确保各服务之间的数据交互正确无误。

## 测试环境

- Redis: 本地实例或 Docker 容器
- 各服务: comsrv, modsrv, hissrv, alarmsrv, netsrv
- 模拟器: Modbus TCP 设备模拟器
- 监控: Prometheus + Grafana

## 测试场景

### 1. 端到端数据流测试

#### 测试步骤
1. 启动 Redis 和所有服务
2. 启动 Modbus 模拟器（端口 502）
3. 配置 comsrv 连接到模拟器
4. 验证数据流：设备 → comsrv → Redis → modsrv/hissrv

#### 验证点
- comsrv 正确写入扁平化键值
- modsrv 能够读取并计算
- hissrv 正确归档到 InfluxDB

#### 测试脚本
```bash
#!/bin/bash
# test_data_flow.sh

echo "=== VoltageEMS 数据流测试 ==="

# 1. 检查 Redis 连接
redis-cli ping || { echo "Redis 未运行"; exit 1; }

# 2. 清理测试数据
redis-cli --scan --pattern "1001:*" | xargs -r redis-cli del

# 3. 启动服务（假设使用 systemd 或 docker-compose）
echo "启动所有服务..."
docker-compose up -d

# 4. 等待服务就绪
sleep 10

# 5. 写入测试数据
echo "写入测试数据..."
redis-cli set "1001:m:10001" "25.6:$(date +%s)000"
redis-cli set "1001:m:10002" "380.5:$(date +%s)000"
redis-cli set "1001:s:20001" "1:$(date +%s)000"

# 6. 验证数据
echo "验证数据..."
VALUE=$(redis-cli get "1001:m:10001")
if [[ $VALUE == *"25.6"* ]]; then
    echo "✓ 温度数据写入成功"
else
    echo "✗ 温度数据写入失败"
fi

# 7. 检查 modsrv 计算结果
sleep 5
CALC_RESULT=$(redis-cli get "2001:m:30001")
if [[ -n $CALC_RESULT ]]; then
    echo "✓ modsrv 计算成功"
else
    echo "✗ modsrv 计算失败"
fi

# 8. 检查 hissrv 归档
# 这里需要查询 InfluxDB
```

### 2. 性能测试

#### 批量写入测试
```python
#!/usr/bin/env python3
# performance_test.py

import redis
import time
import random
from concurrent.futures import ThreadPoolExecutor

r = redis.Redis(host='localhost', port=6379, decode_responses=True)

def write_points(channel_id, start_id, count):
    """批量写入测试数据"""
    pipe = r.pipeline()
    timestamp = int(time.time() * 1000)
    
    for i in range(count):
        point_id = start_id + i
        value = round(random.uniform(0, 100), 2)
        key = f"{channel_id}:m:{point_id}"
        pipe.set(key, f"{value}:{timestamp}")
    
    start = time.time()
    pipe.execute()
    elapsed = time.time() - start
    
    print(f"Channel {channel_id}: 写入 {count} 个点，耗时 {elapsed:.3f}秒")
    return elapsed

def main():
    print("=== 性能测试 ===")
    
    # 单通道测试
    print("\n1. 单通道批量写入")
    write_points(1001, 10001, 1000)
    
    # 多通道并发测试
    print("\n2. 多通道并发写入")
    with ThreadPoolExecutor(max_workers=10) as executor:
        futures = []
        for i in range(10):
            channel_id = 2001 + i
            future = executor.submit(write_points, channel_id, 10001, 1000)
            futures.append(future)
        
        total_time = sum(f.result() for f in futures)
        print(f"总耗时: {total_time:.3f}秒")
    
    # 查询测试
    print("\n3. 批量查询测试")
    keys = [f"1001:m:{10001+i}" for i in range(100)]
    
    start = time.time()
    values = r.mget(keys)
    elapsed = time.time() - start
    
    valid_count = sum(1 for v in values if v is not None)
    print(f"查询 100 个点，耗时 {elapsed:.3f}秒，有效数据 {valid_count} 个")

if __name__ == "__main__":
    main()
```

### 3. 故障恢复测试

#### Redis 重启测试
1. 正常运行时记录数据
2. 停止 Redis
3. 验证服务降级行为
4. 重启 Redis
5. 验证数据恢复

#### 网络分区测试
1. 使用 tc 命令模拟网络延迟
2. 验证超时和重试机制
3. 恢复网络
4. 验证数据一致性

### 4. 数据一致性测试

```python
#!/usr/bin/env python3
# consistency_test.py

import redis
import json
import time

def verify_data_consistency():
    """验证数据一致性"""
    r = redis.Redis(host='localhost', port=6379, decode_responses=True)
    
    # 测试数据
    test_data = {
        "1001:m:10001": ("25.6", "温度传感器1"),
        "1001:m:10002": ("380.5", "电压"),
        "1001:s:20001": ("1", "主开关"),
    }
    
    # 写入数据和配置
    timestamp = int(time.time() * 1000)
    for key, (value, name) in test_data.items():
        # 写入实时数据
        r.set(key, f"{value}:{timestamp}")
        
        # 写入配置
        parts = key.split(':')
        config_key = f"cfg:{parts[0]}:{parts[1]}:{parts[2]}"
        config = {
            "name": name,
            "unit": "°C" if "温度" in name else "V" if "电压" in name else "",
            "scale": 1.0,
            "offset": 0.0,
            "address": "1:3:100"
        }
        r.set(config_key, json.dumps(config))
    
    # 验证数据
    print("=== 数据一致性验证 ===")
    errors = 0
    
    for key, (expected_value, name) in test_data.items():
        # 验证实时数据
        data = r.get(key)
        if data:
            value, ts = data.split(':')
            if value == expected_value:
                print(f"✓ {key}: {value} - 正确")
            else:
                print(f"✗ {key}: 期望 {expected_value}, 实际 {value}")
                errors += 1
        else:
            print(f"✗ {key}: 数据缺失")
            errors += 1
        
        # 验证配置
        parts = key.split(':')
        config_key = f"cfg:{parts[0]}:{parts[1]}:{parts[2]}"
        config_data = r.get(config_key)
        if config_data:
            config = json.loads(config_data)
            if config['name'] == name:
                print(f"✓ {config_key}: 配置正确")
            else:
                print(f"✗ {config_key}: 配置错误")
                errors += 1
    
    return errors == 0

if __name__ == "__main__":
    if verify_data_consistency():
        print("\n所有数据一致性测试通过！")
    else:
        print("\n数据一致性测试失败！")
        exit(1)
```

### 5. 监控验证

#### Prometheus 指标验证
```bash
# 检查 comsrv 指标
curl -s http://localhost:9100/metrics | grep comsrv_points_written_total

# 检查 Redis 连接池
curl -s http://localhost:9100/metrics | grep redis_connection_pool_size
```

#### Grafana 仪表板
- 实时数据流量
- 延迟分布
- 错误率
- 系统资源使用

## 测试报告模板

```markdown
# VoltageEMS 系统测试报告

**测试日期**: 2025-07-11
**测试版本**: v1.0.0
**测试环境**: 开发环境

## 测试结果汇总

| 测试项 | 状态 | 说明 |
|--------|------|------|
| 数据流测试 | ✓ 通过 | 所有服务正常通信 |
| 性能测试 | ✓ 通过 | 满足设计指标 |
| 故障恢复 | ✓ 通过 | 正确降级和恢复 |
| 数据一致性 | ✓ 通过 | 数据完整无误 |

## 性能指标

- 单点写入延迟: P99 < 1ms
- 批量写入吞吐: 15000 点/秒
- 单点查询延迟: P99 < 0.5ms
- 内存使用: 10000点 = 1.2MB

## 发现的问题

1. 无

## 建议改进

1. 增加数据压缩选项
2. 实现本地缓存层
3. 优化批量操作大小

## 结论

系统通过所有测试，扁平化存储架构性能优异，满足设计要求。
```

## 自动化测试脚本

创建 `run_all_tests.sh`：

```bash
#!/bin/bash
# run_all_tests.sh - 运行所有系统测试

set -e

echo "=== VoltageEMS 系统集成测试 ==="
echo "开始时间: $(date)"

# 1. 环境检查
echo -e "\n[1/5] 环境检查..."
./scripts/check_environment.sh

# 2. 数据流测试
echo -e "\n[2/5] 数据流测试..."
./test_data_flow.sh

# 3. 性能测试
echo -e "\n[3/5] 性能测试..."
python3 performance_test.py

# 4. 一致性测试
echo -e "\n[4/5] 数据一致性测试..."
python3 consistency_test.py

# 5. 生成报告
echo -e "\n[5/5] 生成测试报告..."
./generate_report.sh > test_report_$(date +%Y%m%d_%H%M%S).md

echo -e "\n测试完成！"
echo "结束时间: $(date)"
```

## 持续集成

### GitHub Actions 配置
```yaml
name: System Integration Test

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      redis:
        image: redis:7-alpine
        ports:
          - 6379:6379
          
    steps:
    - uses: actions/checkout@v3
    
    - name: Setup Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        
    - name: Build Services
      run: cargo build --release
      
    - name: Run Integration Tests
      run: ./run_all_tests.sh
      
    - name: Upload Test Results
      uses: actions/upload-artifact@v3
      with:
        name: test-results
        path: test_report_*.md
```

## 故障排查指南

### 常见问题

1. **Redis 连接失败**
   - 检查 Redis 是否运行
   - 验证连接参数
   - 检查防火墙设置

2. **数据写入但无法读取**
   - 检查键名格式
   - 验证 Redis 持久化配置
   - 查看服务日志

3. **性能下降**
   - 监控 Redis 内存使用
   - 检查网络延迟
   - 分析慢查询日志