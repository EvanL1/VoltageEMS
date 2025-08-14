# VoltageEMS 服务职责分工文档

## 架构概览

VoltageEMS 采用混合架构设计：
- **Rust 服务**：负责 I/O 密集型操作、协议处理、系统接口
- **Redis Lua Functions**：负责业务逻辑、数据处理、高频计算
- **Python 服务**：负责 API 聚合、数据转发、历史存储

## 服务职责矩阵

| 服务 | 语言 | 主要职责 | Redis Lua 交互 |
|------|------|----------|---------------|
| comsrv | Rust | 协议通信、数据采集 | 调用 Lua 写入数据 |
| modsrv | Rust | 模型框架、触发计算 | 调用 Lua 执行计算 |
| alarmsrv | Rust | 告警框架、通知推送 | 调用 Lua 判断条件 |
| rulesrv | Rust | 规则框架、动作执行 | 调用 Lua 评估规则 |
| apigateway | Python | REST API、WebSocket | 读取 Redis 数据 |
| netsrv | Python | 数据转发、协议转换 | 调用 Lua 批量收集 |
| hissrv | Python | 历史存储、数据归档 | 读取 Redis 数据 |

## 详细职责分工

### 1. comsrv（通信服务）

#### Rust 负责：
- **协议处理**
  - Modbus TCP/RTU 协议栈实现
  - Virtual 虚拟协议实现
  - gRPC 服务端实现
  - 协议插件管理框架
- **I/O 操作**
  - TCP/串口连接管理
  - 异步 I/O 处理
  - 连接池维护
  - 超时和重试机制
- **数据采集**
  - 定时轮询设备
  - 数据帧解析
  - CRC 校验
  - 字节序转换（ABCD/DCBA等）
- **配置管理**
  - 加载 CSV 点表配置
  - 通道配置管理
  - 协议参数配置

#### Lua Functions：
```lua
-- comsrv_write_data
-- 功能：原子写入通道数据并触发下游计算
-- 参数：channel_id, data_type, values
-- 返回：写入结果
function comsrv_write_data(keys, args)
    local channel_id = keys[1]
    local data_type = args[1]
    local values = cjson.decode(args[2])
    
    -- 1. 写入数据到 Hash
    local key = string.format("comsrv:%s:%s", channel_id, data_type)
    for point_id, value in pairs(values) do
        redis.call('HSET', key, point_id, value)
    end
    
    -- 2. 更新时间戳
    redis.call('HSET', key, '_timestamp', args[3])
    
    -- 3. 触发模型计算（如果需要）
    if data_type == "T" then
        -- 调用 modsrv_calculate
    end
    
    return "OK"
end

-- comsrv_batch_write
-- 功能：批量写入多个通道数据
-- 参数：批量数据JSON
-- 返回：写入统计
function comsrv_batch_write(keys, args)
    local batch_data = cjson.decode(args[1])
    local success_count = 0
    local error_count = 0
    
    for _, item in ipairs(batch_data) do
        local key = string.format("comsrv:%s:%s", item.channel_id, item.data_type)
        local ok, err = pcall(function()
            for point_id, value in pairs(item.values) do
                redis.call('HSET', key, point_id, value)
            end
            redis.call('HSET', key, '_timestamp', item.timestamp)
        end)
        
        if ok then
            success_count = success_count + 1
        else
            error_count = error_count + 1
        end
    end
    
    return cjson.encode({
        success = success_count,
        error = error_count,
        total = success_count + error_count
    })
end

-- comsrv_validate_data
-- 功能：数据质量校验
-- 参数：channel_id, point_id, value
-- 返回：校验结果
function comsrv_validate_data(keys, args)
    local channel_id = keys[1]
    local point_id = args[1]
    local value = tonumber(args[2])
    
    -- 获取点位配置
    local config_key = string.format("config:point:%s:%s", channel_id, point_id)
    local min = tonumber(redis.call('HGET', config_key, 'min') or -999999)
    local max = tonumber(redis.call('HGET', config_key, 'max') or 999999)
    
    -- 范围检查
    if value < min or value > max then
        return cjson.encode({
            valid = false,
            reason = "out_of_range",
            min = min,
            max = max,
            value = value
        })
    end
    
    -- 变化率检查
    local last_key = string.format("comsrv:%s:T", channel_id)
    local last_value = tonumber(redis.call('HGET', last_key, point_id) or 0)
    local change_rate = math.abs((value - last_value) / last_value)
    
    if change_rate > 0.5 then  -- 变化超过50%
        return cjson.encode({
            valid = false,
            reason = "rapid_change",
            last_value = last_value,
            current_value = value,
            change_rate = change_rate
        })
    end
    
    return cjson.encode({valid = true})
end
```

