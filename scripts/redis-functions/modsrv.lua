#!lua name=modsrv_engine

-- ========================================
-- ModSrv Lua引擎 - 模型管理核心功能
-- 支持 measurement/action 分离架构
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
    
    -- 验证必要字段
    if not model.mappings then
        return redis.error_reply("Model must have mappings")
    end
    
    -- 存储模型定义
    local model_key = 'modsrv:model:' .. model_id
    redis.call('SET', model_key, model_json)
    redis.call('SADD', 'modsrv:models', model_id)
    
    -- 如果有模板，添加到模板索引
    if model.template then
        redis.call('SADD', 'modsrv:models:by_template:' .. model.template, model_id)
    end
    
    -- 初始化measurement和action存储
    local measurement_key = 'modsrv:model:' .. model_id .. ':measurement'
    local action_key = 'modsrv:model:' .. model_id .. ':action'
    
    -- 设置初始更新时间戳
    local timestamp = redis.call('TIME')[1]
    redis.call('HSET', measurement_key, '__updated', timestamp)
    redis.call('HSET', action_key, '__updated', timestamp)
    
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
        if ok and model.template then
            redis.call('SREM', 'modsrv:models:by_template:' .. model.template, model_id)
        end
    end
    
    -- 删除模型及其数据
    redis.call('DEL', model_key)
    redis.call('DEL', 'modsrv:model:' .. model_id .. ':measurement')
    redis.call('DEL', 'modsrv:model:' .. model_id .. ':action')
    redis.call('SREM', 'modsrv:models', model_id)
    
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

-- ==================== 数据操作 ====================

-- 同步测量数据（从通道到模型）
local function modsrv_sync_measurement(keys, args)
    local model_id = keys[1]
    
    -- 获取模型定义
    local model_json = redis.call('GET', 'modsrv:model:' .. model_id)
    if not model_json then
        return redis.error_reply("Model not found")
    end
    
    local model = cjson.decode(model_json)
    if not model.mappings or not model.mappings.measurement then
        return cjson.encode({synced = 0})
    end
    
    local measurement_key = 'modsrv:model:' .. model_id .. ':measurement'
    local synced_count = 0
    
    -- 遍历measurement映射
    for name, mapping in pairs(model.mappings.measurement) do
        if mapping.channel and mapping.point_id and mapping.type then
            -- 构建源键
            local source_key = 'comsrv:' .. mapping.channel .. ':' .. mapping.type
            -- 获取值
            local value = redis.call('HGET', source_key, tostring(mapping.point_id))
            
            if value then
                -- 存储到模型measurement
                redis.call('HSET', measurement_key, name, value)
                synced_count = synced_count + 1
            end
        end
    end
    
    -- 更新时间戳
    if synced_count > 0 then
        redis.call('HSET', measurement_key, '__updated', redis.call('TIME')[1])
    end
    
    return cjson.encode({
        model_id = model_id,
        synced = synced_count,
        timestamp = redis.call('TIME')[1]
    })
end

-- 执行动作（从模型到通道）
local function modsrv_execute_action(keys, args)
    local model_id = keys[1]
    local action_name = args[1]
    local value = args[2]
    
    if not action_name or not value then
        return redis.error_reply("Action name and value required")
    end
    
    -- 获取模型定义
    local model_json = redis.call('GET', 'modsrv:model:' .. model_id)
    if not model_json then
        return redis.error_reply("Model not found")
    end
    
    local model = cjson.decode(model_json)
    if not model.mappings or not model.mappings.action then
        return redis.error_reply("Model has no action mappings")
    end
    
    -- 查找动作映射
    local mapping = model.mappings.action[action_name]
    if not mapping then
        return redis.error_reply("Action '" .. action_name .. "' not found")
    end
    
    -- 存储动作值
    local action_key = 'modsrv:model:' .. model_id .. ':action'
    redis.call('HSET', action_key, action_name, value)
    redis.call('HSET', action_key, '__updated', redis.call('TIME')[1])
    
    -- 写入目标通道
    if mapping.channel and mapping.point_id and mapping.type then
        local target_key = 'comsrv:' .. mapping.channel .. ':' .. mapping.type
        redis.call('HSET', target_key, tostring(mapping.point_id), value)
        
        -- 发布命令事件
        local event = cjson.encode({
            model_id = model_id,
            action = action_name,
            value = value,
            channel = mapping.channel,
            point_id = mapping.point_id,
            type = mapping.type,
            timestamp = redis.call('TIME')[1]
        })
        redis.call('PUBLISH', 'modsrv:action:' .. model_id, event)
        
        return cjson.encode({
            status = "OK",
            action = action_name,
            value = value,
            channel = mapping.channel,
            point_id = mapping.point_id
        })
    end
    
    return redis.error_reply("Invalid action mapping")
