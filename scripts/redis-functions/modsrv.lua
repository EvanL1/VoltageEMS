#!lua name=modsrv_engine

-- ========================================
-- ModSrv Lua引擎 (精简版)
-- 模型和模板管理的核心功能
-- ========================================

-- ==================== 模板管理 ====================

-- 创建或更新模板
local function modsrv_upsert_template(keys, args)
    local template_id = keys[1]
    local template_json = args[1]
    
    if not template_id or not template_json then
        return redis.error_reply("Template ID and JSON required")
    end
    
    -- 验证JSON格式
    local ok, template = pcall(cjson.decode, template_json)
    if not ok then
        return redis.error_reply("Invalid JSON: " .. tostring(template))
    end
    
    -- 存储模板
    local template_key = 'modsrv:template:' .. template_id
    redis.call('SET', template_key, template_json)
    redis.call('SADD', 'modsrv:templates', template_id)
    
    return redis.status_reply("OK")
end

-- 获取模板
local function modsrv_get_template(keys, args)
    local template_id = keys[1]
    local template_key = 'modsrv:template:' .. template_id
    
    local template_json = redis.call('GET', template_key)
    if not template_json then
        return cjson.encode({error = "Template not found"})
    end
    
    return template_json
end

-- 删除模板
local function modsrv_delete_template(keys, args)
    local template_id = keys[1]
    local template_key = 'modsrv:template:' .. template_id
    
    redis.call('DEL', template_key)
    redis.call('SREM', 'modsrv:templates', template_id)
    
    return redis.status_reply("OK")
end

-- 列出所有模板
local function modsrv_list_templates(keys, args)
    local template_ids = redis.call('SMEMBERS', 'modsrv:templates')
    local templates = {}
    
    for _, template_id in ipairs(template_ids) do
        local template_json = redis.call('GET', 'modsrv:template:' .. template_id)
        if template_json then
            table.insert(templates, cjson.decode(template_json))
        end
    end
    
    return cjson.encode(templates)
end

-- ==================== 模型管理 ====================

-- 创建或更新模型
local function modsrv_upsert_model(keys, args)
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
    redis.call('SADD', 'modsrv:models', model_id)
    
    -- 如果有模板，添加到模板索引
    if model.template_id then
        redis.call('SADD', 'modsrv:models:by_template:' .. model.template_id, model_id)
    end
    
    return redis.status_reply("OK")
end

-- 获取模型
local function modsrv_get_model(keys, args)
    local model_id = keys[1]
    local model_key = 'modsrv:model:' .. model_id
    
    local model_json = redis.call('GET', model_key)
    if not model_json then
        return cjson.encode({error = "Model not found"})
    end
    
    return model_json
end

-- 删除模型
local function modsrv_delete_model(keys, args)
    local model_id = keys[1]
    local model_key = 'modsrv:model:' .. model_id
    
    -- 获取模型用于清理索引
    local model_json = redis.call('GET', model_key)
    if model_json then
        local ok, model = pcall(cjson.decode, model_json)
        if ok and model.template_id then
            redis.call('SREM', 'modsrv:models:by_template:' .. model.template_id, model_id)
        end
    end
    
    -- 删除模型
    redis.call('DEL', model_key)
    redis.call('SREM', 'modsrv:models', model_id)
    
    -- 删除模型数据
    redis.call('DEL', 'modsrv:model:' .. model_id .. ':data')
    
    return redis.status_reply("OK")
end

-- 列出所有模型
local function modsrv_list_models(keys, args)
    local model_ids = redis.call('SMEMBERS', 'modsrv:models')
    local models = {}
    
    for _, model_id in ipairs(model_ids) do
        local model_json = redis.call('GET', 'modsrv:model:' .. model_id)
        if model_json then
            table.insert(models, cjson.decode(model_json))
        end
    end
    
    return cjson.encode(models)
end

-- ==================== 模型数据操作 ====================

-- 获取模型数据
local function modsrv_get_model_data(keys, args)
    local model_id = keys[1]
    
    -- 检查模型是否存在
    local model_key = 'modsrv:model:' .. model_id
    local model_json = redis.call('GET', model_key)
    if not model_json then
        return cjson.encode({error = "Model not found"})
    end
    
    local model = cjson.decode(model_json)
    
    -- 获取模型数据
    local data_key = 'modsrv:model:' .. model_id .. ':data'
    local data_fields = redis.call('HGETALL', data_key)
    
    -- 转换为键值对
    local data = {}
    for i = 1, #data_fields, 2 do
        data[data_fields[i]] = data_fields[i + 1]
    end
    
    -- 如果有映射，获取实时数据
    if model.channel_id then
        local telemetry_key = 'comsrv:' .. model.channel_id .. ':T'
        local signal_key = 'comsrv:' .. model.channel_id .. ':S'
        
        -- 获取遥测数据
        if model.telemetry_points then
            for _, point_id in ipairs(model.telemetry_points) do
                local value = redis.call('HGET', telemetry_key, tostring(point_id))
                if value then
                    data['T' .. point_id] = value
                end
            end
        end
        
        -- 获取信号数据
        if model.signal_points then
            for _, point_id in ipairs(model.signal_points) do
                local value = redis.call('HGET', signal_key, tostring(point_id))
                if value then
                    data['S' .. point_id] = value
                end
            end
        end
    end
    
    return cjson.encode({
        model_id = model_id,
        model_name = model.name,
        data = data,
        timestamp = redis.call('TIME')[1]
    })
end

-- ==================== 注册函数 ====================

redis.register_function('modsrv_upsert_template', modsrv_upsert_template)
redis.register_function('modsrv_get_template', modsrv_get_template)
redis.register_function('modsrv_delete_template', modsrv_delete_template)
redis.register_function('modsrv_list_templates', modsrv_list_templates)

redis.register_function('modsrv_upsert_model', modsrv_upsert_model)
redis.register_function('modsrv_get_model', modsrv_get_model)
redis.register_function('modsrv_delete_model', modsrv_delete_model)
redis.register_function('modsrv_list_models', modsrv_list_models)

redis.register_function('modsrv_get_model_data', modsrv_get_model_data)