#!lua name=rule_engine_v3

-- ========================================
-- VoltageEMS 完整规则引擎 V3
-- 生产级Lua实现
-- ========================================

-- ==================== 核心规则执行 ====================

-- 执行单个规则
local function rule_execute(keys, args)
    local rule_id = keys[1]
    
    -- 获取规则
    local rule_key = 'rulesrv:rule:' .. rule_id
    local rule_json = redis.call('GET', rule_key)
    if not rule_json then
        return redis.error_reply("Rule not found: " .. rule_id)
    end
    
    -- 解析规则基本信息
    local enabled = string.find(rule_json, '"enabled"%s*:%s*true') ~= nil
    if not enabled then
        return cjson.encode({
            rule_id = rule_id,
            success = true,
            conditions_met = false,
            message = "Rule is disabled"
        })
    end
    
    -- 检查冷却时间
    local cooldown_seconds = tonumber(string.match(rule_json, '"cooldown_seconds"%s*:%s*(%d+)'))
    if cooldown_seconds then
        local cooldown_key = 'rulesrv:cooldown:' .. rule_id
        if redis.call('EXISTS', cooldown_key) == 1 then
            local ttl = redis.call('TTL', cooldown_key)
            return cjson.encode({
                rule_id = rule_id,
                success = true,
                conditions_met = false,
                message = "Rule in cooldown, " .. ttl .. " seconds remaining"
            })
        end
    end
    
    -- 评估条件
    local conditions_met = evaluate_rule_conditions(rule_json)
    
    local actions_executed = {}
    if conditions_met then
        -- 执行动作
        actions_executed = execute_rule_actions(rule_json, rule_id)
        
        -- 设置冷却时间
        if cooldown_seconds then
            redis.call('SETEX', 'rulesrv:cooldown:' .. rule_id, cooldown_seconds, '1')
        end
    end
    
    -- 记录执行结果
    local execution_id = redis.call('INCR', 'rulesrv:execution:counter')
    local timestamp = redis.call('TIME')
    local result = {
        rule_id = rule_id,
        execution_id = tostring(execution_id),
        timestamp = timestamp[1] .. "." .. timestamp[2],
        conditions_met = conditions_met,
        actions_executed = actions_executed,
        success = true
    }
    
    -- 存储执行历史
    local result_key = 'rulesrv:execution:' .. execution_id
    redis.call('SETEX', result_key, 86400, cjson.encode(result))  -- 保存24小时
    
    -- 更新规则统计
    local stats_key = 'rulesrv:rule:' .. rule_id .. ':stats'
    redis.call('HINCRBY', stats_key, 'total_executions', 1)
    if conditions_met then
        redis.call('HINCRBY', stats_key, 'conditions_met_count', 1)
    end
    redis.call('HSET', stats_key, 'last_execution', timestamp[1])
    
    return cjson.encode(result)
end

-- 评估规则条件
local function evaluate_rule_conditions(rule_json)
    -- 提取条件组
    local conditions_start = string.find(rule_json, '"conditions"%s*:%s*{')
    if not conditions_start then
        return false
    end
    
    -- 获取逻辑运算符 (AND/OR)
    local logic_op = string.match(rule_json, '"operator"%s*:%s*"([^"]+)"')
    if not logic_op then
        logic_op = "AND"
    end
    
    -- 查找所有条件
    local conditions = {}
    for cond in string.gmatch(rule_json, '{%s*"source"%s*:%s*"([^"]+)"%s*,%s*"operator"%s*:%s*"([^"]+)"%s*,%s*"value"%s*:%s*([^}]+)') do
        local source, operator, value = string.match(cond, '([^"]+)"[^"]*"([^"]+)"[^%d%-]*([%d%.%-]+)')
        if source and operator and value then
            table.insert(conditions, {
                source = source,
                operator = operator,
                value = tonumber(value) or value
            })
        end
    end
    
    -- 简化的条件提取（适用于V3格式）
    local iter = string.gmatch(rule_json, '"source"%s*:%s*"([^"]+)"%s*,%s*"operator"%s*:%s*"([^"]+)"%s*,%s*"value"%s*:%s*([^,}]+)')
    for source, operator, value_str in iter do
        -- 清理value字符串
        value_str = string.gsub(value_str, '%s+', '')
        local value = tonumber(value_str) or value_str
        
        local result = evaluate_single_condition(source, operator, value)
        
        -- 短路评估
        if logic_op == "AND" and not result then
            return false
        elseif logic_op == "OR" and result then
            return true
        end
    end
    
    -- 返回最终结果
    return logic_op == "AND"
end

