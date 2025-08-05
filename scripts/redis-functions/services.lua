#!lua name=services

-- ========================================
-- VoltageEMS 服务适配层
-- 整合所有服务的特定函数
-- ========================================

-- ==================== AlarmSrv 适配 ====================

-- AlarmSrv 存储告警（使用通用存储）
local function alarmsrv_store_alarm(keys, args)
    local alarm_id = keys[1]
    local alarm_data = args[1]
    local alarm = cjson.decode(alarm_data)

    -- 准备告警实体
    local entity = {
        entity_id = alarm_id,
        entity_type = "alarm",
        data = alarm_data,
        indexes = {
            { type = "single", field = "status",   value = alarm.status },
            { type = "single", field = "level",    value = alarm.level },
            { type = "single", field = "category", value = alarm.category or "general" },
            { type = "sorted", field = "time",     score = alarm.created_at or redis.call('TIME')[1] }
        }
    }

    -- 直接实现存储逻辑
    local entity_key = 'alarmsrv:' .. alarm_id
    
    -- 存储实体数据
    redis.call('HSET', entity_key, 'type', 'alarm', 'data', alarm_data, 'updated_at', tostring(redis.call('TIME')[1]))
    
    -- 处理索引
    for _, idx in ipairs(entity.indexes) do
        if idx.type == "single" then
            local idx_key = string.format("idx:alarm:%s:%s", idx.field, idx.value)
            redis.call('SADD', idx_key, entity_key)
            redis.call('EXPIRE', idx_key, 86400)
        elseif idx.type == "sorted" then
            local idx_key = string.format("idx:alarm:%s", idx.field)
            redis.call('ZADD', idx_key, idx.score or 0, entity_key)
        end
    end
    
    return redis.status_reply('OK')
end

-- ==================== HisSrv 适配 ====================

-- HisSrv 数据收集
local function hissrv_collect_data(keys, args)
    local source_pattern = args[1] or "comsrv:*"
    local batch_size = tonumber(args[2] or 1000)

    -- 直接实现批量收集逻辑
    local cursor = "0"
    local collected_data = {}
    local total_collected = 0
    
    repeat
        local result = redis.call('SCAN', cursor, 'MATCH', source_pattern, 'COUNT', batch_size)
        cursor = result[1]
        local keys_found = result[2]
        
        for _, key in ipairs(keys_found) do
            if total_collected >= batch_size then
                break
            end
            
            local key_type = redis.call('TYPE', key).ok
            if key_type == "hash" then
                local hash_data = redis.call('HGETALL', key)
                if #hash_data > 0 then
                    local item = { key = key, data = {} }
                    for i = 1, #hash_data, 2 do
                        item.data[hash_data[i]] = hash_data[i+1]
                    end
                    table.insert(collected_data, item)
                    total_collected = total_collected + 1
                end
            end
        end
    until cursor == "0" or total_collected >= batch_size
    
    -- 创建批次
    local batch_id = "batch_" .. redis.call('TIME')[1] .. "_" .. redis.call('TIME')[2]
    local batch_key = 'hissrv:batch:' .. batch_id
    local data_key = batch_key .. ':data'
    
    -- 存储批次信息
    redis.call('HSET', batch_key, 
        'id', batch_id,
        'status', 'created',
        'size', tostring(total_collected),
        'created_at', tostring(redis.call('TIME')[1])
    )
    
    -- 存储批次数据
    for _, item in ipairs(collected_data) do
        redis.call('RPUSH', data_key, cjson.encode(item))
    end
    
    -- 设置过期时间
    redis.call('EXPIRE', batch_key, 3600)
    redis.call('EXPIRE', data_key, 3600)
    
    return cjson.encode({
        batch_id = batch_id,
        size = total_collected,
        status = 'created'
    })
end

