#!lua name=hissrv_engine

-- HisSrv Engine Lua Functions
-- 处理历史数据收集、转换和批处理

local function batch_key(batch_id)
    return "hissrv:batch:" .. batch_id
end

local function mapping_key(mapping_id)
    return "hissrv:mapping:" .. mapping_id
end

local function active_mappings_key()
    return "hissrv:mappings:active"
end

-- 配置数据映射
local function configure_mapping(keys, args)
    local mapping_id = keys[1]
    local config_json = args[1]
    
    local config = cjson.decode(config_json)
    local key = mapping_key(mapping_id)
    
    -- 存储映射配置
    redis.call('HSET', key,
        'config', config_json,
        'source_pattern', config.source_pattern or '',
        'measurement', config.measurement or '',
        'enabled', config.enabled and '1' or '0'
    )
    
    -- 如果启用，添加到活跃映射集合
    if config.enabled then
        redis.call('SADD', active_mappings_key(), mapping_id)
    else
        redis.call('SREM', active_mappings_key(), mapping_id)
    end
    
    return "OK"
end

-- 解析 Redis 键名（例如 "comsrv:1001:T"）
local function parse_redis_key(key)
    local parts = {}
    for part in string.gmatch(key, "[^:]+") do
        table.insert(parts, part)
    end
    
    if #parts >= 3 then
        return {
            service = parts[1],
            channel = parts[2],
            type = parts[3],
            full_key = key
        }
    end
    
    return nil
end

-- 转换为 InfluxDB Line Protocol
local function convert_to_line_protocol(measurement, key_parts, data, config)
    -- 构建标签
    local tags = {}
    
    -- 添加静态标签
    if config.tags then
        for k, v in pairs(config.tags) do
            if k:sub(1, 10) == "__extract_" then
                -- 从键名中提取字段
                local field_name = k:sub(11)
                tags[field_name] = key_parts[field_name] or ""
            else
                tags[k] = v
            end
        end
    end
    
    -- 添加通道标签
    tags['channel'] = key_parts.channel
    
    -- Build fields
    local fields = {}
    local field_mappings = config.field_mappings or {}
    
    -- 将数据数组转换为哈希表
    local data_hash = {}
    for i = 1, #data, 2 do
        data_hash[data[i]] = data[i+1]
    end
    
    -- 处理字段映射
    for source_field, target_field in pairs(field_mappings) do
        local value = data_hash[source_field]
        if value then
            -- 尝试转换为数字
            local num_value = tonumber(value)
            if num_value then
                fields[target_field] = num_value
            else
                -- 字符串值需要引号
                fields[target_field] = '"' .. value .. '"'
            end
        end
    end
    
    -- 如果没有字段，跳过这条记录
    if next(fields) == nil then
        return nil
    end
    
    -- 构建 Line Protocol 格式
    -- measurement,tag1=value1,tag2=value2 field1=value1,field2=value2 timestamp
    local line_parts = {measurement}
    
    -- 添加标签
    local tag_parts = {}
    for k, v in pairs(tags) do
        table.insert(tag_parts, k .. "=" .. v)
    end
    if #tag_parts > 0 then
        line_parts[1] = line_parts[1] .. "," .. table.concat(tag_parts, ",")
    end
    
    -- 添加字段
    local field_parts = {}
    for k, v in pairs(fields) do
        table.insert(field_parts, k .. "=" .. v)
    end
    table.insert(line_parts, table.concat(field_parts, ","))
    
    -- Add timestamp（纳秒）
    local timestamp = redis.call('TIME')
    local ns_timestamp = timestamp[1] * 1000000000 + timestamp[2] * 1000
    table.insert(line_parts, tostring(ns_timestamp))
    
    return table.concat(line_parts, " ")
end

-- 收集特定映射的数据
local function collect_data_for_mapping(config)
    local lines = {}
    local measurement = config.measurement
    local source_pattern = config.source_pattern
    
    -- 匹配源模式获取键
    local keys = redis.call('KEYS', source_pattern)
    
    for _, key in ipairs(keys) do
        -- 解析键名提取信息
        local key_parts = parse_redis_key(key)
        if key_parts then
            -- 获取数据
            local data = redis.call('HGETALL', key)
            if #data > 0 then
                -- 转换为 Line Protocol
                local line = convert_to_line_protocol(measurement, key_parts, data, config)
                if line then
                    table.insert(lines, line)
                end
            end
        end
    end
    
    return lines
