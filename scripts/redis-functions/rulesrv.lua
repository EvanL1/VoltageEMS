#!lua name=rule_engine

-- ========================================
-- RuleSrv Lua引擎 (精简版)
-- 规则管理和执行的核心功能
-- ========================================

-- ==================== 规则管理 ====================

-- 创建或更新规则
local function rulesrv_upsert_rule(keys, args)
    local rule_id = keys[1]
    local rule_json = args[1]
    
    if not rule_id or not rule_json then
        return redis.error_reply("Rule ID and JSON required")
    end
    
    -- 验证JSON格式
    local ok, rule = pcall(cjson.decode, rule_json)
    if not ok then
        return redis.error_reply("Invalid JSON")
    end
    
    -- 设置默认值
    rule.id = rule_id
    rule.enabled = rule.enabled ~= false  -- 默认启用
    rule.created_at = rule.created_at or redis.call('TIME')[1]
    rule.updated_at = redis.call('TIME')[1]
    
    -- 存储规则
    local rule_key = 'rulesrv:rule:' .. rule_id
    redis.call('SET', rule_key, cjson.encode(rule))
    
    -- 添加到索引
    redis.call('SADD', 'rulesrv:rules', rule_id)
    if rule.enabled then
        redis.call('SADD', 'rulesrv:rules:enabled', rule_id)
    end
    
    return redis.status_reply("OK")
end

-- 获取规则
local function rulesrv_get_rule(keys, args)
    local rule_id = keys[1]
    local rule_key = 'rulesrv:rule:' .. rule_id
    
    local rule_json = redis.call('GET', rule_key)
    if not rule_json then
        return cjson.encode({error = "Rule not found"})
    end
    
    return rule_json
end

-- 删除规则
local function rulesrv_delete_rule(keys, args)
    local rule_id = keys[1]
    local rule_key = 'rulesrv:rule:' .. rule_id
    
    -- 清理索引
    redis.call('SREM', 'rulesrv:rules', rule_id)
    redis.call('SREM', 'rulesrv:rules:enabled', rule_id)
    
    -- 删除规则
    redis.call('DEL', rule_key)
    
    return redis.status_reply("OK")
end

-- 列出规则
local function rulesrv_list_rules(keys, args)
    local rule_ids = redis.call('SMEMBERS', 'rulesrv:rules')
    local rules = {}
    
    for _, rule_id in ipairs(rule_ids) do
        local rule_json = redis.call('GET', 'rulesrv:rule:' .. rule_id)
        if rule_json then
            table.insert(rules, cjson.decode(rule_json))
        end
    end
    
    return cjson.encode(rules)
end

-- 启用规则
local function rulesrv_enable_rule(keys, args)
    local rule_id = keys[1]
    local rule_key = 'rulesrv:rule:' .. rule_id
    
    local rule_json = redis.call('GET', rule_key)
    if not rule_json then
        return redis.error_reply("Rule not found")
    end
    
    local rule = cjson.decode(rule_json)
    rule.enabled = true
    rule.updated_at = redis.call('TIME')[1]
    
    redis.call('SET', rule_key, cjson.encode(rule))
    redis.call('SADD', 'rulesrv:rules:enabled', rule_id)
    
    return redis.status_reply("OK")
end

-- 禁用规则
local function rulesrv_disable_rule(keys, args)
    local rule_id = keys[1]
    local rule_key = 'rulesrv:rule:' .. rule_id
    
    local rule_json = redis.call('GET', rule_key)
    if not rule_json then
        return redis.error_reply("Rule not found")
    end
    
    local rule = cjson.decode(rule_json)
    rule.enabled = false
    rule.updated_at = redis.call('TIME')[1]
    
    redis.call('SET', rule_key, cjson.encode(rule))
    redis.call('SREM', 'rulesrv:rules:enabled', rule_id)
    
    return redis.status_reply("OK")
end

-- ==================== 规则执行 ====================

-- 简单条件评估
local function evaluate_simple_condition(source, operator, target)
    -- 获取源值
    local value = nil
    if string.find(source, "^comsrv:") then
        -- comsrv格式: comsrv:channel:type:point
        local parts = {}
        for part in string.gmatch(source, "[^:]+") do
            table.insert(parts, part)
        end
        if #parts == 4 then
            local key = parts[1] .. ":" .. parts[2] .. ":" .. parts[3]
            value = redis.call('HGET', key, parts[4])
        end
    else
        -- 直接Redis键
        value = redis.call('GET', source)
    end
    
    if not value then return false end
    
    -- 转换类型
    local num_value = tonumber(value)
    local num_target = tonumber(target)
    
    -- 比较
    if operator == ">" and num_value and num_target then
        return num_value > num_target
    elseif operator == ">=" and num_value and num_target then
        return num_value >= num_target
    elseif operator == "<" and num_value and num_target then
        return num_value < num_target
    elseif operator == "<=" and num_value and num_target then
        return num_value <= num_target
    elseif operator == "==" then
        return value == target
    elseif operator == "!=" then
        return value ~= target
    end
    
    return false
