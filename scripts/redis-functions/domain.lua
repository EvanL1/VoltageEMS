#!lua name=domain

-- ========================================
-- VoltageEMS 领域功能函数
-- 整合告警、规则、同步等领域功能
-- ========================================

-- ==================== 告警功能 ====================

-- 存储告警
local function store_alarm(keys, args)
    if #keys ~= 1 or #args ~= 1 then
        return redis.error_reply("Wrong number of arguments")
    end
    
    local alarm_id = keys[1]
    local alarm_data = args[1]
    
    -- 解析 JSON
    local alarm = cjson.decode(alarm_data)
    if not alarm then
        return redis.error_reply("Invalid alarm data")
    end
    
    local alarm_key = 'alarmsrv:' .. alarm_id
    
    -- 存储告警数据
    local fields = {}
    for k, v in pairs(alarm) do
        table.insert(fields, k)
        table.insert(fields, tostring(v))
    end
    
    if #fields > 0 then
        redis.call('HSET', alarm_key, unpack(fields))
    end
    
    -- Set expiration time（如果配置了）
    if alarm.ttl then
        redis.call('EXPIRE', alarm_key, alarm.ttl)
    end
    
    -- 更新索引
    -- Index by status
    if alarm.status then
        redis.call('SADD', 'idx:alarm:status:' .. alarm.status, alarm_id)
    end
    
    -- Index by level
    if alarm.level then
        redis.call('SADD', 'idx:alarm:level:' .. alarm.level, alarm_id)
    end
    
    -- 按类别索引
    if alarm.category then
        redis.call('SADD', 'idx:alarm:category:' .. alarm.category, alarm_id)
    end
    
    -- Sort by time索引
    local timestamp = alarm.created_at or redis.call('TIME')[1]
    redis.call('ZADD', 'idx:alarm:time', timestamp, alarm_id)
    
    -- Update statistics
    redis.call('HINCRBY', 'stats:alarm', 'total', 1)
    if alarm.status then
        redis.call('HINCRBY', 'stats:alarm:status', alarm.status, 1)
    end
    if alarm.level then
        redis.call('HINCRBY', 'stats:alarm:level', alarm.level, 1)
    end
    
    -- 发布事件
    redis.call('PUBLISH', 'event:alarm:created', cjson.encode({
        alarm_id = alarm_id,
        level = alarm.level,
        status = alarm.status,
        timestamp = timestamp
    }))
    
    return redis.status_reply('OK')
end

-- Acknowledge alarm
local function acknowledge_alarm(keys, args)
    if #keys ~= 1 or #args < 1 then
        return redis.error_reply("Wrong number of arguments")
    end
    
    local alarm_id = keys[1]
    local user = args[1]
    local comment = args[2] or ""
    
    local alarm_key = 'alarmsrv:' .. alarm_id
    
    -- 检查告警是否存在
    if redis.call('EXISTS', alarm_key) == 0 then
        return redis.error_reply("Alarm not found")
    end
    
    -- 获取当前状态
    local current_status = redis.call('HGET', alarm_key, 'status')
    
    -- Update status
    local timestamp = redis.call('TIME')[1]
    redis.call('HSET', alarm_key,
        'status', 'acknowledged',
        'acknowledged_by', user,
        'acknowledged_at', timestamp,
        'acknowledge_comment', comment
    )
    
    -- 更新索引
    if current_status then
        redis.call('SREM', 'idx:alarm:status:' .. current_status, alarm_id)
    end
    redis.call('SADD', 'idx:alarm:status:acknowledged', alarm_id)
    
    -- Update statistics
    if current_status then
        redis.call('HINCRBY', 'stats:alarm:status', current_status, -1)
    end
    redis.call('HINCRBY', 'stats:alarm:status', 'acknowledged', 1)
    
    -- 发布事件
    redis.call('PUBLISH', 'event:alarm:acknowledged', cjson.encode({
        alarm_id = alarm_id,
        user = user,
        timestamp = timestamp
    }))
    
    return redis.status_reply('OK')
end