-- HisSrv 转换为Line Protocol
local function hissrv_convert_to_line_protocol(keys, args)
    -- 直接实现Line Protocol转换
    if #args < 1 then
        return redis.error_reply("Usage: batch_data_json [config_json]")
    end
    
    local batch_data = cjson.decode(args[1])
    local config = args[2] and cjson.decode(args[2]) or {}
    local lines = {}
    
    -- 处理批次数据
    if batch_data.batch then
        for _, item in ipairs(batch_data.batch) do
            if item.key and item.data then
                -- 解析key格式: service:channel:type
                local parts = {}
                for part in string.gmatch(item.key, "[^:]+") do
                    table.insert(parts, part)
                end
                
                if #parts >= 3 then
                    local service = parts[1]
                    local channel = parts[2]
                    local telemetry_type = parts[3]
                    
                    -- 构建measurement名称
                    local measurement = service .. "_" .. telemetry_type
                    
                    -- 构建tags
                    local tags = string.format("channel=%s,service=%s", channel, service)
                    
                    -- 构建fields
                    local fields = {}
                    for point_id, value in pairs(item.data) do
                        table.insert(fields, string.format("point_%s=%s", point_id, value))
                    end
                    
                    if #fields > 0 then
                        local timestamp = redis.call('TIME')[1] .. '000000000' -- nanoseconds
                        local line = string.format("%s,%s %s %s", 
                            measurement, tags, table.concat(fields, ","), timestamp)
                        table.insert(lines, line)
                    end
                end
            end
        end
    end
    
    return table.concat(lines, "\n")
end

-- HisSrv 获取批次数据
local function hissrv_get_batch(keys, args)
    local batch_id = keys[1]
    local batch_key = 'hissrv:batch:' .. batch_id

    -- 获取批次信息
    local batch_info = redis.call('HGETALL', batch_key)
    if #batch_info == 0 then
        return redis.error_reply("Batch not found")
    end

    local batch = {}
    for i = 1, #batch_info, 2 do
        batch[batch_info[i]] = batch_info[i + 1]
    end

    -- 获取批次数据
    local data_key = batch_key .. ':data'
    local data = redis.call('LRANGE', data_key, 0, -1)

    return cjson.encode({
        id = batch_id,
        info = batch,
        data = data
    })
end

-- HisSrv 确认批次
local function hissrv_ack_batch(keys, args)
    if #keys < 1 or #args < 1 then
        return redis.error_reply("Usage: batch_id status")
    end

    local batch_id = keys[1]
    local status = args[1]
    local batch_key = 'hissrv:batch:' .. batch_id

    -- 检查批次是否存在
    if redis.call('EXISTS', batch_key) == 0 then
        return redis.error_reply("Batch not found")
    end

    -- 更新批次状态
    redis.call('HSET', batch_key, 'status', status, 'ack_time', redis.call('TIME')[1])

    -- 如果状态是written，可以清理数据
    if status == "written" then
        local data_key = batch_key .. ':data'
        redis.call('DEL', data_key)
        -- 设置批次信息过期时间
        redis.call('EXPIRE', batch_key, 3600) -- 1小时后过期
    end

    return redis.status_reply('OK')
end