### 2. modsrv（模型服务）

#### Rust 负责：
- **服务框架**
  - HTTP API 接口
  - 健康检查
  - 配置加载
  - 定时触发器
- **模型管理**
  - 加载模型配置（models.yaml）
  - 模型调度
  - 结果存储

#### Lua Functions：
```lua
-- modsrv_calculate
-- 功能：执行能效计算、功率因数等模型计算
-- 参数：model_id, input_points
-- 返回：计算结果
function modsrv_calculate(keys, args)
    local model_id = keys[1]
    local channel_id = args[1]
    
    -- 1. 获取输入数据
    local voltage = redis.call('HGET', 'comsrv:' .. channel_id .. ':T', '1')
    local current = redis.call('HGET', 'comsrv:' .. channel_id .. ':T', '2')
    local power_factor = redis.call('HGET', 'comsrv:' .. channel_id .. ':T', '3')
    
    -- 2. 执行计算
    local active_power = voltage * current * power_factor
    local reactive_power = voltage * current * math.sqrt(1 - power_factor^2)
    local apparent_power = voltage * current
    
    -- 3. 存储结果
    local result_key = string.format("modsrv:realtime:module:%s", model_id)
    redis.call('HSET', result_key, 'active_power', active_power)
    redis.call('HSET', result_key, 'reactive_power', reactive_power)
    redis.call('HSET', result_key, 'apparent_power', apparent_power)
    redis.call('HSET', result_key, '_timestamp', args[2])
    
    -- 4. 触发告警检查
    -- alarmsrv_check(...)
    
    return cjson.encode({
        active_power = active_power,
        reactive_power = reactive_power,
        apparent_power = apparent_power
    })
end

-- modsrv_efficiency
-- 功能：计算设备效率
function modsrv_efficiency(keys, args)
    local device_id = keys[1]
    local channel_id = args[1]
    local period = args[2] or "hour"  -- hour, day, month
    
    -- 获取输入功率和输出功率
    local input_power = tonumber(redis.call('HGET', 'comsrv:' .. channel_id .. ':T', 'input_power') or 0)
    local output_power = tonumber(redis.call('HGET', 'comsrv:' .. channel_id .. ':T', 'output_power') or 0)
    
    -- 计算瞬时效率
    local efficiency = 0
    if input_power > 0 then
        efficiency = (output_power / input_power) * 100
    end
    
    -- 存储效率值
    local result_key = string.format("modsrv:efficiency:%s", device_id)
    redis.call('HSET', result_key, 'instant', efficiency)
    redis.call('HSET', result_key, 'input_power', input_power)
    redis.call('HSET', result_key, 'output_power', output_power)
    
    -- 更新统计数据
    local stats_key = string.format("modsrv:efficiency:stats:%s:%s", device_id, period)
    redis.call('ZADD', stats_key, efficiency, args[3])  -- timestamp as score
    
    -- 计算平均效率
    local values = redis.call('ZRANGE', stats_key, 0, -1)
    local sum = 0
    for _, v in ipairs(values) do
        sum = sum + tonumber(v)
    end
    local avg_efficiency = sum / #values
    
    redis.call('HSET', result_key, 'average_' .. period, avg_efficiency)
    
    return cjson.encode({
        instant = efficiency,
        average = avg_efficiency,
        input = input_power,
        output = output_power
    })
end

-- modsrv_statistics
-- 功能：统计分析（最大值、最小值、平均值等）
function modsrv_statistics(keys, args)
    local channel_id = keys[1]
    local point_id = args[1]
    local period = args[2]  -- "minute", "hour", "day"
    local timestamp = tonumber(args[3])
    
    -- 获取当前值
    local current_value = tonumber(redis.call('HGET', 'comsrv:' .. channel_id .. ':T', point_id) or 0)
    
    -- 统计键
    local stats_key = string.format("stats:%s:%s:%s", channel_id, point_id, period)
    
    -- 添加到有序集合（时间戳作为分数）
    redis.call('ZADD', stats_key, timestamp, current_value .. ':' .. timestamp)
    
    -- 清理过期数据
    local expire_time = timestamp - (period == "minute" and 60 or period == "hour" and 3600 or 86400)
    redis.call('ZREMRANGEBYSCORE', stats_key, '-inf', expire_time)
    
    -- 获取所有值进行统计
    local data_points = redis.call('ZRANGE', stats_key, 0, -1)
    local values = {}
    for _, dp in ipairs(data_points) do
        local value = tonumber(string.match(dp, "^([^:]+)"))
        table.insert(values, value)
    end
    
    -- 计算统计值
    local count = #values
    if count == 0 then
        return cjson.encode({error = "no data"})
    end
    
    table.sort(values)
    local min = values[1]
    local max = values[count]
    local sum = 0
    for _, v in ipairs(values) do
        sum = sum + v
    end
    local avg = sum / count
    
    -- 计算标准差
    local variance = 0
    for _, v in ipairs(values) do
        variance = variance + (v - avg) ^ 2
    end
    local stddev = math.sqrt(variance / count)
    
    -- 存储统计结果
    local result_key = string.format("modsrv:stats:%s:%s:%s", channel_id, point_id, period)
    redis.call('HSET', result_key, 'min', min)
    redis.call('HSET', result_key, 'max', max)
    redis.call('HSET', result_key, 'avg', avg)
    redis.call('HSET', result_key, 'stddev', stddev)
    redis.call('HSET', result_key, 'count', count)
    redis.call('HSET', result_key, '_timestamp', timestamp)
    
    return cjson.encode({
        min = min,
        max = max,
        avg = avg,
        stddev = stddev,
        count = count,
        period = period
    })
end

-- modsrv_aggregate
-- 功能：数据聚合计算
function modsrv_aggregate(keys, args)
    local group_id = keys[1]
    local operation = args[1]  -- "sum", "avg", "max", "min"
    local channels = cjson.decode(args[2])  -- 通道列表
    local point_id = args[3]
    
    local values = {}
    for _, channel_id in ipairs(channels) do
        local value = tonumber(redis.call('HGET', 'comsrv:' .. channel_id .. ':T', point_id) or 0)
        table.insert(values, value)
    end
    
    local result = 0
    if operation == "sum" then
        for _, v in ipairs(values) do
            result = result + v
        end
    elseif operation == "avg" then
        local sum = 0
        for _, v in ipairs(values) do
            sum = sum + v
        end
        result = sum / #values
    elseif operation == "max" then
        result = math.max(table.unpack(values))
    elseif operation == "min" then
        result = math.min(table.unpack(values))
    end
    
    -- 存储聚合结果
    local result_key = string.format("modsrv:aggregate:%s:%s", group_id, point_id)
    redis.call('HSET', result_key, 'value', result)
    redis.call('HSET', result_key, 'operation', operation)
    redis.call('HSET', result_key, 'count', #values)
    redis.call('HSET', result_key, '_timestamp', args[4])
    
    return tostring(result)
end
```

