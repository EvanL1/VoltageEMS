-- 规则引擎脚本
-- 用于执行复杂的业务规则和控制逻辑
-- 支持 DAG（有向无环图）规则执行

local action = ARGV[1]

-- 辅助函数：评估条件表达式
local function evaluate_condition(condition, values)
    -- 条件格式: "point_name operator value"
    -- 例如: "voltage > 250" 或 "status == 1"
    local point, operator, threshold = string.match(condition, "([%w_]+)%s*([<>=!]+)%s*([%d%.%-]+)")

    if not point or not operator or not threshold then
        return false
    end

    local value = tonumber(values[point])
    threshold = tonumber(threshold)

    if not value or not threshold then
        return false
    end

    -- 执行比较
    if operator == ">" then
        return value > threshold
    elseif operator == ">=" then
        return value >= threshold
    elseif operator == "<" then
        return value < threshold
    elseif operator == "<=" then
        return value <= threshold
    elseif operator == "==" then
        return math.abs(value - threshold) < 0.0001 -- 浮点数比较
    elseif operator == "!=" then
        return math.abs(value - threshold) >= 0.0001
    end

    return false
end

-- 辅助函数：获取规则输入数据
local function get_rule_inputs(rule_id)
    local inputs = {}

    -- 获取规则的输入配置
    local input_pattern = string.format("rule:%s:input:*", rule_id)
    local input_keys = redis.call('KEYS', input_pattern)

    for _, input_key in ipairs(input_keys) do
        local input_config = redis.call('GET', input_key)
        if input_config then
            -- 输入格式: "service:model_id:point_name"
            local service, model_id, point_name = string.match(input_config, "([^:]+):([^:]+):([^:]+)")
            if service and model_id and point_name then
                local data_key = string.format("%s:%s:measurement", service, model_id)
                local value = redis.call('HGET', data_key, point_name)
                if value then
                    -- 从 input key 中提取别名
                    local alias = string.match(input_key, "rule:[^:]+:input:(.+)")
                    inputs[alias] = tonumber(value) or value
                end
            end
        end
    end

    return inputs
end

if action == "execute_rule" then
    -- 执行单个规则
    local rule_id = ARGV[2]
    local force = ARGV[3] == "true" -- 是否强制执行（忽略使能状态）

    -- 检查规则是否启用
    if not force then
        local enabled = redis.call('HGET', string.format("rule:%s", rule_id), "enabled")
        if enabled ~= "true" then
            return "RULE_DISABLED"
        end
    end

    -- 获取规则定义
    local rule_key = string.format("rule:%s", rule_id)
    local rule_type = redis.call('HGET', rule_key, "type") or "simple"

    if rule_type == "simple" then
        -- 简单规则：单条件单动作
        local condition = redis.call('HGET', rule_key, "condition")
        local action_type = redis.call('HGET', rule_key, "action_type")
        local action_target = redis.call('HGET', rule_key, "action_target")
        local action_value = redis.call('HGET', rule_key, "action_value")

        if not condition or not action_type then
            return "INVALID_RULE"
        end

        -- 获取输入数据
        local inputs = get_rule_inputs(rule_id)

        -- 评估条件
        if evaluate_condition(condition, inputs) then
            -- 执行动作
            if action_type == "control" then
                -- 发送控制命令
                local model_id, control_name = string.match(action_target, "([^:]+):([^:]+)")
                if model_id and control_name then
                    -- 调用 unified_sync 发送控制
                    redis.call('EVAL', redis.call('GET', 'script:unified_sync'), 0,
                        "send_control", model_id, control_name, action_value)
                end
            elseif action_type == "alarm" then
                -- 创建告警
                local alarm_data = string.format("%s:%s:%s:%s",
                    rule_id, action_target, action_value, redis.call('TIME')[1])
                redis.call('LPUSH', 'alarm:queue', alarm_data)
            elseif action_type == "notify" then
                -- 发送通知
                redis.call('PUBLISH', 'notify:' .. action_target, action_value)
            end

            -- 记录执行历史
            local history_key = string.format("rule:history:%s", rule_id)
            redis.call('LPUSH', history_key, string.format("%s:triggered:%s",
                redis.call('TIME')[1], cjson.encode(inputs)))
            redis.call('LTRIM', history_key, 0, 99) -- 保留最近100条

            return "TRIGGERED"
        end

        return "CONDITION_NOT_MET"
    elseif rule_type == "dag" then
        -- DAG 规则：多步骤执行
        local dag_key = string.format("rule:%s:dag", rule_id)
        local nodes = redis.call('HGETALL', dag_key)

        if #nodes == 0 then
            return "NO_DAG_NODES"
        end

        -- 构建节点图
        local node_map = {}
        for i = 1, #nodes, 2 do
            local node_id = nodes[i]
            local node_data = cjson.decode(nodes[i + 1])
            node_map[node_id] = node_data
        end

        -- 拓扑排序并执行
        local executed = {}
        local results = {}
        local inputs = get_rule_inputs(rule_id)

        -- 简化的执行逻辑（假设已经按顺序存储）
        for node_id, node in pairs(node_map) do
            if node.type == "condition" then
                -- 评估条件节点
                local result = evaluate_condition(node.expression, inputs)
                results[node_id] = result

                -- 如果条件不满足且是必要节点，停止执行
                if not result and node.required then
                    return "CONDITION_FAILED:" .. node_id
                end
            elseif node.type == "calculation" then
                -- 计算节点
                if node.operation == "average" then
                    local sum = 0
                    local count = 0
                    for _, input in ipairs(node.inputs) do
                        if inputs[input] then
                            sum = sum + inputs[input]
                            count = count + 1
                        end
                    end
                    results[node_id] = count > 0 and (sum / count) or 0
                elseif node.operation == "sum" then
                    local sum = 0
                    for _, input in ipairs(node.inputs) do
                        sum = sum + (inputs[input] or 0)
                    end
                    results[node_id] = sum
                elseif node.operation == "max" then
                    local max_val = nil
                    for _, input in ipairs(node.inputs) do
                        if inputs[input] then
                            max_val = max_val and math.max(max_val, inputs[input]) or inputs[input]
                        end
                    end
                    results[node_id] = max_val or 0
                elseif node.operation == "min" then
                    local min_val = nil
                    for _, input in ipairs(node.inputs) do
                        if inputs[input] then
                            min_val = min_val and math.min(min_val, inputs[input]) or inputs[input]
                        end
                    end
                    results[node_id] = min_val or 0
                end

                -- 将计算结果加入输入中，供后续节点使用
                if results[node_id] then
                    inputs[node_id] = results[node_id]
                end
            elseif node.type == "action" then
                -- 执行动作节点
                local should_execute = true

                -- 检查前置条件
                if node.dependencies then
                    for _, dep in ipairs(node.dependencies) do
                        if not results[dep] then
                            should_execute = false
                            break
                        end
                    end
                end

                if should_execute then
                    -- 执行具体动作
                    if node.action_type == "control" then
                        redis.call('EVAL', redis.call('GET', 'script:unified_sync'), 0,
                            "send_control", node.model_id, node.control_name, node.value)
                    elseif node.action_type == "alarm" then
                        local alarm_data = string.format("%s:%s:%s:%s",
                            rule_id, node_id, node.message or "Rule triggered", redis.call('TIME')[1])
                        redis.call('LPUSH', 'alarm:queue', alarm_data)
                    end

                    executed[node_id] = true
                end
            end
        end

        -- 记录执行结果
        local history_key = string.format("rule:history:%s", rule_id)
        redis.call('LPUSH', history_key, string.format("%s:dag:%s",
            redis.call('TIME')[1], cjson.encode(executed)))
        redis.call('LTRIM', history_key, 0, 99)

        return "DAG_EXECUTED:" .. cjson.encode(executed)
    end

    return "UNKNOWN_RULE_TYPE"