-- HisSrv 获取批次的Line Protocol数据
local function hissrv_get_batch_lines(keys, args)
    if #keys < 1 then
        return redis.error_reply("Usage: batch_id")
    end

    local batch_id = keys[1]
    local batch_key = 'hissrv:batch:' .. batch_id

    -- 检查批次是否存在
    if redis.call('EXISTS', batch_key) == 0 then
        return redis.error_reply("Batch not found")
    end

    -- 获取Line Protocol数据
    local lines_key = batch_key .. ':lines'
    local lines = redis.call('LRANGE', lines_key, 0, -1)

    if #lines == 0 then
        -- 如果没有预先转换的lines，尝试从原始数据转换
        local data_key = batch_key .. ':data'
        local raw_data = redis.call('LRANGE', data_key, 0, -1)

        if #raw_data > 0 then
            -- 使用line_protocol_converter转换数据
            local batch_data = {
                batch = {}
            }

            for _, item in ipairs(raw_data) do
                table.insert(batch_data.batch, cjson.decode(item))
            end

            -- 直接进行Line Protocol转换
            local lines_temp = {}
            for _, item in ipairs(raw_data) do
                local decoded_item = cjson.decode(item)
                if decoded_item.key and decoded_item.data then
                    -- 解析key格式
                    local parts = {}
                    for part in string.gmatch(decoded_item.key, "[^:]+") do
                        table.insert(parts, part)
                    end
                    
                    if #parts >= 3 then
                        local service = parts[1]
                        local channel = parts[2]
                        local telemetry_type = parts[3]
                        
                        local measurement = service .. "_" .. telemetry_type
                        local tags = string.format("channel=%s,service=%s", channel, service)
                        
                        local fields = {}
                        for point_id, value in pairs(decoded_item.data) do
                            table.insert(fields, string.format("point_%s=%s", point_id, value))
                        end
                        
                        if #fields > 0 then
                            local timestamp = redis.call('TIME')[1] .. '000000000'
                            local line = string.format("%s,%s %s %s", 
                                measurement, tags, table.concat(fields, ","), timestamp)
                            table.insert(lines_temp, line)
                        end
                    end
                end
            end
            
            local converted = table.concat(lines_temp, "\n")

            -- 将转换后的数据按行分割
            for line in string.gmatch(converted, "[^\n]+") do
                table.insert(lines, line)
            end

            -- 缓存转换后的数据
            if #lines > 0 then
                for _, line in ipairs(lines) do
                    redis.call('RPUSH', lines_key, line)
                end
                redis.call('EXPIRE', lines_key, 3600) -- 1小时过期
            end
        end
    end

    -- 返回Line Protocol格式的数据
    return table.concat(lines, "\n")
end

-- ==================== ModSrv ====================

-- ModSrv 初始化映射
local function modsrv_init_mappings(keys, args)
    if #args < 1 then
        return redis.error_reply("Usage: mappings_json")
    end

    local mappings = cjson.decode(args[1])
    local init_count = 0

    for model_id, model_mappings in pairs(mappings) do
        local mapping_key = 'modsrv:mapping:' .. model_id

        -- 存储监测映射
        if model_mappings.monitoring then
            for name, point_id in pairs(model_mappings.monitoring) do
                redis.call('HSET', mapping_key .. ':monitoring', name, point_id)
                init_count = init_count + 1
            end
        end

        -- 存储控制映射
        if model_mappings.control then
            for name, point_id in pairs(model_mappings.control) do
                redis.call('HSET', mapping_key .. ':control', name, point_id)
                init_count = init_count + 1
            end
        end
    end

    return init_count
end

-- ModSrv 同步测量数据
local function modsrv_sync_measurement(keys, args)
    if #keys < 1 or #args < 3 then
        return redis.error_reply("Usage: channel_id telemetry_type point_id value [timestamp]")
    end

    local channel_id = keys[1]
    local telemetry_type = args[1]
    local point_id = args[2]
    local value = args[3]
    local timestamp = args[4] or redis.call('TIME')[1]

    -- 存储到Hash
    local hash_key = 'comsrv:' .. channel_id .. ':' .. telemetry_type
    redis.call('HSET', hash_key, point_id, value)

    -- 发布到频道
    local pub_channel = hash_key
    redis.call('PUBLISH', pub_channel, cjson.encode({
        point_id = point_id,
        value = value,
        timestamp = timestamp
    }))

    -- 更新同步时间
    redis.call('HSET', 'modsrv:sync:status', channel_id, timestamp)

    return redis.status_reply('OK')
end

