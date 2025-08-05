#!lua name=rule_engine

-- ========================================
-- 纯Lua实现的规则引擎
-- 完全在Redis内部执行，无需Rust
-- ========================================

-- 简单的JSON解析（仅支持基础功能）
local function json_decode(str)
    -- 这是一个极简的JSON解析器
    -- 在生产环境中，应该使用Redis内置的cjson
    return load("return " .. str:gsub('"%s*:%s*', '='):gsub('{', '{['):gsub('}', ']}'):gsub('%[,', '[nil,'))()
end

-- 执行单个规则
local function execute_rule(keys, args)
    local rule_id = keys[1]
    local context = args[1] and cjson.decode(args[1]) or {}
    
    -- 获取规则
    local rule_key = 'rulesrv:rule:' .. rule_id
    local rule_json = redis.call('GET', rule_key)
    if not rule_json then
        return cjson.encode({
            success = false,
            error = "Rule not found: " .. rule_id
        })
    end
    
    local rule = cjson.decode(rule_json)
    
    -- 检查规则是否启用
    if not rule.enabled then
        return cjson.encode({
            success = false,
            error = "Rule is disabled"
        })
    end
    
    -- 检查冷却时间
    local cooldown_key = 'rulesrv:cooldown:' .. rule_id
    if rule.cooldown_seconds and redis.call('EXISTS', cooldown_key) == 1 then
        local ttl = redis.call('TTL', cooldown_key)
        return cjson.encode({
            success = true,
            conditions_met = false,
            message = "Rule in cooldown, " .. ttl .. " seconds remaining"
        })
    end
    
    -- 评估条件
    local conditions_met = evaluate_conditions(rule.conditions)
    
    local actions_executed = {}
    if conditions_met then
        -- 执行动作
        for i, action in ipairs(rule.actions) do
            local result = execute_action(action, rule_id)
            table.insert(actions_executed, "action_" .. (i-1) .. ": " .. result)
        end
        
        -- 设置冷却时间
        if rule.cooldown_seconds then
            redis.call('SETEX', cooldown_key, rule.cooldown_seconds, '1')
        end
    end
    
    -- 记录执行结果
    local execution_id = redis.call('INCR', 'rulesrv:execution:counter')
    local result = {
        rule_id = rule_id,
        execution_id = tostring(execution_id),
        timestamp = os.date("!%Y-%m-%dT%H:%M:%SZ"),
        conditions_met = conditions_met,
        actions_executed = actions_executed,
        success = true,
        duration_ms = 0  -- Lua中难以精确测量
    }
    
    -- 存储执行结果
    local result_key = 'rulesrv:execution:' .. execution_id
    redis.call('SETEX', result_key, 86400, cjson.encode(result))  -- 保存24小时
    
    -- 更新统计信息
    local stats_key = 'rulesrv:rule:' .. rule_id .. ':stats'
    redis.call('HSET', stats_key, 
        'last_execution', result.timestamp,
        'last_result', tostring(conditions_met),
        'conditions_met', tostring(conditions_met)
    )
    
    return cjson.encode(result)
end

-- 评估条件组
function evaluate_conditions(condition_group)
    local operator = condition_group.operator
    local conditions = condition_group.conditions
    
    for _, condition in ipairs(conditions) do
        local result = evaluate_condition(condition)
        
        -- 短路评估
        if operator == "AND" and not result then
            return false
        elseif operator == "OR" and result then
            return true
        end
    end
    
    -- 最终结果
    if operator == "AND" then
        return true  -- 所有条件都为真
    else
        return false  -- 没有条件为真
    end
end

-- 评估单个条件
function evaluate_condition(condition)
    local source_value = get_source_value(condition.source)
    if source_value == nil then
        return false
    end
    
    return compare_values(source_value, condition.operator, condition.value)
end

-- 获取数据源的值
function get_source_value(source)
    -- 处理不同的数据源格式
    if string.find(source, "%.") and not string.find(source, "^comsrv:") then
        -- 直接的Redis key（如 battery.soc）
        local value = redis.call('GET', source)
        return value and tonumber(value) or value
    elseif string.find(source, "^comsrv:") and string.find(source, "%.") then
        -- Hash field格式（如 comsrv:1001:T.10001）
        local parts = {}
        for part in string.gmatch(source, "([^.]+)") do
            table.insert(parts, part)
        end
        
        if #parts == 2 then
            local hash_key = parts[1]
            local field = parts[2]
            local value = redis.call('HGET', hash_key, field)
            return value and tonumber(value) or value
        end
    else
        -- 直接的Redis key
        local value = redis.call('GET', source)
        return value and tonumber(value) or value
    end
    
    return nil