### 3. alarmsrv（告警服务）

#### Rust 负责：
- **告警框架**
  - 告警配置管理
  - 告警状态维护
  - 告警历史记录
- **通知系统**
  - 邮件发送
  - 短信推送
  - WebSocket 推送
  - 第三方集成
- **告警管理**
  - 告警确认
  - 告警抑制
  - 告警升级

#### Lua Functions：
```lua
-- alarmsrv_check
-- 功能：检查告警条件
-- 参数：alarm_rule_id, channel_id, point_id, value
-- 返回：告警状态
function alarmsrv_check(keys, args)
    local rule_id = keys[1]
    local value = tonumber(args[1])
    
    -- 1. 获取告警规则
    local rule = redis.call('HGETALL', 'alarm:rule:' .. rule_id)
    local threshold = tonumber(rule.threshold)
    local condition = rule.condition
    
    -- 2. 判断条件
    local triggered = false
    if condition == ">" and value > threshold then
        triggered = true
    elseif condition == "<" and value < threshold then
        triggered = true
    elseif condition == "==" and value == threshold then
        triggered = true
    end
    
    -- 3. 更新告警状态
    if triggered then
        redis.call('HSET', 'alarm:active:' .. rule_id, 'status', '1')
        redis.call('HSET', 'alarm:active:' .. rule_id, 'value', value)
        redis.call('HSET', 'alarm:active:' .. rule_id, 'timestamp', args[2])
        
        -- 4. 记录告警历史
        redis.call('ZADD', 'alarm:history', args[2], rule_id .. ':' .. value)
    else
        -- 恢复告警
        redis.call('HDEL', 'alarm:active:' .. rule_id)
    end
    
    return triggered and "TRIGGERED" or "NORMAL"
end

-- alarmsrv_batch_check
-- 功能：批量检查多个告警规则
function alarmsrv_batch_check(keys, args)
    local rule_ids = cjson.decode(args[1])
    local results = {}
    
    for _, rule_id in ipairs(rule_ids) do
        -- 获取规则配置
        local rule = redis.call('HGETALL', 'alarm:rule:' .. rule_id)
        if #rule > 0 then
            local channel_id = rule.channel_id
            local point_id = rule.point_id
            local threshold = tonumber(rule.threshold)
            local condition = rule.condition
            
            -- 获取当前值
            local value = tonumber(redis.call('HGET', 'comsrv:' .. channel_id .. ':T', point_id) or 0)
            
            -- 判断条件
            local triggered = false
            if condition == ">" and value > threshold then
                triggered = true
            elseif condition == "<" and value < threshold then
                triggered = true
            elseif condition == ">=" and value >= threshold then
                triggered = true
            elseif condition == "<=" and value <= threshold then
                triggered = true
            end
            
            table.insert(results, {
                rule_id = rule_id,
                triggered = triggered,
                value = value,
                threshold = threshold
            })
            
            -- 更新告警状态
            if triggered then
                redis.call('HSET', 'alarm:active:' .. rule_id, 'status', '1')
                redis.call('HSET', 'alarm:active:' .. rule_id, 'value', value)
            end
        end
    end
    
    return cjson.encode(results)
end

-- alarmsrv_acknowledge
-- 功能：确认告警
function alarmsrv_acknowledge(keys, args)
    local alarm_id = keys[1]
    local user_id = args[1]
    local comment = args[2]
    local timestamp = args[3]
    
    -- 检查告警是否存在
    local alarm = redis.call('HGETALL', 'alarm:active:' .. alarm_id)
    if #alarm == 0 then
        return cjson.encode({error = "alarm not found"})
    end
    
    -- 更新告警状态
    redis.call('HSET', 'alarm:active:' .. alarm_id, 'acknowledged', '1')
    redis.call('HSET', 'alarm:active:' .. alarm_id, 'ack_user', user_id)
    redis.call('HSET', 'alarm:active:' .. alarm_id, 'ack_comment', comment)
    redis.call('HSET', 'alarm:active:' .. alarm_id, 'ack_time', timestamp)
    
    -- 记录确认历史
    redis.call('ZADD', 'alarm:ack:history', timestamp, 
        alarm_id .. ':' .. user_id .. ':' .. timestamp)
    
    return cjson.encode({success = true, alarm_id = alarm_id})
end

-- alarmsrv_suppress
-- 功能：告警抑制
function alarmsrv_suppress(keys, args)
    local pattern = keys[1]  -- 抑制模式
    local duration = tonumber(args[1])  -- 抑制时长（秒）
    local reason = args[2]
    local timestamp = tonumber(args[3])
    
    -- 设置抑制规则
    local suppress_key = 'alarm:suppress:' .. pattern
    redis.call('HSET', suppress_key, 'pattern', pattern)
    redis.call('HSET', suppress_key, 'start_time', timestamp)
    redis.call('HSET', suppress_key, 'end_time', timestamp + duration)
    redis.call('HSET', suppress_key, 'reason', reason)
    
    -- 设置过期时间
    redis.call('EXPIRE', suppress_key, duration)
    
    return cjson.encode({
        pattern = pattern,
        duration = duration,
        end_time = timestamp + duration
    })
end
```

