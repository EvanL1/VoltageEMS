#!lua name=comsrv

-- ========================================
-- ComSrv Specific Functions
-- Communication Service Redis Functions
-- Patched version with critical fixes
-- ========================================

-- Redis 8.0+ uses redis.cjson instead of require("cjson")
local cjson = redis.cjson or cjson

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

-- Batch update telemetry data for a channel
local function comsrv_batch_update(keys, args)
	if #args < 3 then
		return redis.error_reply("Usage: channel_id telemetry_type updates_json")
	end

	local channel_id = args[1]
	local telemetry_type = args[2] -- T, S, C, or A
	-- Safe JSON decode
	local updates, err = safe_decode(args[3], "updates")
	if err then
		return err
	end

	-- Build Redis key
	local hash_key = string.format("comsrv:%s:%s", channel_id, telemetry_type)

	-- Prepare batch update data
	local update_data = {}
	local update_count = 0

	for point_id, value in pairs(updates) do
		table.insert(update_data, tostring(point_id))
		-- Handle different value types
		if type(value) == "boolean" then
			table.insert(update_data, value and "1" or "0")
		else
			table.insert(update_data, tostring(value))
		end
		update_count = update_count + 1
	end

	-- Perform batch update
	if #update_data > 0 then
		redis.call("HSET", hash_key, unpack(update_data))
	end

	-- Update timestamp
	local timestamp_key = string.format("comsrv:%s:meta", channel_id)
	redis.call(
		"HSET",
		timestamp_key,
		"last_update",
		redis.call("TIME")[1],
		"last_update_type",
		telemetry_type,
		"last_update_count",
		update_count
	)

	-- Publish update event for subscribers
	redis.call(
		"PUBLISH",
		string.format("comsrv:update:%s:%s", channel_id, telemetry_type),
		cjson.encode({
			channel_id = channel_id,
			telemetry_type = telemetry_type,
			count = update_count,
			timestamp = redis.call("TIME")[1],
		})
	)

	-- Trigger alarm checking if enabled
	if telemetry_type == "T" or telemetry_type == "S" then
		-- Publish to alarm service for threshold checking
		redis.call(
			"PUBLISH",
			"alarmsrv:check",
			cjson.encode({
				source = "comsrv",
				channel_id = channel_id,
				telemetry_type = telemetry_type,
				points = updates,
			})
		)
	end

	return cjson.encode({
		status = "success",
		channel_id = channel_id,
		telemetry_type = telemetry_type,
		updated_count = update_count,
	})
end

-- Process command trigger (YK/YT)
local function comsrv_command_trigger(keys, args)
	if #args < 3 then
		return redis.error_reply("Usage: channel_id command_type command_json")
	end

	local channel_id = args[1]
	local command_type = args[2] -- "C" for Control (YK) or "A" for Adjustment (YT)
	-- Safe JSON decode
	local command, err = safe_decode(args[3], "command")
	if err then
		return err
	end

	-- Validate command type
	if command_type ~= "C" and command_type ~= "A" then
		return redis.error_reply("Invalid command type. Must be 'C' (Control) or 'A' (Adjustment)")
	end

	-- Generate command ID
	local command_id = string.format(
		"%s_%s_%d_%d",
		channel_id,
		command_type,
		redis.call("TIME")[1],
		redis.call("INCR", "comsrv:command:counter")
	)

	-- Store command in queue
	local queue_key = string.format("comsrv:cmd:%s:%s", channel_id, command_type)
	local command_data = cjson.encode({
		command_id = command_id,
		channel_id = channel_id,
		point_id = command.point_id,
		value = command.value,
		timestamp = redis.call("TIME")[1],
		source = command.source or "manual",
		priority = command.priority or 5,
	})

	-- Use BLPOP-compatible list for command queue
	redis.call("LPUSH", queue_key, command_data)

	-- Set TTL for command queue (avoid infinite accumulation)
	redis.call("EXPIRE", queue_key, 300) -- 5 minutes TTL

	-- Store command history
	local history_key = string.format("comsrv:cmd:history:%s", channel_id)
	redis.call("ZADD", history_key, redis.call("TIME")[1], command_data)
	-- Keep only last 1000 commands
	redis.call("ZREMRANGEBYRANK", history_key, 0, -1001)

	-- Update command statistics
	redis.call(
		"HINCRBY",
		string.format("comsrv:stats:%s", channel_id),
		command_type == "C" and "control_commands" or "adjustment_commands",
		1
	)

	-- Publish command event
	redis.call("PUBLISH", string.format("comsrv:command:%s", channel_id), command_data)

	return cjson.encode({
		status = "success",
		command_id = command_id,
		channel_id = channel_id,
		command_type = command_type,
		queued = true,
	})
