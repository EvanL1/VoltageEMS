#!lua name=core

-- ========================================
-- VoltageEMS 核心通用函数 (80%逻辑)
-- ========================================

-- 通用实体存储
local function generic_store(keys, args)
    if #keys < 1 or #args < 2 then
        return redis.error_reply("Usage: entity_key entity_type entity_data [indexes_json]")
    end
    
    local entity_key = keys[1]
    local entity_type = args[1]
    local entity_data = args[2]
    local indexes = args[3] and cjson.decode(args[3]) or {}
    
    -- 存储实体数据
    redis.call('HSET', entity_key, 'type', entity_type, 'data', entity_data, 'updated_at', tostring(redis.call('TIME')[1]))
    
    -- 处理索引
    for _, idx in ipairs(indexes) do
        if idx.type == "single" then
            local idx_key = string.format("idx:%s:%s:%s", entity_type, idx.field, idx.value)
            redis.call('SADD', idx_key, entity_key)
            redis.call('EXPIRE', idx_key, idx.ttl or 86400)
        elseif idx.type == "sorted" then
            local idx_key = string.format("idx:%s:%s", entity_type, idx.field)
            redis.call('ZADD', idx_key, idx.score or 0, entity_key)
        elseif idx.type == "composite" then
            local values = {}
            for _, field in ipairs(idx.fields) do
                table.insert(values, idx.values[field] or "")
            end
            local idx_key = string.format("idx:%s:%s:%s", entity_type, table.concat(idx.fields, ":"), table.concat(values, ":"))
            redis.call('SADD', idx_key, entity_key)
            redis.call('EXPIRE', idx_key, idx.ttl or 86400)
        end
    end
    
    -- 更新统计
    redis.call('HINCRBY', string.format("stats:%s", entity_type), 'total_stored', 1)
    
    return redis.status_reply('OK')
end

-- 通用批量同步
local function generic_batch_sync(keys, args)
    if #args < 3 then
        return redis.error_reply("Usage: source_pattern dest_prefix transform_type [options_json]")
    end
    
    local source_pattern = args[1]
    local dest_prefix = args[2]
    local transform_type = args[3]
    local options = args[4] and cjson.decode(args[4]) or {}
    
    local cursor = "0"
    local synced_count = 0
    local batch_size = options.batch_size or 100
    
    repeat
        local result = redis.call('SCAN', cursor, 'MATCH', source_pattern, 'COUNT', batch_size)
        cursor = result[1]
        local keys = result[2]
        
        for _, key in ipairs(keys) do
            local key_type = redis.call('TYPE', key).ok
            local dest_key = dest_prefix .. ":" .. key:match("([^:]+)$")
            
            if key_type == "string" then
                local value = redis.call('GET', key)
                if transform_type == "copy" then
                    redis.call('SET', dest_key, value)
                elseif transform_type == "increment" then
                    redis.call('INCRBY', dest_key, tonumber(value) or 0)
                elseif transform_type == "append" then
                    redis.call('APPEND', dest_key, value)
                end
            elseif key_type == "hash" then
                local hash_data = redis.call('HGETALL', key)
                if transform_type == "copy" then
                    if #hash_data > 0 then
                        redis.call('HSET', dest_key, unpack(hash_data))
                    end
                elseif transform_type == "merge" then
                    for i = 1, #hash_data, 2 do
                        redis.call('HSET', dest_key, hash_data[i], hash_data[i+1])
                    end
                end
            elseif key_type == "list" then
                local list_data = redis.call('LRANGE', key, 0, -1)
                if transform_type == "copy" then
                    redis.call('DEL', dest_key)
                    for _, value in ipairs(list_data) do
                        redis.call('RPUSH', dest_key, value)
                    end
                elseif transform_type == "append" then
                    for _, value in ipairs(list_data) do
                        redis.call('RPUSH', dest_key, value)
                    end
                end
            end
            
            synced_count = synced_count + 1
            
            -- 设置过期时间
            if options.ttl then
                redis.call('EXPIRE', dest_key, options.ttl)
            end
        end
    until cursor == "0"
    
    return cjson.encode({
        status = "success",
        synced_count = synced_count
    })
end

