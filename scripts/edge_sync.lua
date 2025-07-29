-- 边端设备数据同步脚本
-- 用于ComsRv和ModSrv之间的数据同步

local action = ARGV[1]

if action == "sync_measurement" then
    -- ComsRv测量数据同步到ModSrv
    local channel = ARGV[2]
    local point = ARGV[3]
    local value = ARGV[4]

    -- 查找映射
    local mapping_key = "mapping:c2m:" .. channel .. ":" .. point
    local mapping = redis.call('GET', mapping_key)

    if mapping then
        -- 映射格式: "model_id:point_name"
        local colon_pos = string.find(mapping, ":")
        if colon_pos then
            local model_id = string.sub(mapping, 1, colon_pos - 1)
            local point_name = string.sub(mapping, colon_pos + 1)

            -- 更新ModSrv数据
            local modsrv_key = "modsrv:" .. model_id .. ":measurement"
            redis.call('HSET', modsrv_key, point_name, value)

            -- 发布更新通知（用于WebSocket）
            redis.call('PUBLISH', 'modsrv:' .. model_id .. ':update', point_name .. ':' .. value)

            -- 简单的告警检查
            local threshold_key = "alarm:threshold:" .. model_id .. ":" .. point_name
            local threshold = redis.call('GET', threshold_key)
            if threshold and tonumber(value) > tonumber(threshold) then
                -- 创建告警
                local alarm_id = redis.call('INCR', 'alarm:counter')
                redis.call('LPUSH', 'alarm:queue',
                    alarm_id .. ':' .. model_id .. ':' .. point_name .. ':' .. value .. ':' .. redis.call('TIME')[1])
            end
        end
    end
elseif action == "send_control" then
    -- ModSrv控制命令同步到ComsRv
    local model_id = ARGV[2]
    local control_name = ARGV[3]
    local value = ARGV[4]

    -- 查找反向映射
    local mapping_key = "mapping:m2c:" .. model_id .. ":" .. control_name
    local mapping = redis.call('GET', mapping_key)

    if mapping then
        local colon_pos = string.find(mapping, ":")
        if colon_pos then
            local channel = string.sub(mapping, 1, colon_pos - 1)
            local point = string.sub(mapping, colon_pos + 1)

            -- 写入控制命令
            local cmd_key = "cmd:" .. channel .. ":control"
            redis.call('HSET', cmd_key, point, value)

            -- 发布控制命令
            redis.call('PUBLISH', cmd_key, point .. ':' .. value)
        end
    end
elseif action == "get_values" then
    -- 获取模型的所有当前值
    local model_id = ARGV[2]
    local modsrv_key = "modsrv:" .. model_id .. ":measurement"
    return redis.call('HGETALL', modsrv_key)
elseif action == "init_mapping" then
    -- 初始化映射（用于启动时加载）
    local mapping_type = ARGV[2] -- "c2m" or "m2c"
    local key = ARGV[3]
    local value = ARGV[4]

    redis.call('SET', 'mapping:' .. mapping_type .. ':' .. key, value)
    return 'OK'
elseif action == "clear_mappings" then
    -- 清理所有映射（用于重新加载）
    local keys = redis.call('KEYS', 'mapping:*')
    if #keys > 0 then
        redis.call('DEL', unpack(keys))
    end
    return #keys
end

return 'OK'