elseif action == "execute_rule_group" then
    -- 执行规则组
    local group_id = ARGV[2]

    -- 获取组内所有规则
    local group_key = string.format("rule:group:%s", group_id)
    local rules = redis.call('SMEMBERS', group_key)

    if #rules == 0 then
        return "EMPTY_GROUP"
    end

    local results = {}
    for _, rule_id in ipairs(rules) do
        -- 递归执行每个规则
        local result = redis.call('EVAL', SCRIPT, 0, "execute_rule", rule_id)
        results[rule_id] = result
    end

    return cjson.encode(results)
elseif action == "schedule_rule" then
    -- 定时规则调度（由外部定时器触发）
    local schedule_type = ARGV[2] -- "cron" 或 "interval"

    -- 获取需要执行的规则
    local scheduled_rules = redis.call('SMEMBERS', 'rule:scheduled:' .. schedule_type)

    local executed = {}
    for _, rule_id in ipairs(scheduled_rules) do
        local last_run_key = string.format("rule:%s:last_run", rule_id)
        local last_run = redis.call('GET', last_run_key)
        local now = redis.call('TIME')[1]

        -- 检查是否需要执行
        local should_run = false
        if schedule_type == "interval" then
            local interval = tonumber(redis.call('HGET', string.format("rule:%s", rule_id), "interval"))
            if interval and (not last_run or (now - tonumber(last_run)) >= interval) then
                should_run = true
            end
        else
            -- cron 类型由外部调度器决定
            should_run = true
        end

        if should_run then
            local result = redis.call('EVAL', SCRIPT, 0, "execute_rule", rule_id)
            executed[rule_id] = result
            redis.call('SET', last_run_key, now)
        end
    end

    return cjson.encode(executed)
elseif action == "test_rule" then
    -- 测试规则（不执行动作）
    local rule_id = ARGV[2]
    local test_inputs = ARGV[3] -- JSON 格式的测试输入

    local rule_key = string.format("rule:%s", rule_id)
    local condition = redis.call('HGET', rule_key, "condition")

    if not condition then
        return "NO_CONDITION"
    end

    -- 使用测试输入或实际输入
    local inputs
    if test_inputs then
        inputs = cjson.decode(test_inputs)
    else
        inputs = get_rule_inputs(rule_id)
    end

    -- 评估条件
    local result = evaluate_condition(condition, inputs)

    return cjson.encode({
        rule_id = rule_id,
        condition = condition,
        inputs = inputs,
        result = result
    })
elseif action == "get_rule_status" then
    -- 获取规则状态
    local rule_id = ARGV[2]

    local rule_key = string.format("rule:%s", rule_id)
    local enabled = redis.call('HGET', rule_key, "enabled")
    local last_run = redis.call('GET', string.format("rule:%s:last_run", rule_id))
    local history = redis.call('LRANGE', string.format("rule:history:%s", rule_id), 0, 9)

    return cjson.encode({
        rule_id = rule_id,
        enabled = enabled == "true",
        last_run = last_run,
        recent_history = history
    })
else
    return 'UNKNOWN_ACTION'
end
