#!lua name=comsrv_sync

-- Sync function from ComsRv to ModSrv
local function sync_comsrv_to_modsrv(keys, args)
	local channel_id = keys[1]
	local telemetry_type = keys[2]
	local updates_json = args[1]

	-- Parse JSON updates
	local updates = cjson.decode(updates_json)
	local sync_count = 0

	-- Process each update
	for _, update in ipairs(updates) do
		local point_id = update.point_id
		local value = update.value

		-- Store in Redis hash
		local key = string.format("comsrv:%s:%s", channel_id, telemetry_type)
		redis.call("HSET", key, tostring(point_id), tostring(value))

		-- Also store with timestamp for historical data
		local ts_key = string.format("comsrv:%s:%s:ts", channel_id, telemetry_type)
		local timestamp = redis.call("TIME")[1]
		redis.call("ZADD", ts_key, timestamp, string.format("%s:%s", point_id, value))

		sync_count = sync_count + 1
	end

	-- Return sync result
	local result = {
		sync_count = sync_count,
		channel_id = channel_id,
		telemetry_type = telemetry_type,
	}

	return cjson.encode(result)
end

-- Register function
redis.register_function("sync_comsrv_to_modsrv", sync_comsrv_to_modsrv)
