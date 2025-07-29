-- ComsRv 双向数据同步脚本
-- 负责 ComsRv 与其他服务间的数据路由
-- 保持简单，只做数据转发，不做复杂计算

local action = ARGV[1]

if action == "sync_measurement" then
    -- 正向同步：ComsRv 测量数据 → ModSrv/AlarmSrv
    local channel_id = ARGV[2]
    local telemetry_type = ARGV[3] -- m, s, c, a
    local point_id = ARGV[4]
    local value = ARGV[5]
    local timestamp = ARGV[6] or tostring(redis.call('TIME')[1])

    -- 构建映射键
    local mapping_key = string.format("mapping:comsrv:%s:%s:%s", channel_id, telemetry_type, point_id)
    local mapping = redis.call('GET', mapping_key)

    if mapping then
        -- 映射格式: "service:model_id:point_name" 例如 "modsrv:power_meter:voltage_a"
        local service, model_id, point_name = string.match(mapping, "([^:]+):([^:]+):([^:]+)")

        if service and model_id and point_name then
            -- 更新目标服务的数据
            local target_key = string.format("%s:%s:measurement", service, model_id)
            redis.call('HSET', target_key, point_name, value)

            -- 发布更新通知
            local pub_channel = string.format("%s:%s:update", service, model_id)
            local message = string.format("%s:%s:%s", point_name, value, timestamp)
            redis.call('PUBLISH', pub_channel, message)

            -- 如果是告警服务，检查阈值
            if service == "alarmsrv" then
                local threshold_key = string.format("alarm:threshold:%s:%s", model_id, point_name)
                local threshold = redis.call('GET', threshold_key)
                if threshold and tonumber(value) > tonumber(threshold) then
                    -- 创建告警
                    local alarm_data = string.format("%s:%s:%s:%s", model_id, point_name, value, timestamp)
                    redis.call('LPUSH', 'alarm:queue', alarm_data)
                end
            end

            return 'OK'
        end
    end

    return 'NO_MAPPING'
elseif action == "sync_control" then
    -- 反向同步：ModSrv 控制命令 → ComsRv
    local model_id = ARGV[2]
    local control_name = ARGV[3]
    local value = ARGV[4]
    local timestamp = ARGV[5] or tostring(redis.call('TIME')[1])

    -- 查找反向映射
    local reverse_mapping_key = string.format("mapping:reverse:%s:%s", model_id, control_name)
    local mapping = redis.call('GET', reverse_mapping_key)

    if mapping then
        -- 映射格式: "channel_id:telemetry_type:point_id" 例如 "1001:c:1"
        local channel_id, telemetry_type, point_id = string.match(mapping, "([^:]+):([^:]+):([^:]+)")

        if channel_id and telemetry_type and point_id then
            -- 写入控制命令到 ComsRv 的命令队列
            local cmd_key = string.format("cmd:%s:%s", channel_id, telemetry_type)
            redis.call('HSET', cmd_key, point_id, value)

            -- 发布控制命令通知
            local pub_channel = string.format("cmd:%s:%s", channel_id, telemetry_type)
            local message = string.format("%s:%s:%s", point_id, value, timestamp)
            redis.call('PUBLISH', pub_channel, message)

            -- 记录命令历史
            local history_key = string.format("cmd:history:%s", channel_id)
            local history_entry = string.format("%s|%s|%s|%s|%s", timestamp, telemetry_type, point_id, value, model_id)
            redis.call('LPUSH', history_key, history_entry)
            redis.call('LTRIM', history_key, 0, 999) -- 保留最近1000条

            return 'OK'
        end
    end

    return 'NO_MAPPING'
elseif action == "batch_sync" then
    -- 批量同步：支持一次同步多个点位
    local updates = cjson.decode(ARGV[2])
    local success_count = 0
    local failed_count = 0

    for _, update in ipairs(updates) do
        -- 每个 update 包含: channel_id, telemetry_type, point_id, value
        local mapping_key = string.format("mapping:comsrv:%s:%s:%s",
            update.channel_id, update.telemetry_type, update.point_id)
        local mapping = redis.call('GET', mapping_key)

        if mapping then
            local service, model_id, point_name = string.match(mapping, "([^:]+):([^:]+):([^:]+)")
            if service and model_id and point_name then
                local target_key = string.format("%s:%s:measurement", service, model_id)
                redis.call('HSET', target_key, point_name, tostring(update.value))
                success_count = success_count + 1
            else
                failed_count = failed_count + 1
            end
        else
            failed_count = failed_count + 1
        end
    end

    -- 批量发布通知
    if success_count > 0 then
        redis.call('PUBLISH', 'sync:batch:complete', string.format("%d:%d", success_count, failed_count))
    end

    return string.format("SUCCESS:%d,FAILED:%d", success_count, failed_count)
elseif action == "get_mapping_stats" then
    -- 获取映射统计信息
    local comsrv_mappings = redis.call('KEYS', 'mapping:comsrv:*')
    local reverse_mappings = redis.call('KEYS', 'mapping:reverse:*')

    return string.format("FORWARD:%d,REVERSE:%d", #comsrv_mappings, #reverse_mappings)
else
    return 'UNKNOWN_ACTION'
end
