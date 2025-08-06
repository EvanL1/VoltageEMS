#!lua name=sync_engine

-- ========================================
-- 通用数据同步引擎
-- 配置驱动的服务间数据同步机制
-- ========================================

local cjson = require('cjson')

-- ==================== 配置管理 ====================

-- 存储同步规则配置
local function sync_config_set(keys, args)
    local rule_id = keys[1]
    local config_json = args[1]
    
    if not rule_id or not config_json then
        return redis.error_reply("Rule ID and configuration required")
    end
    
    -- 验证配置格式
    local ok, config = pcall(cjson.decode, config_json)
    if not ok then
        return redis.error_reply("Invalid JSON configuration: " .. tostring(config))
    end
    
    -- 验证必要字段
    if not config.source or not config.target then
        return redis.error_reply("Configuration must have 'source' and 'target' sections")
    end
    
    -- 存储配置
    local config_key = 'sync:config:' .. rule_id
    redis.call('SET', config_key, config_json)
    
    -- 添加到规则索引
    redis.call('SADD', 'sync:rules', rule_id)
    
    -- 如果启用，添加到活动规则
    if config.enabled ~= false then
        redis.call('SADD', 'sync:rules:active', rule_id)
    end
    
    return redis.status_reply("OK")
end

-- 获取同步规则配置
local function sync_config_get(keys, args)
    local rule_id = keys[1]
    
    if not rule_id then
        -- 返回所有规则
        local rules = redis.call('SMEMBERS', 'sync:rules')
        local configs = {}
        for _, id in ipairs(rules) do
            local config_json = redis.call('GET', 'sync:config:' .. id)
            if config_json then
                table.insert(configs, {
                    rule_id = id,
                    config = cjson.decode(config_json)
                })
            end
        end
        return cjson.encode(configs)
    end
    
    -- 返回特定规则
    local config_key = 'sync:config:' .. rule_id
    local config_json = redis.call('GET', config_key)
    
    if not config_json then
        return redis.error_reply("Rule not found: " .. rule_id)
    end
    
    return config_json
end

-- 删除同步规则
local function sync_config_delete(keys, args)
    local rule_id = keys[1]
    
    if not rule_id then
        return redis.error_reply("Rule ID required")
    end
    
    -- 删除配置
    redis.call('DEL', 'sync:config:' .. rule_id)
    
    -- 从索引中移除
    redis.call('SREM', 'sync:rules', rule_id)
    redis.call('SREM', 'sync:rules:active', rule_id)
    
    -- 清理相关的反向映射
    local reverse_keys = redis.call('KEYS', 'sync:reverse:' .. rule_id .. ':*')
    for _, key in ipairs(reverse_keys) do
        redis.call('DEL', key)
    end
    
    return redis.status_reply("OK")
end

-- ==================== 数据映射 ====================

-- 解析模式中的变量
local function parse_pattern_variables(pattern, values)
    -- 替换 $1, $2 等变量
    local result = pattern
    for i, value in ipairs(values) do
        result = string.gsub(result, "$" .. i, value)
    end
    
    -- 替换命名变量 ${name}
    if type(values) == "table" then
        for key, value in pairs(values) do
            if type(key) == "string" then
                result = string.gsub(result, "${" .. key .. "}", tostring(value))
            end
        end
    end
    
    return result
end

-- 从源键提取变量
local function extract_variables_from_key(source_pattern, actual_key)
    -- 简单的模式匹配：将 * 转换为捕获组
    local lua_pattern = string.gsub(source_pattern, "*", "(.-)")
    lua_pattern = string.gsub(lua_pattern, ":", "%%:")
    
    local captures = {string.match(actual_key, "^" .. lua_pattern .. "$")}
    return captures
end

-- ==================== 数据转换 ====================

-- 内置转换函数
local transform_functions = {}

-- 直接映射（不转换）
transform_functions.direct = function(value, config)
    return value
end

-- 数值转换
transform_functions.numeric = function(value, config)
    local num = tonumber(value)
    if not num then
        return nil
    end
    
    -- 应用缩放和偏移
    if config.scale then
        num = num * config.scale
    end
    if config.offset then
        num = num + config.offset
    end
    
    return tostring(num)
end

-- JSON 字段提取
transform_functions.json_extract = function(value, config)
    local ok, data = pcall(cjson.decode, value)
    if not ok then
        return value
    end
    
    -- 提取指定字段
    if config.field then
        return data[config.field]
    end
    
    return value
end

-- 应用转换函数
local function apply_transform(value, transform_config)
    if not transform_config or not transform_config.type then
        return value
    end
    
    local func = transform_functions[transform_config.type]
    if func then
        return func(value, transform_config)
    end
    
    -- 如果指定了自定义函数，尝试调用
    if transform_config.custom_function then
        -- 这里可以扩展支持自定义 Lua 函数
        return value
    end
    
    return value
end

-- ==================== 同步执行 ====================

