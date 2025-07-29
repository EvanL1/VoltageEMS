-- 数据处理脚本
-- 用于数据聚合、统计计算和批量处理
-- 支持 API Gateway 和历史数据处理需求

local action = ARGV[1]

-- 辅助函数：计算统计值
local function calculate_stats(values)
    if #values == 0 then
        return { count = 0 }
    end

    local sum = 0
    local min = values[1]
    local max = values[1]

    for _, v in ipairs(values) do
        sum = sum + v
        min = math.min(min, v)
        max = math.max(max, v)
    end

    local avg = sum / #values

    -- 计算标准差
    local variance = 0
    for _, v in ipairs(values) do
        variance = variance + (v - avg) ^ 2
    end
    local stddev = math.sqrt(variance / #values)

    return {
        count = #values,
        sum = sum,
        avg = avg,
        min = min,
        max = max,
        stddev = stddev
    }
end

-- 辅助函数：时间窗口对齐
local function align_timestamp(ts, window)
    -- window: "1m", "5m", "15m", "1h", "1d"
    local seconds = {
        ["1m"] = 60,
        ["5m"] = 300,
        ["15m"] = 900,
        ["1h"] = 3600,
        ["4h"] = 14400,
        ["1d"] = 86400
    }

    local window_seconds = seconds[window] or 60
    return math.floor(ts / window_seconds) * window_seconds
end

if action == "aggregate_data" then
    -- 聚合多个数据源的数据（API Gateway 使用）
    local request_id = ARGV[2]
    local sources = cjson.decode(ARGV[3]) -- [{service, model_id, points}]
    local options = ARGV[4] and cjson.decode(ARGV[4]) or {}

    local result = {
        request_id = request_id,
        timestamp = redis.call('TIME')[1],
        data = {}
    }

    for _, source in ipairs(sources) do
        local service = source.service
        local model_id = source.model_id
        local points = source.points or {}

        local key = string.format("%s:%s:measurement", service, model_id)
        local source_data = {}

        if #points > 0 then
            -- 获取指定点位
            for _, point in ipairs(points) do
                local value = redis.call('HGET', key, point)
                if value then
                    source_data[point] = tonumber(value) or value
                end
            end
        else
            -- 获取所有点位
            local all_data = redis.call('HGETALL', key)
            for i = 1, #all_data, 2 do
                source_data[all_data[i]] = tonumber(all_data[i + 1]) or all_data[i + 1]
            end
        end

        -- 应用过滤器
        if options.filters then
            for point, filter in pairs(options.filters) do
                if source_data[point] then
                    local value = source_data[point]
                    if filter.min and value < filter.min then
                        source_data[point] = nil
                    elseif filter.max and value > filter.max then
                        source_data[point] = nil
                    end
                end
            end
        end

        -- 应用转换
        if options.transforms then
            for point, transform in pairs(options.transforms) do
                if source_data[point] and transform.scale then
                    source_data[point] = source_data[point] * transform.scale
                end
                if source_data[point] and transform.offset then
                    source_data[point] = source_data[point] + transform.offset
                end
            end
        end

        result.data[string.format("%s:%s", service, model_id)] = source_data
    end

    -- 缓存结果（可选）
    if options.cache_ttl then
        local cache_key = string.format("cache:aggregate:%s", request_id)
        redis.call('SETEX', cache_key, options.cache_ttl, cjson.encode(result))
    end

    return cjson.encode(result)
elseif action == "calculate_statistics" then
    -- 计算统计数据
    local service = ARGV[2]
    local model_id = ARGV[3]
    local point_name = ARGV[4]
    local window = ARGV[5] or "5m"          -- 时间窗口
    local periods = tonumber(ARGV[6]) or 12 -- 历史周期数

    -- 获取历史数据
    local history_pattern = string.format("history:%s:%s:%s:*", service, model_id, point_name)
    local history_keys = redis.call('KEYS', history_pattern)

    local current_time = redis.call('TIME')[1]
    local window_start = align_timestamp(current_time, window)

    -- 按时间窗口分组数据
    local windowed_data = {}
    for _, key in ipairs(history_keys) do
        local timestamp = tonumber(string.match(key, ":(%d+)$"))
        if timestamp then
            local window_ts = align_timestamp(timestamp, window)
            if not windowed_data[window_ts] then
                windowed_data[window_ts] = {}
            end

            local value = redis.call('GET', key)
            if value then
                table.insert(windowed_data[window_ts], tonumber(value))
            end
        end
    end

    -- 计算每个窗口的统计值
    local stats_by_window = {}
    local sorted_windows = {}

    for window_ts, values in pairs(windowed_data) do
        table.insert(sorted_windows, window_ts)
        stats_by_window[window_ts] = calculate_stats(values)
    end

    table.sort(sorted_windows, function(a, b) return a > b end)

    -- 只保留最近的周期
    local recent_stats = {}
    for i = 1, math.min(periods, #sorted_windows) do
        local window_ts = sorted_windows[i]
        recent_stats[window_ts] = stats_by_window[window_ts]
    end

    -- 计算当前值
    local current_key = string.format("%s:%s:measurement", service, model_id)
    local current_value = redis.call('HGET', current_key, point_name)

    local result = {
        service = service,
        model_id = model_id,
        point_name = point_name,
        current_value = tonumber(current_value),
        window = window,
        statistics = recent_stats
    }

    return cjson.encode(result)
elseif action == "batch_calculate" then
    -- 批量计算（用于 ModSrv 的复杂计算）
    local calc_id = ARGV[2]
    local expression = ARGV[3]           -- 计算表达式
    local inputs = cjson.decode(ARGV[4]) -- 输入数据源

    -- 获取所有输入数据
    local values = {}
    for var_name, source in pairs(inputs) do
        local key = string.format("%s:%s:measurement", source.service, source.model_id)
        local value = redis.call('HGET', key, source.point)
        values[var_name] = tonumber(value) or 0
    end

    -- 简单的表达式计算（实际应用中可能需要更复杂的解析器）
    local result = 0

    if expression == "sum" then
        for _, v in pairs(values) do
            result = result + v
        end
    elseif expression == "average" then
        local sum = 0
        local count = 0
        for _, v in pairs(values) do
            sum = sum + v
            count = count + 1
        end
        result = count > 0 and (sum / count) or 0
    elseif expression == "max" then
        result = nil
        for _, v in pairs(values) do
            result = result and math.max(result, v) or v
        end
    elseif expression == "min" then
        result = nil
        for _, v in pairs(values) do
            result = result and math.min(result, v) or v
        end
    elseif string.find(expression, "custom:") then
        -- 自定义表达式（简化示例）
        local formula = string.sub(expression, 8)

        -- 替换变量
        for var, val in pairs(values) do
            formula = string.gsub(formula, var, tostring(val))
        end

        -- 安全执行（实际应用中需要更严格的沙箱）
        local fn = load("return " .. formula)
        if fn then
            local ok, res = pcall(fn)
            if ok then
                result = res
            end
        end
    end

    -- 存储计算结果
    local result_key = string.format("calc:result:%s", calc_id)
    redis.call('HSET', result_key, "value", result)
    redis.call('HSET', result_key, "timestamp", redis.call('TIME')[1])
    redis.call('HSET', result_key, "inputs", cjson.encode(values))

    -- 设置过期时间
    redis.call('EXPIRE', result_key, 3600) -- 1小时

    return tostring(result)
elseif action == "prepare_history_batch" then
    -- 准备历史数据批次（用于 HisSrv）
    local service = ARGV[2]
    local batch_size = tonumber(ARGV[3]) or 1000
    local older_than = tonumber(ARGV[4]) or 300 -- 默认5分钟前的数据

    local current_time = redis.call('TIME')[1]
    local cutoff_time = current_time - older_than

    -- 扫描需要归档的数据
    local pattern = string.format("%s:*:measurement", service)
    local keys = redis.call('KEYS', pattern)

    local batch = {
        batch_id = string.format("%s:%s", service, current_time),
        service = service,
        timestamp = current_time,
        data = {}
    }

    local count = 0
    for _, key in ipairs(keys) do
        if count >= batch_size then
            break
        end

        -- 提取 model_id
        local model_id = string.match(key, service .. ":([^:]+):measurement")
        if model_id then
            local data = redis.call('HGETALL', key)

            if #data > 0 then
                local model_data = {}
                for i = 1, #data, 2 do
                    model_data[data[i]] = data[i + 1]
                    count = count + 1

                    if count >= batch_size then
                        break
                    end
                end

                batch.data[model_id] = model_data
            end
        end
    end

    -- 将批次写入队列
    if count > 0 then
        local queue_key = string.format("history:batch:%s", service)
        redis.call('LPUSH', queue_key, cjson.encode(batch))

        -- 限制队列长度
        redis.call('LTRIM', queue_key, 0, 99)
    end

    return cjson.encode({
        batch_id = batch.batch_id,
        count = count,
        queued = count > 0
    })
elseif action == "cache_invalidate" then
    -- 缓存失效处理
    local pattern = ARGV[2]

    local keys = redis.call('KEYS', 'cache:' .. pattern)
    local deleted = 0

    if #keys > 0 then
        deleted = redis.call('DEL', unpack(keys))
    end

    return tostring(deleted)
elseif action == "get_data_summary" then
    -- 获取数据摘要（用于仪表板）
    local services = cjson.decode(ARGV[2]) -- 要统计的服务列表

    local summary = {
        timestamp = redis.call('TIME')[1],
        services = {}
    }

    for _, service in ipairs(services) do
        local service_stats = {
            models = 0,
            total_points = 0,
            active_alarms = 0
        }

        -- 统计模型数
        local model_pattern = string.format("%s:*:measurement", service)
        local model_keys = redis.call('KEYS', model_pattern)
        service_stats.models = #model_keys

        -- 统计数据点
        for _, key in ipairs(model_keys) do
            local point_count = redis.call('HLEN', key)
            service_stats.total_points = service_stats.total_points + point_count
        end

        -- 统计活跃告警
        local alarm_pattern = string.format("alarm:active:%s:*", service)
        local alarm_keys = redis.call('KEYS', alarm_pattern)
        service_stats.active_alarms = #alarm_keys

        summary.services[service] = service_stats
    end

    return cjson.encode(summary)
else
    return 'UNKNOWN_ACTION'
end
