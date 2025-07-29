-- 统一的数据同步脚本
-- 支持所有微服务间的数据同步
-- 兼容多种映射格式，支持灵活的数据路由

local action = ARGV[1]

-- 辅助函数：解析映射值
local function parse_mapping(mapping)
    -- 尝试三段格式 (service:model_id:point_name)
    local service, model_id, point_name = string.match(mapping, "([^:]+):([^:]+):([^:]+)")
    if service and model_id and point_name then
        return service, model_id, point_name
    end
    
    -- 尝试两段格式 (model_id:point_name)，默认服务为 modsrv
    local model_id, point_name = string.match(mapping, "([^:]+):([^:]+)")
    if model_id and point_name then
        return "modsrv", model_id, point_name
    end
    
    return nil, nil, nil
end

-- 辅助函数：查找映射（支持多种格式）
local function find_mapping(primary_key, fallback_key)
    local mapping = redis.call('GET', primary_key)
    if not mapping and fallback_key then
        mapping = redis.call('GET', fallback_key)
    end
    return mapping
end

if action == "sync_measurement" then
    -- 正向同步：ComsRv 测量数据 → ModSrv/AlarmSrv
    local channel_id = ARGV[2]
    local telemetry_type = ARGV[3] or "m"  -- 默认为测量类型
    local point_id = ARGV[4]
    local value = ARGV[5]
    local timestamp = ARGV[6] or tostring(redis.call('TIME')[1])
    
    -- 构建映射键（支持两种格式）
    local new_format_key = string.format("mapping:comsrv:%s:%s:%s", channel_id, telemetry_type, point_id)
    local old_format_key = string.format("mapping:c2m:%s:%s", channel_id, point_id)
    
    local mapping = find_mapping(new_format_key, old_format_key)
    
    if mapping then
        local service, model_id, point_name = parse_mapping(mapping)
        
        if service and model_id and point_name then
            -- 更新目标服务的数据
            local target_key = string.format("%s:%s:measurement", service, model_id)
            redis.call('HSET', target_key, point_name, value)
            
            -- 发布更新通知
            local pub_channel = string.format("%s:%s:update", service, model_id)
            local message = string.format("%s:%s:%s", point_name, value, timestamp)
            redis.call('PUBLISH', pub_channel, message)
            
            -- 告警检查（兼容两种格式）
            if service == "alarmsrv" or service == "modsrv" then
                local threshold_key = string.format("alarm:threshold:%s:%s", model_id, point_name)
                local threshold = redis.call('GET', threshold_key)
                if threshold and tonumber(value) > tonumber(threshold) then
                    -- 创建告警（兼容两种格式）
                    local alarm_id = redis.call('INCR', 'alarm:counter')
                    local alarm_data = string.format("%s:%s:%s:%s:%s", 
                        alarm_id or timestamp, model_id, point_name, value, timestamp)
                    redis.call('LPUSH', 'alarm:queue', alarm_data)
                end
            end
            
            return 'OK'
        end
    end
    
    return 'NO_MAPPING'

elseif action == "send_control" or action == "sync_control" then
    -- 反向同步：ModSrv 控制命令 → ComsRv
    local model_id = ARGV[2]
    local control_name = ARGV[3]
    local value = ARGV[4]
    local timestamp = ARGV[5] or tostring(redis.call('TIME')[1])
    
    -- 查找反向映射（支持两种格式）
    local new_format_key = string.format("mapping:reverse:%s:%s", model_id, control_name)
    local old_format_key = string.format("mapping:m2c:%s:%s", model_id, control_name)
    
    local mapping = find_mapping(new_format_key, old_format_key)
    
    if mapping then
        -- 解析映射（支持两种格式）
        local channel_id, telemetry_type, point_id
        
        -- 尝试三段格式 (channel_id:telemetry_type:point_id)
        channel_id, telemetry_type, point_id = string.match(mapping, "([^:]+):([^:]+):([^:]+)")
        
        if not channel_id then
            -- 尝试两段格式 (channel:point)，默认类型为控制
            channel_id, point_id = string.match(mapping, "([^:]+):([^:]+)")
            telemetry_type = "c"  -- 默认为控制类型
        end
        
        if channel_id and point_id then
            -- 写入控制命令
            local cmd_key = string.format("cmd:%s:%s", channel_id, telemetry_type or "control")
            redis.call('HSET', cmd_key, point_id, value)
            
            -- 发布控制命令通知
            redis.call('PUBLISH', cmd_key, string.format("%s:%s:%s", point_id, value, timestamp))
            
            -- 记录命令历史
            local history_key = string.format("cmd:history:%s", channel_id)
            local history_entry = string.format("%s|%s|%s|%s|%s", 
                timestamp, telemetry_type or "control", point_id, value, model_id)
            redis.call('LPUSH', history_key, history_entry)
            redis.call('LTRIM', history_key, 0, 999)
            
            return 'OK'
        end
    end
    
    return 'NO_MAPPING'