-- ModSrv 发送控制命令
local function modsrv_send_control(keys, args)
    if #keys < 1 or #args < 2 then
        return redis.error_reply("Usage: model_id control_name value")
    end

    local model_id = keys[1]
    local control_name = args[1]
    local value = args[2]

    -- 获取控制点映射
    local mapping_key = 'modsrv:mapping:' .. model_id .. ':control'
    local point_id = redis.call('HGET', mapping_key, control_name)

    if not point_id then
        return redis.error_reply("Control mapping not found")
    end

    -- 发送控制命令
    local control_data = {
        model_id = model_id,
        control_name = control_name,
        point_id = point_id,
        value = value,
        timestamp = redis.call('TIME')[1]
    }

    -- 发布控制命令
    redis.call('PUBLISH', 'control:' .. model_id, cjson.encode(control_data))

    -- 记录控制历史
    local history_key = 'modsrv:control:history:' .. model_id
    redis.call('LPUSH', history_key, cjson.encode(control_data))
    redis.call('LTRIM', history_key, 0, 99)

    return cjson.encode({
        status = "sent",
        point_id = point_id
    })
end

-- ModSrv 获取模型值
local function modsrv_get_values(keys, args)
    if #keys < 1 then
        return redis.error_reply("Usage: model_id")
    end
    
    local model_id = keys[1]
    local result = {}
    
    -- 获取模型的输入映射
    local inputs_key = 'modsrv:model:' .. model_id .. ':inputs'
    local inputs = redis.call('HGETALL', inputs_key)
    
    -- 如果没有配置输入映射，尝试从监测点映射获取
    if #inputs == 0 then
        -- 从power_monitor_ch1的配置中提取输入
        -- 根据配置文件，输入映射到comsrv的数据点
        local input_mapping = {
            voltage_a = "comsrv:1:T:1",
            voltage_b = "comsrv:1:T:2", 
            voltage_c = "comsrv:1:T:3",
            current_a = "comsrv:1:T:4",
            current_b = "comsrv:1:T:5",
            current_c = "comsrv:1:T:6",
            power = "comsrv:1:T:7",
            reactive_power = "comsrv:1:T:8",
            power_factor = "comsrv:1:T:9",
            breaker_status = "comsrv:1:S:1"
        }
        
        -- 读取每个输入的值
        for name, source in pairs(input_mapping) do
            -- 解析source格式: service:channel:type:point
            local parts = {}
            for part in string.gmatch(source, "[^:]+") do
                table.insert(parts, part)
            end
            
            if #parts == 4 then
                local service = parts[1]
                local channel = parts[2]
                local type = parts[3]
                local point = parts[4]
                
                local hash_key = service .. ':' .. channel .. ':' .. type
                local value = redis.call('HGET', hash_key, point)
                
                if value then
                    result[name] = value
                end
            end
        end
        
        -- 计算衍生值
        local voltage_a = tonumber(result.voltage_a) or 0
        local voltage_b = tonumber(result.voltage_b) or 0
        local voltage_c = tonumber(result.voltage_c) or 0
        local current_a = tonumber(result.current_a) or 0
        local current_b = tonumber(result.current_b) or 0
        local current_c = tonumber(result.current_c) or 0
        local power = tonumber(result.power) or 0
        
        -- 计算平均值
        result.voltage_avg = string.format("%.2f", (voltage_a + voltage_b + voltage_c) / 3)
        result.current_avg = string.format("%.2f", (current_a + current_b + current_c) / 3)
        
        -- 计算电压不平衡度
        local voltage_avg = (voltage_a + voltage_b + voltage_c) / 3
        local max_deviation = math.max(
            math.abs(voltage_a - voltage_avg),
            math.abs(voltage_b - voltage_avg),
            math.abs(voltage_c - voltage_avg)
        )
        result.voltage_imbalance = string.format("%.2f", (max_deviation / voltage_avg) * 100)
        
        -- 计算功率利用率（假设额定功率为10000W）
        local rated_power = 10000
        result.power_utilization = string.format("%.2f", (power / rated_power) * 100)
    end
    
    return cjson.encode(result)
