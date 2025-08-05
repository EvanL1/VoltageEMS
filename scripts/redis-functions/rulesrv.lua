#!lua name=rulesrv_engine

-- ========================================
-- RuleSrv Lua引擎
-- 规则引擎的高性能实现
-- ========================================

local cjson = require('cjson')

-- ==================== 工具函数 ====================

-- 获取数据源的值
local function get_source_value(source)
    local value = nil
    
    -- 处理不同类型的数据源
    if string.find(source, "^comsrv:") then
        -- comsrv数据格式：comsrv:channel:type:pointId
        -- 例如：comsrv:1001:T:1
        local channel, data_type, point_id = string.match(source, "comsrv:(%d+):([TSCA]):(%d+)")
        if channel and data_type and point_id then
            local hash_key = string.format("comsrv:%s:%s", channel, data_type)
            local raw_value = redis.call('HGET', hash_key, point_id)
            if raw_value then
                value = tonumber(raw_value) or raw_value
            end
        end
    elseif string.find(source, "%.") and not string.find(source, "^comsrv:") then
        -- 直接的Redis键（如battery.soc）
        local raw_value = redis.call('GET', source)
        if raw_value then
            value = tonumber(raw_value) or raw_value
        end
    else
        -- 其他格式，尝试作为hash处理
        local parts = {}
        for part in string.gmatch(source, "[^:]+") do
            table.insert(parts, part)
        end
        
        if #parts >= 2 then
            local hash_key = parts[1] .. ":" .. parts[2]
            local field = parts[3]
            if field then
                local raw_value = redis.call('HGET', hash_key, field)
                if raw_value then
                    value = tonumber(raw_value) or raw_value
                end
            end
        end
    end
    
    return value
end

-- 比较操作符
local function evaluate_comparison(left, op, right)
    -- 转换为数字（如果可能）
    local left_num = tonumber(left)
    local right_num = tonumber(right)
    
    if op == "==" then
        return left == right
    elseif op == "!=" then
        return left ~= right
    elseif op == ">" and left_num and right_num then
        return left_num > right_num
    elseif op == ">=" and left_num and right_num then
        return left_num >= right_num
    elseif op == "<" and left_num and right_num then
        return left_num < right_num
    elseif op == "<=" and left_num and right_num then
        return left_num <= right_num
    elseif op == "contains" and type(left) == "string" and type(right) == "string" then
        return string.find(left, right) ~= nil
    end
    
    return false
end

-- 评估单个条件
local function evaluate_condition(condition)
    local source_value = get_source_value(condition.source)
    
    if source_value == nil then
        return false
    end
    
    return evaluate_comparison(source_value, condition.op, condition.value)
end

-- 评估条件组
local function evaluate_condition_group(group)
    if not group.conditions or #group.conditions == 0 then
        return false
    end
    
    local results = {}
    for _, condition in ipairs(group.conditions) do
        table.insert(results, evaluate_condition(condition))
    end
    
    if group.logic == "AND" then
        for _, result in ipairs(results) do
            if not result then
                return false
            end
        end
        return true
    else -- OR
        for _, result in ipairs(results) do
            if result then
                return true
            end
        end
        return false
    end
end

-- 评估规则条件
local function evaluate_rule_conditions(rule)
    if not rule.condition_groups or #rule.condition_groups == 0 then
        return false
    end
    
    local results = {}
    for _, group in ipairs(rule.condition_groups) do
        table.insert(results, evaluate_condition_group(group))
    end
    
    if rule.condition_logic == "AND" then
        for _, result in ipairs(results) do
            if not result then
                return false
            end
        end
        return true
    else -- OR
        for _, result in ipairs(results) do
            if result then
                return true
            end
        end
        return false
    end
end

-- 执行动作
local function execute_action(action)
    local result = {
        action_type = action.action_type,
        success = false,
        error = nil
    }
    
    if action.action_type == "set_value" then
        -- 设置值动作
        local target = action.target
        local value = action.value
        
        if string.find(target, "^comsrv:") then
            -- comsrv格式
            local channel, data_type, point_id = string.match(target, "comsrv:(%d+):([TSCA]):(%d+)")
            if channel and data_type and point_id then
                local hash_key = string.format("comsrv:%s:%s", channel, data_type)
                redis.call('HSET', hash_key, point_id, value)
                result.success = true
            else
                result.error = "Invalid comsrv target format"
            end
        elseif string.find(target, "%.") then
            -- 直接的Redis键
            redis.call('SET', target, value)
            result.success = true
        else
            result.error = "Unsupported target format"
        end
        
    elseif action.action_type == "publish" then
        -- 发布消息动作
        local message = cjson.encode({
            rule_id = action.rule_id,
            topic = action.topic,
            payload = action.payload,
            timestamp = redis.call('TIME')[1]
        })
        redis.call('PUBLISH', action.topic, message)
        result.success = true
        
    elseif action.action_type == "device_control" then
        -- 设备控制动作
        local control_msg = cjson.encode({
            device_id = action.device_id,
            command = action.command,
            parameters = action.parameters,
            timestamp = redis.call('TIME')[1]
        })
        redis.call('PUBLISH', 'device:control:' .. action.device_id, control_msg)
        result.success = true
        
    elseif action.action_type == "notify" then
        -- 通知动作
        local notify_msg = cjson.encode({
            level = action.level,
            message = action.message,
            recipients = action.recipients,
            timestamp = redis.call('TIME')[1]
        })
        redis.call('PUBLISH', 'notification:' .. action.level, notify_msg)
        result.success = true
        
    else
        result.error = "Unknown action type: " .. action.action_type
    end
    
    return result
