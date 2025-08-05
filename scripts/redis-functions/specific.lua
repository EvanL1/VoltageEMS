#!lua name=specific

-- ========================================
-- VoltageEMS 特定功能函数
-- 仅包含无法通用化的特殊逻辑（20%）
-- ========================================

-- 前置声明辅助函数
local convert_single_to_line_protocol
local format_field_value

-- DAG 执行器（仅用于 rulesrv）
local function dag_executor(keys, args)
    if #keys < 1 or #args < 2 then
        return redis.error_reply("Usage: rule_id dag_definition variables_json")
    end
    
    local rule_id = keys[1]
    local dag_def = cjson.decode(args[1])
    local variables = cjson.decode(args[2])
    
    -- 构建节点索引
    local node_map = {}
    for _, node in ipairs(dag_def.nodes) do
        node_map[node.id] = node
    end
    
    -- 构建邻接表和入度计数
    local adjacency = {}
    local in_degree = {}
    local node_results = {}
    
    -- 初始化
    for _, node in ipairs(dag_def.nodes) do
        adjacency[node.id] = {}
        in_degree[node.id] = 0
        node_results[node.id] = nil
    end
    
    -- 构建图
    for _, edge in ipairs(dag_def.edges) do
        table.insert(adjacency[edge.from], {
            to = edge.to,
            condition = edge.condition
        })
        in_degree[edge.to] = in_degree[edge.to] + 1
    end
    
    -- 拓扑排序 + 执行
    local queue = {}
    local executed = {}
    
    -- 找到所有入度为0的节点
    for node_id, degree in pairs(in_degree) do
        if degree == 0 then
            table.insert(queue, node_id)
        end
    end
    
    -- 执行DAG
    while #queue > 0 do
        local current_id = table.remove(queue, 1)
        local node = node_map[current_id]
        
        -- Execute node
        local result = nil
        if node.type == "condition" then
            -- 评估条件
            local condition = node.config.condition
            local field_value = variables[condition.field]
            
            if condition.operator == ">" then
                result = tonumber(field_value) > tonumber(condition.value)
            elseif condition.operator == ">=" then
                result = tonumber(field_value) >= tonumber(condition.value)
            elseif condition.operator == "<" then
                result = tonumber(field_value) < tonumber(condition.value)
            elseif condition.operator == "<=" then
                result = tonumber(field_value) <= tonumber(condition.value)
            elseif condition.operator == "==" then
                result = field_value == condition.value
            elseif condition.operator == "!=" then
                result = field_value ~= condition.value
            else
                result = false
            end
            
        elseif node.type == "action" then
            -- 执行动作
            local action = node.config.action
            if action.type == "set_variable" then
                variables[action.name] = action.value
                result = true
            elseif action.type == "publish" then
                redis.call('PUBLISH', action.channel, cjson.encode({
                    rule_id = rule_id,
                    action = action.name,
                    data = variables
                }))
                result = true
            elseif action.type == "store" then
                redis.call('HSET', action.key, action.field, action.value or cjson.encode(variables))
                result = true
            else
                result = false
            end
            
        elseif node.type == "transform" then
            -- 数据转换
            local transform = node.config.transform
            if transform.type == "calculate" then
                local a = tonumber(variables[transform.a]) or 0
                local b = tonumber(variables[transform.b]) or 0
                
                if transform.operator == "+" then
                    variables[transform.output] = a + b
                elseif transform.operator == "-" then
                    variables[transform.output] = a - b
                elseif transform.operator == "*" then
                    variables[transform.output] = a * b
                elseif transform.operator == "/" then
                    variables[transform.output] = b ~= 0 and (a / b) or 0
                end
                result = true
            else
                result = false
            end
        end
        
        node_results[current_id] = result
        table.insert(executed, {
            node_id = current_id,
            type = node.type,
            result = result
        })
        
        -- 处理后继节点
        for _, edge in ipairs(adjacency[current_id]) do
            local should_proceed = true
            
            -- 检查边条件
            if edge.condition then
                if edge.condition == "true" and not result then
                    should_proceed = false
                elseif edge.condition == "false" and result then
                    should_proceed = false
                end
            end
            
            if should_proceed then
                in_degree[edge.to] = in_degree[edge.to] - 1
                if in_degree[edge.to] == 0 then
                    table.insert(queue, edge.to)
                end
            end
        end
    end
    
    -- 检查是否所有节点都被执行
    local all_executed = true
    for node_id, _ in pairs(node_map) do
        if node_results[node_id] == nil then
            all_executed = false
            break
        end
    end
    
    return cjson.encode({
        rule_id = rule_id,
        executed = executed,
        variables = variables,
        completed = all_executed
    })