end

-- ==================== NetSrv 适配 ====================

-- NetSrv 数据收集
local function netsrv_collect_data(keys, args)
    local source_pattern = args[1] or "network:*"
    local batch_size = tonumber(args[2] or 100)

    -- 直接实现网络数据收集
    local cursor = "0"
    local collected_count = 0
    
    repeat
        local result = redis.call('SCAN', cursor, 'MATCH', source_pattern, 'COUNT', batch_size)
        cursor = result[1]
        local keys_found = result[2]
        
        for _, key in ipairs(keys_found) do
            if collected_count >= batch_size then
                break
            end
            
            -- 计数统计
            collected_count = collected_count + 1
        end
    until cursor == "0" or collected_count >= batch_size
    
    return cjson.encode({
        collected = collected_count,
        pattern = source_pattern
    })
end

-- NetSrv 转发数据
local function netsrv_forward_data(keys, args)
    local destination = keys[1]
    local data = cjson.decode(args[1])
    local forward_config = args[2] and cjson.decode(args[2]) or {}

    -- 构建转发数据
    local forward_data = {
        source = "netsrv",
        destination = destination,
        data = data,
        timestamp = redis.call('TIME')[1],
        metadata = forward_config.metadata or {}
    }

    -- 发布到目标频道
    redis.call('PUBLISH', 'forward:' .. destination, cjson.encode(forward_data))

    -- 记录转发历史
    local history_key = 'netsrv:forward:history'
    redis.call('LPUSH', history_key, cjson.encode({
        destination = destination,
        timestamp = forward_data.timestamp,
        data_size = #args[1]
    }))
    redis.call('LTRIM', history_key, 0, 999)

    -- 更新统计
    redis.call('HINCRBY', 'netsrv:stats:forward', destination, 1)
    redis.call('HINCRBY', 'netsrv:stats:forward', 'total', 1)

    return redis.status_reply('OK')
end

-- NetSrv 获取统计
local function netsrv_get_stats(keys, args)
    local stats_type = args[1] or "summary"

    if stats_type == "summary" then
        local stats = {}

        -- 转发统计
        local forward_stats = redis.call('HGETALL', 'netsrv:stats:forward')
        stats.forward = {}
        for i = 1, #forward_stats, 2 do
            stats.forward[forward_stats[i]] = tonumber(forward_stats[i + 1])
        end

        -- 收集统计
        local collect_stats = redis.call('HGETALL', 'netsrv:stats:collect')
        stats.collect = {}
        for i = 1, #collect_stats, 2 do
            stats.collect[collect_stats[i]] = tonumber(collect_stats[i + 1])
        end

        return cjson.encode(stats)
    elseif stats_type == "detailed" then
        -- 获取详细统计，包括历史记录
        local history = redis.call('LRANGE', 'netsrv:forward:history', 0, 99)
        local history_data = {}

        for _, item in ipairs(history) do
            table.insert(history_data, cjson.decode(item))
        end

        return cjson.encode({
            recent_forwards = history_data
        })
    end

    return redis.error_reply("Unknown stats type")
end

-- NetSrv 配置路由
local function netsrv_configure_route(keys, args)
    if #keys < 1 or #args < 1 then
        return redis.error_reply("Usage: route_id route_config_json")
    end

    local route_id = keys[1]
    local route_config = cjson.decode(args[1])

    -- 存储路由配置
    local route_key = 'netsrv:route:' .. route_id
    redis.call('HSET', route_key,
        'id', route_id,
        'source', route_config.source or '',
        'destination', route_config.destination or '',
        'enabled', tostring(route_config.enabled),
        'transform', cjson.encode(route_config.transform or {}),
        'filters', cjson.encode(route_config.filters or {}),
        'updated_at', redis.call('TIME')[1]
    )

    -- 更新路由索引
    if route_config.enabled then
        redis.call('SADD', 'netsrv:routes:enabled', route_id)
    else
        redis.call('SREM', 'netsrv:routes:enabled', route_id)
    end

    -- 更新源到路由的映射
    if route_config.source then
        redis.call('SADD', 'netsrv:routes:by_source:' .. route_config.source, route_id)
    end

    return redis.status_reply('OK')