-- 评估单个条件
local function evaluate_single_condition(source, operator, compare_value)
    -- 获取源值
    local source_value = nil
    
    -- 处理不同的source格式
    if string.find(source, ":") and string.find(source, "%.") then
        -- Hash field格式: comsrv:1001:T.1
        local hash_key, field = string.match(source, "([^.]+)%.(.+)")
        if hash_key and field then
            local val = redis.call('HGET', hash_key, field)
            source_value = val and tonumber(val) or val
        end
    else
        -- 普通key格式
        local val = redis.call('GET', source)
        source_value = val and tonumber(val) or val
    end
    
    if source_value == nil then
        return false
    end
    
    -- 比较操作
    local source_num = tonumber(source_value)
    local compare_num = tonumber(compare_value)
    
    if operator == ">" and source_num and compare_num then
        return source_num > compare_num
    elseif operator == ">=" and source_num and compare_num then
        return source_num >= compare_num
    elseif operator == "<" and source_num and compare_num then
        return source_num < compare_num
    elseif operator == "<=" and source_num and compare_num then
        return source_num <= compare_num
    elseif operator == "==" then
        return source_value == compare_value or (source_num and compare_num and source_num == compare_num)
    elseif operator == "!=" then
        return source_value ~= compare_value
    elseif operator == "contains" then
        return string.find(tostring(source_value), tostring(compare_value)) ~= nil
    end
    
    return false
end

-- 执行规则动作
local function execute_rule_actions(rule_json, rule_id)
    local actions_executed = {}
    local action_index = 0
    
    -- 简化的动作提取
    for action_block in string.gmatch(rule_json, '"action_type"%s*:%s*"([^"]+)"[^{]*{([^}]+)}') do
        local action_type, config = string.match(action_block, '([^"]+)"[^{]*{([^}]+)')
        if action_type then
            action_index = action_index + 1
            local result = execute_single_action(action_type, config, rule_id)
            table.insert(actions_executed, "action_" .. (action_index - 1) .. ": " .. result)
        end
    end
    
    -- 更完整的动作解析
    local actions_start = string.find(rule_json, '"actions"%s*:%s*%[')
    if actions_start then
        local actions_section = string.sub(rule_json, actions_start)
        
        -- 解析notify动作
        for msg in string.gmatch(actions_section, '"action_type"%s*:%s*"notify".-"message"%s*:%s*"([^"]+)"') do
            action_index = action_index + 1
            local result = execute_notify_action(msg, rule_id)
            table.insert(actions_executed, "action_" .. (action_index - 1) .. ": " .. result)
        end
        
        -- 解析set_value动作
        for key, value in string.gmatch(actions_section, '"action_type"%s*:%s*"set_value".-"key"%s*:%s*"([^"]+)".-"value"%s*:%s*([^,}]+)') do
            action_index = action_index + 1
            local result = execute_set_value_action(key, value)
            table.insert(actions_executed, "action_" .. (action_index - 1) .. ": " .. result)
        end
    end
    
    return actions_executed
end

-- 执行通知动作
local function execute_notify_action(message, rule_id)
    local timestamp = redis.call('TIME')
    local notification = string.format(
        '{"timestamp":%s,"level":"info","message":"%s","source":"rule_engine","rule_id":"%s"}',
        timestamp[1], message, rule_id
    )
    redis.call('PUBLISH', 'ems:notifications', notification)
    return "Notification sent: " .. message
end

-- 执行设置值动作
local function execute_set_value_action(key, value)
    redis.call('SET', key, value)
    return "Set value: " .. key .. " = " .. tostring(value)
end

-- 执行单个动作（通用）
local function execute_single_action(action_type, config, rule_id)
    if action_type == "notify" then
        local message = string.match(config, '"message"%s*:%s*"([^"]+)"')
        if message then
            return execute_notify_action(message, rule_id)
        end
    elseif action_type == "set_value" then
        local key = string.match(config, '"key"%s*:%s*"([^"]+)"')
        local value = string.match(config, '"value"%s*:%s*([^,}]+)')
        if key and value then
            return execute_set_value_action(key, value)
        end
    elseif action_type == "device_control" then
        -- 实现设备控制
        local device_id = string.match(config, '"device_id"%s*:%s*"([^"]+)"')
        local point = string.match(config, '"point"%s*:%s*"([^"]+)"')
        local value = string.match(config, '"value"%s*:%s*([^,}]+)')
        if device_id and point and value then
            local cmd_id = "cmd_" .. redis.call('INCR', 'ems:control:cmd:counter')
            -- 存储控制命令
            local cmd_key = "ems:control:cmd:" .. cmd_id
            redis.call('SET', cmd_key, config)
            redis.call('PUBLISH', 'ems:control:queue', cmd_id)
            return "Device control queued: " .. cmd_id
        end
    end
    
    return "Unknown action: " .. action_type
end

-- ==================== 规则管理 ====================

-- 创建或更新规则
local function rule_upsert(keys, args)
    local rule_json = args[1]
    if not rule_json then
        return redis.error_reply("Rule JSON required")
    end
    
    -- 提取rule ID
    local rule_id = string.match(rule_json, '"id"%s*:%s*"([^"]+)"')
    if not rule_id then
        return redis.error_reply("Rule ID not found")
    end
    
    -- 存储规则
    local rule_key = 'rulesrv:rule:' .. rule_id
    redis.call('SET', rule_key, rule_json)
    
    -- 更新索引
    redis.call('SADD', 'rulesrv:rules', rule_id)
    
    -- 如果规则启用，添加到启用列表
    if string.find(rule_json, '"enabled"%s*:%s*true') then
        redis.call('SADD', 'rulesrv:rules:enabled', rule_id)
    else
        redis.call('SREM', 'rulesrv:rules:enabled', rule_id)
    end
    
    return redis.status_reply("OK")
