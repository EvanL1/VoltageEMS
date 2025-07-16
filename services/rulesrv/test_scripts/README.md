# rulesrv 测试脚本

这些Python脚本用于测试rulesrv规则引擎服务的功能。

## 环境准备

1. 安装依赖：
```bash
# 使用uv环境
uv pip install -r requirements.txt

# 或使用pip
pip install -r requirements.txt
```

2. 确保Redis服务正在运行：
```bash
redis-cli ping
# 应返回 PONG
```

3. 确保rulesrv服务正在运行（可选，用于API测试）：
```bash
cd ../
cargo run
```

## 脚本说明

### 1. test_rule_definition.py - 规则定义和存储

创建和管理规则定义，支持DAG格式规则。

```bash
# 创建所有示例规则
uv run python test_rule_definition.py --action create-all

# 列出已保存的规则
uv run python test_rule_definition.py --action list

# 删除指定规则
uv run python test_rule_definition.py --action delete --rule-id temp_monitor_001
```

示例规则包括：
- **温度监控规则**：温度>30°C时开启冷却设备
- **功率限制规则**：功率>100kW时降低负载
- **综合告警规则**：温度和功率同时超限触发告警
- **模型输出规则**：根据modsrv效率触发优化

### 2. test_data_publisher.py - 数据发布

模拟各种数据源发布数据到Redis。

```bash
# 持续模拟模式（温度、功率、模型输出）
uv run python test_data_publisher.py --mode continuous

# 场景测试模式
uv run python test_data_publisher.py --mode scenario --scenario high_temp
uv run python test_data_publisher.py --mode scenario --scenario high_power
uv run python test_data_publisher.py --mode scenario --scenario combined_alarm
uv run python test_data_publisher.py --mode scenario --scenario low_efficiency

# 监控通道消息
uv run python test_data_publisher.py --mode monitor

# 发布单个数据点
uv run python test_data_publisher.py --mode single \
    --channel-id 1001 --point-type m --point-id 10001 --value 35.5
```

### 3. test_rule_trigger.py - 规则触发测试

通过API触发规则执行并监控结果。

```bash
# 列出所有规则
uv run python test_rule_trigger.py --action list

# 手动触发规则
uv run python test_rule_trigger.py --action trigger --rule-id temp_monitor_001

# 使用输入数据触发
uv run python test_rule_trigger.py --action trigger --rule-id temp_monitor_001 \
    --input '{"input_temp": 35.0}'

# 测试所有规则（使用预设数据）
uv run python test_rule_trigger.py --action test

# 监控规则执行
uv run python test_rule_trigger.py --action monitor --rule-id temp_monitor_001

# 查看执行历史
uv run python test_rule_trigger.py --action history --rule-id temp_monitor_001
```

### 4. test_integration.py - 集成测试

完整的端到端测试流程。

```bash
# 运行所有集成测试
uv run python test_integration.py --mode full

# 运行单个测试场景
uv run python test_integration.py --mode scenario --scenario simple_control

# 实时监控数据流
uv run python test_integration.py --mode monitor --monitor-duration 60
```

### 5. monitor_redis.py - Redis监控工具

实时监控Redis中的数据变化。

```bash
# 监控所有内容（通道、键值、规则执行）
uv run python monitor_redis.py --mode all

# 仅监控通道消息
uv run python monitor_redis.py --mode channels

# 仅监控键值变化
uv run python monitor_redis.py --mode keys

# 仅监控规则执行
uv run python monitor_redis.py --mode rules

# 显示当前状态快照
uv run python monitor_redis.py --mode status
```

### 6. clean_test_data.py - 数据清理

清理测试过程中产生的数据。

```bash
# 交互式清理
uv run python clean_test_data.py

# 清理所有测试数据
uv run python clean_test_data.py --mode all

# 模拟运行（不实际删除）
uv run python clean_test_data.py --mode all --dry-run

# 仅清理测试规则
uv run python clean_test_data.py --mode rules --pattern "test_*"

# 清理超过24小时的执行记录
uv run python clean_test_data.py --mode executions --age-hours 24
```

## 测试流程建议

### 基础功能测试

1. 创建规则：
```bash
uv run python test_rule_definition.py --action create-all
```

2. 启动数据模拟：
```bash
uv run python test_data_publisher.py --mode continuous
```

3. 在另一个终端监控：
```bash
uv run python monitor_redis.py --mode all
```

4. 查看规则是否被触发。

### API测试（需要rulesrv服务运行）

1. 确保服务运行后，手动触发规则：
```bash
uv run python test_rule_trigger.py --action trigger --rule-id temp_monitor_001 \
    --input '{"input_temp": 35.0}'
```

2. 查看执行历史：
```bash
uv run python test_rule_trigger.py --action history --rule-id temp_monitor_001
```

### 完整集成测试

运行自动化集成测试：
```bash
uv run python test_integration.py --mode full
```

### 清理

测试完成后清理数据：
```bash
uv run python clean_test_data.py --mode all
```

## 规则定义格式说明

### DAG规则结构

```json
{
    "id": "规则ID",
    "name": "规则名称",
    "enabled": true,
    "nodes": [
        {
            "id": "节点ID",
            "type": "节点类型",
            "config": { /* 节点配置 */ }
        }
    ],
    "edges": [
        {
            "from": "源节点ID",
            "to": "目标节点ID",
            "condition": "可选条件表达式"
        }
    ]
}
```

### 节点类型

1. **Input** - 数据输入
   - 点位数据：`{channel_id}:{type}:{point_id}`
   - modsrv输出：`modsrv:{model_id}:{output_name}`

2. **Condition** - 条件判断
   - 支持表达式：`==`, `!=`, `>`, `<`
   - 变量引用：`$variable_name`

3. **Transform** - 数据转换
   - scale：数值缩放
   - threshold：阈值判断

4. **Action** - 执行动作
   - control：控制命令
   - alarm：告警触发

5. **Aggregate** - 聚合操作
   - and：逻辑与
   - or：逻辑或
   - sum：求和
   - avg：平均值

## 注意事项

1. 所有脚本默认连接本地Redis (localhost:6379)
2. 可通过命令行参数指定其他Redis地址
3. 规则执行需要rulesrv服务订阅相应通道
4. 测试数据使用特定前缀（test_）便于清理
5. 监控脚本会产生大量输出，建议重定向到文件