end

-- ==================== 规则管理函数 ====================

-- 创建或更新规则
local function rule_upsert(keys, args)
    local rule_id = keys[1]
    local rule_json = args[1]
    
    if not rule_id or not rule_json then
        return redis.error_reply("Rule ID and JSON required")
    end
    
    -- 验证JSON格式
    local ok, rule = pcall(cjson.decode, rule_json)
    if not ok then
        return redis.error_reply("Invalid JSON: " .. tostring(rule))
    end
    
    -- 验证规则结构
    if not rule.name or not rule.condition_groups or not rule.actions then
        return redis.error_reply("Rule must have name, condition_groups and actions")
    end
    
    -- 存储规则
    local rule_key = 'rulesrv:rule:' .. rule_id
    redis.call('SET', rule_key, rule_json)
    
    -- 更新索引
    redis.call('SADD', 'rulesrv:rules', rule_id)
    
    -- 如果规则启用，添加到活跃规则集
    if rule.enabled then
        redis.call('SADD', 'rulesrv:rules:active', rule_id)
    else
        redis.call('SREM', 'rulesrv:rules:active', rule_id)
    end
    
    -- 更新规则元数据
    local meta_key = 'rulesrv:rule:' .. rule_id .. ':meta'
    redis.call('HSET', meta_key, 
        'last_updated', redis.call('TIME')[1],
        'execution_count', 0,
        'last_triggered', 0
    )
    
    return redis.status_reply("OK")
end

-- 获取规则
local function rule_get(keys, args)
    local rule_id = keys[1]
    local rule_key = 'rulesrv:rule:' .. rule_id
    
    local rule_json = redis.call('GET', rule_key)
    if not rule_json then
        return redis.error_reply("Rule not found: " .. rule_id)
    end
    
    return rule_json
end

-- 删除规则
local function rule_delete(keys, args)
    local rule_id = keys[1]
    local rule_key = 'rulesrv:rule:' .. rule_id
    
    -- 删除规则
    redis.call('DEL', rule_key)
    redis.call('DEL', 'rulesrv:rule:' .. rule_id .. ':meta')
    redis.call('DEL', 'rulesrv:rule:' .. rule_id .. ':state')
    
    -- 从索引中移除
    redis.call('SREM', 'rulesrv:rules', rule_id)
    redis.call('SREM', 'rulesrv:rules:active', rule_id)
    
    return redis.status_reply("OK")
end

-- 列出规则
local function rule_list(keys, args)
    local filter = args[1] and cjson.decode(args[1]) or {}
    local rules = {}
    
    local rule_ids
    if filter.active_only then
        rule_ids = redis.call('SMEMBERS', 'rulesrv:rules:active')
    else
        rule_ids = redis.call('SMEMBERS', 'rulesrv:rules')
    end
    
    for _, rule_id in ipairs(rule_ids) do
        local rule_json = redis.call('GET', 'rulesrv:rule:' .. rule_id)
        if rule_json then
            local rule = cjson.decode(rule_json)
            -- 添加元数据
            local meta = redis.call('HGETALL', 'rulesrv:rule:' .. rule_id .. ':meta')
            if meta and #meta > 0 then
                rule.metadata = {}
                for i = 1, #meta, 2 do
                    rule.metadata[meta[i]] = meta[i + 1]
                end
            end
            table.insert(rules, rule)
        end
    end
    
    return cjson.encode(rules)
end

-- ==================== 规则执行函数 ====================