-- 通用查询
local function generic_query(keys, args)
    if #args < 1 then
        return redis.error_reply("Usage: query_config_json")
    end
    
    local config = cjson.decode(args[1])
    local results = {}
    
    if config.type == "index" then
        -- 基于索引查询
        local idx_key = string.format("idx:%s:%s:%s", config.entity_type, config.field, config.value)
        local entity_keys = redis.call('SMEMBERS', idx_key)
        
        for _, entity_key in ipairs(entity_keys) do
            local data = redis.call('HGET', entity_key, 'data')
            if data then
                table.insert(results, {
                    key = entity_key,
                    data = cjson.decode(data)
                })
            end
        end
    elseif config.type == "range" then
        -- 范围查询（用于排序索引）
        local idx_key = string.format("idx:%s:%s", config.entity_type, config.field)
        local entity_keys = redis.call('ZRANGEBYSCORE', idx_key, config.min or '-inf', config.max or '+inf', 'LIMIT', config.offset or 0, config.limit or 100)
        
        for _, entity_key in ipairs(entity_keys) do
            local data = redis.call('HGET', entity_key, 'data')
            if data then
                table.insert(results, {
                    key = entity_key,
                    data = cjson.decode(data)
                })
            end
        end
    elseif config.type == "pattern" then
        -- 模式匹配查询
        local cursor = "0"
        local count = 0
        
        repeat
            local scan_result = redis.call('SCAN', cursor, 'MATCH', config.pattern, 'COUNT', 100)
            cursor = scan_result[1]
            
            for _, key in ipairs(scan_result[2]) do
                if count < (config.limit or 1000) then
                    local data = redis.call('HGET', key, 'data')
                    if data then
                        table.insert(results, {
                            key = key,
                            data = cjson.decode(data)
                        })
                        count = count + 1
                    end
                end
            end
        until cursor == "0" or count >= (config.limit or 1000)
    end
    
    -- 应用过滤器
    if config.filters then
        local filtered_results = {}
        for _, result in ipairs(results) do
            local match = true
            for field, expected in pairs(config.filters) do
                if result.data[field] ~= expected then
                    match = false
                    break
                end
            end
            if match then
                table.insert(filtered_results, result)
            end
        end
        results = filtered_results
    end
    
    -- 排序
    if config.sort_by then
        table.sort(results, function(a, b)
            local a_val = a.data[config.sort_by]
            local b_val = b.data[config.sort_by]
            if config.sort_order == "desc" then
                return a_val > b_val
            else
                return a_val < b_val
            end
        end)
    end
    
    -- 分页
    if config.page and config.page_size then
        local start_idx = (config.page - 1) * config.page_size + 1
        local end_idx = config.page * config.page_size
        local paged_results = {}
        for i = start_idx, math.min(end_idx, #results) do
            table.insert(paged_results, results[i])
        end
        results = paged_results
    end
    
    return cjson.encode({
        status = "success",
        count = #results,
        data = results
    })
end

-- 实体管理器
local function entity_manager(keys, args)
    if #args < 2 then
        return redis.error_reply("Usage: action entity_type [params...]")
    end
    
    local action = args[1]
    local entity_type = args[2]
    
    if action == "count" then
        local pattern = string.format("%s:*", entity_type)
        local count = 0
        local cursor = "0"
        
        repeat
            local result = redis.call('SCAN', cursor, 'MATCH', pattern, 'COUNT', 1000)
            cursor = result[1]
            count = count + #result[2]
        until cursor == "0"
        
        return count
        
    elseif action == "cleanup" then
        local ttl = tonumber(args[3] or 86400)
        local pattern = string.format("%s:*", entity_type)
        local cleaned = 0
        local cursor = "0"
        
        repeat
            local result = redis.call('SCAN', cursor, 'MATCH', pattern, 'COUNT', 100)
            cursor = result[1]
            
            for _, key in ipairs(result[2]) do
                local updated_at = redis.call('HGET', key, 'updated_at')
                if updated_at then
                    local age = redis.call('TIME')[1] - tonumber(updated_at)
                    if age > ttl then
                        redis.call('DEL', key)
                        cleaned = cleaned + 1
                    end
                end
            end
        until cursor == "0"
        
        return cleaned
        
    elseif action == "reindex" then
        -- 重建索引
        local pattern = string.format("%s:*", entity_type)
        local reindexed = 0
        local cursor = "0"
        
        -- 清理旧索引
        local idx_pattern = string.format("idx:%s:*", entity_type)
        local idx_cursor = "0"
        repeat
            local result = redis.call('SCAN', idx_cursor, 'MATCH', idx_pattern, 'COUNT', 100)
            idx_cursor = result[1]
            for _, idx_key in ipairs(result[2]) do
                redis.call('DEL', idx_key)
            end
        until idx_cursor == "0"
        
        -- 重建索引
        repeat
            local result = redis.call('SCAN', cursor, 'MATCH', pattern, 'COUNT', 100)
            cursor = result[1]
            
            for _, key in ipairs(result[2]) do
                local data = redis.call('HGET', key, 'data')
                if data then
                    local entity = cjson.decode(data)
                    -- 这里需要根据实体类型定义索引规则
                    reindexed = reindexed + 1
                end
            end
        until cursor == "0"
        
        return reindexed
    else
        return redis.error_reply("Unknown action: " .. action)
    end
end

-- 通用状态机
local function generic_state_machine(keys, args)
    if #keys < 1 or #args < 2 then
        return redis.error_reply("Usage: entity_key current_state transition [context_json]")
    end
    
    local entity_key = keys[1]
    local current_state = args[1]
    local transition = args[2]
    local context = args[3] and cjson.decode(args[3]) or {}
    
    -- 状态机定义（可以从配置中读取）
    local state_machines = {
        alarm = {
            states = {"active", "acknowledged", "resolved", "closed"},
            transitions = {
                active = {acknowledge = "acknowledged", resolve = "resolved"},
                acknowledged = {resolve = "resolved", reactivate = "active"},
                resolved = {close = "closed", reactivate = "active"},
                closed = {}
            }
        },
        rule = {
            states = {"draft", "active", "paused", "disabled"},
            transitions = {
                draft = {activate = "active"},
                active = {pause = "paused", disable = "disabled"},
                paused = {resume = "active", disable = "disabled"},
                disabled = {reactivate = "draft"}
            }
        }
    }
    
    -- 获取实体类型
    local entity_type = redis.call('HGET', entity_key, 'type')
    if not entity_type then
        return redis.error_reply("Entity not found")
    end
    
    local sm = state_machines[entity_type]
    if not sm then
        return redis.error_reply("No state machine defined for entity type: " .. entity_type)
    end
    
    -- 验证当前状态
    local actual_state = redis.call('HGET', entity_key, 'state')
    if actual_state ~= current_state then
        return redis.error_reply("State mismatch. Expected: " .. current_state .. ", Actual: " .. tostring(actual_state))
    end
    
    -- 验证转换
    local next_state = sm.transitions[current_state] and sm.transitions[current_state][transition]
    if not next_state then
        return redis.error_reply("Invalid transition: " .. transition .. " from state: " .. current_state)
    end
    
    -- 执行转换
    redis.call('HSET', entity_key, 'state', next_state, 'last_transition', transition, 'transition_time', tostring(redis.call('TIME')[1]))
    
    -- 记录状态历史
    local history_key = entity_key .. ":state_history"
    redis.call('LPUSH', history_key, cjson.encode({
        from = current_state,
        to = next_state,
        transition = transition,
        timestamp = redis.call('TIME')[1],
        context = context
    }))
    redis.call('LTRIM', history_key, 0, 99) -- 保留最近100条
    
    -- 发布状态变更事件
    redis.call('PUBLISH', string.format("state_change:%s", entity_type), cjson.encode({
        entity_key = entity_key,
        from = current_state,
        to = next_state,
        transition = transition
    }))
    
    return cjson.encode({
        status = "success",
        from = current_state,
        to = next_state,
        transition = transition
    })
end

-- 通用多维索引管理
local function generic_multi_index(keys, args)
    if #args < 2 then
        return redis.error_reply("Usage: action index_config_json [params...]")
    end
    
    local action = args[1]
    local config = cjson.decode(args[2])
    
    if action == "add" then
        local entity_key = args[3]
        local values = cjson.decode(args[4])
        
        -- 单字段索引
        if config.single_fields then
            for _, field in ipairs(config.single_fields) do
                if values[field] then
                    local idx_key = string.format("idx:%s:%s:%s", config.entity_type, field, values[field])
                    redis.call('SADD', idx_key, entity_key)
                    if config.ttl then
                        redis.call('EXPIRE', idx_key, config.ttl)
                    end
                end
            end
        end
        
        -- 复合索引
        if config.composite_indexes then
            for _, comp_idx in ipairs(config.composite_indexes) do
                local idx_values = {}
                local all_present = true
                
                for _, field in ipairs(comp_idx.fields) do
                    if values[field] then
                        table.insert(idx_values, values[field])
                    else
                        all_present = false
                        break
                    end
                end
                
                if all_present then
                    local idx_key = string.format("idx:%s:%s:%s", 
                        config.entity_type, 
                        table.concat(comp_idx.fields, "+"),
                        table.concat(idx_values, "+"))
                    redis.call('SADD', idx_key, entity_key)
                    if config.ttl then
                        redis.call('EXPIRE', idx_key, config.ttl)
                    end
                end
            end
        end
        
        -- 排序索引
        if config.sorted_fields then
            for _, field_config in ipairs(config.sorted_fields) do
                local field = field_config.field
                local score = tonumber(values[field]) or 0
                
                if field_config.transform == "timestamp" then
                    score = tonumber(values[field]) or redis.call('TIME')[1]
                end
                
                local idx_key = string.format("idx:%s:%s:sorted", config.entity_type, field)
                redis.call('ZADD', idx_key, score, entity_key)
            end
        end
        
        return redis.status_reply('OK')
        
    elseif action == "remove" then
        local entity_key = args[3]
        
        -- 删除所有相关索引
        local cursor = "0"
        local pattern = string.format("idx:%s:*", config.entity_type)
        
        repeat
            local result = redis.call('SCAN', cursor, 'MATCH', pattern, 'COUNT', 100)
            cursor = result[1]
            
            for _, idx_key in ipairs(result[2]) do
                local key_type = redis.call('TYPE', idx_key).ok
                if key_type == "set" then
                    redis.call('SREM', idx_key, entity_key)
                elseif key_type == "zset" then
                    redis.call('ZREM', idx_key, entity_key)
                end
            end
        until cursor == "0"
        
        return redis.status_reply('OK')
        
    elseif action == "query" then
        local query_params = cjson.decode(args[3])
        local results = {}
        
        if query_params.type == "intersection" then
            -- 多条件交集查询
            local idx_keys = {}
            for field, value in pairs(query_params.conditions) do
                table.insert(idx_keys, string.format("idx:%s:%s:%s", config.entity_type, field, value))
            end
            
            if #idx_keys > 0 then
                results = redis.call('SINTER', unpack(idx_keys))
            end
            
        elseif query_params.type == "union" then
            -- 多条件并集查询
            local idx_keys = {}
            for field, value in pairs(query_params.conditions) do
                table.insert(idx_keys, string.format("idx:%s:%s:%s", config.entity_type, field, value))
            end
            
            if #idx_keys > 0 then
                results = redis.call('SUNION', unpack(idx_keys))
            end
            
        elseif query_params.type == "range" then
            -- 范围查询
            local idx_key = string.format("idx:%s:%s:sorted", config.entity_type, query_params.field)
            results = redis.call('ZRANGEBYSCORE', idx_key, 
                query_params.min or '-inf', 
                query_params.max or '+inf',
                'LIMIT', query_params.offset or 0, query_params.limit or 100)
        end
        
        return cjson.encode(results)
        
    else
        return redis.error_reply("Unknown action: " .. action)
    end
end

-- 通用条件评估器
local function generic_condition_eval(keys, args)
    if #args < 2 then
        return redis.error_reply("Usage: conditions_json context_json")
    end
    
    local conditions = cjson.decode(args[1])
    local context = cjson.decode(args[2])
    
    -- 评估单个条件
    local function evaluate_single_condition(condition, ctx)
        local field_value = ctx[condition.field]
        local operator = condition.operator
        local expected = condition.value
        
        if operator == "eq" then
            return field_value == expected
        elseif operator == "ne" then
            return field_value ~= expected
        elseif operator == "gt" then
            return tonumber(field_value) > tonumber(expected)
        elseif operator == "gte" then
            return tonumber(field_value) >= tonumber(expected)
        elseif operator == "lt" then
            return tonumber(field_value) < tonumber(expected)
        elseif operator == "lte" then
            return tonumber(field_value) <= tonumber(expected)
        elseif operator == "in" then
            for _, v in ipairs(expected) do
                if field_value == v then
                    return true
                end
            end
            return false
        elseif operator == "not_in" then
            for _, v in ipairs(expected) do
                if field_value == v then
                    return false
                end
            end
            return true
        elseif operator == "contains" then
            return string.find(tostring(field_value), tostring(expected)) ~= nil
        elseif operator == "regex" then
            -- Redis Lua doesn't have regex, use pattern matching
            return string.match(tostring(field_value), tostring(expected)) ~= nil
        elseif operator == "exists" then
            return field_value ~= nil
        elseif operator == "not_exists" then
            return field_value == nil
        else
            return false
        end
    end
    
    -- 评估条件组
    local function evaluate_conditions(conds, ctx, logic)
        local results = {}
        
        for _, cond in ipairs(conds) do
            if cond.conditions then
                -- 嵌套条件组
                table.insert(results, evaluate_conditions(cond.conditions, ctx, cond.logic or "and"))
            else
                -- 单个条件
                table.insert(results, evaluate_single_condition(cond, ctx))
            end
        end
        
        if logic == "or" then
            for _, result in ipairs(results) do
                if result then
                    return true
                end
            end
            return false
        else -- and
            for _, result in ipairs(results) do
                if not result then
                    return false
                end
            end
            return true
        end
    end
    
    local result = evaluate_conditions(conditions.conditions or {conditions}, context, conditions.logic or "and")
    
    return cjson.encode({
        result = result,
        context = context
    })
end

-- 通用批量数据收集器
local function generic_batch_collect(keys, args)
    if #args < 1 then
        return redis.error_reply("Usage: collect_config_json")
    end
    
    local config = cjson.decode(args[1])
    local results = {}
    
    -- 收集数据
    for _, source in ipairs(config.sources) do
        if source.type == "keys" then
            -- 直接指定的键
            for _, key in ipairs(source.keys) do
                local key_type = redis.call('TYPE', key).ok
                local data = nil
                
                if key_type == "string" then
                    data = redis.call('GET', key)
                elseif key_type == "hash" then
                    data = redis.call('HGETALL', key)
                elseif key_type == "list" then
                    data = redis.call('LRANGE', key, 0, -1)
                elseif key_type == "set" then
                    data = redis.call('SMEMBERS', key)
                elseif key_type == "zset" then
                    data = redis.call('ZRANGE', key, 0, -1, 'WITHSCORES')
                end
                
                if data then
                    table.insert(results, {
                        key = key,
                        type = key_type,
                        data = data
                    })
                end
            end
            
        elseif source.type == "pattern" then
            -- 模式匹配
            local cursor = "0"
            local count = 0
            local limit = source.limit or 1000
            
            repeat
                local scan_result = redis.call('SCAN', cursor, 'MATCH', source.pattern, 'COUNT', 100)
                cursor = scan_result[1]
                
                for _, key in ipairs(scan_result[2]) do
                    if count < limit then
                        local key_type = redis.call('TYPE', key).ok
                        local data = nil
                        
                        if key_type == "string" then
                            data = redis.call('GET', key)
                        elseif key_type == "hash" then
                            data = redis.call('HGETALL', key)
                        end
                        
                        if data then
                            table.insert(results, {
                                key = key,
                                type = key_type,
                                data = data
                            })
                            count = count + 1
                        end
                    end
                end
            until cursor == "0" or count >= limit
            
        elseif source.type == "index" then
            -- 从索引收集
            local idx_key = string.format("idx:%s:%s:%s", source.entity_type, source.field, source.value)
            local entity_keys = redis.call('SMEMBERS', idx_key)
            
            for _, entity_key in ipairs(entity_keys) do
                local data = redis.call('HGETALL', entity_key)
                if #data > 0 then
                    table.insert(results, {
                        key = entity_key,
                        type = "hash",
                        data = data
                    })
                end
            end
        end
    end
    
    -- 应用转换
    if config.transform then
        local transformed_results = {}
        
        for _, result in ipairs(results) do
            if config.transform == "flatten" and result.type == "hash" then
                -- 将hash扁平化为对象
                local obj = {}
                for i = 1, #result.data, 2 do
                    obj[result.data[i]] = result.data[i + 1]
                end
                table.insert(transformed_results, {
                    key = result.key,
                    data = obj
                })
            else
                table.insert(transformed_results, result)
            end
        end
        
        results = transformed_results
    end
    
    -- 聚合
    if config.aggregate then
        local aggregated = {}
        
        if config.aggregate.type == "count" then
            aggregated.count = #results
        elseif config.aggregate.type == "group_by" then
            aggregated.groups = {}
            
            for _, result in ipairs(results) do
                -- 假设数据已经被扁平化
                local group_value = result.data[config.aggregate.field]
                if group_value then
                    if not aggregated.groups[group_value] then
                        aggregated.groups[group_value] = {}
                    end
                    table.insert(aggregated.groups[group_value], result)
                end
            end
        end
        
        return cjson.encode(aggregated)
    end
    
    return cjson.encode({
        count = #results,
        data = results
    })
end

-- 通用事件发布器
local function generic_event_publish(keys, args)
    if #args < 2 then
        return redis.error_reply("Usage: event_type event_data [options_json]")
    end
    
    local event_type = args[1]
    local event_data = args[2]
    local options = args[3] and cjson.decode(args[3]) or {}
    
    local event_id = redis.call('INCR', 'event:id:counter')
    local timestamp = redis.call('TIME')
    
    local event = {
        id = event_id,
        type = event_type,
        data = cjson.decode(event_data),
        timestamp = timestamp[1] * 1000 + math.floor(timestamp[2] / 1000),
        source = options.source or "system"
    }
    
    -- 存储事件
    if options.persist then
        local event_key = string.format("event:%s:%d", event_type, event_id)
        redis.call('HSET', event_key, 
            'id', event_id,
            'type', event_type,
            'data', event_data,
            'timestamp', event.timestamp,
            'source', event.source
        )
        
        if options.ttl then
            redis.call('EXPIRE', event_key, options.ttl)
        end
        
        -- 添加到事件流
        local stream_key = string.format("event:stream:%s", event_type)
        redis.call('XADD', stream_key, 'MAXLEN', '~', options.max_stream_length or 10000, '*',
            'id', event_id,
            'data', event_data,
            'source', event.source
        )
    end
    
    -- 发布到频道
    local channels = {
        string.format("event:%s", event_type),
        "event:all"
    }
    
    if options.additional_channels then
        for _, channel in ipairs(options.additional_channels) do
            table.insert(channels, channel)
        end
    end
    
    local event_json = cjson.encode(event)
    for _, channel in ipairs(channels) do
        redis.call('PUBLISH', channel, event_json)
    end
    
    -- 更新统计
    redis.call('HINCRBY', 'event:stats', event_type, 1)
    redis.call('HINCRBY', 'event:stats', 'total', 1)
    
    return cjson.encode({
        event_id = event_id,
        channels = channels,
        persisted = options.persist or false
    })
end

-- 通用统计引擎
local function generic_statistics(keys, args)
    if #args < 2 then
        return redis.error_reply("Usage: action stat_config_json [params...]")
    end
    
    local action = args[1]
    local config = cjson.decode(args[2])
    
    if action == "increment" then
        -- 增量统计
        local value = tonumber(args[3] or 1)
        local results = {}
        
        for _, stat in ipairs(config.stats) do
            local stat_key = string.format("stat:%s:%s", config.namespace, stat.name)
            
            if stat.type == "counter" then
                local new_value = redis.call('HINCRBY', stat_key, stat.field or 'value', value)
                table.insert(results, {name = stat.name, value = new_value})
                
            elseif stat.type == "gauge" then
                redis.call('HSET', stat_key, stat.field or 'value', value)
                redis.call('HSET', stat_key, 'updated_at', redis.call('TIME')[1])
                table.insert(results, {name = stat.name, value = value})
                
            elseif stat.type == "histogram" then
                -- 简单的直方图实现
                local bucket = math.floor(value / (stat.bucket_size or 10)) * (stat.bucket_size or 10)
                redis.call('HINCRBY', stat_key, 'bucket:' .. bucket, 1)
                redis.call('HINCRBY', stat_key, 'count', 1)
                redis.call('HINCRBYFLOAT', stat_key, 'sum', value)
                
                local count = tonumber(redis.call('HGET', stat_key, 'count'))
                local sum = tonumber(redis.call('HGET', stat_key, 'sum'))
                table.insert(results, {
                    name = stat.name, 
                    count = count,
                    sum = sum,
                    avg = sum / count
                })
            end
            
            -- 时间窗口统计
            if stat.time_window then
                local window_key = string.format("%s:window:%d", stat_key, 
                    math.floor(redis.call('TIME')[1] / stat.time_window))
                redis.call('HINCRBY', window_key, 'value', value)
                redis.call('EXPIRE', window_key, stat.time_window * 2)
            end
        end
        
        return cjson.encode(results)
        
    elseif action == "get" then
        -- 获取统计
        local results = {}
        
        for _, stat in ipairs(config.stats) do
            local stat_key = string.format("stat:%s:%s", config.namespace, stat.name)
            local stat_data = {}
            
            if stat.type == "counter" or stat.type == "gauge" then
                stat_data.value = tonumber(redis.call('HGET', stat_key, stat.field or 'value')) or 0
                
            elseif stat.type == "histogram" then
                stat_data.count = tonumber(redis.call('HGET', stat_key, 'count')) or 0
                stat_data.sum = tonumber(redis.call('HGET', stat_key, 'sum')) or 0
                stat_data.avg = stat_data.count > 0 and (stat_data.sum / stat_data.count) or 0
                
                -- 获取分布
                if args[3] == "detailed" then
                    stat_data.distribution = {}
                    local all_fields = redis.call('HGETALL', stat_key)
                    for i = 1, #all_fields, 2 do
                        if string.sub(all_fields[i], 1, 7) == "bucket:" then
                            local bucket = string.sub(all_fields[i], 8)
                            stat_data.distribution[bucket] = tonumber(all_fields[i + 1])
                        end
                    end
                end
            end
            
            -- 时间窗口统计
            if stat.time_window and args[3] == "windowed" then
                stat_data.windows = {}
                local current_time = redis.call('TIME')[1]
                local windows_to_check = tonumber(args[4] or 5)
                
                for i = 0, windows_to_check - 1 do
                    local window_time = math.floor((current_time - i * stat.time_window) / stat.time_window)
                    local window_key = string.format("%s:window:%d", stat_key, window_time)
                    local window_value = tonumber(redis.call('HGET', window_key, 'value')) or 0
                    table.insert(stat_data.windows, {
                        time = window_time * stat.time_window,
                        value = window_value
                    })
                end
            end
            
            table.insert(results, {
                name = stat.name,
                type = stat.type,
                data = stat_data
            })
        end
        
        return cjson.encode(results)
        
    elseif action == "reset" then
        -- 重置统计
        local reset_count = 0
        
        for _, stat in ipairs(config.stats) do
            local stat_key = string.format("stat:%s:%s", config.namespace, stat.name)
            redis.call('DEL', stat_key)
            
            -- 删除时间窗口数据
            if stat.time_window then
                local pattern = stat_key .. ":window:*"
                local cursor = "0"
                repeat
                    local result = redis.call('SCAN', cursor, 'MATCH', pattern, 'COUNT', 100)
                    cursor = result[1]
                    for _, key in ipairs(result[2]) do
                        redis.call('DEL', key)
                    end
                until cursor == "0"
            end
            
            reset_count = reset_count + 1
        end
        
        return reset_count
        
    else
        return redis.error_reply("Unknown action: " .. action)
    end
end

-- 注册函数
redis.register_function('generic_store', generic_store)
redis.register_function('generic_batch_sync', generic_batch_sync)
redis.register_function('generic_query', generic_query)
redis.register_function('entity_manager', entity_manager)
redis.register_function('generic_state_machine', generic_state_machine)
redis.register_function('generic_multi_index', generic_multi_index)
redis.register_function('generic_condition_eval', generic_condition_eval)
redis.register_function('generic_batch_collect', generic_batch_collect)
redis.register_function('generic_event_publish', generic_event_publish)
redis.register_function('generic_statistics', generic_statistics)