#!lua name=services

-- ========================================
-- VoltageEMS Services Functions
-- Version: 0.0.1
-- ========================================

-- Use cjson global (available in Redis 8.0)
-- In functions, cjson is global, not under redis namespace

-- Forward declarations for internal functions
local check_alarm_for_value
local modsrv_process_control

-- ============ Communication Service Functions ============

-- Write telemetry data to hash
local function comsrv_write_telemetry(keys, args)
	if #keys < 1 or #args < 1 then
		return redis.error_reply("Usage: hash_key points_json")
	end

	local hash_key = keys[1]
	local points_json = args[1]

	-- Parse points data
	local ok, points = pcall(cjson.decode, points_json)
	if not ok then
		return redis.error_reply("Invalid points JSON: " .. tostring(points))
	end

	-- Batch write to hash
	local write_count = 0
	for point_id, value in pairs(points) do
		redis.call("HSET", hash_key, tostring(point_id), tostring(value))
		write_count = write_count + 1

		-- Check alarms for this data point
		check_alarm_for_value(hash_key, tostring(point_id), value)
	end

	-- Update timestamp
	redis.call("HSET", hash_key, "_updated_at", redis.call("TIME")[1])

	return write_count
end

-- Write control commands
local function comsrv_write_control(keys, args)
	if #keys < 1 or #args < 2 then
		return redis.error_reply("Usage: control_key point_id value [ttl]")
	end

	local control_key = keys[1]
	local point_id = args[1]
	local value = args[2]
	local ttl = tonumber(args[3]) or 60

	-- Write control value
	redis.call("HSET", control_key, tostring(point_id), tostring(value))

	-- Set expiration
	redis.call("EXPIRE", control_key, ttl)

	-- Store control event for processing
	local event = {
		type = "control",
		key = control_key,
		point = point_id,
		value = value,
		timestamp = redis.call("TIME")[1],
	}

	redis.call("LPUSH", "control:events", cjson.encode(event))
	redis.call("LTRIM", "control:events", 0, 999)

	return redis.status_reply("OK")
end

-- Read telemetry data
local function comsrv_read_telemetry(keys, args)
	if #keys < 1 then
		return redis.error_reply("Usage: hash_key [point_ids_json]")
	end

	local hash_key = keys[1]
	local point_ids_json = args[1]

	local result = {}

	if point_ids_json then
		-- Read specific points
		local ok, point_ids = pcall(cjson.decode, point_ids_json)
		if not ok then
			return redis.error_reply("Invalid point IDs JSON")
		end

		for _, point_id in ipairs(point_ids) do
			local value = redis.call("HGET", hash_key, tostring(point_id))
			if value then
				result[tostring(point_id)] = value
			end
		end
	else
		-- Read all points
		local all_data = redis.call("HGETALL", hash_key)
		for i = 1, #all_data, 2 do
			local field = all_data[i]
			local value = all_data[i + 1]
			if not string.match(field, "^_") then -- Skip meta fields
				result[field] = value
			end
		end
	end

	return cjson.encode(result)
end

-- Trigger command execution
local function comsrv_trigger_command(keys, args)
	if #args < 3 then
		return redis.error_reply("Usage: channel_id point_id value [priority]")
	end

	local channel_id = args[1]
	local point_id = args[2]
	local value = args[3]
	local priority = tonumber(args[4]) or 5

	-- Create command structure
	local command = {
		channel = channel_id,
		point = point_id,
		value = value,
		priority = priority,
		timestamp = redis.call("TIME")[1],
		status = "pending",
	}

	local command_json = cjson.encode(command)

	-- Store in command queue
	local queue_key = string.format("cmd:queue:%s", channel_id)
	redis.call("ZADD", queue_key, priority, command_json)

	-- Store trigger event for processing
	redis.call("LPUSH", string.format("cmd:trigger:%s", channel_id), command_json)
	redis.call("LTRIM", string.format("cmd:trigger:%s", channel_id), 0, 99)

	-- Store last command
	redis.call("SET", string.format("cmd:last:%s:%s", channel_id, point_id), command_json, "EX", 300)

	return redis.status_reply("OK")
end