end

-- NetSrv 获取路由列表
local function netsrv_get_routes(keys, args)
    local filter = args[1] or "all"
    local routes = {}
    local route_ids = {}

    if filter == "enabled" then
        route_ids = redis.call('SMEMBERS', 'netsrv:routes:enabled')
    elseif filter == "all" then
        -- 获取所有路由
        local cursor = "0"
        local pattern = "netsrv:route:*"

        repeat
            local result = redis.call('SCAN', cursor, 'MATCH', pattern, 'COUNT', 100)
            cursor = result[1]

            for _, key in ipairs(result[2]) do
                local route_id = string.match(key, "netsrv:route:(.+)")
                if route_id then
                    table.insert(route_ids, route_id)
                end
            end
        until cursor == "0"
    else
        -- 按源过滤
        route_ids = redis.call('SMEMBERS', 'netsrv:routes:by_source:' .. filter)
    end

    -- 获取路由详情
    for _, route_id in ipairs(route_ids) do
        local route_key = 'netsrv:route:' .. route_id
        local route_data = redis.call('HGETALL', route_key)

        if #route_data > 0 then
            local route = {}
            for i = 1, #route_data, 2 do
                route[route_data[i]] = route_data[i + 1]
            end

            -- 解析JSON字段
            if route.transform then
                route.transform = cjson.decode(route.transform)
            end
            if route.filters then
                route.filters = cjson.decode(route.filters)
            end

            table.insert(routes, route)
        end
    end

    return cjson.encode({
        total = #routes,
        routes = routes
    })
end

-- NetSrv 清理队列
local function netsrv_clear_queues(keys, args)
    local queue_type = args[1] or "all"
    local cleared = 0

    if queue_type == "all" or queue_type == "forward" then
        -- 清理转发队列
        local cursor = "0"
        local pattern = "netsrv:queue:forward:*"

        repeat
            local result = redis.call('SCAN', cursor, 'MATCH', pattern, 'COUNT', 100)
            cursor = result[1]

            for _, key in ipairs(result[2]) do
                redis.call('DEL', key)
                cleared = cleared + 1
            end
        until cursor == "0"
    end

    if queue_type == "all" or queue_type == "retry" then
        -- 清理重试队列
        local cursor = "0"
        local pattern = "netsrv:queue:retry:*"

        repeat
            local result = redis.call('SCAN', cursor, 'MATCH', pattern, 'COUNT', 100)
            cursor = result[1]

            for _, key in ipairs(result[2]) do
                redis.call('DEL', key)
                cleared = cleared + 1
            end
        until cursor == "0"
    end

    if queue_type == "all" or queue_type == "dead" then
        -- 清理死信队列
        local cursor = "0"
        local pattern = "netsrv:queue:dead:*"

        repeat
            local result = redis.call('SCAN', cursor, 'MATCH', pattern, 'COUNT', 100)
            cursor = result[1]

            for _, key in ipairs(result[2]) do
                redis.call('DEL', key)
                cleared = cleared + 1
            end
        until cursor == "0"
    end

    -- 重置统计
    if queue_type == "all" then
        redis.call('DEL', 'netsrv:stats:forward')
        redis.call('DEL', 'netsrv:stats:collect')
    end

    return cjson.encode({
        cleared = cleared,
        queue_type = queue_type
    })
end

-- ==================== RuleSrv 适配 ====================

