-- Performance test script for Redis
local start_time = redis.call('TIME')
local start_sec = start_time[1]
local start_usec = start_time[2]

-- Test 1: Hash write performance
for i = 1, 10000 do
    redis.call('HSET', 'perf:hash', 'field' .. i, 'value' .. i)
end

-- Test 2: Lua function performance
for i = 1, 100 do
    redis.call('FCALL', 'model_upsert', 1, 'perf_model_' .. i, '{"name":"Performance Test ' .. i .. '"}')
end

local end_time = redis.call('TIME')
local end_sec = end_time[1]
local end_usec = end_time[2]

local elapsed_ms = (end_sec - start_sec) * 1000 + (end_usec - start_usec) / 1000

return {
    hash_writes = 10000,
    function_calls = 100,
    elapsed_ms = elapsed_ms,
    hash_ops_per_sec = math.floor(10000 * 1000 / elapsed_ms),
    function_ops_per_sec = math.floor(100 * 1000 / elapsed_ms)
}