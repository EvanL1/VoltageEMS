#!lua name=alarm_engine

-- ========================================
-- AlarmSrv Lua引擎 (精简版)
-- 告警管理的核心功能
-- ========================================

-- ==================== 告警管理 ====================

-- 触发告警
local function alarmsrv_trigger_alarm(keys, args)
    local alarm_id = keys[1]
    local alarm_json = args[1]
    
    if not alarm_id or not alarm_json then
        return redis.error_reply("Alarm ID and JSON required")
    end
    
    -- 验证JSON格式
    local ok, alarm = pcall(cjson.decode, alarm_json)
    if not ok then
        return redis.error_reply("Invalid JSON")
    end
    
    -- 设置默认值
    alarm.id = alarm_id
    alarm.status = alarm.status or "Active"
    alarm.created_at = alarm.created_at or redis.call('TIME')[1]
    alarm.updated_at = alarm.created_at
    
    -- 存储告警
    local alarm_key = 'alarmsrv:alarm:' .. alarm_id
    redis.call('SET', alarm_key, cjson.encode(alarm))
    
    -- 添加到索引
    redis.call('SADD', 'alarmsrv:alarms', alarm_id)
    redis.call('SADD', 'alarmsrv:status:' .. alarm.status, alarm_id)
    
    if alarm.level then
        redis.call('SADD', 'alarmsrv:level:' .. alarm.level, alarm_id)
    end
    
    return redis.status_reply("OK")
end

-- 获取告警
local function alarmsrv_get_alarm(keys, args)
    local alarm_id = keys[1]
    local alarm_key = 'alarmsrv:alarm:' .. alarm_id
    
    local alarm_json = redis.call('GET', alarm_key)
    if not alarm_json then
        return cjson.encode({error = "Alarm not found"})
    end
    
    return alarm_json
end

-- 清除告警
local function alarmsrv_clear_alarm(keys, args)
    local alarm_id = keys[1]
    local alarm_key = 'alarmsrv:alarm:' .. alarm_id
    
    -- 获取告警用于清理索引
    local alarm_json = redis.call('GET', alarm_key)
    if alarm_json then
        local ok, alarm = pcall(cjson.decode, alarm_json)
        if ok then
            redis.call('SREM', 'alarmsrv:status:' .. alarm.status, alarm_id)
            if alarm.level then
                redis.call('SREM', 'alarmsrv:level:' .. alarm.level, alarm_id)
            end
        end
    end
    
    -- 删除告警
    redis.call('DEL', alarm_key)
    redis.call('SREM', 'alarmsrv:alarms', alarm_id)
    
    return redis.status_reply("OK")
end

-- 确认告警
local function alarmsrv_acknowledge_alarm(keys, args)
    local alarm_id = keys[1]
    local ack_data_json = args[1]
    
    local alarm_key = 'alarmsrv:alarm:' .. alarm_id
    local alarm_json = redis.call('GET', alarm_key)
    if not alarm_json then
        return redis.error_reply("Alarm not found")
    end
    
    local alarm = cjson.decode(alarm_json)
    local old_status = alarm.status
    
    -- 更新状态
    alarm.status = "Acknowledged"
    alarm.acknowledged_at = redis.call('TIME')[1]
    
    if ack_data_json then
        local ok, ack_data = pcall(cjson.decode, ack_data_json)
        if ok then
            alarm.acknowledged_by = ack_data.user
            alarm.acknowledge_note = ack_data.note
        end
    end
    
    -- 保存更新
    redis.call('SET', alarm_key, cjson.encode(alarm))
    
    -- 更新索引
    redis.call('SREM', 'alarmsrv:status:' .. old_status, alarm_id)
    redis.call('SADD', 'alarmsrv:status:Acknowledged', alarm_id)
    
    return redis.status_reply("OK")
end

