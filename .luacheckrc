-- Luacheck configuration for Redis Lua scripts
-- Redis provides these as globals in the Lua environment

globals = {
    "redis",      -- Redis command interface
    "cjson",      -- JSON encoding/decoding library
    "unpack",     -- Lua 5.1 unpack (Redis uses Lua 5.1)
}

-- Ignore warnings for Redis function libraries
files["scripts/redis-functions/*.lua"] = {
    -- These are Redis function libraries, not standalone scripts
    ignore = {
        "111",  -- setting non-standard global variable
        "112",  -- mutating non-standard global variable
        "113",  -- accessing undefined variable
        "142",  -- setting undefined global variable
        "143",  -- accessing undefined global variable
        "212",  -- unused argument (Redis functions must have keys/args parameters)
    }
}

-- Standard Lua settings
std = "lua51"   -- Redis uses Lua 5.1
max_line_length = 120