-- Resolve alarm
local function resolve_alarm(keys, args)
    if #keys ~= 1 then
        return redis.error_reply("Wrong number of arguments")
    end
    
    local alarm_id = keys[1]
    local resolution = args[1] or ""
    
    local alarm_key = 'alarmsrv:' .. alarm_id
    
    -- 检查告警是否存在
    if redis.call('EXISTS', alarm_key) == 0 then
        return redis.error_reply("Alarm not found")
    end
    
    -- 获取当前状态
    local current_status = redis.call('HGET', alarm_key, 'status')
    
    -- Update status
    local timestamp = redis.call('TIME')[1]
    redis.call('HSET', alarm_key,
        'status', 'resolved',
        'resolved_at', timestamp,
        'resolution', resolution
    )
    
    -- 更新索引
    if current_status then
        redis.call('SREM', 'idx:alarm:status:' .. current_status, alarm_id)
    end
    redis.call('SADD', 'idx:alarm:status:resolved', alarm_id)
    
    -- Update statistics
    if current_status then
        redis.call('HINCRBY', 'stats:alarm:status', current_status, -1)
    end
    redis.call('HINCRBY', 'stats:alarm:status', 'resolved', 1)
    
    -- 发布事件
    redis.call('PUBLISH', 'event:alarm:resolved', cjson.encode({
        alarm_id = alarm_id,
        timestamp = timestamp
    }))
    
    return redis.status_reply('OK')
end

-- 清理过期告警
local function cleanup_old_alarms(keys, args)
    local days_to_keep = tonumber(args[1] or 30)
    local max_to_delete = tonumber(args[2] or 1000)
    
    local cutoff_time = redis.call('TIME')[1] - (days_to_keep * 86400)
    
    -- 从时间索引中获取要删除的告警
    local old_alarms = redis.call('ZRANGEBYSCORE', 'idx:alarm:time', '-inf', cutoff_time, 'LIMIT', 0, max_to_delete)
    
    local deleted = 0
    for _, alarm_id in ipairs(old_alarms) do
        local alarm_key = 'alarmsrv:' .. alarm_id
        
        -- 获取告警信息用于清理索引
        local alarm_data = redis.call('HGETALL', alarm_key)
        local alarm = {}
        for i = 1, #alarm_data, 2 do
            alarm[alarm_data[i]] = alarm_data[i + 1]
        end
        
        -- 清理索引
        if alarm.status then
            redis.call('SREM', 'idx:alarm:status:' .. alarm.status, alarm_id)
        end
        if alarm.level then
            redis.call('SREM', 'idx:alarm:level:' .. alarm.level, alarm_id)
        end
        if alarm.category then
            redis.call('SREM', 'idx:alarm:category:' .. alarm.category, alarm_id)
        end
        redis.call('ZREM', 'idx:alarm:time', alarm_id)
        
        -- 删除告警数据
        redis.call('DEL', alarm_key)
        
        deleted = deleted + 1
    end
    
    -- Update statistics
    redis.call('HINCRBY', 'stats:alarm', 'cleaned', deleted)
    
    return deleted
end

-- Query alarms
local function query_alarms(keys, args)
    if #args < 1 then
        return redis.error_reply("Wrong number of arguments")
    end
    
    local query = cjson.decode(args[1])
    local results = {}
    local alarm_ids = {}
    
    -- 根据查询条件获取告警ID
    if query.status then
        alarm_ids = redis.call('SMEMBERS', 'idx:alarm:status:' .. query.status)
    elseif query.level then
        alarm_ids = redis.call('SMEMBERS', 'idx:alarm:level:' .. query.level)
    elseif query.category then
        alarm_ids = redis.call('SMEMBERS', 'idx:alarm:category:' .. query.category)
    elseif query.time_range then
        alarm_ids = redis.call('ZRANGEBYSCORE', 'idx:alarm:time',
            query.time_range.start or '-inf',
            query.time_range['end'] or '+inf',
            'LIMIT', query.offset or 0, query.limit or 100)
    else
        -- 默认返回最新的告警
        alarm_ids = redis.call('ZREVRANGE', 'idx:alarm:time', 
            query.offset or 0, (query.offset or 0) + (query.limit or 100) - 1)
    end
    
    -- 获取告警详情
    for _, alarm_id in ipairs(alarm_ids) do
        local alarm_key = 'alarmsrv:' .. alarm_id
        local alarm_data = redis.call('HGETALL', alarm_key)
        
        if #alarm_data > 0 then
            local alarm = {id = alarm_id}
            for i = 1, #alarm_data, 2 do
                alarm[alarm_data[i]] = alarm_data[i + 1]
            end
            table.insert(results, alarm)
        end
    end
    
    -- Sort（如果需要）
    if query.sort_by and #results > 0 then
        table.sort(results, function(a, b)
            if query.sort_order == 'desc' then
                return (a[query.sort_by] or '') > (b[query.sort_by] or '')
            else
                return (a[query.sort_by] or '') < (b[query.sort_by] or '')
            end
        end)
    end
    
    return cjson.encode({
        total = #results,
        data = results
    })
