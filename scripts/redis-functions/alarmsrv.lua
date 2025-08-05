#!lua name=alarm_engine

-- Alarm Engine Lua Functions
-- 处理告警的创建、查询、确认和解决

local function alarm_key(alarm_id)
    return "alarmsrv:" .. alarm_id
end

local function alarm_index_key()
    return "alarmsrv:index"
end

local function alarm_status_key(status)
    return "alarmsrv:status:" .. status
end

local function alarm_level_key(level)
    return "alarmsrv:level:" .. level
end

local function alarm_source_key(source)
    return "alarmsrv:source:" .. source
end

-- 存储告警
local function store_alarm(keys, args)
    local alarm_id = keys[1]
    local alarm_json = args[1]
    
    -- Parse alarm data
    local alarm = cjson.decode(alarm_json)
    local key = alarm_key(alarm_id)
    
    -- 存储告警数据
    redis.call('HSET', key, 
        'data', alarm_json,
        'id', alarm_id,
        'title', alarm.title or '',
        'level', tostring(alarm.level),
        'status', tostring(alarm.status),
        'source', alarm.source or '',
        'priority', tostring(alarm.priority or 0),
        'created_at', alarm.created_at or '',
        'updated_at', alarm.updated_at or ''
    )
    
    -- 添加到索引
    redis.call('ZADD', alarm_index_key(), alarm.priority or 0, alarm_id)
    
    -- 添加到状态索引
    if alarm.status then
        redis.call('SADD', alarm_status_key(alarm.status), alarm_id)
    end
    
    -- 添加到级别索引
    if alarm.level then
        redis.call('SADD', alarm_level_key(alarm.level), alarm_id)
    end
    
    -- 添加到源索引
    if alarm.source and alarm.source ~= '' then
        redis.call('SADD', alarm_source_key(alarm.source), alarm_id)
    end
    
    return "OK"
end

-- 获取告警
local function get_alarm(keys, args)
    local alarm_id = keys[1]
    local key = alarm_key(alarm_id)
    
    local alarm_data = redis.call('HGET', key, 'data')
    if not alarm_data then
        return nil
    end
    
    return alarm_data
end

-- Acknowledge alarm
local function acknowledge_alarm(keys, args)
    local alarm_id = keys[1]
    local user = args[1]
    local timestamp = args[2]
    
    local key = alarm_key(alarm_id)
    
    -- 获取当前告警数据
    local alarm_data = redis.call('HGET', key, 'data')
    if not alarm_data then
        error("Alarm not found: " .. alarm_id)
    end
    
    local alarm = cjson.decode(alarm_data)
    local old_status = alarm.status
    
    -- Update alarm status
    alarm.status = "Acknowledged"
    alarm.acknowledged_by = user
    alarm.acknowledged_at = timestamp
    alarm.updated_at = timestamp
    
    -- 保存更新后的告警
    local updated_json = cjson.encode(alarm)
    redis.call('HSET', key, 
        'data', updated_json,
        'status', 'Acknowledged',
        'updated_at', timestamp
    )
    
    -- Update status索引
    if old_status then
        redis.call('SREM', alarm_status_key(old_status), alarm_id)
    end
    redis.call('SADD', alarm_status_key("Acknowledged"), alarm_id)
    
    return updated_json
end

-- Resolve alarm
local function resolve_alarm(keys, args)
    local alarm_id = keys[1]
    local user = args[1]
    local timestamp = args[2]
    
    local key = alarm_key(alarm_id)
    
    -- 获取当前告警数据
    local alarm_data = redis.call('HGET', key, 'data')
    if not alarm_data then
        error("Alarm not found: " .. alarm_id)
    end
    
    local alarm = cjson.decode(alarm_data)
    local old_status = alarm.status
    
    -- Update alarm status
    alarm.status = "Resolved"
    alarm.resolved_by = user
    alarm.resolved_at = timestamp
    alarm.updated_at = timestamp
    
    -- 保存更新后的告警
    local updated_json = cjson.encode(alarm)
    redis.call('HSET', key, 
        'data', updated_json,
        'status', 'Resolved',
        'updated_at', timestamp
    )
    
    -- Update status索引
    if old_status then
        redis.call('SREM', alarm_status_key(old_status), alarm_id)
    end
    redis.call('SADD', alarm_status_key("Resolved"), alarm_id)
    
    -- 从主索引中降低优先级（已解决的告警）
    redis.call('ZADD', alarm_index_key(), -1, alarm_id)
    
    return updated_json
end