-- 执行单个规则
local function rule_execute(keys, args)
    local rule_id = keys[1]
    local force = args[1] == "true"
    
    -- 获取规则
    local rule_json = redis.call('GET', 'rulesrv:rule:' .. rule_id)
    if not rule_json then
        return redis.error_reply("Rule not found: " .. rule_id)
    end
    
    local rule = cjson.decode(rule_json)
    
    -- 检查规则是否启用
    if not rule.enabled and not force then
        return cjson.encode({
            rule_id = rule_id,
            executed = false,
            reason = "Rule is disabled"
        })
    end
    
    -- 检查冷却时间
    if rule.cooldown and rule.cooldown > 0 and not force then
        local state_key = 'rulesrv:rule:' .. rule_id .. ':state'
        local last_executed = redis.call('HGET', state_key, 'last_executed')
        if last_executed then
            local current_time = redis.call('TIME')[1]
            local time_diff = current_time - tonumber(last_executed)
            if time_diff < rule.cooldown then
                return cjson.encode({
                    rule_id = rule_id,
                    executed = false,
                    reason = "In cooldown period",
                    remaining_cooldown = rule.cooldown - time_diff
                })
            end
        end
    end
    
    -- 评估条件
    local conditions_met = evaluate_rule_conditions(rule)
    
    -- 更新执行计数
    redis.call('HINCRBY', 'rulesrv:rule:' .. rule_id .. ':meta', 'execution_count', 1)
    
    if not conditions_met then
        return cjson.encode({
            rule_id = rule_id,
            executed = false,
            reason = "Conditions not met"
        })
    end
    
    -- 执行动作
    local action_results = {}
    for _, action in ipairs(rule.actions) do
        action.rule_id = rule_id  -- 添加规则ID到动作中
        local result = execute_action(action)
        table.insert(action_results, result)
    end
    
    -- 更新状态
    local current_time = redis.call('TIME')[1]
    redis.call('HSET', 'rulesrv:rule:' .. rule_id .. ':state',
        'last_executed', current_time,
        'last_triggered', current_time
    )
    redis.call('HSET', 'rulesrv:rule:' .. rule_id .. ':meta',
        'last_triggered', current_time
    )
    
    -- 发布规则执行事件
    local event = {
        rule_id = rule_id,
        rule_name = rule.name,
        executed = true,
        timestamp = current_time,
        action_results = action_results
    }
    redis.call('PUBLISH', 'rulesrv:rule_executed', cjson.encode(event))
    
    return cjson.encode({
        rule_id = rule_id,
        executed = true,
        action_results = action_results,
        timestamp = current_time
    })
end

-- 批量执行规则
local function rule_execute_batch(keys, args)
    local results = {}
    local active_rules = redis.call('SMEMBERS', 'rulesrv:rules:active')
    
    for _, rule_id in ipairs(active_rules) do
        local result = rule_execute({rule_id}, {})
        table.insert(results, {
            rule_id = rule_id,
            result = cjson.decode(result)
        })
    end
    
    return cjson.encode({
        total_rules = #active_rules,
        results = results,
        timestamp = redis.call('TIME')[1]
    })
end

-- ==================== 规则状态和统计 ====================

-- 获取规则执行统计
local function rule_stats(keys, args)
    local rule_id = keys[1]
    
    if rule_id then
        -- 单个规则的统计
        local meta = redis.call('HGETALL', 'rulesrv:rule:' .. rule_id .. ':meta')
        local state = redis.call('HGETALL', 'rulesrv:rule:' .. rule_id .. ':state')
        
        local stats = {}
        for i = 1, #meta, 2 do
            stats[meta[i]] = meta[i + 1]
        end
        for i = 1, #state, 2 do
            stats[state[i]] = state[i + 1]
        end
        
        return cjson.encode(stats)
    else
        -- 所有规则的统计
        local total_rules = redis.call('SCARD', 'rulesrv:rules')
        local active_rules = redis.call('SCARD', 'rulesrv:rules:active')
        
        return cjson.encode({
            total_rules = total_rules,
            active_rules = active_rules,
            inactive_rules = total_rules - active_rules,
            timestamp = redis.call('TIME')[1]
        })
    end
end

-- 启用/禁用规则
local function rule_enable(keys, args)
    local rule_id = keys[1]
    local enabled = args[1] == "true"
    
    local rule_json = redis.call('GET', 'rulesrv:rule:' .. rule_id)
    if not rule_json then
        return redis.error_reply("Rule not found: " .. rule_id)
    end
    
    local rule = cjson.decode(rule_json)
    rule.enabled = enabled
    
    -- 更新规则
    redis.call('SET', 'rulesrv:rule:' .. rule_id, cjson.encode(rule))
    
    -- 更新活跃规则集
    if enabled then
        redis.call('SADD', 'rulesrv:rules:active', rule_id)
    else
        redis.call('SREM', 'rulesrv:rules:active', rule_id)
    end
    
    return redis.status_reply("OK")
end

-- 清除规则统计
local function rule_clear_stats(keys, args)
    local rule_id = keys[1]
    
    if rule_id then
        -- 清除单个规则的统计
        redis.call('HSET', 'rulesrv:rule:' .. rule_id .. ':meta',
            'execution_count', 0,
            'last_triggered', 0
        )
        redis.call('DEL', 'rulesrv:rule:' .. rule_id .. ':state')
    else
        -- 清除所有规则的统计
        local rule_ids = redis.call('SMEMBERS', 'rulesrv:rules')
        for _, id in ipairs(rule_ids) do
            redis.call('HSET', 'rulesrv:rule:' .. id .. ':meta',
                'execution_count', 0,
                'last_triggered', 0
            )
            redis.call('DEL', 'rulesrv:rule:' .. id .. ':state')
        end
    end
    
    return redis.status_reply("OK")
end

-- ==================== 注册函数 ====================

redis.register_function('rule_upsert', rule_upsert)
redis.register_function('rule_get', rule_get)
redis.register_function('rule_delete', rule_delete)
redis.register_function('rule_list', rule_list)
redis.register_function('rule_execute', rule_execute)
redis.register_function('rule_execute_batch', rule_execute_batch)
redis.register_function('rule_stats', rule_stats)
redis.register_function('rule_enable', rule_enable)
redis.register_function('rule_clear_stats', rule_clear_stats)