end

-- ==================== 规则功能 ====================

-- 保存规则
local function save_rule(keys, args)
    if #keys < 1 or #args < 1 then
        return redis.error_reply("Wrong number of arguments")
    end
    
    local rule_id = keys[1]
    local rule_data = args[1]
    
    local rule = cjson.decode(rule_data)
    if not rule then
        return redis.error_reply("Invalid rule data")
    end
    
    local rule_key = 'rulesrv:rule:' .. rule_id
    
    -- 存储规则数据
    redis.call('HSET', rule_key,
        'id', rule_id,
        'name', rule.name or '',
        'description', rule.description or '',
        'enabled', tostring(rule.enabled),
        'priority', rule.priority or 0,
        'group_id', rule.group_id or '',
        'conditions', cjson.encode(rule.conditions or {}),
        'actions', cjson.encode(rule.actions or {}),
        'schedule', rule.schedule or '',
        'data', rule_data,
        'updated_at', redis.call('TIME')[1]
    )
    
    -- 更新索引
    if rule.enabled then
        redis.call('SADD', 'idx:rule:enabled', rule_id)
    else
        redis.call('SREM', 'idx:rule:enabled', rule_id)
    end
    
    if rule.group_id and rule.group_id ~= '' then
        redis.call('SADD', 'idx:rule:group:' .. rule.group_id, rule_id)
    end
    
    -- 按优先级排序索引
    redis.call('ZADD', 'idx:rule:priority', rule.priority or 0, rule_id)
    
    -- Update statistics
    redis.call('HINCRBY', 'stats:rule', 'total', 1)
    
    return redis.status_reply('OK')
end

-- 删除规则
local function delete_rule(keys, args)
    if #keys < 1 then
        return redis.error_reply("Wrong number of arguments")
    end
    
    local rule_id = keys[1]
    local rule_key = 'rulesrv:rule:' .. rule_id
    
    -- 获取规则信息用于清理索引
    local rule_data = redis.call('HGETALL', rule_key)
    if #rule_data == 0 then
        return redis.error_reply("Rule not found")
    end
    
    local rule = {}
    for i = 1, #rule_data, 2 do
        rule[rule_data[i]] = rule_data[i + 1]
    end
    
    -- 清理索引
    redis.call('SREM', 'idx:rule:enabled', rule_id)
    if rule.group_id and rule.group_id ~= '' then
        redis.call('SREM', 'idx:rule:group:' .. rule.group_id, rule_id)
    end
    redis.call('ZREM', 'idx:rule:priority', rule_id)
    
    -- 删除规则数据
    redis.call('DEL', rule_key)
    
    -- 删除执行历史
    redis.call('DEL', rule_key .. ':history')
    
    -- Update statistics
    redis.call('HINCRBY', 'stats:rule', 'total', -1)
    
    return redis.status_reply('OK')
end

-- 保存规则组
local function save_rule_group(keys, args)
    if #keys < 1 or #args < 1 then
        return redis.error_reply("Wrong number of arguments")
    end
    
    local group_id = keys[1]
    local group_data = args[1]
    
    local group = cjson.decode(group_data)
    if not group then
        return redis.error_reply("Invalid group data")
    end
    
    local group_key = 'rulesrv:group:' .. group_id
    
    -- 存储规则组数据
    redis.call('HSET', group_key,
        'id', group_id,
        'name', group.name or '',
        'description', group.description or '',
        'enabled', tostring(group.enabled),
        'priority', group.priority or 0,
        'data', group_data,
        'updated_at', redis.call('TIME')[1]
    )
    
    -- 更新索引
    if group.enabled then
        redis.call('SADD', 'idx:rule_group:enabled', group_id)
    else
        redis.call('SREM', 'idx:rule_group:enabled', group_id)
    end
    
    -- Update statistics
    redis.call('HINCRBY', 'stats:rule_group', 'total', 1)
    
    return redis.status_reply('OK')
