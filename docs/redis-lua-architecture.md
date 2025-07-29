# VoltageEMS Redis+Lua 混合架构设计

## 1. 架构概述

VoltageEMS采用Redis+Lua脚本作为核心数据流转层，实现服务间的松耦合和高性能数据同步。这种架构将复杂的跨服务协调逻辑下沉到Redis层，让各个微服务专注于自己的核心职责。

### 1.1 架构原则

- **数据驱动**: 以Redis为中心的数据流转
- **松耦合**: 服务间通过Redis通信，互不依赖
- **高性能**: 数据同步在Redis内部完成，最小化网络开销
- **易维护**: 同步逻辑集中管理，便于修改和调试

### 1.2 整体架构图

```
┌─────────────────────────────────────────────────────────────┐
│                      应用层                                  │
│            Web UI | Mobile App | HMI/SCADA                  │
└─────────────────────┬───────────────────────────────────────┘
                      │
                ┌─────┴─────┐
                │API Gateway│ ← 轻量级路由层
                └─────┬─────┘
                      │
┌─────────────────────┴──────────────────────────────────────┐
│                    Redis 数据流转层                          │
│                                                            │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                  Lua Scripts                        │   │
│  │  • 数据同步  • 告警检测  • 规则引擎  • 消息路由           │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                            │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              Redis Data Structures                  │   │
│  │  • Hash存储  • Pub/Sub通道  • Stream队列  • 缓存       │   │
│  └─────────────────────────────────────────────────────┘   │
└──┬──────────┬────────┬─────────┬──────────┬──────────┬─────┘
   │          │        │         │          │          │
┌──┴───┐  ┌───┴──┐  ┌──┴───┐  ┌──┴───┐  ┌───┴────┐  ┌──┴──┐
│comsrv│  │modsrv│  │hissrv│  │netsrv│  │alarmsrv│  │ ... │
└──┬───┘  └──────┘  └──────┘  └──────┘  └────────┘  └─────┘
   │        (轻量)   (轻量)    (核心)     (轻量)
   │
┌──┴──────────────────────────────┐
│            设备层                │
│   Modbus | IEC60870 | CAN | ... │
└─────────────────────────────────┘
```

## 2. 服务职责划分

### 2.1 核心服务（保留复杂逻辑）

#### ComsRv - 工业协议网关
- **保留功能**: 协议解析、设备通信、连接管理
- **简化功能**: 数据同步交给Lua脚本
```rust
// 只负责协议转换和数据写入
async fn handle_modbus_data(&self, data: ModbusData) -> Result<()> {
    let key = format!("comsrv:{}:m", data.channel);
    self.redis.hset(&key, &data.point.to_string(), &format!("{:.6}", data.value)).await?;

    // 触发Lua同步脚本
    self.redis.evalsha(&self.sync_script_sha, &[],
        &["c2m", &data.channel.to_string(), &data.point.to_string(), &format!("{:.6}", data.value)]
    ).await?;
}
```

#### NetSrv - 云服务网关
- **保留功能**: HTTP/MQTT客户端、认证、数据加密
- **简化功能**: 从Stream读取待发送数据
```rust
// 消费Redis Stream中的数据
async fn consume_cloud_queue(&self) -> Result<()> {
    loop {
        let messages = self.redis.xreadgroup("cloud_group", "netsrv-1",
            &[("cloud:queue", ">")], None
        ).await?;

        for msg in messages {
            self.send_to_cloud(msg).await?;
        }
    }
}
```

### 2.2 轻量服务（主要提供API）

#### ModSrv - 模型服务
- **简化为**: 纯API服务 + WebSocket推送
- **数据来源**: Redis Hash（由Lua脚本维护）
```rust
pub struct ModSrv {
    redis: Arc<Mutex<RedisClient>>,
    models: HashMap<String, ModelConfig>,  // 只存储模型元数据
}

impl ModSrv {
    // GET /models/{id}/values
    async fn get_model_values(&self, model_id: &str) -> Result<Json> {
        let data = self.redis.hgetall(&format!("modsrv:{}:measurement", model_id)).await?;
        Ok(Json(data))
    }

    // POST /models/{id}/control/{name}
    async fn send_control(&self, model_id: &str, control: &str, value: f64) -> Result<()> {
        self.redis.evalsha(&self.control_script_sha, &[],
            &[model_id, control, &format!("{:.6}", value)]
        ).await?;
        Ok(())
    }
}
```

#### AlarmSrv - 告警服务
- **简化为**: 告警查询API + 通知发送
- **告警检测**: 由Lua脚本完成
```rust
// 只负责告警展示和通知
pub struct LightweightAlarmSrv {
    async fn get_active_alarms(&self) -> Result<Vec<Alarm>> {
        let alarm_ids = self.redis.smembers("alarm:active").await?;
        // 批量获取告警详情
    }

    async fn send_notifications(&self) -> Result<()> {
        // 从队列读取待发送的通知
        while let Some(notification) = self.redis.rpop("notification:queue").await? {
            self.send_notification(notification).await?;
        }
    }
}
```