### 4. rulesrv（规则引擎）

#### Rust 负责：
- **规则管理**
  - 规则配置加载
  - 规则调度器
  - 规则版本控制
- **动作执行**
  - 控制命令下发
  - 联动控制
  - 脚本执行
- **规则监控**
  - 执行统计
  - 性能监控

#### Lua Functions：
```lua
-- rulesrv_evaluate
-- 功能：评估规则条件并触发动作
-- 参数：rule_id, context
-- 返回：执行结果
function rulesrv_evaluate(keys, args)
    local rule_id = keys[1]
    local context = cjson.decode(args[1])
    
    -- 1. 获取规则定义
    local rule = redis.call('HGETALL', 'rule:' .. rule_id)
    
    -- 2. 评估条件
    local conditions_met = true
    for _, condition in ipairs(rule.conditions) do
        -- 评估每个条件
        local value = redis.call('HGET', condition.key, condition.field)
        if not evaluate_condition(value, condition.operator, condition.threshold) then
            conditions_met = false
            break
        end
    end
    
    -- 3. 执行动作
    if conditions_met then
        for _, action in ipairs(rule.actions) do
            if action.type == "control" then
                -- 下发控制命令
                redis.call('LPUSH', 'control:queue', cjson.encode(action))
            elseif action.type == "notification" then
                -- 发送通知
                redis.call('LPUSH', 'notification:queue', cjson.encode(action))
            end
        end
    end
    
    return conditions_met and "EXECUTED" or "SKIPPED"
end

-- rulesrv_cascade
-- 功能：级联规则处理
function rulesrv_cascade(keys, args)
    local parent_rule_id = keys[1]
    local trigger_result = args[1]  -- "EXECUTED" or "SKIPPED"
    local timestamp = args[2]
    
    -- 获取子规则列表
    local child_rules = redis.call('SMEMBERS', 'rule:cascade:' .. parent_rule_id)
    local results = {}
    
    for _, child_rule_id in ipairs(child_rules) do
        -- 获取子规则配置
        local child_rule = redis.call('HGETALL', 'rule:' .. child_rule_id)
        
        -- 检查级联条件
        local should_trigger = false
        if child_rule.cascade_condition == "on_success" and trigger_result == "EXECUTED" then
            should_trigger = true
        elseif child_rule.cascade_condition == "on_failure" and trigger_result == "SKIPPED" then
            should_trigger = true
        elseif child_rule.cascade_condition == "always" then
            should_trigger = true
        end
        
        if should_trigger then
            -- 执行子规则
            local child_result = rulesrv_evaluate({child_rule_id}, {timestamp})
            table.insert(results, {
                rule_id = child_rule_id,
                result = child_result
            })
        end
    end
    
    return cjson.encode({
        parent_rule = parent_rule_id,
        cascaded_rules = #results,
        results = results
    })
end

-- rulesrv_schedule
-- 功能：定时规则调度
function rulesrv_schedule(keys, args)
    local schedule_pattern = keys[1]  -- cron pattern or interval
    local current_time = tonumber(args[1])
    
    -- 获取匹配调度模式的规则
    local scheduled_rules = redis.call('SMEMBERS', 'rule:schedule:' .. schedule_pattern)
    local executed = {}
    local skipped = {}
    
    for _, rule_id in ipairs(scheduled_rules) do
        -- 检查上次执行时间
        local last_exec = tonumber(redis.call('HGET', 'rule:last_exec:' .. rule_id, 'time') or 0)
        local interval = tonumber(redis.call('HGET', 'rule:' .. rule_id, 'interval') or 60)
        
        if current_time - last_exec >= interval then
            -- 执行规则
            local result = rulesrv_evaluate({rule_id}, {cjson.encode({})})
            
            -- 更新执行时间
            redis.call('HSET', 'rule:last_exec:' .. rule_id, 'time', current_time)
            redis.call('HSET', 'rule:last_exec:' .. rule_id, 'result', result)
            
            if result == "EXECUTED" then
                table.insert(executed, rule_id)
            else
                table.insert(skipped, rule_id)
            end
        end
    end
    
    return cjson.encode({
        pattern = schedule_pattern,
        executed = executed,
        skipped = skipped,
        total = #executed + #skipped
    })
end

-- rulesrv_complex_condition
-- 功能：复杂条件评估（支持AND/OR/NOT逻辑）
function rulesrv_complex_condition(keys, args)
    local rule_id = keys[1]
    local context = cjson.decode(args[1])
    
    -- 获取规则的复杂条件
    local condition_json = redis.call('HGET', 'rule:' .. rule_id, 'complex_condition')
    if not condition_json then
        return "NO_CONDITION"
    end
    
    local condition = cjson.decode(condition_json)
    
    -- 递归评估条件
    local function evaluate_node(node)
        if node.type == "value" then
            -- 获取实际值
            local value = tonumber(redis.call('HGET', node.key, node.field) or 0)
            local threshold = tonumber(node.threshold)
            
            if node.operator == ">" then
                return value > threshold
            elseif node.operator == "<" then
                return value < threshold
            elseif node.operator == ">=" then
                return value >= threshold
            elseif node.operator == "<=" then
                return value <= threshold
            elseif node.operator == "==" then
                return value == threshold
            elseif node.operator == "!=" then
                return value ~= threshold
            end
        elseif node.type == "AND" then
            for _, child in ipairs(node.children) do
                if not evaluate_node(child) then
                    return false
                end
            end
            return true
        elseif node.type == "OR" then
            for _, child in ipairs(node.children) do
                if evaluate_node(child) then
                    return true
                end
            end
            return false
        elseif node.type == "NOT" then
            return not evaluate_node(node.child)
        end
    end
    
    local result = evaluate_node(condition)
    
    -- 记录评估历史
    redis.call('ZADD', 'rule:eval:history:' .. rule_id, args[2], 
        tostring(result) .. ':' .. args[2])
    
    return result and "TRUE" or "FALSE"
end
```