-- 执行单个同步操作
local function sync_execute(keys, args)
    local rule_id = keys[1]
    local source_key = keys[2]
    local target_key = keys[3]
    
    -- 获取规则配置
    local config_json = redis.call('GET', 'sync:config:' .. rule_id)
    if not config_json then
        return redis.error_reply("Rule not found: " .. rule_id)
    end
    
    local config = cjson.decode(config_json)
    
    -- 如果规则未启用，跳过
    if config.enabled == false then
        return redis.status_reply("RULE_DISABLED")
    end
    
    -- 处理源数据
    local source_type = config.source.type or "hash"
    local target_type = config.target.type or "hash"
    local sync_count = 0
    
    if source_type == "hash" and target_type == "hash" then
        -- Hash 到 Hash 的同步
        local fields = {}
        
        if config.source.fields then
            -- 指定字段同步
            fields = config.source.fields
        else
            -- 获取所有字段
            local all_fields = redis.call('HKEYS', source_key)
            fields = all_fields
        end
        
        -- 同步每个字段
        for _, field in ipairs(fields) do
            local value = redis.call('HGET', source_key, field)
            if value then
                -- 应用字段映射
                local target_field = field
                if config.field_mapping and config.field_mapping[field] then
                    target_field = config.field_mapping[field]
                end
                
                -- 应用转换
                if config.transform then
                    value = apply_transform(value, config.transform)
                end
                
                -- 写入目标
                redis.call('HSET', target_key, target_field, value)
                sync_count = sync_count + 1
                
                -- 建立反向映射
                if config.reverse_mapping and config.reverse_mapping.enabled then
                    local reverse_key = string.format("sync:reverse:%s:%s:%s", 
                        rule_id, source_key, field)
                    redis.call('SET', reverse_key, 
                        string.format("%s:%s", target_key, target_field))
                end
            end
        end
        
    elseif source_type == "string" and target_type == "string" then
        -- String 到 String 的同步
        local value = redis.call('GET', source_key)
        if value then
            -- 应用转换
            if config.transform then
                value = apply_transform(value, config.transform)
            end
            
            redis.call('SET', target_key, value)
            sync_count = 1
        end
        
    elseif source_type == "hash" and target_type == "string" then
        -- Hash 到 String 的同步（可能需要序列化）
        local hash_data = redis.call('HGETALL', source_key)
        local data = {}
        
        for i = 1, #hash_data, 2 do
            data[hash_data[i]] = hash_data[i + 1]
        end
        
        if next(data) then
            local value = cjson.encode(data)
            
            -- 应用转换
            if config.transform then
                value = apply_transform(value, config.transform)
            end
            
            redis.call('SET', target_key, value)
            sync_count = 1
        end
    end
    
    -- 更新统计
    redis.call('HINCRBY', 'sync:stats:' .. rule_id, 'sync_count', sync_count)
    redis.call('HSET', 'sync:stats:' .. rule_id, 'last_sync', redis.call('TIME')[1])
    
    return cjson.encode({
        rule_id = rule_id,
        source_key = source_key,
        target_key = target_key,
        sync_count = sync_count
    })
end

-- 批量同步执行
local function sync_batch_execute(keys, args)
    local rule_id = keys[1]
    local batch_json = args[1]
    
    if not batch_json then
        return redis.error_reply("Batch data required")
    end
    
    local ok, batch = pcall(cjson.decode, batch_json)
    if not ok then
        return redis.error_reply("Invalid batch JSON")
    end
    
    local results = {}
    local total_synced = 0
    
    for _, item in ipairs(batch) do
        if item.source_key and item.target_key then
            local result = sync_execute({rule_id, item.source_key, item.target_key}, {})
            if type(result) == "string" then
                local sync_result = cjson.decode(result)
                total_synced = total_synced + sync_result.sync_count
                table.insert(results, sync_result)
            end
        end
    end
    
    return cjson.encode({
        rule_id = rule_id,
        batch_size = #batch,
        total_synced = total_synced,
        results = results
    })
end

-- 基于模式的自动同步
local function sync_pattern_execute(keys, args)
    local rule_id = keys[1]
    
    -- 获取规则配置
    local config_json = redis.call('GET', 'sync:config:' .. rule_id)
    if not config_json then
        return redis.error_reply("Rule not found: " .. rule_id)
    end
    
    local config = cjson.decode(config_json)
    
    -- 查找匹配源模式的所有键
    local source_pattern = config.source.pattern or "*"
    local source_keys = redis.call('KEYS', source_pattern)
    
    local sync_results = {}
    local total_synced = 0
    
    for _, source_key in ipairs(source_keys) do
        -- 从源键提取变量
        local variables = extract_variables_from_key(config.source.pattern, source_key)
        
        -- 构建目标键
        local target_key = parse_pattern_variables(config.target.pattern, variables)
        
        -- 执行同步
        local result = sync_execute({rule_id, source_key, target_key}, {})
        if type(result) == "string" then
            local sync_result = cjson.decode(result)
            total_synced = total_synced + sync_result.sync_count
            table.insert(sync_results, sync_result)
        end
    end
    
    return cjson.encode({
        rule_id = rule_id,
        matched_keys = #source_keys,
        total_synced = total_synced,
        results = sync_results
    })
end

-- ==================== 反向查询 ====================