elseif action == "batch_sync" then
    -- 批量同步
    local updates = cjson.decode(ARGV[2])
    local success_count = 0
    local failed_count = 0
    
    for _, update in ipairs(updates) do
        local channel_id = update.channel_id or update.channel
        local telemetry_type = update.telemetry_type or "m"
        local point_id = update.point_id or update.point
        local value = update.value
        
        -- 构建映射键
        local new_format_key = string.format("mapping:comsrv:%s:%s:%s", 
            channel_id, telemetry_type, point_id)
        local old_format_key = string.format("mapping:c2m:%s:%s", channel_id, point_id)
        
        local mapping = find_mapping(new_format_key, old_format_key)
        
        if mapping then
            local service, model_id, point_name = parse_mapping(mapping)
            if service and model_id and point_name then
                local target_key = string.format("%s:%s:measurement", service, model_id)
                redis.call('HSET', target_key, point_name, tostring(value))
                success_count = success_count + 1
            else
                failed_count = failed_count + 1
            end
        else
            failed_count = failed_count + 1
        end
    end
    
    if success_count > 0 then
        redis.call('PUBLISH', 'sync:batch:complete', string.format("%d:%d", success_count, failed_count))
    end
    
    return string.format("SUCCESS:%d,FAILED:%d", success_count, failed_count)

elseif action == "get_values" then
    -- 获取模型的所有当前值
    local model_id = ARGV[2]
    local service = ARGV[3] or "modsrv"
    local data_type = ARGV[4] or "measurement"
    local key = string.format("%s:%s:%s", service, model_id, data_type)
    return redis.call('HGETALL', key)

elseif action == "init_mapping" then
    -- 初始化映射
    local mapping_type = ARGV[2]  -- "c2m", "m2c", "comsrv", "reverse"
    local key = ARGV[3]
    local value = ARGV[4]
    
    -- 根据类型构建完整的映射键
    local full_key
    if mapping_type == "c2m" or mapping_type == "m2c" then
        full_key = string.format("mapping:%s:%s", mapping_type, key)
    elseif mapping_type == "comsrv" or mapping_type == "reverse" then
        full_key = string.format("mapping:%s:%s", mapping_type, key)
    else
        full_key = string.format("mapping:%s", key)
    end
    
    redis.call('SET', full_key, value)
    return 'OK'

elseif action == "clear_mappings" then
    -- 清理映射
    local pattern = ARGV[2] or "*"  -- 可选的模式参数
    local keys = redis.call('KEYS', 'mapping:' .. pattern)
    if #keys > 0 then
        redis.call('DEL', unpack(keys))
    end
    return #keys