### 2.3 Lua脚本层（数据流转核心）

#### 数据同步脚本
```lua
-- data_sync.lua
-- 负责 ComsRv ↔ ModSrv 的双向数据同步
local sync_type = ARGV[1]
local channel = ARGV[2]
local point = ARGV[3]
local value = ARGV[4]

if sync_type == "c2m" then
    -- ComsRv → ModSrv 同步
    local mapping = redis.call('HGET', 'mapping:c2m', channel .. ':' .. point)
    if mapping then
        local model_id, point_name = string.match(mapping, "([^:]+):([^:]+)")
        -- 更新ModSrv数据
        redis.call('HSET', 'modsrv:' .. model_id .. ':measurement', point_name, value)
        -- 发布更新事件（用于WebSocket）
        redis.call('PUBLISH', 'modsrv:' .. model_id .. ':update', point_name .. ':' .. value)
        -- 触发告警检查
        redis.call('EVAL', alarm_check_script, 1, model_id, point_name, value)
    end
elseif sync_type == "m2c" then
    -- ModSrv → ComsRv 同步（控制命令）
    local model_id = ARGV[2]
    local control_name = ARGV[3]
    local value = ARGV[4]

    local mapping = redis.call('HGET', 'mapping:m2c', model_id .. ':' .. control_name)
    if mapping then
        local channel, point = string.match(mapping, "([^:]+):([^:]+)")
        redis.call('HSET', 'cmd:' .. channel .. ':control', point, value)
        redis.call('PUBLISH', 'cmd:' .. channel .. ':control', point .. ':' .. value)
    end
end
```

#### 告警检测脚本
```lua
-- alarm_check.lua
local model_id = ARGV[1]
local point_name = ARGV[2]
local value = tonumber(ARGV[3])

-- 获取告警规则
local rules = redis.call('HGETALL', 'alarm:rules:' .. model_id .. ':' .. point_name)

for i = 1, #rules, 2 do
    local rule_id = rules[i]
    local rule_json = rules[i+1]
    local rule = cjson.decode(rule_json)

    -- 检查阈值
    local triggered = false
    if rule.operator == ">" and value > tonumber(rule.threshold) then
        triggered = true
    elseif rule.operator == "<" and value < tonumber(rule.threshold) then
        triggered = true
    end

    if triggered then
        -- 创建告警
        local alarm_id = redis.call('INCR', 'alarm:counter')
        local alarm_key = 'alarm:' .. alarm_id

        redis.call('HMSET', alarm_key,
            'id', alarm_id,
            'model_id', model_id,
            'point_name', point_name,
            'value', tostring(value),
            'rule_id', rule_id,
            'timestamp', ARGV[4] or tostring(os.time()),
            'status', 'active',
            'severity', rule.severity or 'medium'
        )

        -- 加入活动告警集合
        redis.call('SADD', 'alarm:active', alarm_id)

        -- 发送通知
        local notification = cjson.encode({
            alarm_id = alarm_id,
            message = string.format("%s.%s = %.6f 超过阈值 %.6f",
                model_id, point_name, value, rule.threshold)
        })
        redis.call('LPUSH', 'notification:queue', notification)

        -- 发布告警事件
        redis.call('PUBLISH', 'alarm:new', alarm_id)
    end
end
```

#### 规则引擎脚本
```lua
-- rule_engine.lua
local rule_id = ARGV[1]

-- 获取规则定义
local rule = redis.call('HGETALL', 'rule:' .. rule_id)
local rule_data = {}
for i = 1, #rule, 2 do
    rule_data[rule[i]] = rule[i+1]
end

local conditions = cjson.decode(rule_data.conditions)
local actions = cjson.decode(rule_data.actions)

-- 评估所有条件
local all_conditions_met = true
for _, condition in ipairs(conditions) do
    local value = redis.call('HGET', condition.key, condition.field)
    if not value then
        all_conditions_met = false
        break
    end

    local num_value = tonumber(value)
    local threshold = tonumber(condition.threshold)

    local condition_met = false
    if condition.operator == ">" then
        condition_met = num_value > threshold
    elseif condition.operator == "<" then
        condition_met = num_value < threshold
    elseif condition.operator == "==" then
        condition_met = math.abs(num_value - threshold) < 0.000001
    end

    if not condition_met then
        all_conditions_met = false
        break
    end
end

-- 执行动作
if all_conditions_met then
    for _, action in ipairs(actions) do
        if action.type == "control" then
            -- 发送控制命令
            redis.call('EVAL', data_sync_script, 0,
                "m2c", action.model_id, action.control_name, action.value)

        elseif action.type == "alarm" then
            -- 触发告警
            redis.call('EVAL', alarm_trigger_script, 0,
                action.severity, action.message)

        elseif action.type == "notification" then
            -- 发送通知
            redis.call('LPUSH', 'notification:queue', cjson.encode(action))
        end
    end

    -- 记录规则执行
    redis.call('HINCRBY', 'rule:stats:' .. rule_id, 'execution_count', 1)
    redis.call('HSET', 'rule:stats:' .. rule_id, 'last_execution', os.time())
end
```

