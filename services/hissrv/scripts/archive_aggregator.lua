-- archive_aggregator.lua - 历史数据聚合脚本
-- 功能：从原始数据中聚合出1分钟、5分钟等时间窗口的统计数据
-- 用法：redis-cli EVAL "$(cat archive_aggregator.lua)" 0 aggregate_1m

local action = ARGV[1] or "aggregate_1m"
local current_time = tonumber(ARGV[2]) or os.time()

-- 辅助函数：计算数组的统计值
local function calculate_stats(values)
    if #values == 0 then
        return nil, nil, nil
    end
    
    local sum = 0
    local min = values[1]
    local max = values[1]
    
    for _, v in ipairs(values) do
        sum = sum + v
        if v < min then min = v end
        if v > max then max = v end
    end
    
    return sum / #values, min, max  -- avg, min, max
end

-- 辅助函数：扫描并获取指定模式的键
local function scan_keys(pattern)
    local cursor = "0"
    local keys = {}
    
    repeat
        local result = redis.call("SCAN", cursor, "MATCH", pattern, "COUNT", 100)
        cursor = result[1]
        for _, key in ipairs(result[2]) do
            table.insert(keys, key)
        end
    until cursor == "0"
    
    return keys
end

-- 1分钟聚合
if action == "aggregate_1m" then
    local minute_bucket = math.floor(current_time / 60) * 60
    local archive_key = string.format("archive:1m:%d", minute_bucket)
    
    -- 扫描所有 comsrv 通道的测量数据
    local channels = scan_keys("comsrv:*:m")
    
    for _, channel_key in ipairs(channels) do
        -- 提取 channel ID
        local channel_id = string.match(channel_key, "comsrv:(%d+):m")
        if channel_id then
            -- 获取该通道的所有测量点
            local measurements = redis.call("HGETALL", channel_key)
            
            -- 临时存储各测点的值
            local voltage_values = {}
            local current_values = {}
            local power_values = {}
            
            -- 解析测量数据（假设点位ID规则：10001-10099为电压，10101-10199为电流，10201-10299为功率）
            for i = 1, #measurements, 2 do
                local point_id = tonumber(measurements[i])
                local value = tonumber(measurements[i + 1])
                
                if point_id and value then
                    if point_id >= 10001 and point_id <= 10099 then
                        table.insert(voltage_values, value)
                    elseif point_id >= 10101 and point_id <= 10199 then
                        table.insert(current_values, value)
                    elseif point_id >= 10201 and point_id <= 10299 then
                        table.insert(power_values, value)
                    end
                end
            end
            
            -- 计算统计值并存储
            local v_avg, v_min, v_max = calculate_stats(voltage_values)
            local c_avg, _, _ = calculate_stats(current_values)
            local p_avg, _, _ = calculate_stats(power_values)
            
            if v_avg then
                local channel_archive_key = string.format("%s:%s", archive_key, channel_id)
                redis.call("HSET", channel_archive_key, "voltage_avg", string.format("%.6f", v_avg))
                redis.call("HSET", channel_archive_key, "voltage_min", string.format("%.6f", v_min))
                redis.call("HSET", channel_archive_key, "voltage_max", string.format("%.6f", v_max))
                
                if c_avg then
                    redis.call("HSET", channel_archive_key, "current_avg", string.format("%.6f", c_avg))
                end
                
                if p_avg then
                    redis.call("HSET", channel_archive_key, "power_avg", string.format("%.6f", p_avg))
                end
                
                -- 添加时间戳
                redis.call("HSET", channel_archive_key, "timestamp", tostring(minute_bucket))
                
                -- 设置过期时间（2小时）
                redis.call("EXPIRE", channel_archive_key, 7200)
            end
        end
    end
    
    return string.format("1m aggregation completed for timestamp %d", minute_bucket)

-- 5分钟聚合（从1分钟数据聚合）
elseif action == "aggregate_5m" then
    local five_min_bucket = math.floor(current_time / 300) * 300
    local archive_key = string.format("archive:5m:%d", five_min_bucket)
    
    -- 获取最近5个1分钟的数据
    local minute_keys = {}
    for i = 0, 4 do
        local minute_ts = five_min_bucket - i * 60
        table.insert(minute_keys, string.format("archive:1m:%d:*", minute_ts))
    end
    
    -- 按通道聚合
    local channels_data = {}
    
    for _, pattern in ipairs(minute_keys) do
        local keys = scan_keys(pattern)
        for _, key in ipairs(keys) do
            local channel_id = string.match(key, "archive:1m:%d+:(%d+)")
            if channel_id then
                if not channels_data[channel_id] then
                    channels_data[channel_id] = {
                        voltage_values = {},
                        current_values = {},
                        power_values = {}
                    }
                end
                
                local data = redis.call("HGETALL", key)
                for i = 1, #data, 2 do
                    local field = data[i]
                    local value = tonumber(data[i + 1])
                    
                    if field == "voltage_avg" and value then
                        table.insert(channels_data[channel_id].voltage_values, value)
                    elseif field == "current_avg" and value then
                        table.insert(channels_data[channel_id].current_values, value)
                    elseif field == "power_avg" and value then
                        table.insert(channels_data[channel_id].power_values, value)
                    end
                end
            end
        end
    end
    
    -- 计算5分钟统计值
    for channel_id, data in pairs(channels_data) do
        local v_avg, _, _ = calculate_stats(data.voltage_values)
        local c_avg, _, _ = calculate_stats(data.current_values)
        local p_avg, _, _ = calculate_stats(data.power_values)
        
        if v_avg then
            local channel_archive_key = string.format("%s:%s", archive_key, channel_id)
            redis.call("HSET", channel_archive_key, "voltage_avg", string.format("%.6f", v_avg))
            
            if c_avg then
                redis.call("HSET", channel_archive_key, "current_avg", string.format("%.6f", c_avg))
            end
            
            if p_avg then
                redis.call("HSET", channel_archive_key, "power_avg", string.format("%.6f", p_avg))
            end
            
            redis.call("HSET", channel_archive_key, "timestamp", tostring(five_min_bucket))
            redis.call("EXPIRE", channel_archive_key, 7200)
        end
    end
    
    return string.format("5m aggregation completed for timestamp %d", five_min_bucket)

-- JSON格式推送到列表（用于复杂数据）
elseif action == "push_json" then
    local data = {
        timestamp = current_time,
        measurement = "custom_metrics",
        tags = {
            source = "lua_script",
            type = "aggregated"
        },
        fields = {
            test_value = 123.456
        }
    }
    
    local json_str = cjson.encode(data)
    redis.call("LPUSH", "archive:pending", json_str)
    
    return "JSON data pushed to archive:pending"

else
    return "Unknown action: " .. action
end