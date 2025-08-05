#!lua name=modsrv_engine

-- ========================================
-- ModSrv Lua引擎
-- 模型管理和数据映射的高性能实现
-- ========================================

-- ==================== 模型实例管理 ====================

-- 创建或更新模型实例
local function model_upsert(keys, args)
    local model_id = keys[1]
    local model_json = args[1]
    
    if not model_id or not model_json then
        return redis.error_reply("Model ID and JSON required")
    end
    
    -- 验证JSON格式
    local ok, model = pcall(cjson.decode, model_json)
    if not ok then
        return redis.error_reply("Invalid JSON: " .. tostring(model))
    end
    
    -- 存储模型
    local model_key = 'modsrv:model:' .. model_id
    redis.call('SET', model_key, model_json)
    
    -- 更新索引
    redis.call('SADD', 'modsrv:models', model_id)
    
    -- 如果有模板，添加到模板索引
    if model.template then
        redis.call('SADD', 'modsrv:models:by_template:' .. model.template, model_id)
    end
    
    -- 创建反向映射
    if model.mapping then
        local channel = model.mapping.channel
        
        -- 数据点映射
        if model.mapping.data then
            for point_name, point_id in pairs(model.mapping.data) do
                local reverse_key = string.format("modsrv:reverse:%d:%d", channel, point_id)
                redis.call('SET', reverse_key, model_id .. ':' .. point_name)
            end
        end
        
        -- 动作映射
        if model.mapping.action then
            for action_name, point_id in pairs(model.mapping.action) do
                local reverse_key = string.format("modsrv:reverse:action:%d:%d", channel, point_id)
                redis.call('SET', reverse_key, model_id .. ':' .. action_name)
            end
        end
    end
    
    return redis.status_reply("OK")
end

-- 获取模型实例
local function model_get(keys, args)
    local model_id = keys[1]
    local model_key = 'modsrv:model:' .. model_id
    
    local model_json = redis.call('GET', model_key)
    if not model_json then
        return redis.error_reply("Model not found: " .. model_id)
    end
    
    return model_json
end

-- 删除模型实例
local function model_delete(keys, args)
    local model_id = keys[1]
    local model_key = 'modsrv:model:' .. model_id
    
    -- 获取模型用于清理映射
    local model_json = redis.call('GET', model_key)
    if model_json then
        local ok, model = pcall(cjson.decode, model_json)
        if ok and model.mapping then
            local channel = model.mapping.channel
            
            -- Clean data点反向映射
            if model.mapping.data then
                for _, point_id in pairs(model.mapping.data) do
                    redis.call('DEL', string.format("modsrv:reverse:%d:%d", channel, point_id))
                end
            end
            
            -- 清理动作反向映射
            if model.mapping.action then
                for _, point_id in pairs(model.mapping.action) do
                    redis.call('DEL', string.format("modsrv:reverse:action:%d:%d", channel, point_id))
                end
            end
        end
        
        -- 从模板索引中移除
        if ok and model.template then
            redis.call('SREM', 'modsrv:models:by_template:' .. model.template, model_id)
        end
    end
    
    -- 删除模型
    redis.call('DEL', model_key)
    redis.call('SREM', 'modsrv:models', model_id)
    
    return redis.status_reply("OK")
end

-- 列出模型实例
local function model_list(keys, args)
    local filter = args[1] and cjson.decode(args[1]) or {}
    local models = {}
    
    local model_ids
    if filter.template then
        -- 按模板过滤
        model_ids = redis.call('SMEMBERS', 'modsrv:models:by_template:' .. filter.template)
    else
        -- 所有模型
        model_ids = redis.call('SMEMBERS', 'modsrv:models')
    end
    
    for _, model_id in ipairs(model_ids) do
        local model_json = redis.call('GET', 'modsrv:model:' .. model_id)
        if model_json then
            table.insert(models, model_json)
        end
    end
    
    return cjson.encode(models)
end

-- ==================== 数据操作 ====================

-- 获取模型值（带映射）
local function model_get_value(keys, args)
    local model_id = keys[1]
    local point_name = keys[2]
    
    -- 获取模型
    local model_key = 'modsrv:model:' .. model_id
    local model_json = redis.call('GET', model_key)
    if not model_json then
        return redis.error_reply("Model not found")
    end
    
    local model = cjson.decode(model_json)
    if not model.mapping or not model.mapping.data then
        return redis.error_reply("Model has no data mapping")
    end
    
    local channel = model.mapping.channel
    local point_id = model.mapping.data[point_name]
    
    if not point_id then
        return redis.error_reply("Point '" .. point_name .. "' not found in model")
    end
    
    -- 获取实际值
    local value_key = string.format("comsrv:%d:T", channel)
    local value = redis.call('HGET', value_key, tostring(point_id))
    
    if not value then
        return redis.nil_bulk_reply()
    end
    
    -- 返回结果
    local result = {
        model_id = model_id,
        point_name = point_name,
        channel = channel,
        point_id = point_id,
        value = tonumber(value) or value,
        timestamp = redis.call('TIME')[1]
    }
    
    return cjson.encode(result)
end