end

-- Get four-telemetry data for a channel
local function comsrv_get_four_telemetry(keys, args)
	if #keys < 1 then
		return redis.error_reply("Usage: channel_id [telemetry_types] [point_ids]")
	end

	local channel_id = keys[1]
	-- Safe JSON decode with defaults
	local telemetry_types = { "T", "S", "C", "A" }
	if args[1] then
		local decoded = safe_decode(args[1], "telemetry types")
		if decoded then
			telemetry_types = decoded
		end
	end

	local point_ids = nil
	if args[2] then
		point_ids = safe_decode(args[2], "point IDs")
	end

	local result = {}

	for _, telemetry_type in ipairs(telemetry_types) do
		local hash_key = string.format("comsrv:%s:%s", channel_id, telemetry_type)
		local data = {}

		if point_ids then
			-- Get specific points
			local values = redis.call("HMGET", hash_key, unpack(point_ids))
			for i, point_id in ipairs(point_ids) do
				if values[i] ~= false then
					data[tostring(point_id)] = values[i]
				end
			end
		else
			-- Get all points
			local all_data = redis.call("HGETALL", hash_key)
			for i = 1, #all_data, 2 do
				data[all_data[i]] = all_data[i + 1]
			end
		end

		result[telemetry_type] = data
	end

	-- Get metadata
	local meta_key = string.format("comsrv:%s:meta", channel_id)
	local metadata = redis.call("HGETALL", meta_key)
	local meta = {}
	for i = 1, #metadata, 2 do
		meta[metadata[i]] = metadata[i + 1]
	end

	return cjson.encode({
		channel_id = channel_id,
		data = result,
		metadata = meta,
		timestamp = redis.call("TIME")[1],
	})
end

-- Handle server mode data request
local function comsrv_server_handle_read(keys, args)
	if #args < 3 then
		return redis.error_reply("Usage: channel_id telemetry_type start_address count")
	end

	local channel_id = args[1]
	local telemetry_type = args[2]
	local start_address = tonumber(args[3])
	local count = tonumber(args[4])

	local hash_key = string.format("comsrv:%s:%s", channel_id, telemetry_type)
	local result = {}

	-- Read sequential points from start_address
	for i = 0, count - 1 do
		local point_id = start_address + i
		local value = redis.call("HGET", hash_key, tostring(point_id))

		if value ~= false then
			table.insert(result, value)
		else
			-- Return default value based on telemetry type
			if telemetry_type == "S" or telemetry_type == "C" then
				table.insert(result, "0") -- Default false for boolean
			else
				table.insert(result, "0.0") -- Default 0.0 for numeric
			end
		end
	end

	-- Update access statistics
	redis.call("HINCRBY", string.format("comsrv:stats:%s", channel_id), "read_requests", 1)
	redis.call("HINCRBY", string.format("comsrv:stats:%s", channel_id), "points_read", count)

	return cjson.encode({
		status = "success",
		channel_id = channel_id,
		telemetry_type = telemetry_type,
		start_address = start_address,
		count = count,
		values = result,
	})
end

-- Handle server mode data write
local function comsrv_server_handle_write(keys, args)
	if #args < 4 then
		return redis.error_reply("Usage: channel_id telemetry_type address value")
	end

	local channel_id = args[1]
	local telemetry_type = args[2]
	local address = tostring(args[3])
	local value = args[4]

	local hash_key = string.format("comsrv:%s:%s", channel_id, telemetry_type)

	-- Write single value
	redis.call("HSET", hash_key, address, value)

	-- Update metadata
	local meta_key = string.format("comsrv:%s:meta", channel_id)
	redis.call(
		"HSET",
		meta_key,
		"last_write",
		redis.call("TIME")[1],
		"last_write_type",
		telemetry_type,
		"last_write_address",
		address
	)

	-- Publish write event
	redis.call(
		"PUBLISH",
		string.format("comsrv:write:%s:%s", channel_id, telemetry_type),
		cjson.encode({
			channel_id = channel_id,
			telemetry_type = telemetry_type,
			address = address,
			value = value,
			timestamp = redis.call("TIME")[1],
		})
	)

	-- Update write statistics
	redis.call("HINCRBY", string.format("comsrv:stats:%s", channel_id), "write_requests", 1)

	return cjson.encode({
		status = "success",
		channel_id = channel_id,
		telemetry_type = telemetry_type,
		address = address,
		value = value,
	})
end

