#!lua name=hissrv_engine

-- HisSrv Engine Lua Functions
-- 简化版历史数据收集函数
-- 负责从Redis收集数据并转换为InfluxDB Line Protocol格式

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
            type = parts[3]
        }
    end
    
    return nil
end

-- 转换单个点位为 InfluxDB Line Protocol
local function point_to_line_protocol(key_parts, point_id, value, timestamp_ns)
    -- 根据类型确定measurement名称
    local type_map = {
        T = "telemetry",
        S = "signal", 
        C = "control",
        A = "adjustment"
    }
    local measurement = type_map[key_parts.type] or "unknown"
    
    -- 构建Line Protocol: measurement,channel=xxx,type=xxx point_xxx=value timestamp
    local line = string.format(
        "%s,service=%s,channel=%s,type=%s point_%s=%s %s",
        measurement,
        key_parts.service,
        key_parts.channel,
        key_parts.type,
        point_id,
        value,
        timestamp_ns
    )
    
    return line
end

-- 收集批次数据（主函数）
local function collect_batch(keys, args)
    local batch_id = keys[1]
    local sources_json = args[1]
    
    -- 解析数据源模式
    local sources = cjson.decode(sources_json)
    local lines = {}
    local point_count = 0
    
    -- 获取当前时间戳（纳秒）
    local timestamp = redis.call('TIME')
    local timestamp_ns = timestamp[1] * 1000000000 + timestamp[2] * 1000
    
    -- 遍历每个数据源模式
    for _, pattern in ipairs(sources) do
        -- 获取匹配的键
        local matched_keys = redis.call('KEYS', pattern)
        
        for _, key in ipairs(matched_keys) do
            -- 解析键名
            local key_parts = parse_redis_key(key)
            if key_parts then
                -- 获取该键的所有数据
                local data = redis.call('HGETALL', key)
                
                -- 将数据转换为哈希表
                for i = 1, #data, 2 do
                    local point_id = data[i]
                    local value = data[i+1]
                    
                    -- 尝试转换为数字
                    local num_value = tonumber(value)
                    if num_value then
                        -- 生成Line Protocol行
                        local line = point_to_line_protocol(
                            key_parts, 
                            point_id, 
                            num_value, 
                            timestamp_ns
                        )
                        table.insert(lines, line)
                        point_count = point_count + 1
                    end
                end
            end
        end
    end
    
    -- 返回结果
    local result = {
        point_count = point_count,
        lines = table.concat(lines, "\n")
    }
    
    return cjson.encode(result)
end

-- 获取收集统计信息
local function get_stats(keys, args)
    local stats = {
        timestamp = redis.call('TIME')[1],
        keys_count = 0,
        points_count = 0
    }
    
    -- 统计comsrv数据
    local comsrv_keys = redis.call('KEYS', 'comsrv:*:*')
    stats.keys_count = #comsrv_keys
    
    -- 统计总点数
    for _, key in ipairs(comsrv_keys) do
        local count = redis.call('HLEN', key)
        stats.points_count = stats.points_count + count
    end
    
    return cjson.encode(stats)
end

-- 注册函数
redis.register_function('hissrv_collect_batch', collect_batch)
redis.register_function('hissrv_get_stats', get_stats)