-- Process command queue
local function comsrv_process_queue(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: channel_id [limit]")
	end

	local channel_id = args[1]
	local limit = tonumber(args[2]) or 10

	local queue_key = string.format("cmd:queue:%s", channel_id)

	-- Get high priority commands
	local commands = redis.call("ZREVRANGEBYSCORE", queue_key, "+inf", "-inf", "LIMIT", 0, limit)

	local processed = {}

	for _, cmd_json in ipairs(commands) do
		local ok, cmd = pcall(cjson.decode, cmd_json)
		if ok then
			-- Mark as processing
			cmd.status = "processing"
			cmd.processed_at = redis.call("TIME")[1]

			-- Remove from queue
			redis.call("ZREM", queue_key, cmd_json)

			-- Store in processing set
			local proc_key = string.format("cmd:processing:%s", channel_id)
			redis.call("SADD", proc_key, cjson.encode(cmd))
			redis.call("EXPIRE", proc_key, 300)

			table.insert(processed, cmd)
		end
	end

	return cjson.encode(processed)
end

-- Complete command execution
local function comsrv_complete_command(keys, args)
	if #args < 3 then
		return redis.error_reply("Usage: channel_id point_id status [result]")
	end

	local channel_id = args[1]
	local point_id = args[2]
	local status = args[3]
	local result = args[4]

	-- Update command status
	local last_key = string.format("cmd:last:%s:%s", channel_id, point_id)
	local cmd_json = redis.call("GET", last_key)

	if cmd_json then
		local ok, cmd = pcall(cjson.decode, cmd_json)
		if ok then
			cmd.status = status
			cmd.completed_at = redis.call("TIME")[1]
			if result then
				cmd.result = result
			end

			-- Store completion
			local complete_key = string.format("cmd:complete:%s:%s", channel_id, point_id)
			redis.call("SET", complete_key, cjson.encode(cmd), "EX", 3600)

			-- Store completion event
			redis.call("LPUSH", string.format("cmd:complete:%s", channel_id), cjson.encode(cmd))
			redis.call("LTRIM", string.format("cmd:complete:%s", channel_id), 0, 99)
		end
	end

	return redis.status_reply("OK")
end

-- Get channel status
local function comsrv_channel_status(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: channel_id")
	end

	local channel_id = args[1]

	-- Use SCAN instead of KEYS to count telemetry keys
	local cursor = "0"
	local telemetry_count = 0
	repeat
		local result = redis.call("SCAN", cursor, "MATCH", string.format("comsrv:%s:*", channel_id), "COUNT", 100)
		cursor = result[1]
		telemetry_count = telemetry_count + #result[2]
	until cursor == "0"

	local status = {
		channel = channel_id,
		queue_size = redis.call("ZCARD", string.format("cmd:queue:%s", channel_id)),
		processing = redis.call("SCARD", string.format("cmd:processing:%s", channel_id)),
		telemetry_keys = telemetry_count,
		last_update = redis.call("HGET", string.format("comsrv:%s:T", channel_id), "_updated_at"),
	}

	return cjson.encode(status)
end

-- Batch telemetry write
local function comsrv_batch_write(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: batch_data_json")
	end

	local ok, batch_data = pcall(cjson.decode, args[1])
	if not ok then
		return redis.error_reply("Invalid batch data JSON")
	end

	local results = {}

	for channel_id, channel_data in pairs(batch_data) do
		for data_type, points in pairs(channel_data) do
			local hash_key = string.format("comsrv:%s:%s", channel_id, data_type)
			local count = 0

			for point_id, value in pairs(points) do
				redis.call("HSET", hash_key, tostring(point_id), tostring(value))
				count = count + 1
			end

			redis.call("HSET", hash_key, "_updated_at", redis.call("TIME")[1])

			results[hash_key] = count
		end
	end

	return cjson.encode(results)
end

-- ============ Alarm Check Functions ============

-- Simple alarm check without deadband
check_alarm_for_value = function(source_key, field, value)
	-- Find alarm rules associated with this data point
	local rules_key = string.format("idx:alarm:watch:%s:%s", source_key, field)
	local rule_ids = redis.call("SMEMBERS", rules_key)

	for _, rule_id in ipairs(rule_ids) do
		-- Get alarm rule configuration (use alarm:rule prefix)
		local rule = redis.call(
			"HMGET",
			string.format("alarm:rule:%s", rule_id),
			"threshold", -- Trigger threshold
			"operator", -- Operator: >, <, ==
			"enabled" -- Rule enabled status
		)

		-- Skip if rule is disabled
		if rule[3] ~= "false" then
			local threshold = tonumber(rule[1])
			if threshold then
				local operator = rule[2] or ">"

				-- Get current alarm status
				local alarm_key = string.format("alarm:%s", rule_id)
				local current_status = redis.call("HGET", alarm_key, "status")

				-- Evaluate condition
				local num_value = tonumber(value)
				if num_value then
					local condition_met = false
					if operator == ">" then
						condition_met = (num_value > threshold)
					elseif operator == "<" then
						condition_met = (num_value < threshold)
					elseif operator == "==" then
						condition_met = (num_value == threshold)
					elseif operator == ">=" then
						condition_met = (num_value >= threshold)
					elseif operator == "<=" then
						condition_met = (num_value <= threshold)
					elseif operator == "!=" then
						condition_met = (num_value ~= threshold)
					end

					-- Simple state machine
					if condition_met and current_status ~= "active" then
						-- Trigger alarm
						redis.call(
							"HMSET",
							alarm_key,
							"status",
							"active",
							"rule_id",
							rule_id,
							"source_key",
							source_key,
							"field",
							field,
							"trigger_value",
							value,
							"current_value",
							value,
							"threshold",
							threshold,
							"operator",
							operator,
							"triggered_at",
							redis.call("TIME")[1]
						)

						-- Add to active alarm index
						redis.call("SADD", "idx:alarm:active", rule_id)

						-- Record alarm event
						redis.call(
							"LPUSH",
							"alarm:events",
							cjson.encode({
								type = "triggered",
								rule_id = rule_id,
								value = value,
								threshold = threshold,
								timestamp = redis.call("TIME")[1],
							})
						)
						redis.call("LTRIM", "alarm:events", 0, 999)
					elseif not condition_met and current_status == "active" then
						-- Clear alarm
						redis.call(
							"HMSET",
							alarm_key,
							"status",
							"cleared",
							"clear_value",
							value,
							"cleared_at",
							redis.call("TIME")[1]
						)

						-- Remove from active alarm index
						redis.call("SREM", "idx:alarm:active", rule_id)

						-- Record clear event
						redis.call(
							"LPUSH",
							"alarm:events",
							cjson.encode({
								type = "cleared",
								rule_id = rule_id,
								value = value,
								threshold = threshold,
								timestamp = redis.call("TIME")[1],
							})
						)
						redis.call("LTRIM", "alarm:events", 0, 999)
					elseif current_status == "active" then
						-- Update current value
						redis.call("HSET", alarm_key, "current_value", value, "updated_at", redis.call("TIME")[1])
					end
				end -- if num_value
			end -- if threshold
		end -- if enabled
	end
end

-- ============ Alarm Service Functions ============

local function alarmsrv_trigger_alarm(keys, args)
	if #args < 2 then
		return redis.error_reply("Usage: alarm_type alarm_data")
	end

	local alarm_type = args[1]
	local alarm_data = args[2]

	local alarm_key = string.format("alarm:%s:%s", alarm_type, redis.call("TIME")[1])
	redis.call("SET", alarm_key, alarm_data, "EX", 86400)
	redis.call("LPUSH", "alarm:triggered", alarm_data)
	redis.call("LTRIM", "alarm:triggered", 0, 999)

	return redis.status_reply("OK")
end

local function alarmsrv_list_alarms(keys, args)
	local pattern = args[1] or "alarm:*"
	local alarms = {}
	local cursor = "0"

	repeat
		local result = redis.call("SCAN", cursor, "MATCH", pattern, "COUNT", 100)
		cursor = result[1]
		for _, key in ipairs(result[2]) do
			local data = redis.call("GET", key)
			if data then
				table.insert(alarms, data)
			end
		end
	until cursor == "0"

	return cjson.encode(alarms)
end

local function alarmsrv_acknowledge_alarm(keys, args)
	if #keys < 1 then
		return redis.error_reply("Usage: alarm_key")
	end

	local alarm_key = keys[1]
	redis.call("HSET", alarm_key, "acknowledged", "1")
	redis.call("HSET", alarm_key, "ack_time", redis.call("TIME")[1])

	return redis.status_reply("OK")
end

local function alarmsrv_clear_alarm(keys, args)
	if #keys < 1 then
		return redis.error_reply("Usage: alarm_key")
	end

	redis.call("DEL", keys[1])
	return redis.status_reply("OK")
end

-- ============ Model Service Functions ============

local function modsrv_upsert_model(keys, args)
	if #args < 2 then
		return redis.error_reply("Usage: model_id model_data")
	end

	local model_id = args[1]
	local model_data = args[2]

	local model_key = string.format("model:%s", model_id)
	redis.call("SET", model_key, model_data)
	redis.call("SADD", "model:index", model_id)

	return redis.status_reply("OK")
end

local function modsrv_get_model(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: model_id")
	end

	local model_key = string.format("model:%s", args[1])
	return redis.call("GET", model_key) or redis.error_reply("Model not found")
end

local function modsrv_list_models(keys, args)
	local models = redis.call("SMEMBERS", "model:index")
	local result = {}

	for _, model_id in ipairs(models) do
		local data = redis.call("GET", string.format("model:%s", model_id))
		if data then
			table.insert(result, data)
		end
	end

	return cjson.encode(result)
end

local function modsrv_delete_model(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: model_id")
	end

	local model_id = args[1]
	redis.call("DEL", string.format("model:%s", model_id))
	redis.call("SREM", "model:index", model_id)

	return redis.status_reply("OK")
end

local function modsrv_sync_measurement(keys, args)
	if #args < 2 then
		return redis.error_reply("Usage: channel_id measurement_data")
	end

	local channel_id = args[1]
	local measurement_data = args[2]

	-- Store measurement
	local meas_key = string.format("measurement:%s", channel_id)
	redis.call("SET", meas_key, measurement_data)

	-- Parse measurement data for alarm checking
	local ok, data = pcall(cjson.decode, measurement_data)
	if ok then
		-- Check alarms for each field in the measurement
		for field, value in pairs(data) do
			if type(value) == "number" or type(value) == "string" then
				check_alarm_for_value(meas_key, tostring(field), value)
			end
		end
	end

	-- Store sync event for processing
	redis.call("LPUSH", "measurement:sync", measurement_data)
	redis.call("LTRIM", "measurement:sync", 0, 999)

	return redis.status_reply("OK")
end

local function modsrv_execute_action(keys, args)
	if #keys < 1 or #args < 2 then
		return redis.error_reply("Usage: model_id action value [channel_id]")
	end

	local model_id = keys[1]
	local action = args[1]
	local value = args[2]
	local channel_id = args[3]

	-- Get model data to find channel_id and mappings
	local model_key = string.format("model:%s", model_id)
	local model_data = nil

	-- Check if model exists and get its data
	local key_type = redis.call("TYPE", model_key)
	if key_type.ok == "string" then
		model_data = redis.call("GET", model_key)
	elseif key_type.ok == "hash" then
		model_data = redis.call("HGET", model_key, "data")
	end

	if model_data then
		local ok, model = pcall(cjson.decode, model_data)
		if ok then
			-- Use channel_id from model if not provided
			if not channel_id and model.channel_id then
				channel_id = model.channel_id
			end

			-- Find the control point mapping
			local control_point = nil
			if model.mappings and model.mappings.action and model.mappings.action[action] then
				control_point = model.mappings.action[action]
			end

			-- Write to comsrv control channel if we have all the info
			if channel_id and control_point then
				local control_key = string.format("comsrv:%s:C", channel_id)
				redis.call("HSET", control_key, control_point, value)
				redis.call("HSET", control_key, "_executed_at", redis.call("TIME")[1])
			elseif channel_id then
				-- If no mapping, use action name directly as control point
				local control_key = string.format("comsrv:%s:C", channel_id)
				redis.call("HSET", control_key, action, value)
				redis.call("HSET", control_key, "_executed_at", redis.call("TIME")[1])
			end
		end
	end

	-- Also store in model action history
	local action_key = string.format("model:%s:action", model_id)
	redis.call("HSET", action_key, action, value)
	redis.call("HSET", action_key, "_executed_at", redis.call("TIME")[1])

	-- Queue action for tracking
	redis.call(
		"LPUSH",
		string.format("action:queue:%s", model_id),
		cjson.encode({
			action = action,
			value = value,
			channel_id = channel_id,
			timestamp = redis.call("TIME")[1],
		})
	)
	redis.call("LTRIM", string.format("action:queue:%s", model_id), 0, 99)

	return cjson.encode({
		status = "success",
		model_id = model_id,
		action = action,
		value = value,
		channel_id = channel_id or "unknown",
	})
end

-- Process control commands from modsrv:control:* keys
modsrv_process_control = function(keys, args)
	if #keys < 1 then
		return redis.error_reply("Usage: model_id")
	end

	local model_id = keys[1]
	local control_key = string.format("modsrv:control:%s", model_id)

	-- Get all control commands
	local control_data = redis.call("HGETALL", control_key)
	if #control_data == 0 then
		return cjson.encode({ status = "no_commands", model_id = model_id })
	end

	-- Get model configuration
	local model_key = string.format("model:%s", model_id)
	local model_data = nil

	local key_type = redis.call("TYPE", model_key)
	if key_type.ok == "string" then
		model_data = redis.call("GET", model_key)
	elseif key_type.ok == "hash" then
		model_data = redis.call("HGET", model_key, "data")
	end

	local channel_id = nil
	local mappings = nil

	if model_data then
		local ok, model = pcall(cjson.decode, model_data)
		if ok then
			channel_id = model.channel_id
			mappings = model.mappings and model.mappings.action
		end
	end

	if not channel_id then
		return redis.error_reply("Model has no channel_id configured")
	end

	-- Process each control command
	local processed = {}
	local comsrv_control_key = string.format("comsrv:%s:C", channel_id)

	for i = 1, #control_data, 2 do
		local field = control_data[i]
		local value = control_data[i + 1]

		-- Skip metadata fields
		if not string.match(field, "^_") then
			-- Map action to control point if mapping exists
			local control_point = field
			if mappings and mappings[field] then
				control_point = mappings[field]
			end

			-- Write to comsrv control channel
			redis.call("HSET", comsrv_control_key, control_point, value)

			-- Record in model action history
			local action_key = string.format("model:%s:action", model_id)
			redis.call("HSET", action_key, field, value)

			table.insert(processed, {
				action = field,
				control_point = control_point,
				value = value,
			})
		end
	end

	-- Add execution timestamp
	redis.call("HSET", comsrv_control_key, "_executed_at", redis.call("TIME")[1])
	redis.call("HSET", string.format("model:%s:action", model_id), "_executed_at", redis.call("TIME")[1])

	-- Clear processed commands
	redis.call("DEL", control_key)

	return cjson.encode({
		status = "success",
		model_id = model_id,
		channel_id = channel_id,
		commands_processed = #processed / 2,
		commands = processed,
	})
end

-- ============ Rule Service Functions ============

local function rulesrv_upsert_rule(keys, args)
	if #args < 2 then
		return redis.error_reply("Usage: rule_id rule_data")
	end

	local rule_id = args[1]
	local rule_data = args[2]

	local rule_key = string.format("rule:%s", rule_id)
	redis.call("SET", rule_key, rule_data)
	redis.call("SADD", "rule:index", rule_id)
	redis.call("HSET", "rule:status", rule_id, "enabled")

	return redis.status_reply("OK")
end

local function rulesrv_execute_batch(keys, args)
	if #keys < 1 then
		return redis.error_reply("Usage: batch_id")
	end

	local batch_id = keys[1]
	local executed_count = 0
	local rules_checked = 0
	local rules_triggered = 0

	-- Get all enabled rules
	local rule_ids = redis.call("SMEMBERS", "rule:index")

	for _, rule_id in ipairs(rule_ids) do
		rules_checked = rules_checked + 1

		-- Get rule data (handle both string and hash format)
		local rule_key = string.format("rule:%s", rule_id)
		local rule_data = nil

		-- Check key type first
		local key_type = redis.call("TYPE", rule_key)
		if key_type.ok == "string" then
			rule_data = redis.call("GET", rule_key)
		elseif key_type.ok == "hash" then
			rule_data = redis.call("HGET", rule_key, "data")
		end

		if rule_data then
			local ok, rule = pcall(cjson.decode, rule_data)
			if ok and rule.enabled ~= false then
				-- Evaluate rule condition
				local should_execute = false

				if rule.condition and rule.condition.type == "threshold" then
					-- Get value from Redis
					local value = redis.call("HGET", rule.condition.source, rule.condition.field)
					if value then
						value = tonumber(value)
						local threshold = tonumber(rule.condition.threshold)

						if rule.condition.operator == ">" and value > threshold then
							should_execute = true
						elseif rule.condition.operator == "<" and value < threshold then
							should_execute = true
						elseif rule.condition.operator == ">=" and value >= threshold then
							should_execute = true
						elseif rule.condition.operator == "<=" and value <= threshold then
							should_execute = true
						elseif rule.condition.operator == "==" and value == threshold then
							should_execute = true
						end
					end
				end

				-- Execute actions if condition met
				if should_execute then
					rules_triggered = rules_triggered + 1

					-- Execute actions
					if rule.actions then
						for _, action in ipairs(rule.actions) do
							if action.type == "redis_set" then
								redis.call("SET", action.key, action.value)
							elseif action.type == "redis_hset" then
								redis.call("HSET", action.key, action.field, action.value)

								-- Auto-process modsrv:control commands
								if string.match(action.key, "^modsrv:control:") then
									local model_id = string.match(action.key, "^modsrv:control:(.+)$")
									if model_id then
										-- Directly process the control command
										local process_result = modsrv_process_control({ model_id }, {})
										-- Publish event for notification
										redis.call("PUBLISH", "modsrv:control:processed", process_result)
									end
								end
							elseif action.type == "publish" then
								redis.call("PUBLISH", action.channel, action.message or "Rule triggered: " .. rule_id)
							end
						end
					end
				end
				executed_count = executed_count + 1
			end
		end
	end

	return cjson.encode({
		batch_id = batch_id,
		rules_executed = executed_count,
		rules_triggered = rules_triggered,
	})
end

local function rulesrv_list_rules(keys, args)
	local rules = redis.call("SMEMBERS", "rule:index")
	local result = {}

	for _, rule_id in ipairs(rules) do
		local data = redis.call("GET", string.format("rule:%s", rule_id))
		local status = redis.call("HGET", "rule:status", rule_id)
		if data then
			local rule = cjson.decode(data)
			rule.status = status
			table.insert(result, cjson.encode(rule))
		end
	end

	return cjson.encode(result)
end

local function rulesrv_enable_rule(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: rule_id")
	end

	redis.call("HSET", "rule:status", args[1], "enabled")
	return redis.status_reply("OK")
end

local function rulesrv_disable_rule(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: rule_id")
	end

	redis.call("HSET", "rule:status", args[1], "disabled")
	return redis.status_reply("OK")
end

-- ============ History Service Functions ============

local function hissrv_collect_batch(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: batch_data")
	end

	local batch_data = args[1]
	local ok, data = pcall(cjson.decode, batch_data)
	if not ok then
		return redis.error_reply("Invalid batch JSON")
	end

	local count = 0
	for channel_id, points in pairs(data) do
		local history_key = string.format("history:%s:%s", channel_id, redis.call("TIME")[1])
		for point_id, value in pairs(points) do
			redis.call("ZADD", history_key, redis.call("TIME")[1], string.format("%s:%s", point_id, value))
			count = count + 1
		end
		-- Set expiration for historical data (30 days)
		redis.call("EXPIRE", history_key, 2592000)
	end

	return count
end

-- ============ Sync Functions ============

local function sync_channel_data(keys, args)
	if #args < 2 then
		return redis.error_reply("Usage: source_pattern target_prefix")
	end

	local source_pattern = args[1]
	local target_prefix = args[2]

	local cursor = "0"
	local sync_count = 0

	repeat
		local result = redis.call("SCAN", cursor, "MATCH", source_pattern, "COUNT", 100)
		cursor = result[1]

		for _, source_key in ipairs(result[2]) do
			local data = redis.call("HGETALL", source_key)
			if #data > 0 then
				local target_key = source_key:gsub("^[^:]+", target_prefix)
				for i = 1, #data, 2 do
					redis.call("HSET", target_key, data[i], data[i + 1])
				end
				sync_count = sync_count + 1
			end
		end
	until cursor == "0"

	return sync_count
end

local function sync_comsrv_to_modsrv(keys, args)
	-- Sync telemetry data from comsrv to modsrv format
	local count = 0
	local cursor = "0"

	repeat
		local result = redis.call("SCAN", cursor, "MATCH", "comsrv:*:T", "COUNT", 100)
		cursor = result[1]

		for _, key in ipairs(result[2]) do
			local channel = key:match("comsrv:(%d+):T")
			if channel then
				local data = redis.call("HGETALL", key)
				if #data > 0 then
					local model_key = string.format("model:channel:%s", channel)
					for i = 1, #data, 2 do
						redis.call("HSET", model_key, data[i], data[i + 1])
					end
					count = count + 1
				end
			end
		end
	until cursor == "0"

	return count
end

local function sync_pattern_execute(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: pattern_name")
	end

	local pattern = args[1]

	if pattern == "comsrv_to_alarmsrv" then
		-- Check alarms based on telemetry data
		local alarms = {}
		-- Simplified alarm checking logic
		return cjson.encode(alarms)
	elseif pattern == "comsrv_to_hissrv" then
		-- Archive telemetry data to history
		return sync_channel_data(keys, { "comsrv:*:T", "history" })
	else
		return redis.error_reply("Unknown sync pattern: " .. pattern)
	end
end

local function generic_batch_init_points(keys, args)
	if #args < 4 then
		return redis.error_reply("Usage: channel_id redis_type points_json options_json")
	end

	local channel_id = args[1]
	local redis_type = args[2]
	local points_json = args[3]
	local options_json = args[4]

	-- Parse points JSON
	local ok, points = pcall(cjson.decode, points_json)
	if not ok then
		return redis.error_reply("Invalid points JSON: " .. tostring(points))
	end

	-- Parse options JSON
	local ok2, options = pcall(cjson.decode, options_json)
	if not ok2 then
		return redis.error_reply("Invalid options JSON: " .. tostring(options))
	end

	local count = 0
	-- Initialize each point with default value based on type
	for point_id, point_data in pairs(points) do
		local hash_key = string.format("comsrv:%s:%s", channel_id, redis_type)
		local default_value = options.default_value or "0"

		-- Set initial value for the point
		redis.call("HSET", hash_key, tostring(point_id), default_value)
		count = count + 1
	end

	return tostring(count)
end

-- ============ Statistics Functions ============

local function alarmsrv_get_statistics(keys, args)
	local stats = {
		total_alarms = redis.call("DBSIZE"),
		active_alarms = #redis.call("KEYS", "alarm:*"),
		acknowledged = #redis.call("KEYS", "alarm:*:ack"),
	}
	return cjson.encode(stats)
end

local function rulesrv_get_statistics(keys, args)
	local stats = {
		total_rules = redis.call("SCARD", "rule:index"),
		enabled_rules = 0,
		disabled_rules = 0,
	}

	local statuses = redis.call("HGETALL", "rule:status")
	for i = 2, #statuses, 2 do
		if statuses[i] == "enabled" then
			stats.enabled_rules = stats.enabled_rules + 1
		else
			stats.disabled_rules = stats.disabled_rules + 1
		end
	end

	return cjson.encode(stats)
end

-- ============ Additional Service Functions ============

local function alarmsrv_get_alarm(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: alarm_id")
	end

	local alarm_key = string.format("alarm:%s", args[1])
	local alarm_data = redis.call("HGETALL", alarm_key)

	if #alarm_data == 0 then
		return redis.error_reply("Alarm not found")
	end

	-- Convert to object
	local alarm = {}
	for i = 1, #alarm_data, 2 do
		alarm[alarm_data[i]] = alarm_data[i + 1]
	end

	return cjson.encode(alarm)
end

local function alarmsrv_list_active_alarms(keys, args)
	local active_rule_ids = redis.call("SMEMBERS", "idx:alarm:active")
	local alarms = {}

	for _, rule_id in ipairs(active_rule_ids) do
		local alarm_key = string.format("alarm:%s", rule_id)
		local alarm_data = redis.call("HGETALL", alarm_key)

		if #alarm_data > 0 then
			local alarm = { rule_id = rule_id }
			for i = 1, #alarm_data, 2 do
				alarm[alarm_data[i]] = alarm_data[i + 1]
			end
			table.insert(alarms, alarm)
		end
	end

	return cjson.encode(alarms)
end

-- Note: alarmsrv_get_statistics function removed as it was unused
-- Can be added back if needed for statistics dashboard

local function alarmsrv_create_rule(keys, args)
	if #args < 2 then
		return redis.error_reply("Usage: alarm_rule_id alarm_config_json")
	end

	local rule_id = args[1]
	local config_json = args[2]

	-- Parse rule configuration
	local ok, config = pcall(cjson.decode, config_json)
	if not ok then
		return redis.error_reply("Invalid alarm rule configuration JSON")
	end

	-- Validate required fields
	if not config.source_key or not config.field or not config.threshold then
		return redis.error_reply("Missing required fields: source_key, field, threshold")
	end

	-- Set defaults
	config.operator = config.operator or ">"
	config.enabled = config.enabled ~= false -- Default to true
	config.alarm_level = config.alarm_level or "Warning"
	config.alarm_title = config.alarm_title or string.format("Alarm for %s.%s", config.source_key, config.field)

	-- Store alarm rule in Hash format (use alarm:rule prefix to avoid conflict)
	local rule_key = string.format("alarm:rule:%s", rule_id)
	redis.call(
		"HMSET",
		rule_key,
		"id",
		rule_id,
		"source_key",
		config.source_key,
		"field",
		config.field,
		"threshold",
		tostring(config.threshold),
		"operator",
		config.operator,
		"enabled",
		tostring(config.enabled),
		"alarm_level",
		config.alarm_level,
		"alarm_title",
		config.alarm_title,
		"created_at",
		redis.call("TIME")[1]
	)

	-- Create reverse index for efficient rule lookup
	local watch_key = string.format("idx:alarm:watch:%s:%s", config.source_key, config.field)
	redis.call("SADD", watch_key, rule_id)

	-- Add to alarm rule index (separate from rulesrv rules)
	redis.call("SADD", "alarm:rule:index", rule_id)

	-- Store complete config as JSON for reference
	redis.call("SET", string.format("alarm:rule:config:%s", rule_id), config_json)

	return redis.status_reply("OK")
end

local function alarmsrv_upsert_rule(keys, args)
	return alarmsrv_create_rule(keys, args) -- Use the new create_rule function
end

local function alarmsrv_get_rule(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: alarm_rule_id")
	end

	local rule_key = string.format("alarm:rule:%s", args[1])
	local rule_data = redis.call("HGETALL", rule_key)

	if #rule_data == 0 then
		return redis.error_reply("Alarm rule not found")
	end

	-- Convert to object
	local rule = {}
	for i = 1, #rule_data, 2 do
		rule[rule_data[i]] = rule_data[i + 1]
	end

	return cjson.encode(rule)
end

local function alarmsrv_list_rules(keys, args)
	local rule_ids = redis.call("SMEMBERS", "alarm:rule:index")
	local rules = {}

	for _, rule_id in ipairs(rule_ids) do
		local rule_key = string.format("alarm:rule:%s", rule_id)
		local rule_data = redis.call("HGETALL", rule_key)

		if #rule_data > 0 then
			local rule = { id = rule_id }
			for i = 1, #rule_data, 2 do
				rule[rule_data[i]] = rule_data[i + 1]
			end
			table.insert(rules, rule)
		end
	end

	return cjson.encode(rules)
end

local function alarmsrv_enable_rule(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: alarm_rule_id")
	end

	local rule_id = args[1]
	local rule_key = string.format("alarm:rule:%s", rule_id)

	-- Check if rule exists
	local exists = redis.call("EXISTS", rule_key)
	if exists == 0 then
		return redis.error_reply("Alarm rule not found")
	end

	-- Enable the rule
	redis.call("HSET", rule_key, "enabled", "true")

	return redis.status_reply("OK")
end

local function alarmsrv_disable_rule(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: alarm_rule_id")
	end

	local rule_id = args[1]
	local rule_key = string.format("alarm:rule:%s", rule_id)

	-- Check if rule exists
	local exists = redis.call("EXISTS", rule_key)
	if exists == 0 then
		return redis.error_reply("Alarm rule not found")
	end

	-- Disable the rule
	redis.call("HSET", rule_key, "enabled", "false")

	-- If there's an active alarm for this rule, clear it
	local alarm_key = string.format("alarm:%s", rule_id)
	local status = redis.call("HGET", alarm_key, "status")
	if status == "active" then
		redis.call("HSET", alarm_key, "status", "cleared")
		redis.call("HSET", alarm_key, "cleared_at", redis.call("TIME")[1])
		redis.call("HSET", alarm_key, "clear_reason", "rule_disabled")
		redis.call("SREM", "idx:alarm:active", rule_id)

		-- Record disable event
		redis.call(
			"LPUSH",
			"alarm:events",
			cjson.encode({
				type = "cleared_by_disable",
				rule_id = rule_id,
				timestamp = redis.call("TIME")[1],
			})
		)
		redis.call("LTRIM", "alarm:events", 0, 999)
	end

	return redis.status_reply("OK")
end

local function alarmsrv_delete_rule(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: alarm_rule_id")
	end

	local rule_id = args[1]
	local rule_key = string.format("alarm:rule:%s", rule_id)

	-- Get rule details before deletion to clean up indices
	local rule_data = redis.call("HMGET", rule_key, "source_key", "field")
	local source_key = rule_data[1]
	local field = rule_data[2]

	if source_key and field then
		-- Remove from watch index
		local watch_key = string.format("idx:alarm:watch:%s:%s", source_key, field)
		redis.call("SREM", watch_key, rule_id)
	end

	-- Delete alarm rule and associated data
	redis.call("DEL", rule_key)
	redis.call("DEL", string.format("alarm:rule:config:%s", rule_id))
	redis.call("DEL", string.format("alarm:%s", rule_id))

	-- Remove from alarm indices
	redis.call("SREM", "alarm:rule:index", rule_id)
	redis.call("SREM", "idx:alarm:active", rule_id)

	return redis.status_reply("OK")
end

local function modsrv_upsert_template(keys, args)
	if #args < 2 then
		return redis.error_reply("Usage: template_id template_data")
	end

	local template_id = args[1]
	local template_data = args[2]

	redis.call("SET", string.format("template:%s", template_id), template_data)
	redis.call("SADD", "template:index", template_id)

	return redis.status_reply("OK")
end

local function modsrv_get_template(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: template_id")
	end

	local template_key = string.format("template:%s", args[1])
	return redis.call("GET", template_key) or redis.error_reply("Template not found")
end

local function modsrv_list_templates(keys, args)
	local templates = redis.call("SMEMBERS", "template:index")
	local result = {}

	for _, template_id in ipairs(templates) do
		local data = redis.call("GET", string.format("template:%s", template_id))
		if data then
			table.insert(result, data)
		end
	end

	return cjson.encode(result)
end

local function modsrv_delete_template(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: template_id")
	end

	local template_id = args[1]
	redis.call("DEL", string.format("template:%s", template_id))
	redis.call("SREM", "template:index", template_id)

	return redis.status_reply("OK")
end

local function modsrv_get_model_data(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: model_id")
	end

	local model_id = args[1]
	local model_key = string.format("model:%s", model_id)
	local data_key = string.format("model:data:%s", model_id)

	local model = redis.call("GET", model_key)
	local data = redis.call("HGETALL", data_key)

	return cjson.encode({
		model = model,
		data = data,
	})
end

local function modsrv_sync_all_measurements(keys, args)
	local count = 0
	local cursor = "0"

	repeat
		local result = redis.call("SCAN", cursor, "MATCH", "measurement:*", "COUNT", 100)
		cursor = result[1]
		count = count + #result[2]
	until cursor == "0"

	return count
end

local function rulesrv_get_rule(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: rule_id")
	end

	local rule_key = string.format("rule:%s", args[1])
	return redis.call("GET", rule_key) or redis.error_reply("Rule not found")
end

local function rulesrv_delete_rule(keys, args)
	if #args < 1 then
		return redis.error_reply("Usage: rule_id")
	end

	local rule_id = args[1]
	redis.call("DEL", string.format("rule:%s", rule_id))
	redis.call("SREM", "rule:index", rule_id)
	redis.call("HDEL", "rule:status", rule_id)

	return redis.status_reply("OK")
end

local function rulesrv_list_executions(keys, args)
	local executions = redis.call("LRANGE", "rule:executions", 0, 99)
	return cjson.encode(executions)
end

-- Register all functions
redis.register_function("comsrv_write_telemetry", comsrv_write_telemetry)
redis.register_function("comsrv_write_control", comsrv_write_control)
redis.register_function("comsrv_read_telemetry", comsrv_read_telemetry)
redis.register_function("comsrv_trigger_command", comsrv_trigger_command)
redis.register_function("comsrv_process_queue", comsrv_process_queue)
redis.register_function("comsrv_complete_command", comsrv_complete_command)
redis.register_function("comsrv_channel_status", comsrv_channel_status)
redis.register_function("comsrv_batch_write", comsrv_batch_write)

redis.register_function("alarmsrv_trigger_alarm", alarmsrv_trigger_alarm)
redis.register_function("alarmsrv_list_alarms", alarmsrv_list_alarms)
redis.register_function("alarmsrv_acknowledge_alarm", alarmsrv_acknowledge_alarm)
redis.register_function("alarmsrv_clear_alarm", alarmsrv_clear_alarm)
redis.register_function("alarmsrv_get_alarm", alarmsrv_get_alarm)
redis.register_function("alarmsrv_create_rule", alarmsrv_create_rule)
redis.register_function("alarmsrv_upsert_rule", alarmsrv_upsert_rule)
redis.register_function("alarmsrv_get_rule", alarmsrv_get_rule)
redis.register_function("alarmsrv_list_rules", alarmsrv_list_rules)
redis.register_function("alarmsrv_enable_rule", alarmsrv_enable_rule)
redis.register_function("alarmsrv_disable_rule", alarmsrv_disable_rule)
redis.register_function("alarmsrv_delete_rule", alarmsrv_delete_rule)
redis.register_function("alarmsrv_list_active_alarms", alarmsrv_list_active_alarms)
redis.register_function("alarmsrv_get_statistics", alarmsrv_get_statistics)

redis.register_function("modsrv_upsert_model", modsrv_upsert_model)
redis.register_function("modsrv_get_model", modsrv_get_model)
redis.register_function("modsrv_list_models", modsrv_list_models)
redis.register_function("modsrv_delete_model", modsrv_delete_model)
redis.register_function("modsrv_upsert_template", modsrv_upsert_template)
redis.register_function("modsrv_get_template", modsrv_get_template)
redis.register_function("modsrv_list_templates", modsrv_list_templates)
redis.register_function("modsrv_delete_template", modsrv_delete_template)
redis.register_function("modsrv_sync_measurement", modsrv_sync_measurement)
redis.register_function("modsrv_execute_action", modsrv_execute_action)
redis.register_function("modsrv_process_control", modsrv_process_control)
redis.register_function("modsrv_get_model_data", modsrv_get_model_data)
redis.register_function("modsrv_sync_all_measurements", modsrv_sync_all_measurements)

redis.register_function("rulesrv_upsert_rule", rulesrv_upsert_rule)
redis.register_function("rulesrv_execute_batch", rulesrv_execute_batch)
redis.register_function("rulesrv_list_rules", rulesrv_list_rules)
redis.register_function("rulesrv_get_rule", rulesrv_get_rule)
redis.register_function("rulesrv_delete_rule", rulesrv_delete_rule)
redis.register_function("rulesrv_enable_rule", rulesrv_enable_rule)
redis.register_function("rulesrv_disable_rule", rulesrv_disable_rule)
redis.register_function("rulesrv_get_statistics", rulesrv_get_statistics)
redis.register_function("rulesrv_list_executions", rulesrv_list_executions)

redis.register_function("hissrv_collect_batch", hissrv_collect_batch)

redis.register_function("sync_channel_data", sync_channel_data)
redis.register_function("sync_comsrv_to_modsrv", sync_comsrv_to_modsrv)
redis.register_function("sync_pattern_execute", sync_pattern_execute)
redis.register_function("generic_batch_init_points", generic_batch_init_points)