end

-- 执行批量规则
local function rulesrv_execute_batch(keys, args)
    local batch_id = keys[1]
    local batch_size = tonumber(args[1]) or 100
    
    -- 获取启用的规则
    local rule_ids = redis.call('SMEMBERS', 'rulesrv:rules:enabled')
    local rules_executed = 0
    local rules_triggered = 0
    local execution_log = {}
    
    for _, rule_id in ipairs(rule_ids) do
        if rules_executed >= batch_size then break end
        
        local rule_json = redis.call('GET', 'rulesrv:rule:' .. rule_id)
        if rule_json then
            local ok, rule = pcall(cjson.decode, rule_json)
            if ok and rule.condition then
                -- 简单条件评估
                local triggered = false
                if rule.condition.source and rule.condition.operator and rule.condition.target then
                    triggered = evaluate_simple_condition(
                        rule.condition.source,
                        rule.condition.operator,
                        rule.condition.target
                    )
                end
                
                if triggered then
                    rules_triggered = rules_triggered + 1
                    
                    -- 执行动作
                    if rule.action then
                        if rule.action.type == "set" and rule.action.target and rule.action.value then
                            redis.call('SET', rule.action.target, rule.action.value)
                        elseif rule.action.type == "publish" and rule.action.channel and rule.action.message then
                            redis.call('PUBLISH', rule.action.channel, rule.action.message)
                        end
                    end
                    
                    table.insert(execution_log, {
                        rule_id = rule_id,
                        triggered = true,
                        timestamp = redis.call('TIME')[1]
                    })
                end
            end
            rules_executed = rules_executed + 1
        end
    end
    
    -- 保存执行历史
    local history_key = 'rulesrv:execution:' .. batch_id
    redis.call('SET', history_key, cjson.encode({
        batch_id = batch_id,
        rules_executed = rules_executed,
        rules_triggered = rules_triggered,
        timestamp = redis.call('TIME')[1],
        log = execution_log
    }))
    redis.call('EXPIRE', history_key, 3600)  -- 保留1小时
    redis.call('LPUSH', 'rulesrv:executions', batch_id)
    redis.call('LTRIM', 'rulesrv:executions', 0, 99)  -- 只保留最近100次
    
    return cjson.encode({
        rules_executed = rules_executed,
        rules_triggered = rules_triggered
    })
end

-- 列出执行历史
local function rulesrv_list_executions(keys, args)
    local limit = tonumber(keys[1]) or 10
    local batch_ids = redis.call('LRANGE', 'rulesrv:executions', 0, limit - 1)
    local executions = {}
    
    for _, batch_id in ipairs(batch_ids) do
        local history_json = redis.call('GET', 'rulesrv:execution:' .. batch_id)
        if history_json then
            table.insert(executions, cjson.decode(history_json))
        end
    end
    
    return cjson.encode(executions)
end

-- 获取统计信息
local function rulesrv_get_statistics(keys, args)
    local total_rules = redis.call('SCARD', 'rulesrv:rules')
    local enabled_rules = redis.call('SCARD', 'rulesrv:rules:enabled')
    local recent_executions = redis.call('LLEN', 'rulesrv:executions')
    
    -- 获取最近执行信息
    local last_execution = nil
    local last_batch_id = redis.call('LINDEX', 'rulesrv:executions', 0)
    if last_batch_id then
        local history_json = redis.call('GET', 'rulesrv:execution:' .. last_batch_id)
        if history_json then
            last_execution = cjson.decode(history_json)
        end
    end
    
    return cjson.encode({
        total_rules = total_rules,
        enabled_rules = enabled_rules,
        disabled_rules = total_rules - enabled_rules,
        recent_executions = recent_executions,
        last_execution = last_execution,
        timestamp = redis.call('TIME')[1]
    })
end

-- ==================== 注册函数 ====================

redis.register_function('rulesrv_upsert_rule', rulesrv_upsert_rule)
redis.register_function('rulesrv_get_rule', rulesrv_get_rule)
redis.register_function('rulesrv_delete_rule', rulesrv_delete_rule)
redis.register_function('rulesrv_list_rules', rulesrv_list_rules)
redis.register_function('rulesrv_enable_rule', rulesrv_enable_rule)
redis.register_function('rulesrv_disable_rule', rulesrv_disable_rule)

redis.register_function('rulesrv_execute_batch', rulesrv_execute_batch)
redis.register_function('rulesrv_list_executions', rulesrv_list_executions)
redis.register_function('rulesrv_get_statistics', rulesrv_get_statistics)