end

-- 删除规则
local function rule_delete(keys, args)
    local rule_id = keys[1]
    
    -- 删除规则
    local rule_key = 'rulesrv:rule:' .. rule_id
    redis.call('DEL', rule_key)
    
    -- 删除统计
    redis.call('DEL', 'rulesrv:rule:' .. rule_id .. ':stats')
    
    -- 从索引中移除
    redis.call('SREM', 'rulesrv:rules', rule_id)
    redis.call('SREM', 'rulesrv:rules:enabled', rule_id)
    
    -- 删除冷却时间
    redis.call('DEL', 'rulesrv:cooldown:' .. rule_id)
    
    return redis.status_reply("OK")
end

-- 获取规则
local function rule_get(keys, args)
    local rule_id = keys[1]
    local rule_key = 'rulesrv:rule:' .. rule_id
    
    local rule_json = redis.call('GET', rule_key)
    if not rule_json then
        return redis.error_reply("Rule not found")
    end
    
    -- 获取统计信息
    local stats_key = 'rulesrv:rule:' .. rule_id .. ':stats'
    local stats = redis.call('HGETALL', stats_key)
    
    -- 构建响应
    local response = {
        rule = rule_json,
        stats = stats
    }
    
    return cjson.encode(response)
end

-- 列出规则
local function rule_list(keys, args)
    local filter = args[1] and cjson.decode(args[1]) or {}
    local rules = {}
    
    -- 获取规则列表
    local rule_set = filter.enabled and 'rulesrv:rules:enabled' or 'rulesrv:rules'
    local rule_ids = redis.call('SMEMBERS', rule_set)
    
    for _, rule_id in ipairs(rule_ids) do
        local rule_json = redis.call('GET', 'rulesrv:rule:' .. rule_id)
        if rule_json then
            table.insert(rules, rule_json)
        end
    end
    
    return cjson.encode(rules)
end

-- ==================== 批量执行 ====================

-- 批量执行所有启用的规则
local function rules_execute_all(keys, args)
    local results = {}
    local rule_ids = redis.call('SMEMBERS', 'rulesrv:rules:enabled')
    
    for _, rule_id in ipairs(rule_ids) do
        local result = rule_execute({rule_id}, {})
        table.insert(results, cjson.decode(result))
    end
    
    return cjson.encode({
        total = #rule_ids,
        executed = #results,
        results = results
    })
end

-- 按优先级执行规则
local function rules_execute_by_priority(keys, args)
    local results = {}
    
    -- 获取所有启用的规则
    local rule_ids = redis.call('SMEMBERS', 'rulesrv:rules:enabled')
    local rules_with_priority = {}
    
    -- 获取规则优先级
    for _, rule_id in ipairs(rule_ids) do
        local rule_json = redis.call('GET', 'rulesrv:rule:' .. rule_id)
        if rule_json then
            local priority = tonumber(string.match(rule_json, '"priority"%s*:%s*(%d+)')) or 0
            table.insert(rules_with_priority, {id = rule_id, priority = priority})
        end
    end
    
    -- 按优先级排序（高优先级先执行）
    table.sort(rules_with_priority, function(a, b) return a.priority > b.priority end)
    
    -- 执行规则
    for _, rule in ipairs(rules_with_priority) do
        local result = rule_execute({rule.id}, {})
        table.insert(results, cjson.decode(result))
    end
    
    return cjson.encode(results)
end

-- ==================== 工具函数 ====================

-- 清理过期数据
local function rule_cleanup(keys, args)
    local days = tonumber(args[1]) or 7
    local cutoff = redis.call('TIME')[1] - (days * 86400)
    local cleaned = 0
    
    -- 清理执行历史
    local exec_keys = redis.call('KEYS', 'rulesrv:execution:*')
    for _, key in ipairs(exec_keys) do
        local ttl = redis.call('TTL', key)
        if ttl < 0 or ttl > days * 86400 then
            redis.call('DEL', key)
            cleaned = cleaned + 1
        end
    end
    
    return redis.status_reply("Cleaned " .. cleaned .. " records")
end

-- ==================== 注册函数 ====================

redis.register_function('rule_execute', rule_execute)
redis.register_function('rule_upsert', rule_upsert)
redis.register_function('rule_delete', rule_delete)
redis.register_function('rule_get', rule_get)
redis.register_function('rule_list', rule_list)
redis.register_function('rules_execute_all', rules_execute_all)
redis.register_function('rules_execute_by_priority', rules_execute_by_priority)
redis.register_function('rule_cleanup', rule_cleanup)