#!lua name=sync_config_init

-- ========================================
-- 同步配置初始化脚本
-- 预定义常用的同步规则配置
-- ========================================

local cjson = require('cjson')

-- 初始化所有预定义的同步规则
local function init_sync_configs(keys, args)
    local configs_loaded = 0
    local errors = {}
    
    -- ==================== 1. Comsrv 到 Modsrv 同步配置 ====================
    
    -- 1.1 遥测数据同步（T类型）
    local comsrv_to_modsrv_telemetry = {
        enabled = true,
        description = "同步 comsrv 遥测数据到 modsrv 模型",
        source = {
            pattern = "comsrv:*:T",  -- 匹配所有通道的遥测数据
            type = "hash"
        },
        target = {
            -- 使用反向映射查找目标
            use_reverse_mapping = true,
            type = "hash"
        },
        reverse_mapping = {
            enabled = true,
            pattern = "modsrv:reverse:$channel:$point_id"
        },
        transform = {
            type = "direct"  -- 直接映射，不转换
        }
    }
    
    -- 1.2 遥信数据同步（S类型）
    local comsrv_to_modsrv_signal = {
        enabled = true,
        description = "同步 comsrv 遥信数据到 modsrv 模型",
        source = {
            pattern = "comsrv:*:S",
            type = "hash"
        },
        target = {
            use_reverse_mapping = true,
            type = "hash"
        },
        reverse_mapping = {
            enabled = true,
            pattern = "modsrv:reverse:$channel:$point_id"
        },
        transform = {
            type = "direct"
        }
    }
    
    -- 1.3 控制数据同步（C类型）
    local comsrv_to_modsrv_control = {
        enabled = true,
        description = "同步 comsrv 控制数据到 modsrv 模型",
        source = {
            pattern = "comsrv:*:C",
            type = "hash"
        },
        target = {
            use_reverse_mapping = true,
            type = "hash"
        },
        reverse_mapping = {
            enabled = true,
            pattern = "modsrv:reverse:$channel:$point_id"
        },
        transform = {
            type = "direct"
        }
    }
    
    -- 1.4 调节数据同步（A类型）
    local comsrv_to_modsrv_adjustment = {
        enabled = true,
        description = "同步 comsrv 调节数据到 modsrv 模型",
        source = {
            pattern = "comsrv:*:A",
            type = "hash"
        },
        target = {
            use_reverse_mapping = true,
            type = "hash"
        },
        reverse_mapping = {
            enabled = true,
            pattern = "modsrv:reverse:$channel:$point_id"
        },
        transform = {
            type = "direct"
        }
    }
    
    -- ==================== 2. Comsrv 到 Alarmsrv 同步配置 ====================
    
    local comsrv_to_alarmsrv = {
        enabled = true,
        description = "同步 comsrv 数据到 alarmsrv 进行告警检测",
        source = {
            pattern = "comsrv:*:T",  -- 主要监控遥测数据
            type = "hash"
        },
        target = {
            pattern = "alarmsrv:channel:$1:latest",
            type = "hash"
        },
        transform = {
            type = "alarm_check",  -- 告警检测转换
            config = {
                check_thresholds = true,
                generate_events = true
            }
        }
    }
    
    -- ==================== 3. Comsrv 到 Hissrv 同步配置 ====================
    
    local comsrv_to_hissrv = {
        enabled = true,
        description = "同步 comsrv 数据到 hissrv 进行历史存储",
        source = {
            pattern = "comsrv:*:*",  -- 所有类型数据
            type = "hash"
        },
        target = {
            pattern = "hissrv:batch:$timestamp",
            type = "list"  -- 使用列表存储批量数据
        },
        transform = {
            type = "time_series",  -- 时序数据转换
            config = {
                add_timestamp = true,
                batch_size = 1000,
                compression = "gzip"
            }
        }
    }
    
    -- ==================== 4. Modsrv 到 Rulesrv 同步配置 ====================
    
    local modsrv_to_rulesrv = {
        enabled = true,
        description = "同步 modsrv 模型数据到 rulesrv 进行规则引擎处理",
        source = {
            pattern = "modsrv:model:*:measurement",
            type = "hash"
        },
        target = {
            pattern = "rulesrv:input:$model_id",
            type = "hash"
        },
        transform = {
            type = "rule_input",  -- 规则引擎输入转换
            config = {
                flatten = true,
                add_metadata = true
            }
        }
    }
    
    -- ==================== 5. 双向同步配置示例 ====================
    
    local modsrv_to_comsrv = {
        enabled = false,  -- 默认禁用，需要时启用
        description = "反向同步：从 modsrv 写回到 comsrv（用于控制命令）",
        source = {
            pattern = "modsrv:model:*:control",
            type = "hash"
        },
        target = {
            use_reverse_mapping = true,
            type = "hash"
        },
        reverse_mapping = {
            enabled = true,
            pattern = "modsrv:reverse:control:$model_id:$point_name"
        },
        transform = {
            type = "control_command",
            config = {
                validate = true,
                add_timestamp = true
            }
        }
    }
    
    -- ==================== 存储配置 ====================
    
    local configs = {
        ["comsrv_to_modsrv_T"] = comsrv_to_modsrv_telemetry,
        ["comsrv_to_modsrv_S"] = comsrv_to_modsrv_signal,
        ["comsrv_to_modsrv_C"] = comsrv_to_modsrv_control,
        ["comsrv_to_modsrv_A"] = comsrv_to_modsrv_adjustment,
        ["comsrv_to_alarmsrv"] = comsrv_to_alarmsrv,
        ["comsrv_to_hissrv"] = comsrv_to_hissrv,
        ["modsrv_to_rulesrv"] = modsrv_to_rulesrv,
        ["modsrv_to_comsrv"] = modsrv_to_comsrv
    }
    
    -- 逐个加载配置
    for rule_id, config in pairs(configs) do
        local config_json = cjson.encode(config)
        local result = redis.call('FCALL', 'sync_config_set', 1, rule_id, config_json)
        
        if result == "OK" then
            configs_loaded = configs_loaded + 1
        else
            table.insert(errors, {
                rule_id = rule_id,
                error = tostring(result)
            })
        end
    end
    
    -- 返回结果
    return cjson.encode({
        status = #errors == 0 and "success" or "partial",
        configs_loaded = configs_loaded,
        total_configs = 8,
        errors = errors
    })