-- Query alarms
local function query_alarms(keys, args)
    local query_json = args[1]
    local query = query_json and cjson.decode(query_json) or {}
    
    local limit = tonumber(query.limit) or 100
    local offset = tonumber(query.offset) or 0
    
    local alarm_ids = {}
    
    -- 根据查询条件获取告警ID
    if query.status then
        -- 按状态查询
        local status_ids = redis.call('SMEMBERS', alarm_status_key(query.status))
        for _, id in ipairs(status_ids) do
            table.insert(alarm_ids, id)
        end
    elseif query.level then
        -- 按级别查询
        local level_ids = redis.call('SMEMBERS', alarm_level_key(query.level))
        for _, id in ipairs(level_ids) do
            table.insert(alarm_ids, id)
        end
    elseif query.source then
        -- 按源查询
        local source_ids = redis.call('SMEMBERS', alarm_source_key(query.source))
        for _, id in ipairs(source_ids) do
            table.insert(alarm_ids, id)
        end
    else
        -- 无条件查询，按优先级排序
        local all_ids = redis.call('ZREVRANGE', alarm_index_key(), 0, -1)
        alarm_ids = all_ids
    end
    
    -- 获取总数
    local total = #alarm_ids
    
    -- 应用分页
    local result_alarms = {}
    local end_index = math.min(offset + limit, total)
    
    for i = offset + 1, end_index do
        local alarm_data = redis.call('HGET', alarm_key(alarm_ids[i]), 'data')
        if alarm_data then
            table.insert(result_alarms, cjson.decode(alarm_data))
        end
    end
    
    -- 返回分页结果
    return cjson.encode({
        total = total,
        offset = offset,
        limit = limit,
        data = result_alarms
    })
end

-- 获取活跃告警数量
local function get_active_alarm_count(keys, args)
    local new_count = redis.call('SCARD', alarm_status_key("New"))
    local ack_count = redis.call('SCARD', alarm_status_key("Acknowledged"))
    
    return cjson.encode({
        new = new_count,
        acknowledged = ack_count,
        active = new_count + ack_count
    })
end

-- 获取告警统计
local function get_alarm_stats(keys, args)
    local stats = {
        by_status = {},
        by_level = {},
        total = 0
    }
    
    -- 按状态统计
    local statuses = {"New", "Acknowledged", "Resolved"}
    for _, status in ipairs(statuses) do
        stats.by_status[status] = redis.call('SCARD', alarm_status_key(status))
    end
    
    -- 按级别统计
    local levels = {"Critical", "Major", "Minor", "Warning", "Info"}
    for _, level in ipairs(levels) do
        stats.by_level[level] = redis.call('SCARD', alarm_level_key(level))
    end
    
    -- 总数
    stats.total = redis.call('ZCARD', alarm_index_key())
    
    return cjson.encode(stats)
end

-- 批量确认告警
local function acknowledge_alarms_batch(keys, args)
    local alarm_ids_json = args[1]
    local user = args[2]
    local timestamp = args[3]
    
    local alarm_ids = cjson.decode(alarm_ids_json)
    local results = {}
    
    for _, alarm_id in ipairs(alarm_ids) do
        local success, result = pcall(function()
            return acknowledge_alarm({alarm_id}, {user, timestamp})
        end)
        
        if success then
            table.insert(results, {
                id = alarm_id,
                success = true,
                alarm = cjson.decode(result)
            })
        else
            table.insert(results, {
                id = alarm_id,
                success = false,
                error = result
            })
        end
    end
    
    return cjson.encode(results)
end

-- 清理旧告警
local function cleanup_old_alarms(keys, args)
    local days_to_keep = tonumber(args[1]) or 30
    local cutoff_timestamp = args[2]
    
    local resolved_ids = redis.call('SMEMBERS', alarm_status_key("Resolved"))
    local deleted_count = 0
    
    for _, alarm_id in ipairs(resolved_ids) do
        local key = alarm_key(alarm_id)
        local alarm_data = redis.call('HGET', key, 'data')
        
        if alarm_data then
            local alarm = cjson.decode(alarm_data)
            -- 这里简化处理，实际应该比较时间戳
            if alarm.resolved_at and alarm.resolved_at < cutoff_timestamp then
                -- 删除告警
                redis.call('DEL', key)
                redis.call('ZREM', alarm_index_key(), alarm_id)
                redis.call('SREM', alarm_status_key("Resolved"), alarm_id)
                
                if alarm.level then
                    redis.call('SREM', alarm_level_key(alarm.level), alarm_id)
                end
                if alarm.source then
                    redis.call('SREM', alarm_source_key(alarm.source), alarm_id)
                end
                
                deleted_count = deleted_count + 1
            end
        end
    end
    
    return tostring(deleted_count)
end

-- Register functions
redis.register_function('store_alarm', store_alarm)
redis.register_function('get_alarm', get_alarm)
redis.register_function('acknowledge_alarm', acknowledge_alarm)
redis.register_function('resolve_alarm', resolve_alarm)
redis.register_function('query_alarms', query_alarms)
redis.register_function('get_active_alarm_count', get_active_alarm_count)
redis.register_function('get_alarm_stats', get_alarm_stats)
redis.register_function('acknowledge_alarms_batch', acknowledge_alarms_batch)
redis.register_function('cleanup_old_alarms', cleanup_old_alarms)