end

-- 比较值
function compare_values(left, operator, right)
    -- 尝试转换为数字
    local left_num = tonumber(left)
    local right_num = tonumber(right)
    
    if operator == "==" then
        return left == right or (left_num and right_num and left_num == right_num)
    elseif operator == "!=" then
        return left ~= right
    elseif operator == "contains" then
        return string.find(tostring(left), tostring(right)) ~= nil
    elseif left_num and right_num then
        -- 数值比较
        if operator == ">" then
            return left_num > right_num
        elseif operator == ">=" then
            return left_num >= right_num
        elseif operator == "<" then
            return left_num < right_num
        elseif operator == "<=" then
            return left_num <= right_num
        end
    end
    
    return false
end

-- 执行动作
function execute_action(action, rule_id)
    local action_type = action.action_type
    local config = action.config
    
    if action_type == "notify" then
        -- 发送通知
        local notification = {
            timestamp = os.date("!%Y-%m-%dT%H:%M:%SZ"),
            level = config.level,
            message = config.message,
            source = "rule_engine",
            rule_id = rule_id
        }
        redis.call('PUBLISH', 'ems:notifications', cjson.encode(notification))
        return "Notification sent: " .. config.message
        
    elseif action_type == "set_value" then
        -- 设置值
        redis.call('SET', config.key, cjson.encode(config.value))
        if config.ttl then
            redis.call('EXPIRE', config.key, config.ttl)
        end
        return "Set value: " .. config.key .. " = " .. tostring(config.value)
        
    elseif action_type == "publish" then
        -- 发布消息
        redis.call('PUBLISH', config.channel, config.message)
        return "Published to channel: " .. config.channel
        
    elseif action_type == "device_control" then
        -- 设备控制
        local cmd_id = "cmd_" .. redis.call('INCR', 'ems:control:cmd:counter')
        local command = {
            id = cmd_id,
            timestamp = os.date("!%Y-%m-%dT%H:%M:%SZ"),
            target = {
                device_id = config.device_id,
                channel = config.channel
            },
            operation = "write",
            parameters = {
                point = config.point,
                value = config.value
            },
            status = "pending",
            source = "rule_engine",
            timeout = 30,
            priority = 1
        }
        
        local cmd_key = "ems:control:cmd:" .. cmd_id
        redis.call('SET', cmd_key, cjson.encode(command))
        redis.call('PUBLISH', 'ems:control:queue', cmd_id)
        
        return "Device control command queued: " .. cmd_id
    end
    
    return "Unknown action type: " .. action_type
end

-- 批量执行规则
local function execute_rules_batch(keys, args)
    local results = {}
    
    -- 获取所有启用的规则
    local rule_keys = redis.call('KEYS', 'rulesrv:rule:*')
    
    for _, rule_key in ipairs(rule_keys) do
        local rule_json = redis.call('GET', rule_key)
        if rule_json then
            local rule = cjson.decode(rule_json)
            if rule.enabled then
                -- 提取rule_id
                local rule_id = string.match(rule_key, "rulesrv:rule:(.+)")
                local result = execute_rule({rule_id}, args)
                table.insert(results, cjson.decode(result))
            end
        end
    end
    
    return cjson.encode({
        executed = #results,
        results = results
    })
end

-- 注册函数
redis.register_function('rule_execute', execute_rule)
redis.register_function('rules_execute_batch', execute_rules_batch)

-- 示例：创建测试规则的函数
local function create_test_rule(keys, args)
    local rule = {
        id = "lua_test_rule",
        name = "Lua Test Rule",
        description = "Test rule implemented in Lua",
        conditions = {
            operator = "AND",
            conditions = {
                {
                    source = "test_value",
                    operator = ">",
                    value = 50
                }
            }
        },
        actions = {
            {
                action_type = "notify",
                config = {
                    level = "info",
                    message = "Test value is greater than 50!"
                }
            }
        },
        enabled = true,
        priority = 1,
        cooldown_seconds = 60
    }
    
    redis.call('SET', 'rulesrv:rule:' .. rule.id, cjson.encode(rule))
    return "Test rule created"
end

redis.register_function('rule_create_test', create_test_rule)