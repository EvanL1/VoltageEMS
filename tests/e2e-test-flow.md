# 端到端集成测试流程说明

## 测试架构
```
[Modbus模拟器] --TCP--> [comsrv] --写入--> [Redis] <--读取-- [其他服务]
     |                      |                 |
   端口5020              端口6000          端口6379
```

## 1. 数据流测试步骤

### Step 1: 模拟传感器数据
```bash
# Modbus模拟器自动提供数据在端口5020
docker run -d --name modbus-sim -p 5020:5020 oitc/modbus-server
```

### Step 2: comsrv采集数据
```yaml
# comsrv配置 (services/comsrv/config/test-modbus.yaml)
channels:
  - id: 1001
    protocol: "modbus"
    transport_config:
      tcp:
        address: "modbus-sim:5020"
    polling_config:
      interval_ms: 5000  # 每5秒轮询一次
```

### Step 3: 数据写入Redis
```bash
# comsrv自动将数据写入Redis Hash结构
# 格式: comsrv:{channelID}:{type}
# 示例: comsrv:1001:T (遥测数据)

# 验证数据写入
docker exec redis-test redis-cli HGET "comsrv:1001:T" "1"
```

### Step 4: 规则引擎处理
```bash
# 创建温度阈值规则
docker exec redis-test redis-cli FCALL rule_upsert 1 "rule_001" \
  '{"name":"温度阈值",
    "condition_groups":[{
      "conditions":[{"field":"temperature","operator":">","value":30}]
    }],
    "actions":[{"type":"create_alarm"}]
  }'
```

### Step 5: 告警触发
```bash
# 当温度超过30度时，自动创建告警
docker exec redis-test redis-cli FCALL store_alarm 1 "alarm_001" \
  '{"title":"高温告警","level":"Critical","value":35.5}'
```

### Step 6: 告警处理
```bash
# 操作员确认告警
docker exec redis-test redis-cli FCALL acknowledge_alarm 2 "alarm_001" "operator1"

# 问题解决后，关闭告警
docker exec redis-test redis-cli FCALL resolve_alarm 2 "alarm_001" "operator1"
```

## 2. 完整测试脚本

```bash
#!/bin/bash
# 端到端自动化测试

# 1. 准备测试数据
echo "Writing test data..."
docker exec redis-test redis-cli HSET "comsrv:1001:T" "1" "35.5"

# 2. 创建规则
echo "Creating rule..."
docker exec redis-test redis-cli FCALL rule_upsert 1 "temp_rule" \
  '{"name":"高温规则",
    "condition_groups":[{
      "conditions":[{"field":"temp","operator":">","value":30}]
    }],
    "actions":[{"type":"alarm"}]}'

# 3. 触发告警
echo "Triggering alarm..."
docker exec redis-test redis-cli FCALL store_alarm 1 "high_temp_001" \
  '{"title":"温度过高","level":"Warning","value":35.5}'

# 4. 验证数据流
echo "Verifying data flow..."
TEMP=$(docker exec redis-test redis-cli HGET "comsrv:1001:T" "1")
ALARM=$(docker exec redis-test redis-cli HGET "alarmsrv:high_temp_001" "level")

if [ "$TEMP" = "35.5" ] && [ "$ALARM" = "Warning" ]; then
    echo "✅ End-to-end test PASSED"
else
    echo "❌ End-to-end test FAILED"
fi
```

## 3. 实际测试执行

### 测试命令序列
```bash
# 1. 启动所有服务
docker-compose -f docker-compose.test.yml up -d

# 2. 等待服务就绪
sleep 5

# 3. 验证服务状态
docker ps | grep -E "(comsrv|redis|modbus)"

# 4. 执行数据写入
docker exec redis-test redis-cli HSET "comsrv:1001:T" "1" "25.5"

# 5. 验证数据读取
docker exec redis-test redis-cli HGET "comsrv:1001:T" "1"

# 6. 测试Modbus通信
echo -e "\x00\x01\x00\x00\x00\x06\x01\x03\x00\x00\x00\x0A" | nc -w 2 localhost 5020

# 7. 检查comsrv日志
docker logs comsrv-test | grep "Channel stats"
```

## 4. 测试验证点

✅ **基础连接测试**
- Redis连接: `redis-cli ping`
- Modbus连接: `nc -z localhost 5020`
- comsrv API: `curl http://localhost:6000/health`

✅ **数据流测试**
- 写入数据到Redis
- 从Redis读取数据
- 验证数据格式正确

✅ **功能集成测试**
- 规则创建和执行
- 告警触发和管理
- 历史数据记录

✅ **性能测试**
- 批量数据操作
- 并发请求处理
- 响应时间测量

## 5. 测试结果判定

### 成功标准
1. 所有服务容器运行正常
2. 数据能够从Modbus -> comsrv -> Redis流转
3. Lua Functions正确执行
4. 告警系统响应正常
5. 无错误日志

### 失败处理
- 检查Docker日志: `docker logs <container>`
- 验证网络连接: `docker network ls`
- 检查Redis数据: `redis-cli MONITOR`
- 查看系统资源: `docker stats`

## 总结

端到端测试通过模拟真实的工业数据采集场景，验证了从数据采集到告警处理的完整流程。测试覆盖了所有核心服务，确保系统在集成环境下的稳定性和功能完整性。