end

-- 获取所有同步配置的状态
local function get_sync_status(keys, args)
    local rules = redis.call('SMEMBERS', 'sync:rules')
    local status = {}
    
    for _, rule_id in ipairs(rules) do
        local config_json = redis.call('GET', 'sync:config:' .. rule_id)
        if config_json then
            local config = cjson.decode(config_json)
            local stats = redis.call('HGETALL', 'sync:stats:' .. rule_id)
            local stats_map = {}
            
            for i = 1, #stats, 2 do
                stats_map[stats[i]] = stats[i + 1]
            end
            
            table.insert(status, {
                rule_id = rule_id,
                enabled = config.enabled,
                description = config.description,
                source_pattern = config.source and config.source.pattern,
                target_pattern = config.target and config.target.pattern,
                stats = stats_map
            })
        end
    end
    
    return cjson.encode(status)
end

-- 启用/禁用特定的同步规则
local function toggle_sync_rule(keys, args)
    local rule_id = keys[1]
    local enabled = args[1] == "true" or args[1] == "1"
    
    local config_json = redis.call('GET', 'sync:config:' .. rule_id)
    if not config_json then
        return redis.error_reply("Rule not found: " .. rule_id)
    end
    
    local config = cjson.decode(config_json)
    config.enabled = enabled
    
    redis.call('SET', 'sync:config:' .. rule_id, cjson.encode(config))
    
    if enabled then
        redis.call('SADD', 'sync:rules:active', rule_id)
    else
        redis.call('SREM', 'sync:rules:active', rule_id)
    end
    
    return redis.status_reply("OK")
end

-- 批量执行同步（用于定时任务）
local function batch_sync_all(keys, args)
    local active_rules = redis.call('SMEMBERS', 'sync:rules:active')
    local results = {}
    
    for _, rule_id in ipairs(active_rules) do
        local config_json = redis.call('GET', 'sync:config:' .. rule_id)
        if config_json then
            local config = cjson.decode(config_json)
            
            -- 根据规则类型执行不同的同步逻辑
            if string.match(rule_id, "comsrv_to_modsrv") then
                -- 执行 comsrv 到 modsrv 的同步
                local pattern_type = string.match(rule_id, "_([TSCA])$")
                if pattern_type then
                    -- 获取所有相关的 comsrv 数据
                    local keys_pattern = "comsrv:*:" .. pattern_type
                    local comsrv_keys = redis.call('KEYS', keys_pattern)
                    
                    for _, key in ipairs(comsrv_keys) do
                        local channel_id = string.match(key, "comsrv:(%d+):")
                        if channel_id then
                            -- 获取所有点位数据
                            local data = redis.call('HGETALL', key)
                            local updates = {}
                            
                            for i = 1, #data, 2 do
                                table.insert(updates, {
                                    point_id = tonumber(data[i]),
                                    value = tonumber(data[i + 1]) or data[i + 1]
                                })
                            end
                            
                            if #updates > 0 then
                                local sync_result = redis.call('FCALL', 'sync_comsrv_to_modsrv', 
                                    2, channel_id, pattern_type, cjson.encode(updates))
                                table.insert(results, {
                                    rule_id = rule_id,
                                    channel = channel_id,
                                    result = sync_result
                                })
                            end
                        end
                    end
                end
            end
        end
    end
    
    return cjson.encode({
        timestamp = redis.call('TIME')[1],
        rules_processed = #active_rules,
        results = results
    })
end

-- 注册函数
redis.register_function('init_sync_configs', init_sync_configs)
redis.register_function('get_sync_status', get_sync_status)
redis.register_function('toggle_sync_rule', toggle_sync_rule)
redis.register_function('batch_sync_all', batch_sync_all)