### 5. apigateway（API网关）- Python

#### Python 负责：
- **REST API**
  - 通道管理接口
  - 实时数据查询
  - 历史数据查询
  - 配置管理接口
- **WebSocket**
  - 连接管理
  - 订阅管理
  - 数据推送
  - 心跳处理
- **数据聚合**
  - 多服务数据整合
  - 数据格式转换
  - 响应封装

#### Redis 交互：
- 直接读取 Redis Hash 数据
- 不调用 Lua Functions（保持简单）
- 定期轮询数据更新

### 6. netsrv（网络服务）- Python

#### Python 负责：
- **数据收集**
  - 定时从 Redis 收集数据
  - 数据过滤和转换
- **协议转发**
  - MQTT 客户端
  - HTTP 客户端
  - AWS IoT 集成
- **格式转换**
  - JSON 格式化
  - ASCII 格式化
  - 二进制编码

#### Lua Functions：
```lua
-- netsrv_collect_data
-- 功能：批量收集数据用于转发
-- 参数：pattern, batch_size
-- 返回：数据集合
function netsrv_collect_data(keys, args)
    local pattern = args[1]
    local batch_size = tonumber(args[2])
    local data_type = args[3]
    
    -- 1. 扫描匹配的键
    local cursor = "0"
    local keys = {}
    repeat
        local result = redis.call('SCAN', cursor, 'MATCH', pattern, 'COUNT', batch_size)
        cursor = result[1]
        for _, key in ipairs(result[2]) do
            table.insert(keys, key)
        end
    until cursor == "0" or #keys >= batch_size
    
    -- 2. 批量获取数据
    local data = {}
    for _, key in ipairs(keys) do
        local values = redis.call('HGETALL', key)
        table.insert(data, {
            key = key,
            data = values
        })
    end
    
    return cjson.encode({
        total = #keys,
        data = data
    })
end

-- netsrv_forward_data
-- 功能：标记数据已转发并记录状态
function netsrv_forward_data(keys, args)
    local forward_id = keys[1]
    local channel_ids = cjson.decode(args[1])
    local forward_type = args[2]  -- "mqtt", "http", "aws_iot"
    local status = args[3]  -- "success", "failure"
    local timestamp = args[4]
    
    -- 更新转发状态
    for _, channel_id in ipairs(channel_ids) do
        local status_key = string.format("netsrv:forward:%s:%s", forward_type, channel_id)
        redis.call('HSET', status_key, 'last_forward', timestamp)
        redis.call('HSET', status_key, 'status', status)
        redis.call('HINCRBY', status_key, status .. '_count', 1)
    end
    
    -- 记录转发历史
    redis.call('ZADD', 'netsrv:forward:history', timestamp, 
        forward_id .. ':' .. forward_type .. ':' .. status)
    
    -- 清理过期历史（保留7天）
    local expire_time = timestamp - 604800
    redis.call('ZREMRANGEBYSCORE', 'netsrv:forward:history', '-inf', expire_time)
    
    return cjson.encode({
        forward_id = forward_id,
        channels = #channel_ids,
        type = forward_type,
        status = status
    })
end

-- netsrv_filter_data
-- 功能：根据条件过滤数据
function netsrv_filter_data(keys, args)
    local filter_name = keys[1]
    local data_pattern = args[1]
    local conditions = cjson.decode(args[2])
    
    -- 扫描匹配的数据键
    local cursor = "0"
    local filtered_data = {}
    
    repeat
        local result = redis.call('SCAN', cursor, 'MATCH', data_pattern, 'COUNT', 100)
        cursor = result[1]
        
        for _, key in ipairs(result[2]) do
            local data = redis.call('HGETALL', key)
            local include = true
            
            -- 应用过滤条件
            for _, condition in ipairs(conditions) do
                local value = tonumber(data[condition.field])
                if value then
                    if condition.operator == ">" and not (value > condition.threshold) then
                        include = false
                        break
                    elseif condition.operator == "<" and not (value < condition.threshold) then
                        include = false
                        break
                    elseif condition.operator == "==" and not (value == condition.threshold) then
                        include = false
                        break
                    end
                end
            end
            
            if include then
                table.insert(filtered_data, {
                    key = key,
                    data = data
                })
            end
        end
    until cursor == "0" or #filtered_data >= 1000
    
    return cjson.encode({
        filter = filter_name,
        total = #filtered_data,
        data = filtered_data
    })
end

-- netsrv_transform_data
-- 功能：数据格式转换
function netsrv_transform_data(keys, args)
    local transform_type = keys[1]  -- "json_to_csv", "json_to_xml", etc.
    local data = cjson.decode(args[1])
    local options = cjson.decode(args[2] or "{}")
    
    local result = ""
    
    if transform_type == "json_to_csv" then
        -- CSV转换
        local headers = {}
        local values = {}
        
        -- 提取标题
        for key, _ in pairs(data[1] or {}) do
            table.insert(headers, key)
        end
        
        -- 构建CSV
        result = table.concat(headers, ",") .. "\n"
        for _, row in ipairs(data) do
            local row_values = {}
            for _, header in ipairs(headers) do
                table.insert(row_values, tostring(row[header] or ""))
            end
            result = result .. table.concat(row_values, ",") .. "\n"
        end
        
    elseif transform_type == "json_to_binary" then
        -- 二进制格式转换
        local binary_data = {}
        for _, item in ipairs(data) do
            -- 简化的二进制编码示例
            for _, value in pairs(item) do
                if type(value) == "number" then
                    -- 存储数值的字节表示
                    table.insert(binary_data, string.format("%08x", value))
                end
            end
        end
        result = table.concat(binary_data, "")
        
    elseif transform_type == "aggregate" then
        -- 数据聚合
        local aggregated = {}
        for _, item in ipairs(data) do
            for key, value in pairs(item) do
                if type(value) == "number" then
                    aggregated[key] = (aggregated[key] or 0) + value
                end
            end
        end
        result = cjson.encode(aggregated)
    end
    
    -- 缓存转换结果
    local cache_key = "netsrv:transform:cache:" .. transform_type
    redis.call('SET', cache_key, result, 'EX', 300)  -- 缓存5分钟
    
    return result
end
```

