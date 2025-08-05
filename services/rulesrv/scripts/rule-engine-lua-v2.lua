#!lua name=rule_engine

-- ========================================
-- 纯Lua实现的规则引擎 V2
-- 使用Redis 7+内置的cjson
-- ========================================

-- 执行单个规则的简化版本
local function execute_rule(keys, args)
    local rule_id = keys[1]
    
    -- 获取规则
    local rule_key = 'rulesrv:rule:' .. rule_id
    local rule_json = redis.call('GET', rule_key)
    if not rule_json then
        return redis.error_reply("Rule not found: " .. rule_id)
    end
    
    -- 简单的JSON解析示例
    -- 检查规则是否包含enabled=true
    if not string.find(rule_json, '"enabled"%s*:%s*true') then
        return redis.status_reply("Rule is disabled")
    end
    
    -- 简化的条件检查示例
    -- 查找source和value
    local source = string.match(rule_json, '"source"%s*:%s*"([^"]+)"')
    -- 查找conditions数组中的operator，而不是顶层的operator
    local operator = nil
    local _, cond_start = string.find(rule_json, '"conditions"%s*:%s*%[%s*{')
    if cond_start then
        local cond_section = string.sub(rule_json, cond_start)
        operator = string.match(cond_section, '"operator"%s*:%s*"([^"]+)"')
    else
        operator = string.match(rule_json, '"operator"%s*:%s*"([^"]+)"')
    end
    local compare_value = tonumber(string.match(rule_json, '"value"%s*:%s*([%d%.]+)')) or 0
    
    if not source then
        return redis.error_reply("No source found in rule")
    end
    
    -- 获取源数据值
    local source_value = nil
    if string.find(source, "%.") and not string.find(source, "^comsrv:") then
        -- 直接的Redis key（如 battery.soc）
        local value = redis.call('GET', source)
        source_value = tonumber(value)
    else
        -- 其他格式
        local value = redis.call('GET', source)
        source_value = tonumber(value)
    end
    
    if not source_value then
        return redis.status_reply("Source value not found: " .. source)
    end
    
    -- 简化的比较
    local conditions_met = false
    if operator == ">" and source_value > compare_value then
        conditions_met = true
    elseif operator == ">=" and source_value >= compare_value then
        conditions_met = true
    elseif operator == "<" and source_value < compare_value then
        conditions_met = true
    elseif operator == "<=" and source_value <= compare_value then
        conditions_met = true
    elseif operator == "==" and source_value == compare_value then
        conditions_met = true
    end
    
    local result = string.format(
        '{"rule_id":"%s","conditions_met":%s,"source":"%s","value":%s,"operator":"%s","compare":%s}',
        rule_id,
        tostring(conditions_met),
        source,
        tostring(source_value),
        operator,
        tostring(compare_value)
    )
    
    if conditions_met then
        -- 发送通知
        redis.call('PUBLISH', 'ems:notifications', 
            string.format('{"rule":"%s","message":"Condition met: %s %s %s"}', 
                rule_id, source, operator, compare_value))
    end
    
    return result
end

-- 创建简单测试规则
local function create_simple_rule(keys, args)
    local rule_id = keys[1]
    local source = args[1]
    local operator = args[2]
    local value = args[3]
    
    local rule = string.format([[{
        "id": "%s",
        "name": "Simple Rule %s",
        "enabled": true,
        "conditions": {
            "operator": "AND",
            "conditions": [{
                "source": "%s",
                "operator": "%s",
                "value": %s
            }]
        }
    }]], rule_id, rule_id, source, operator, value)
    
    redis.call('SET', 'rulesrv:rule:' .. rule_id, rule)
    return redis.status_reply("Rule created: " .. rule_id)
end

-- 列出所有规则
local function list_rules(keys, args)
    local rule_keys = redis.call('KEYS', 'rulesrv:rule:*')
    local rules = {}
    
    for i, key in ipairs(rule_keys) do
        local rule_id = string.match(key, "rulesrv:rule:(.+)")
        table.insert(rules, rule_id)
    end
    
    return table.concat(rules, ",")
end

-- 注册函数
redis.register_function('rule_execute', execute_rule)
redis.register_function('rule_create_simple', create_simple_rule)
redis.register_function('rule_list', list_rules)