end

-- Line Protocol 转换器（仅用于 hissrv）
local function line_protocol_converter(keys, args)
    if #args < 1 then
        return redis.error_reply("Usage: data_json [config_json]")
    end
    
    local data = cjson.decode(args[1])
    local config = args[2] and cjson.decode(args[2]) or {}
    
    local results = {}
    
    -- 批量转换
    if data.batch then
        for _, item in ipairs(data.batch) do
            local line = convert_single_to_line_protocol(item, config)
            if line then
                table.insert(results, line)
            end
        end
    else
        -- 单条转换
        local line = convert_single_to_line_protocol(data, config)
        if line then
            table.insert(results, line)
        end
    end
    
    return table.concat(results, "\n")
end

-- 辅助函数：转换单条数据为Line Protocol
function convert_single_to_line_protocol(data, config)
    -- measurement,tag1=value1,tag2=value2 field1=value1,field2=value2 timestamp
    
    local measurement = data.measurement or config.default_measurement or "voltage_data"
    
    -- 构建标签
    local tags = {}
    if data.tags then
        for k, v in pairs(data.tags) do
            if v ~= nil and v ~= "" then
                -- 转义特殊字符
                local escaped_key = string.gsub(k, "[ ,=]", "\\%0")
                local escaped_value = string.gsub(tostring(v), "[ ,=]", "\\%0")
                table.insert(tags, escaped_key .. "=" .. escaped_value)
            end
        end
    end
    
    -- 添加默认标签
    if config.default_tags then
        for k, v in pairs(config.default_tags) do
            local escaped_key = string.gsub(k, "[ ,=]", "\\%0")
            local escaped_value = string.gsub(tostring(v), "[ ,=]", "\\%0")
            table.insert(tags, escaped_key .. "=" .. escaped_value)
        end
    end
    
    -- Build fields
    local fields = {}
    if data.fields then
        for k, v in pairs(data.fields) do
            if v ~= nil then
                local escaped_key = string.gsub(k, "[ ,=]", "\\%0")
                local formatted_value = format_field_value(v, config.field_types and config.field_types[k])
                if formatted_value then
                    table.insert(fields, escaped_key .. "=" .. formatted_value)
                end
            end
        end
    end
    
    -- 如果没有字段，返回nil
    if #fields == 0 then
        return nil
    end
    
    -- 构建Line Protocol字符串
    local parts = {measurement}
    
    if #tags > 0 then
        table.sort(tags) -- 标签排序以保证一致性
        parts[1] = parts[1] .. "," .. table.concat(tags, ",")
    end
    
    table.insert(parts, table.concat(fields, ","))
    
    -- Add timestamp
    if data.timestamp then
        -- 确保时间戳是纳秒级
        local ts = tonumber(data.timestamp)
        if ts < 1e15 then -- 如果小于15位，可能是秒级
            ts = ts * 1e9
        elseif ts < 1e18 then -- 如果小于18位，可能是毫秒级
            ts = ts * 1e6
        end
        table.insert(parts, tostring(math.floor(ts)))
    else
        -- 使用当前时间（纳秒）
        local time = redis.call('TIME')
        local ts = time[1] * 1e9 + time[2] * 1e3
        table.insert(parts, tostring(math.floor(ts)))
    end
    
    return table.concat(parts, " ")
end

-- 辅助函数：格式化字段值
function format_field_value(value, field_type)
    if value == nil then
        return nil
    end
    
    field_type = field_type or "auto"
    
    if field_type == "integer" or field_type == "int" then
        return tostring(math.floor(tonumber(value) or 0)) .. "i"
    elseif field_type == "float" or field_type == "double" then
        return tostring(tonumber(value) or 0.0)
    elseif field_type == "boolean" or field_type == "bool" then
        return tostring(value) == "true" and "true" or "false"
    elseif field_type == "string" then
        -- 字符串需要用双引号包围，并转义内部的双引号和反斜杠
        local escaped = string.gsub(tostring(value), '["\\]', "\\%0")
        return '"' .. escaped .. '"'
    else
        -- 自动检测类型
        if type(value) == "boolean" then
            return value and "true" or "false"
        elseif type(value) == "number" then
            -- 检查是否为整数
            if math.floor(value) == value then
                return tostring(value) .. "i"
            else
                return tostring(value)
            end
        else
            -- 默认作为字符串处理
            local escaped = string.gsub(tostring(value), '["\\]', "\\%0")
            return '"' .. escaped .. '"'
        end
    end
end

-- Register functions
redis.register_function('dag_executor', dag_executor)
redis.register_function('line_protocol_converter', line_protocol_converter)