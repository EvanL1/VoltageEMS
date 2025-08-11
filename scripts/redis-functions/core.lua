#!lua name=core

-- ========================================
-- VoltageEMS Core Functions (Simplified)
-- Version: 0.0.1
-- ========================================

-- Redis provided modules
local cjson = require("cjson")

-- Safe JSON decode helper
local function safe_decode(json_str, what)
    if not json_str then
        return nil, redis.error_reply("Missing " .. (what or "JSON"))
    end
    local ok, result = pcall(cjson.decode, json_str)
    if not ok then
        return nil, redis.error_reply("Invalid " .. (what or "JSON") .. ": " .. tostring(result))
    end
    return result
end

-- Safe numeric conversion
local function safe_tonumber(value, default)
    local num = tonumber(value)
    return num ~= nil and num or (default or 0)
end

-- Generic entity storage with indexing
local function generic_store(keys, args)
    if #keys < 1 or #args < 2 then
        return redis.error_reply("Usage: entity_key entity_type entity_data [indexes_json]")
    end

    local entity_key = keys[1]
    local entity_type = args[1]
    local entity_data = args[2]
    local indexes_json = args[3]

    -- Validate entity data
    local entity, err = safe_decode(entity_data, "entity data")
    if err then return err end

    -- Store main entity
    redis.call('HSET', entity_key, 'type', entity_type)
    redis.call('HSET', entity_key, 'data', entity_data)
    redis.call('HSET', entity_key, 'updated_at', redis.call('TIME')[1])

    -- Handle indexes if provided
    if indexes_json then
        local indexes, err = safe_decode(indexes_json, "indexes")
        if err then return err end

        for index_name, index_value in pairs(indexes) do
            local index_key = string.format("idx:%s:%s:%s", entity_type, index_name, tostring(index_value))
            redis.call('SADD', index_key, entity_key)
        end
    end

    return redis.status_reply("OK")
end

-- Generic query with filtering
local function generic_query(keys, args)
    if #args < 1 then
        return redis.error_reply("Usage: entity_type [filter_json] [limit] [offset]")
    end

    local entity_type = args[1]
    local filter_json = args[2]
    local limit = safe_tonumber(args[3], 100)
    local offset = safe_tonumber(args[4], 0)

    local pattern = string.format("%s:*", entity_type)
    local cursor = "0"
    local results = {}
    local count = 0

    -- Scan for matching keys
    repeat
        local scan_result = redis.call('SCAN', cursor, 'MATCH', pattern, 'COUNT', 100)
        cursor = scan_result[1]
        local keys = scan_result[2]

        for _, key in ipairs(keys) do
            if count >= offset and #results < limit then
                local data = redis.call('HGET', key, 'data')
                if data then
                    -- Apply filter if provided
                    if filter_json then
                        local filter = safe_decode(filter_json)
                        local entity = safe_decode(data)
                        local match = true

                        for field, value in pairs(filter) do
                            if entity[field] ~= value then
                                match = false
                                break
                            end
                        end

                        if match then
                            table.insert(results, data)
                        end
                    else
                        table.insert(results, data)
                    end
                end
            end
            count = count + 1
        end
    until cursor == "0" or #results >= limit

    return cjson.encode(results)
end

-- Event storage (no pub/sub)
local function event_store(keys, args)
    if #args < 2 then
        return redis.error_reply("Usage: event_type event_data")
    end

    local event_type = args[1]
    local event_data = args[2]

    -- Validate event data
    local event, err = safe_decode(event_data, "event data")
    if err then return err end

    -- Add timestamp if not present
    if not event.timestamp then
        event.timestamp = redis.call('TIME')[1]
    end

    local enriched_data = cjson.encode(event)

    -- Store event in queue for processing
    local queue_key = string.format("event:queue:%s", event_type)
    redis.call('LPUSH', queue_key, enriched_data)
    redis.call('LTRIM', queue_key, 0, 999)  -- Keep only last 1000 events

    -- Store last event
    redis.call('SET', string.format("event:last:%s", event_type), enriched_data, 'EX', 3600)

    return redis.status_reply("OK")
end

-- Batch operations
local function batch_operation(keys, args)
    if #args < 2 then
        return redis.error_reply("Usage: operation_type operations_json")
    end

    local op_type = args[1]
    local operations_json = args[2]

    local operations, err = safe_decode(operations_json, "operations")
    if err then return err end

    local results = {}

    for _, op in ipairs(operations) do
        local result

        if op_type == "set" then
            result = redis.call('SET', op.key, op.value)
        elseif op_type == "hset" then
            result = redis.call('HSET', op.key, op.field, op.value)
        elseif op_type == "del" then
            result = redis.call('DEL', op.key)
        else
            result = redis.error_reply("Unknown operation type: " .. op_type)
        end

        table.insert(results, result)
    end

    return cjson.encode(results)
end

-- Register functions
redis.register_function('generic_store', generic_store)
redis.register_function('generic_query', generic_query)
redis.register_function('event_store', event_store)
redis.register_function('batch_operation', batch_operation)
redis.register_function('safe_decode', function(keys, args)
    local result, err = safe_decode(args[1], args[2])
    if err then return err end
    return cjson.encode(result)
end)
redis.register_function('safe_tonumber', function(keys, args)
    return safe_tonumber(args[1], args[2])
end)