### 7. hissrv（历史服务）- Python

#### Python 负责：
- **数据采集**
  - 定时从 Redis 采集
  - 数据聚合
  - 批量处理
- **存储管理**
  - InfluxDB 写入
  - 数据压缩
  - 过期清理
- **查询服务**
  - 时序查询
  - 统计分析
  - 数据导出

#### Redis 交互：
- 扫描并读取实时数据
- 写入采集状态
- 不需要 Lua Functions

## 性能考虑

### CPU 密集型（保留 Rust）
- 协议解析（CRC计算、字节操作）
- 数据压缩
- 加密解密
- 大批量数据处理

### I/O 密集型（可用 Python）
- 网络请求
- 数据库查询
- 文件操作
- API 服务

### 高频计算（使用 Lua）
- 实时计算
- 条件判断
- 数据聚合
- 原子操作

## 部署建议

1. **核心服务**（Rust）
   - comsrv: 必须高性能、低延迟
   - modsrv: 计算密集，保持 Rust
   - alarmsrv: 实时性要求高
   - rulesrv: 规则评估需要高效

2. **接口服务**（Python）
   - apigateway: I/O 密集，Python 足够
   - netsrv: 网络转发，Python 生态好
   - hissrv: 批处理为主，Python 合适

3. **Redis Lua**
   - 所有高频、原子性操作
   - 业务逻辑集中管理
   - 减少网络往返