end

-- 删除规则组
local function delete_rule_group(keys, args)
    if #keys < 1 then
        return redis.error_reply("Wrong number of arguments")
    end
    
    local group_id = keys[1]
    local group_key = 'rulesrv:group:' .. group_id
    
    -- 检查规则组是否存在
    if redis.call('EXISTS', group_key) == 0 then
        return redis.error_reply("Rule group not found")
    end
    
    -- 检查是否有规则属于该组
    local group_rules = redis.call('SMEMBERS', 'idx:rule:group:' .. group_id)
    if #group_rules > 0 then
        return redis.error_reply("Cannot delete group with rules")
    end
    
    -- 清理索引
    redis.call('SREM', 'idx:rule_group:enabled', group_id)
    
    -- 删除规则组数据
    redis.call('DEL', group_key)
    
    -- Update statistics
    redis.call('HINCRBY', 'stats:rule_group', 'total', -1)
    
    return redis.status_reply('OK')
end

-- 保存执行历史
local function save_execution_history(keys, args)
    if #keys < 1 or #args < 1 then
        return redis.error_reply("Wrong number of arguments")
    end
    
    local rule_id = keys[1]
    local execution_data = args[1]
    
    local execution = cjson.decode(execution_data)
    if not execution then
        return redis.error_reply("Invalid execution data")
    end
    
    local history_key = 'rulesrv:rule:' .. rule_id .. ':history'
    
    -- 添加执行记录
    redis.call('LPUSH', history_key, execution_data)
    
    -- 只保留最新的N条记录
    redis.call('LTRIM', history_key, 0, 999)
    
    -- Set expiration time（30天）
    redis.call('EXPIRE', history_key, 2592000)
    
    -- 更新规则的最后执行时间
    redis.call('HSET', 'rulesrv:rule:' .. rule_id,
        'last_executed_at', execution.timestamp or redis.call('TIME')[1],
        'last_execution_result', execution.result or 'unknown'
    )
    
    -- Update statistics
    redis.call('HINCRBY', 'stats:rule:execution', 'total', 1)
    if execution.result then
        redis.call('HINCRBY', 'stats:rule:execution', execution.result, 1)
    end
    
    return redis.status_reply('OK')
end

-- ==================== 同步功能 ====================

-- 同步通道数据
local function sync_channel_data(keys, args)
    if #keys < 1 or #args < 4 then
        return redis.error_reply("Usage: channel_id point_type updates_json trigger_alarms timestamp")
    end
    
    local channel_id = keys[1]
    local point_type = args[1]
    local updates_json = args[2]
    local trigger_alarms = args[3] == "true"
    local timestamp = args[4]
    
    -- 解析更新数据
    local updates = cjson.decode(updates_json)
    
    -- 存储到正确的Hash键 (comsrv:channel_id:point_type)
    local hash_key = 'comsrv:' .. channel_id .. ':' .. point_type
    local update_count = 0
    local alarm_count = 0
    
    -- 批量更新数据点
    for _, update in ipairs(updates) do
        local point_id = tostring(update.point_id)
        local value = update.value
        
        -- 写入标准化的6位小数格式
        redis.call('HSET', hash_key, point_id, string.format("%.6f", value))
        update_count = update_count + 1
        
        -- 如果需要触发告警，这里可以添加告警逻辑
        if trigger_alarms then
            -- TODO: 实现告警触发逻辑
        end
    end
    
    -- 发布同步事件
    redis.call('PUBLISH', 'comsrv:' .. channel_id .. ':' .. point_type, cjson.encode({
        channel_id = channel_id,
        point_type = point_type,
        timestamp = timestamp,
        update_count = update_count
    }))
    
    -- Update statistics
    redis.call('HINCRBY', 'stats:sync', 'total_syncs', 1)
    redis.call('HINCRBY', 'stats:sync:channel', channel_id, 1)
    
    -- 返回结果数组 [update_count, alarm_count]
    return cjson.encode({update_count, alarm_count})
end