elseif action == "get_mapping_stats" then
    -- 获取映射统计（支持所有格式）
    local c2m_mappings = redis.call('KEYS', 'mapping:c2m:*')
    local m2c_mappings = redis.call('KEYS', 'mapping:m2c:*')
    local comsrv_mappings = redis.call('KEYS', 'mapping:comsrv:*')
    local reverse_mappings = redis.call('KEYS', 'mapping:reverse:*')
    
    local forward_total = #c2m_mappings + #comsrv_mappings
    local reverse_total = #m2c_mappings + #reverse_mappings
    
    return string.format("FORWARD:%d,REVERSE:%d,C2M:%d,M2C:%d,COMSRV:%d,REVERSE:%d", 
        forward_total, reverse_total, #c2m_mappings, #m2c_mappings, 
        #comsrv_mappings, #reverse_mappings)

elseif action == "sync_to_history" then
    -- 同步数据到历史服务（供 HisSrv 使用）
    local service = ARGV[2]
    local model_id = ARGV[3]
    local batch_size = tonumber(ARGV[4]) or 100
    
    local key = string.format("%s:%s:measurement", service, model_id)
    local data = redis.call('HGETALL', key)
    
    if #data > 0 then
        local timestamp = redis.call('TIME')[1]
        local history_key = string.format("history:pending:%s:%s", service, model_id)
        
        -- 批量准备历史数据
        local batch = {}
        for i = 1, #data, 2 do
            local point_name = data[i]
            local value = data[i + 1]
            table.insert(batch, string.format("%s:%s:%s:%s", model_id, point_name, value, timestamp))
            
            if #batch >= batch_size then
                -- 写入批次
                redis.call('LPUSH', history_key, unpack(batch))
                batch = {}
            end
        end
        
        -- 写入剩余数据
        if #batch > 0 then
            redis.call('LPUSH', history_key, unpack(batch))
        end
        
        return #data / 2  -- 返回处理的点位数
    end
    
    return 0

elseif action == "sync_to_cloud" then
    -- 同步数据到云端（供 NetSrv 使用）
    local model_id = ARGV[2]
    local cloud_type = ARGV[3] or "default"
    
    -- 获取需要上云的数据点
    local mapping_pattern = string.format("mapping:cloud:%s:*", model_id)
    local cloud_points = redis.call('KEYS', mapping_pattern)
    
    if #cloud_points > 0 then
        local cloud_data = {}
        
        for _, mapping_key in ipairs(cloud_points) do
            local point_info = redis.call('GET', mapping_key)
            if point_info then
                local service, src_model, point_name = parse_mapping(point_info)
                if service and src_model and point_name then
                    local data_key = string.format("%s:%s:measurement", service, src_model)
                    local value = redis.call('HGET', data_key, point_name)
                    if value then
                        table.insert(cloud_data, {
                            point = point_name,
                            value = value,
                            timestamp = redis.call('TIME')[1]
                        })
                    end
                end
            end
        end
        
        if #cloud_data > 0 then
            -- 写入云端队列
            local cloud_queue = string.format("cloud:queue:%s", cloud_type)
            redis.call('LPUSH', cloud_queue, cjson.encode({
                model_id = model_id,
                data = cloud_data
            }))
            
            return #cloud_data
        end
    end
    
    return 0

elseif action == "route_to_service" then
    -- 通用服务路由（支持任意服务间的数据传递）
    local src_service = ARGV[2]
    local src_id = ARGV[3]
    local dst_service = ARGV[4]
    local dst_id = ARGV[5]
    local data_type = ARGV[6] or "measurement"
    
    -- 读取源数据
    local src_key = string.format("%s:%s:%s", src_service, src_id, data_type)
    local data = redis.call('HGETALL', src_key)
    
    if #data > 0 then
        -- 查找路由映射
        local route_pattern = string.format("mapping:route:%s:%s:%s:%s:*", 
            src_service, src_id, dst_service, dst_id)
        local routes = redis.call('KEYS', route_pattern)
        
        local routed_count = 0
        for _, route_key in ipairs(routes) do
            local route_info = redis.call('GET', route_key)
            if route_info then
                -- 路由格式: "src_point:dst_point"
                local src_point, dst_point = string.match(route_info, "([^:]+):([^:]+)")
                if src_point and dst_point then
                    -- 查找源数据
                    for i = 1, #data, 2 do
                        if data[i] == src_point then
                            -- 写入目标
                            local dst_key = string.format("%s:%s:%s", dst_service, dst_id, data_type)
                            redis.call('HSET', dst_key, dst_point, data[i + 1])
                            routed_count = routed_count + 1
                            break
                        end
                    end
                end
            end
        end
        
        -- 发布路由完成通知
        if routed_count > 0 then
            redis.call('PUBLISH', string.format("%s:%s:routed", dst_service, dst_id), 
                string.format("%s:%s:%d", src_service, src_id, routed_count))
        end
        
        return routed_count
    end
    
    return 0

elseif action == "get_service_status" then
    -- 获取服务状态（用于监控）
    local service = ARGV[2]
    
    -- 统计各类数据
    local patterns = {
        models = service .. ":*:measurement",
        alarms = service .. ":*:alarm",
        controls = service .. ":*:control",
        mappings = "mapping:*:" .. service .. ":*"
    }
    
    local stats = {}
    for type, pattern in pairs(patterns) do
        local keys = redis.call('KEYS', pattern)
        stats[type] = #keys
    end
    
    return cjson.encode(stats)

else
    return 'UNKNOWN_ACTION'
end