## Lua Functions 总结

### 核心业务函数
| 函数名 | 服务 | 功能描述 | 调用频率 |
|--------|------|----------|----------|
| comsrv_write_data | comsrv | 原子写入通道数据 | 极高(1-10Hz) |
| comsrv_batch_write | comsrv | 批量数据写入 | 高 |
| comsrv_validate_data | comsrv | 数据质量校验 | 高 |
| modsrv_calculate | modsrv | 能效功率计算 | 高 |
| modsrv_efficiency | modsrv | 设备效率计算 | 中 |
| modsrv_statistics | modsrv | 统计分析 | 中 |
| modsrv_aggregate | modsrv | 数据聚合 | 中 |
| alarmsrv_check | alarmsrv | 告警条件检查 | 高 |
| alarmsrv_batch_check | alarmsrv | 批量告警检查 | 高 |
| alarmsrv_acknowledge | alarmsrv | 告警确认 | 低 |
| alarmsrv_suppress | alarmsrv | 告警抑制 | 低 |
| rulesrv_evaluate | rulesrv | 规则评估执行 | 中 |
| rulesrv_cascade | rulesrv | 级联规则处理 | 低 |
| rulesrv_schedule | rulesrv | 定时规则调度 | 中 |
| rulesrv_complex_condition | rulesrv | 复杂条件评估 | 中 |
| netsrv_collect_data | netsrv | 批量数据收集 | 中 |
| netsrv_forward_data | netsrv | 转发状态记录 | 中 |
| netsrv_filter_data | netsrv | 数据过滤 | 低 |
| netsrv_transform_data | netsrv | 格式转换 | 低 |

### 关键设计原则

1. **原子性保证**
   - 所有数据写入和状态更新在 Lua 中原子执行
   - 避免分布式事务和锁竞争

2. **性能优化**
   - 批量操作减少网络往返
   - 结果缓存避免重复计算
   - 自动清理过期数据

3. **级联触发**
   - comsrv 写入数据 → 触发 modsrv 计算
   - modsrv 计算结果 → 触发 alarmsrv 检查
   - alarmsrv 告警 → 触发 rulesrv 动作

4. **数据一致性**
   - 使用时间戳标记数据版本
   - 历史记录用于审计追踪
   - 状态机保证状态转换正确

## 架构优势总结

1. **混合架构优势**
   - Rust: 协议处理、I/O操作、系统接口
   - Lua: 业务逻辑、原子操作、高频计算
   - Python: API服务、数据转发、批处理

2. **性能特点**
   - 毫秒级数据处理延迟
   - 支持10Hz以上数据更新频率
   - 单Redis实例支持10万+点位

3. **可扩展性**
   - 插件式协议扩展
   - Lua函数热更新
   - 水平扩展Python服务

4. **运维友好**
   - 统一的Redis数据中心
   - 完整的监控指标
   - 灵活的配置管理