-- RuleSrv 存储规则
local function rulesrv_store_rule(keys, args)
    -- 直接实现规则存储
    if #keys < 1 or #args < 1 then
        return redis.error_reply("Usage: rule_id rule_data_json")
    end
    
    local rule_id = keys[1]
    local rule_data = args[1]
    local rule = cjson.decode(rule_data)
    
    local rule_key = string.format("rulesrv:rule:%s", rule_id)
    
    -- 存储规则数据
    redis.call('HSET', rule_key,
        'id', rule_id,
        'name', rule.name or '',
        'enabled', rule.enabled and '1' or '0',
        'priority', tostring(rule.priority or 100),
        'group_id', rule.group_id or 'default',
        'data', rule_data,
        'updated_at', tostring(redis.call('TIME')[1])
    )
    
    -- 更新索引
    if rule.enabled then
        redis.call('SADD', 'idx:rule:enabled', rule_id)
        redis.call('SREM', 'idx:rule:disabled', rule_id)
    else
        redis.call('SADD', 'idx:rule:disabled', rule_id)
        redis.call('SREM', 'idx:rule:enabled', rule_id)
    end
    
    -- 优先级索引
    redis.call('ZADD', 'idx:rule:priority', rule.priority or 100, rule_id)
    
    -- 组索引
    redis.call('SADD', 'idx:rule:group:' .. (rule.group_id or 'default'), rule_id)
    
    return redis.status_reply('OK')
end

-- RuleSrv 获取规则
local function rulesrv_get_rule(keys, args)
    local rule_id = keys[1]
    local rule_key = string.format("rulesrv:rule:%s", rule_id)

    local rule_data = redis.call('HGETALL', rule_key)
    if #rule_data == 0 then
        return redis.error_reply("Rule not found")
    end

    local rule = {}
    for i = 1, #rule_data, 2 do
        rule[rule_data[i]] = rule_data[i + 1]
    end

    return cjson.encode(rule)
end

-- RuleSrv 查询规则
local function rulesrv_query_rules(keys, args)
    local query_config = cjson.decode(args[1])
    local results = {}

    -- 根据不同条件查询
    if query_config.group_id then
        -- 按组查询
        local rule_ids = redis.call('SMEMBERS', 'idx:rule:group:' .. query_config.group_id)
        for _, rule_id in ipairs(rule_ids) do
            local rule = cjson.decode(rulesrv_get_rule({ rule_id }, {}))
            if rule then
                table.insert(results, rule)
            end
        end
    elseif query_config.enabled ~= nil then
        -- 按启用状态查询
        local idx_key = query_config.enabled and 'idx:rule:enabled' or 'idx:rule:disabled'
        local rule_ids = redis.call('SMEMBERS', idx_key)

        for _, rule_id in ipairs(rule_ids) do
            local rule = cjson.decode(rulesrv_get_rule({ rule_id }, {}))
            if rule then
                table.insert(results, rule)
            end
        end
    else
        -- 默认查询所有规则（按优先级）
        local rule_ids = redis.call('ZRANGE', 'idx:rule:priority', 0, query_config.limit or 100)
        for _, rule_id in ipairs(rule_ids) do
            local rule = cjson.decode(rulesrv_get_rule({ rule_id }, {}))
            if rule then
                table.insert(results, rule)
            end
        end
    end

    return cjson.encode({
        total = #results,
        rules = results
    })
end