## 3. 数据结构设计

### 3.1 核心数据存储

```yaml
# 实时数据
comsrv:{channelID}:{type}          # Hash - 原始采集数据
modsrv:{modelID}:{type}            # Hash - 模型数据
alarm:{alarmID}                    # Hash - 告警详情
rule:{ruleID}                      # Hash - 规则定义

# 映射关系
mapping:c2m                        # Hash - ComsRv到ModSrv的映射
mapping:m2c                        # Hash - ModSrv到ComsRv的映射

# 队列和流
hissrv:queue                       # Stream - 历史数据队列
cloud:queue                        # Stream - 云端发送队列
notification:queue                 # List - 通知队列

# 集合
alarm:active                       # Set - 活动告警ID集合
model:enabled                      # Set - 启用的模型ID

# 缓存
cache:model:{modelID}:values       # String - 模型数据缓存(JSON)
cache:api:{endpoint}:{params}      # String - API响应缓存
```

### 3.2 Lua脚本管理

```yaml
# 脚本SHA存储
lua:sha:data_sync                  # String - 数据同步脚本SHA
lua:sha:alarm_check                # String - 告警检测脚本SHA
lua:sha:rule_engine                # String - 规则引擎脚本SHA
lua:sha:cache_manager              # String - 缓存管理脚本SHA

# 脚本版本控制
lua:version:data_sync              # String - 脚本版本号
lua:loaded:{version}               # Set - 已加载的脚本版本
```

## 4. 实施方案

### 4.1 第一阶段：基础设施准备
1. 编写核心Lua脚本
2. 建立脚本版本管理机制
3. 实现脚本加载和热更新

### 4.2 第二阶段：服务改造
1. ModSrv简化为API服务
2. AlarmSrv去除检测逻辑
3. ComsRv添加脚本调用

### 4.3 第三阶段：性能优化
1. 实现脚本缓存
2. 优化数据结构
3. 添加监控指标

### 4.4 第四阶段：高级特性
1. 实现脚本调试工具
2. 添加脚本性能分析
3. 支持脚本热更新

## 5. 性能考虑

### 5.1 脚本优化原则
- 避免循环中的Redis调用
- 使用批量操作
- 预编译正则表达式
- 合理使用本地变量

### 5.2 数据结构优化
- Hash field数量控制在1000以内
- Stream消息及时清理
- 合理设置过期时间

### 5.3 监控指标
```lua
-- 在每个脚本中添加性能统计
local start_time = redis.call('TIME')[1] * 1000 + redis.call('TIME')[2] / 1000

-- 脚本逻辑...

local end_time = redis.call('TIME')[1] * 1000 + redis.call('TIME')[2] / 1000
redis.call('HINCRBY', 'lua:stats:' .. script_name, 'count', 1)
redis.call('HINCRBYFLOAT', 'lua:stats:' .. script_name, 'total_time', end_time - start_time)
```

## 6. 开发指南

### 6.1 添加新的数据同步
1. 在`mapping`中添加映射关系
2. 扩展`data_sync.lua`脚本
3. 更新相关服务的API

### 6.2 添加新的告警规则
1. 在`alarm:rules`中定义规则
2. 确保`alarm_check.lua`支持新的操作符
3. 配置告警通知模板

### 6.3 调试Lua脚本
```bash
# 使用redis-cli测试脚本
redis-cli --eval data_sync.lua , c2m 1001 10001 25.123456

# 查看脚本执行统计
redis-cli HGETALL lua:stats:data_sync

# 监控脚本执行
redis-cli MONITOR | grep EVAL
```

## 7. 迁移计划

### 7.1 保持向后兼容
- 保留原有的Pub/Sub通道
- 逐步迁移数据同步逻辑
- 提供切换开关

### 7.2 灰度发布
1. 先在测试环境验证
2. 选择低流量通道试点
3. 逐步扩大使用范围

### 7.3 回滚方案
- 保留原有同步代码
- 通过配置切换新旧方案
- 监控关键指标

## 8. 总结

Redis+Lua混合架构为VoltageEMS带来了：

1. **更高的性能** - 数据同步在Redis内部完成
2. **更好的可维护性** - 逻辑集中，便于管理
3. **更强的灵活性** - 无需重启服务即可修改逻辑
4. **更低的复杂度** - 服务专注核心功能

这种架构特别适合工业IoT场景，能够在保证实时性的同时，降低系统复杂度和运维成本。