-- 通过目标查找源
local function sync_reverse_lookup(keys, args)
    local rule_id = keys[1]
    local target_key = keys[2]
    local target_field = keys[3]
    
    -- 查找反向映射
    local reverse_pattern = string.format("sync:reverse:%s:*", rule_id)
    local reverse_keys = redis.call('KEYS', reverse_pattern)
    
    for _, reverse_key in ipairs(reverse_keys) do
        local mapping = redis.call('GET', reverse_key)
        if mapping then
            local mapped_key, mapped_field = string.match(mapping, "([^:]+):(.+)")
            if mapped_key == target_key and 
               (not target_field or mapped_field == target_field) then
                -- 提取源信息
                local source_info = string.match(reverse_key, 
                    "sync:reverse:" .. rule_id .. ":(.+)")
                return cjson.encode({
                    rule_id = rule_id,
                    source = source_info,
                    target = mapping
                })
            end
        end
    end
    
    return redis.nil_bulk_reply()
end

-- ==================== 统计和监控 ====================

-- 获取同步统计信息
local function sync_stats_get(keys, args)
    local rule_id = keys[1]
    
    if rule_id then
        -- 特定规则的统计
        local stats = redis.call('HGETALL', 'sync:stats:' .. rule_id)
        local result = {}
        
        for i = 1, #stats, 2 do
            result[stats[i]] = stats[i + 1]
        end
        
        return cjson.encode(result)
    else
        -- 所有规则的统计
        local rules = redis.call('SMEMBERS', 'sync:rules')
        local all_stats = {}
        
        for _, id in ipairs(rules) do
            local stats = redis.call('HGETALL', 'sync:stats:' .. id)
            local rule_stats = {rule_id = id}
            
            for i = 1, #stats, 2 do
                rule_stats[stats[i]] = stats[i + 1]
            end
            
            table.insert(all_stats, rule_stats)
        end
        
        return cjson.encode(all_stats)
    end
end

-- 重置统计信息
local function sync_stats_reset(keys, args)
    local rule_id = keys[1]
    
    if rule_id then
        redis.call('DEL', 'sync:stats:' .. rule_id)
    else
        local rules = redis.call('SMEMBERS', 'sync:rules')
        for _, id in ipairs(rules) do
            redis.call('DEL', 'sync:stats:' .. id)
        end
    end
    
    return redis.status_reply("OK")
end

-- ==================== 特定服务集成 ====================

-- Comsrv 到 Modsrv 的专用同步
local function sync_comsrv_to_modsrv(keys, args)
    local channel_id = keys[1]
    local telemetry_type = keys[2]  -- T/S/C/A
    local updates_json = args[1]
    
    if not updates_json then
        return redis.error_reply("Updates required")
    end
    
    local updates = cjson.decode(updates_json)
    local sync_count = 0
    local results = {}
    
    for _, update in ipairs(updates) do
        local point_id = tostring(update.point_id)
        local value = update.value
        
        -- 查找反向映射
        local reverse_key = string.format("modsrv:reverse:%s:%s", channel_id, point_id)
        local mapping_info = redis.call('GET', reverse_key)
        
        if mapping_info then
            local model_id, point_name = string.match(mapping_info, "([^:]+):(.+)")
            if model_id and point_name then
                -- 根据遥测类型决定存储位置
                local storage_key = nil
                if telemetry_type == 'T' or telemetry_type == 'S' then
                    storage_key = string.format("modsrv:model:%s:measurement", model_id)
                elseif telemetry_type == 'C' or telemetry_type == 'A' then
                    storage_key = string.format("modsrv:model:%s:values", model_id)
                end
                
                if storage_key then
                    redis.call('HSET', storage_key, point_name, value)
                    sync_count = sync_count + 1
                    table.insert(results, {
                        model_id = model_id,
                        point_name = point_name,
                        value = value
                    })
                end
            end
        end
    end
    
    -- 更新时间戳
    if sync_count > 0 then
        local timestamp = redis.call('TIME')[1]
        for _, result in ipairs(results) do
            local storage_key = string.format("modsrv:model:%s:measurement", result.model_id)
            redis.call('HSET', storage_key, '__updated', timestamp)
        end
    end
    
    return cjson.encode({
        channel_id = channel_id,
        telemetry_type = telemetry_type,
        sync_count = sync_count,
        results = results
    })
end

-- ==================== 注册函数 ====================

-- 配置管理
redis.register_function('sync_config_set', sync_config_set)
redis.register_function('sync_config_get', sync_config_get)
redis.register_function('sync_config_delete', sync_config_delete)

-- 同步执行
redis.register_function('sync_execute', sync_execute)
redis.register_function('sync_batch_execute', sync_batch_execute)
redis.register_function('sync_pattern_execute', sync_pattern_execute)

-- 反向查询
redis.register_function('sync_reverse_lookup', sync_reverse_lookup)

-- 统计监控
redis.register_function('sync_stats_get', sync_stats_get)
redis.register_function('sync_stats_reset', sync_stats_reset)

-- 特定服务集成
redis.register_function('sync_comsrv_to_modsrv', sync_comsrv_to_modsrv)