-- RuleSrv 执行DAG规则
local function rulesrv_execute_dag(keys, args)
    -- 直接实现DAG执行逻辑
    if #keys < 1 or #args < 1 then
        return redis.error_reply("Usage: dag_id dag_config_json")
    end
    
    local dag_id = keys[1]
    local dag_config = cjson.decode(args[1])
    local results = {}
    
    -- 简化的DAG执行：按优先级顺序执行
    if dag_config.nodes then
        for _, node in ipairs(dag_config.nodes) do
            local node_result = {
                id = node.id,
                status = 'executed',
                timestamp = redis.call('TIME')[1]
            }
            
            -- 这里可以根据node.type执行不同逻辑
            if node.type == 'data_fetch' then
                node_result.data = 'fetched'
            elseif node.type == 'transform' then
                node_result.data = 'transformed'
            elseif node.type == 'output' then
                node_result.data = 'output_generated'
            end
            
            table.insert(results, node_result)
        end
    end
    
    -- 记录执行历史
    local execution_key = 'rulesrv:dag:execution:' .. dag_id
    redis.call('LPUSH', execution_key, cjson.encode({
        dag_id = dag_id,
        results = results,
        executed_at = redis.call('TIME')[1]
    }))
    redis.call('LTRIM', execution_key, 0, 99)
    
    return cjson.encode({
        dag_id = dag_id,
        status = 'completed',
        results = results
    })
end

-- 注册所有服务函数
-- AlarmSrv
redis.register_function('alarmsrv_store_alarm', alarmsrv_store_alarm)

-- HisSrv
redis.register_function('hissrv_collect_data', hissrv_collect_data)
redis.register_function('hissrv_convert_to_line_protocol', hissrv_convert_to_line_protocol)
redis.register_function('hissrv_get_batch', hissrv_get_batch)
redis.register_function('hissrv_ack_batch', hissrv_ack_batch)
redis.register_function('hissrv_get_batch_lines', hissrv_get_batch_lines)

-- HisSrv 配置映射
local function hissrv_configure_mapping(keys, args)
    if #keys < 1 or #args < 1 then
        return redis.error_reply("Usage: mapping_id config_json")
    end

    local mapping_id = keys[1]
    local config = cjson.decode(args[1])

    -- 存储映射配置
    local mapping_key = 'hissrv:mapping:' .. mapping_id
    redis.call('HSET', mapping_key,
        'source_pattern', config.source_pattern or '',
        'measurement', config.measurement or '',
        'enabled', config.enabled and '1' or '0',
        'tags', cjson.encode(config.tags or {}),
        'field_mappings', cjson.encode(config.field_mappings or {})
    )

    -- 设置过期时间（30天）
    redis.call('EXPIRE', mapping_key, 2592000)

    -- 更新映射索引
    redis.call('SADD', 'hissrv:mappings', mapping_id)

    return "OK"
end

redis.register_function('hissrv_configure_mapping', hissrv_configure_mapping)

-- ModSrv 清理映射
local function modsrv_clear_mappings(keys, args)
    local pattern = args[1] or "*"
    local count = 0
    
    -- 获取所有映射键
    local mapping_keys = redis.call('KEYS', 'modsrv:mapping:' .. pattern)
    
    -- 删除所有映射
    for _, key in ipairs(mapping_keys) do
        redis.call('DEL', key)
        count = count + 1
    end
    
    return count
end

-- ModSrv
redis.register_function('modsrv_init_mappings', modsrv_init_mappings)
redis.register_function('modsrv_sync_measurement', modsrv_sync_measurement)
redis.register_function('modsrv_send_control', modsrv_send_control)
redis.register_function('modsrv_get_values', modsrv_get_values)
redis.register_function('modsrv_clear_mappings', modsrv_clear_mappings)

-- NetSrv
redis.register_function('netsrv_collect_data', netsrv_collect_data)
redis.register_function('netsrv_forward_data', netsrv_forward_data)
redis.register_function('netsrv_get_stats', netsrv_get_stats)
redis.register_function('netsrv_configure_route', netsrv_configure_route)
redis.register_function('netsrv_get_routes', netsrv_get_routes)
redis.register_function('netsrv_clear_queues', netsrv_clear_queues)

-- RuleSrv
redis.register_function('rulesrv_store_rule', rulesrv_store_rule)
redis.register_function('rulesrv_get_rule', rulesrv_get_rule)
redis.register_function('rulesrv_query_rules', rulesrv_query_rules)
redis.register_function('rulesrv_execute_dag', rulesrv_execute_dag)