end

-- 创建批次收集数据
local function create_batch(keys, args)
    local batch_id = keys[1]
    local key = batch_key(batch_id)
    
    -- 初始化批次
    redis.call('HSET', key,
        'status', 'collecting',
        'created_at', tostring(redis.call('TIME')[1]),
        'line_count', '0'
    )
    
    -- 获取所有活跃的映射
    local mappings = redis.call('SMEMBERS', active_mappings_key())
    local total_lines = 0
    local lines = {}
    
    for _, mapping_id in ipairs(mappings) do
        local mapping_config = redis.call('HGET', mapping_key(mapping_id), 'config')
        if mapping_config then
            local config = cjson.decode(mapping_config)
            
            -- 收集匹配的数据
            local collected_lines = collect_data_for_mapping(config)
            for _, line in ipairs(collected_lines) do
                table.insert(lines, line)
                total_lines = total_lines + 1
            end
        end
    end
    
    -- 存储行协议数据
    if total_lines > 0 then
        local line_protocol = table.concat(lines, "\n")
        redis.call('HSET', key, 'lines', line_protocol, 'line_count', tostring(total_lines))
    end
    
    redis.call('HSET', key, 'status', 'ready')
    
    return tostring(total_lines)
end

-- 获取批次的 Line Protocol 数据
local function get_batch_lines(keys, args)
    local batch_id = keys[1]
    local key = batch_key(batch_id)
    
    local lines = redis.call('HGET', key, 'lines')
    return lines or ""
end

-- Acknowledge batch处理完成
local function ack_batch(keys, args)
    local batch_id = keys[1]
    local status = args[1]
    local key = batch_key(batch_id)
    
    redis.call('HSET', key, 'status', status, 'acked_at', tostring(redis.call('TIME')[1]))
    
    -- 可选：一段时间后删除批次数据
    redis.call('EXPIRE', key, 3600) -- 1小时后过期
    
    return "OK"
end

-- 获取映射统计
local function get_mapping_stats(keys, args)
    local active_mappings = redis.call('SMEMBERS', active_mappings_key())
    local stats = {
        active_mappings = #active_mappings,
        mappings = {}
    }
    
    for _, mapping_id in ipairs(active_mappings) do
        local config = redis.call('HGET', mapping_key(mapping_id), 'config')
        if config then
            local cfg = cjson.decode(config)
            table.insert(stats.mappings, {
                id = mapping_id,
                source_pattern = cfg.source_pattern,
                measurement = cfg.measurement,
                enabled = cfg.enabled
            })
        end
    end
    
    return cjson.encode(stats)
end

-- 清理旧批次
local function cleanup_old_batches(keys, args)
    local hours_to_keep = tonumber(args[1]) or 24
    local cutoff_time = redis.call('TIME')[1] - (hours_to_keep * 3600)
    
    local batch_keys = redis.call('KEYS', 'hissrv:batch:*')
    local deleted_count = 0
    
    for _, batch_key in ipairs(batch_keys) do
        local created_at = redis.call('HGET', batch_key, 'created_at')
        if created_at and tonumber(created_at) < cutoff_time then
            redis.call('DEL', batch_key)
            deleted_count = deleted_count + 1
        end
    end
    
    return tostring(deleted_count)
end

-- 获取点位历史数据（简化版）
local function get_point_history(keys, args)
    local source = keys[1]
    local point_id = args[1]
    local limit = tonumber(args[2]) or 100
    
    -- 这里应该从 InfluxDB 查询，但在 Lua 中我们只能返回提示
    return cjson.encode({
        message = "Historical data should be queried from InfluxDB",
        source = source,
        point_id = point_id,
        limit = limit
    })
end

-- Register functions
redis.register_function('hissrv_configure_mapping', configure_mapping)
redis.register_function('hissrv_create_batch', create_batch)
redis.register_function('hissrv_get_batch_lines', get_batch_lines)
redis.register_function('hissrv_ack_batch', ack_batch)
redis.register_function('hissrv_get_mapping_stats', get_mapping_stats)
redis.register_function('hissrv_cleanup_old_batches', cleanup_old_batches)
redis.register_function('hissrv_get_point_history', get_point_history)