-- 列出告警
local function alarmsrv_list_alarms(keys, args)
    local query_json = args[1]
    local query = {}
    
    if query_json then
        local ok, parsed = pcall(cjson.decode, query_json)
        if ok then query = parsed end
    end
    
    local alarm_ids = {}
    
    -- 根据查询条件获取告警
    if query.status then
        alarm_ids = redis.call('SMEMBERS', 'alarmsrv:status:' .. query.status)
    elseif query.level then
        alarm_ids = redis.call('SMEMBERS', 'alarmsrv:level:' .. query.level)
    else
        alarm_ids = redis.call('SMEMBERS', 'alarmsrv:alarms')
    end
    
    -- 限制数量
    local limit = tonumber(query.limit) or 100
    local alarms = {}
    local count = 0
    
    for _, alarm_id in ipairs(alarm_ids) do
        if count >= limit then break end
        
        local alarm_json = redis.call('GET', 'alarmsrv:alarm:' .. alarm_id)
        if alarm_json then
            table.insert(alarms, cjson.decode(alarm_json))
            count = count + 1
        end
    end
    
    return cjson.encode(alarms)
end

-- ==================== 告警规则管理 ====================

-- 创建或更新规则
local function alarmsrv_upsert_rule(keys, args)
    local rule_id = keys[1]
    local rule_json = args[1]
    
    if not rule_id or not rule_json then
        return redis.error_reply("Rule ID and JSON required")
    end
    
    -- 存储规则
    local rule_key = 'alarmsrv:rule:' .. rule_id
    redis.call('SET', rule_key, rule_json)
    redis.call('SADD', 'alarmsrv:rules', rule_id)
    
    return redis.status_reply("OK")
end

-- 获取规则
local function alarmsrv_get_rule(keys, args)
    local rule_id = keys[1]
    local rule_key = 'alarmsrv:rule:' .. rule_id
    
    local rule_json = redis.call('GET', rule_key)
    if not rule_json then
        return cjson.encode({error = "Rule not found"})
    end
    
    return rule_json
end

-- 删除规则
local function alarmsrv_delete_rule(keys, args)
    local rule_id = keys[1]
    local rule_key = 'alarmsrv:rule:' .. rule_id
    
    redis.call('DEL', rule_key)
    redis.call('SREM', 'alarmsrv:rules', rule_id)
    
    return redis.status_reply("OK")
end

-- 列出规则
local function alarmsrv_list_rules(keys, args)
    local rule_ids = redis.call('SMEMBERS', 'alarmsrv:rules')
    local rules = {}
    
    for _, rule_id in ipairs(rule_ids) do
        local rule_json = redis.call('GET', 'alarmsrv:rule:' .. rule_id)
        if rule_json then
            table.insert(rules, cjson.decode(rule_json))
        end
    end
    
    return cjson.encode(rules)
end

-- 获取统计信息
local function alarmsrv_get_statistics(keys, args)
    local total_alarms = redis.call('SCARD', 'alarmsrv:alarms')
    local active_alarms = redis.call('SCARD', 'alarmsrv:status:Active')
    local acknowledged_alarms = redis.call('SCARD', 'alarmsrv:status:Acknowledged')
    local total_rules = redis.call('SCARD', 'alarmsrv:rules')
    
    -- 按级别统计
    local levels = {"Critical", "Warning", "Info"}
    local level_stats = {}
    for _, level in ipairs(levels) do
        level_stats[level] = redis.call('SCARD', 'alarmsrv:level:' .. level)
    end
    
    return cjson.encode({
        total = total_alarms,
        active = active_alarms,
        acknowledged = acknowledged_alarms,
        rules = total_rules,
        by_level = level_stats,
        timestamp = redis.call('TIME')[1]
    })
end

-- ==================== 注册函数 ====================

redis.register_function('alarmsrv_trigger_alarm', alarmsrv_trigger_alarm)
redis.register_function('alarmsrv_get_alarm', alarmsrv_get_alarm)
redis.register_function('alarmsrv_clear_alarm', alarmsrv_clear_alarm)
redis.register_function('alarmsrv_acknowledge_alarm', alarmsrv_acknowledge_alarm)
redis.register_function('alarmsrv_list_alarms', alarmsrv_list_alarms)

redis.register_function('alarmsrv_upsert_rule', alarmsrv_upsert_rule)
redis.register_function('alarmsrv_get_rule', alarmsrv_get_rule)
redis.register_function('alarmsrv_delete_rule', alarmsrv_delete_rule)
redis.register_function('alarmsrv_list_rules', alarmsrv_list_rules)

redis.register_function('alarmsrv_get_statistics', alarmsrv_get_statistics)