-- 设置模型值（带映射）
local function model_set_value(keys, args)
    local model_id = keys[1]
    local point_name = keys[2]
    local value = args[1]
    
    if not value then
        return redis.error_reply("Value required")
    end
    
    -- 获取模型
    local model_json = redis.call('GET', 'modsrv:model:' .. model_id)
    if not model_json then
        return redis.error_reply("Model not found")
    end
    
    local model = cjson.decode(model_json)
    local channel = model.mapping.channel
    local point_id = model.mapping.action and model.mapping.action[point_name]
    
    if not point_id then
        -- 尝试在data映射中查找
        point_id = model.mapping.data and model.mapping.data[point_name]
        if not point_id then
            return redis.error_reply("Point '" .. point_name .. "' not found in model")
        end
    end
    
    -- 设置值
    local value_key = string.format("comsrv:%d:C", channel)  -- 控制通道
    redis.call('HSET', value_key, tostring(point_id), value)
    
    -- 发布变更通知
    local notification = {
        model_id = model_id,
        point_name = point_name,
        channel = channel,
        point_id = point_id,
        value = tonumber(value) or value,
        timestamp = redis.call('TIME')[1]
    }
    
    redis.call('PUBLISH', 'modsrv:value_changed', cjson.encode(notification))
    
    return redis.status_reply("OK")
end

-- 批量获取模型值
local function model_get_values_batch(keys, args)
    local model_id = keys[1]
    local point_names = args[1] and cjson.decode(args[1]) or {}
    
    -- 获取模型
    local model_json = redis.call('GET', 'modsrv:model:' .. model_id)
    if not model_json then
        return redis.error_reply("Model not found")
    end
    
    local model = cjson.decode(model_json)
    local channel = model.mapping.channel
    local results = {}
    
    -- 批量获取值
    for _, point_name in ipairs(point_names) do
        local point_id = model.mapping.data[point_name]
        if point_id then
            local value_key = string.format("comsrv:%d:T", channel)
            local value = redis.call('HGET', value_key, tostring(point_id))
            
            table.insert(results, {
                point_name = point_name,
                point_id = point_id,
                value = value and (tonumber(value) or value) or nil
            })
        end
    end
    
    return cjson.encode({
        model_id = model_id,
        channel = channel,
        values = results,
        timestamp = redis.call('TIME')[1]
    })
end

-- ==================== 模板操作 ====================

-- 从模板创建实例
local function model_create_from_template(keys, args)
    local model_id = keys[1]
    local template_id = keys[2]
    local params = args[1] and cjson.decode(args[1]) or {}
    
    -- 获取模板
    local template_key = 'modsrv:template:' .. template_id
    local template_json = redis.call('GET', template_key)
    if not template_json then
        return redis.error_reply("Template not found: " .. template_id)
    end
    
    local template = cjson.decode(template_json)
    
    -- 创建实例
    local model = {
        id = model_id,
        name = params.name or (template.name .. "_" .. model_id),
        template = template_id,
        mapping = {
            channel = params.channel or template.default_channel or 0,
            data = {},
            action = {}
        }
    }
    
    -- 复制映射并应用参数
    if template.data_points then
        for point_name, point_config in pairs(template.data_points) do
            local point_id = params.data_offset and (point_config.base_id + params.data_offset) or point_config.base_id
            model.mapping.data[point_name] = point_id
        end
    end
    
    if template.actions then
        for action_name, action_config in pairs(template.actions) do
            local point_id = params.action_offset and (action_config.base_id + params.action_offset) or action_config.base_id
            model.mapping.action[action_name] = point_id
        end
    end
    
    -- 保存模型
    return model_upsert({model_id}, {cjson.encode(model)})
end

-- ==================== 查询和统计 ====================

-- 通过点位查找模型
local function model_find_by_point(keys, args)
    local channel = tonumber(keys[1])
    local point_id = tonumber(keys[2])
    local point_type = args[1] or "data"  -- "data" or "action"
    
    local reverse_key
    if point_type == "action" then
        reverse_key = string.format("modsrv:reverse:action:%d:%d", channel, point_id)
    else
        reverse_key = string.format("modsrv:reverse:%d:%d", channel, point_id)
    end
    
    local mapping = redis.call('GET', reverse_key)
    if not mapping then
        return redis.nil_bulk_reply()
    end
    
    -- 解析 model_id:point_name
    local model_id, point_name = string.match(mapping, "([^:]+):(.+)")
    
    if model_id then
        local model_json = redis.call('GET', 'modsrv:model:' .. model_id)
        if model_json then
            return cjson.encode({
                model_id = model_id,
                point_name = point_name,
                channel = channel,
                point_id = point_id,
                model = cjson.decode(model_json)
            })
        end
    end
    
    return redis.nil_bulk_reply()
end

-- 获取模型统计信息
local function model_stats(keys, args)
    local total_models = redis.call('SCARD', 'modsrv:models')
    local templates = redis.call('KEYS', 'modsrv:models:by_template:*')
    local template_stats = {}
    
    for _, template_key in ipairs(templates) do
        local template_id = string.match(template_key, "modsrv:models:by_template:(.+)")
        if template_id then
            local count = redis.call('SCARD', template_key)
            table.insert(template_stats, {
                template = template_id,
                count = count
            })
        end
    end
    
    return cjson.encode({
        total_models = total_models,
        templates = template_stats,
        timestamp = redis.call('TIME')[1]
    })
end

-- ==================== 注册函数 ====================

redis.register_function('model_upsert', model_upsert)
redis.register_function('model_get', model_get)
redis.register_function('model_delete', model_delete)
redis.register_function('model_list', model_list)
redis.register_function('model_get_value', model_get_value)
redis.register_function('model_set_value', model_set_value)
redis.register_function('model_get_values_batch', model_get_values_batch)
redis.register_function('model_create_from_template', model_create_from_template)
redis.register_function('model_find_by_point', model_find_by_point)
redis.register_function('model_stats', model_stats)