-- Sync data between channels (for server-client testing)
local function comsrv_sync_channels(keys, args)
	if #args < 2 then
		return redis.error_reply("Usage: source_channel_id dest_channel_id [telemetry_types]")
	end

	local source_channel = args[1]
	local dest_channel = args[2]
	-- Safe JSON decode with defaults
	local telemetry_types = { "T", "S", "C", "A" }
	if args[3] then
		local decoded = safe_decode(args[3], "telemetry types")
		if decoded then
			telemetry_types = decoded
		end
	end

	local synced_points = 0

	for _, telemetry_type in ipairs(telemetry_types) do
		local source_key = string.format("comsrv:%s:%s", source_channel, telemetry_type)
		local dest_key = string.format("comsrv:%s:%s", dest_channel, telemetry_type)

		-- Get all data from source
		local data = redis.call("HGETALL", source_key)

		if #data > 0 then
			-- Copy to destination
			redis.call("HSET", dest_key, unpack(data))
			synced_points = synced_points + (#data / 2)
		end
	end

	-- Update sync metadata
	local meta_key = string.format("comsrv:%s:meta", dest_channel)
	redis.call(
		"HSET",
		meta_key,
		"last_sync",
		redis.call("TIME")[1],
		"sync_source",
		source_channel,
		"sync_points",
		synced_points
	)

	return cjson.encode({
		status = "success",
		source_channel = source_channel,
		dest_channel = dest_channel,
		synced_points = synced_points,
		telemetry_types = telemetry_types,
	})
end

-- Get channel statistics
local function comsrv_get_channel_stats(keys, args)
	if #keys < 1 then
		return redis.error_reply("Usage: channel_id")
	end

	local channel_id = keys[1]
	local stats = {}

	-- Get basic statistics
	local stats_key = string.format("comsrv:stats:%s", channel_id)
	local stat_data = redis.call("HGETALL", stats_key)
	for i = 1, #stat_data, 2 do
		stats[stat_data[i]] = tonumber(stat_data[i + 1]) or stat_data[i + 1]
	end

	-- Count points per telemetry type
	local point_counts = {}
	for _, telemetry_type in ipairs({ "T", "S", "C", "A" }) do
		local hash_key = string.format("comsrv:%s:%s", channel_id, telemetry_type)
		point_counts[telemetry_type] = redis.call("HLEN", hash_key)
	end
	stats.point_counts = point_counts

	-- Get metadata
	local meta_key = string.format("comsrv:%s:meta", channel_id)
	local metadata = redis.call("HGETALL", meta_key)
	local meta = {}
	for i = 1, #metadata, 2 do
		meta[metadata[i]] = metadata[i + 1]
	end
	stats.metadata = meta

	-- Get command queue sizes
	local queue_sizes = {}
	queue_sizes.control = redis.call("LLEN", string.format("comsrv:cmd:%s:C", channel_id))
	queue_sizes.adjustment = redis.call("LLEN", string.format("comsrv:cmd:%s:A", channel_id))
	stats.command_queues = queue_sizes

	return cjson.encode(stats)
end

-- Clean up old data for a channel
local function comsrv_cleanup_channel(keys, args)
	if #keys < 1 then
		return redis.error_reply("Usage: channel_id [older_than_seconds]")
	end

	local channel_id = keys[1]
	local older_than = tonumber(args[1] or 86400) -- Default 24 hours

	local current_time = redis.call("TIME")[1]
	local cutoff_time = current_time - older_than

	local cleaned = {
		commands = 0,
		history = 0,
	}

	-- Clean command history
	local history_key = string.format("comsrv:cmd:history:%s", channel_id)
	cleaned.history = redis.call("ZREMRANGEBYSCORE", history_key, "-inf", cutoff_time)

	-- Clean old command queues if empty
	for _, cmd_type in ipairs({ "C", "A" }) do
		local queue_key = string.format("comsrv:cmd:%s:%s", channel_id, cmd_type)
		if redis.call("LLEN", queue_key) == 0 then
			redis.call("DEL", queue_key)
			cleaned.commands = cleaned.commands + 1
		end
	end

	return cjson.encode({
		status = "success",
		channel_id = channel_id,
		cleaned = cleaned,
		older_than_seconds = older_than,
	})
end

-- Register all functions
redis.register_function("comsrv_batch_update", comsrv_batch_update)
redis.register_function("comsrv_command_trigger", comsrv_command_trigger)
redis.register_function("comsrv_get_four_telemetry", comsrv_get_four_telemetry)
redis.register_function("comsrv_server_handle_read", comsrv_server_handle_read)
redis.register_function("comsrv_server_handle_write", comsrv_server_handle_write)
redis.register_function("comsrv_sync_channels", comsrv_sync_channels)
redis.register_function("comsrv_get_channel_stats", comsrv_get_channel_stats)
redis.register_function("comsrv_cleanup_channel", comsrv_cleanup_channel)