end

-- 获取模型数据（measurement和action）
local function modsrv_get_model_data(keys, args)
    local model_id = keys[1]
    local data_type = args[1]  -- 'measurement', 'action', or nil for both
    
    -- 检查模型是否存在
    local model_json = redis.call('GET', 'modsrv:model:' .. model_id)
    if not model_json then
        return cjson.encode({error = "Model not found"})
    end
    
    local model = cjson.decode(model_json)
    local result = {
        model_id = model_id,
        model_name = model.name,
        timestamp = redis.call('TIME')[1]
    }
    
    -- 获取measurement数据
    if not data_type or data_type == 'measurement' then
        local measurement_key = 'modsrv:model:' .. model_id .. ':measurement'
        local measurement_data = redis.call('HGETALL', measurement_key)
        
        local measurements = {}
        for i = 1, #measurement_data, 2 do
            measurements[measurement_data[i]] = measurement_data[i + 1]
        end
        result.measurement = measurements
    end
    
    -- 获取action数据
    if not data_type or data_type == 'action' then
        local action_key = 'modsrv:model:' .. model_id .. ':action'
        local action_data = redis.call('HGETALL', action_key)
        
        local actions = {}
        for i = 1, #action_data, 2 do
            actions[action_data[i]] = action_data[i + 1]
        end
        result.action = actions
    end
    
    return cjson.encode(result)
end

-- 批量同步所有模型的测量数据
local function modsrv_sync_all_measurements(keys, args)
    local model_ids = redis.call('SMEMBERS', 'modsrv:models')
    local results = {}
    
    for _, model_id in ipairs(model_ids) do
        -- 调用单个模型同步
        local sync_result = modsrv_sync_measurement({model_id}, {})
        local ok, result = pcall(cjson.decode, sync_result)
        
        if ok and not result.error then
            table.insert(results, {
                model_id = model_id,
                synced = result.synced
            })
        end
    end
    
    return cjson.encode({
        total_models = #model_ids,
        synced_models = #results,
        results = results,
        timestamp = redis.call('TIME')[1]
    })
end

-- ==================== 注册函数 ====================

-- 模板管理
redis.register_function('modsrv_upsert_template', modsrv_upsert_template)
redis.register_function('modsrv_get_template', modsrv_get_template)
redis.register_function('modsrv_delete_template', modsrv_delete_template)
redis.register_function('modsrv_list_templates', modsrv_list_templates)

-- 模型管理
redis.register_function('modsrv_upsert_model', modsrv_upsert_model)
redis.register_function('modsrv_get_model', modsrv_get_model)
redis.register_function('modsrv_delete_model', modsrv_delete_model)
redis.register_function('modsrv_list_models', modsrv_list_models)

-- 数据操作
redis.register_function('modsrv_get_model_data', modsrv_get_model_data)
redis.register_function('modsrv_sync_measurement', modsrv_sync_measurement)
redis.register_function('modsrv_execute_action', modsrv_execute_action)
redis.register_function('modsrv_sync_all_measurements', modsrv_sync_all_measurements)