-- 批量同步多个通道
local function sync_all_channels(keys, args)
    if #args < 1 then
        return redis.error_reply("Usage: channels_data_json")
    end
    
    local channels_data = cjson.decode(args[1])
    local results = {
        success = 0,
        failed = 0,
        channels = {}
    }
    
    for channel_id, channel_data in pairs(channels_data) do
        local status, err = pcall(function()
            -- 调用单通道同步
            sync_channel_data({channel_id}, {cjson.encode(channel_data)})
        end)
        
        if status then
            results.success = results.success + 1
            table.insert(results.channels, {
                channel_id = channel_id,
                status = 'success'
            })
        else
            results.failed = results.failed + 1
            table.insert(results.channels, {
                channel_id = channel_id,
                status = 'failed',
                error = err
            })
        end
    end
    
    return cjson.encode(results)
end

-- 计算设备增量
local function calculate_device_delta(keys, args)
    if #keys < 1 then
        return redis.error_reply("Usage: device_id [threshold]")
    end
    
    local device_id = keys[1]
    local threshold = tonumber(args[1] or 0.01)
    
    local current_key = 'sync:device:' .. device_id .. ':current'
    local previous_key = 'sync:device:' .. device_id .. ':previous'
    
    -- 获取当前值和前值
    local current_data = redis.call('HGETALL', current_key)
    local previous_data = redis.call('HGETALL', previous_key)
    
    -- 转换为表
    local current = {}
    for i = 1, #current_data, 2 do
        current[current_data[i]] = tonumber(current_data[i + 1]) or 0
    end
    
    local previous = {}
    for i = 1, #previous_data, 2 do
        previous[previous_data[i]] = tonumber(previous_data[i + 1]) or 0
    end
    
    -- 计算增量
    local deltas = {}
    local has_change = false
    
    for point_id, current_value in pairs(current) do
        local previous_value = previous[point_id] or 0
        local delta = current_value - previous_value
        
        if math.abs(delta) >= threshold then
            deltas[point_id] = {
                current = current_value,
                previous = previous_value,
                delta = delta,
                change_percent = previous_value ~= 0 and (delta / previous_value * 100) or 0
            }
            has_change = true
        end
    end
    
    -- 如果有变化，更新前值
    if has_change then
        redis.call('DEL', previous_key)
        if #current_data > 0 then
            redis.call('HSET', previous_key, unpack(current_data))
        end
        redis.call('EXPIRE', previous_key, 86400) -- 1天过期
    end
    
    return cjson.encode({
        device_id = device_id,
        has_change = has_change,
        deltas = deltas,
        timestamp = redis.call('TIME')[1]
    })
end

-- 设置同步阈值
local function set_thresholds(keys, args)
    if #args < 1 then
        return redis.error_reply("Usage: thresholds_json")
    end
    
    local thresholds = cjson.decode(args[1])
    local config_key = 'sync:config:thresholds'
    
    -- 存储阈值配置
    for device_pattern, threshold_config in pairs(thresholds) do
        redis.call('HSET', config_key, device_pattern, cjson.encode(threshold_config))
    end
    
    -- 更新配置版本
    redis.call('HINCRBY', 'sync:config:meta', 'version', 1)
    redis.call('HSET', 'sync:config:meta', 'updated_at', redis.call('TIME')[1])
    
    return redis.status_reply('OK')
end

-- 注册所有函数
redis.register_function('store_alarm', store_alarm)
redis.register_function('acknowledge_alarm', acknowledge_alarm)
redis.register_function('resolve_alarm', resolve_alarm)
redis.register_function('cleanup_old_alarms', cleanup_old_alarms)
redis.register_function('query_alarms', query_alarms)

redis.register_function('save_rule', save_rule)
redis.register_function('store_rule', save_rule)  -- 别名，向后兼容
redis.register_function('delete_rule', delete_rule)
redis.register_function('save_rule_group', save_rule_group)
redis.register_function('delete_rule_group', delete_rule_group)
redis.register_function('save_execution_history', save_execution_history)

redis.register_function('sync_channel_data', sync_channel_data)
redis.register_function('sync_all_channels', sync_all_channels)
redis.register_function('calculate_device_delta', calculate_device_delta)
redis.register_function('set_thresholds', set_thresholds)