-- Luacheck configuration for Redis Lua functions
globals = {
    "redis",
    "cjson",
    "KEYS",
    "ARGV"
}

-- Redis Lua functions use unpack (Lua 5.1 style)
read_globals = {
    "unpack"
}

ignore = {
    "611", -- Line contains only whitespace
    "212", -- Unused argument (common in Redis function signatures)
    "213", -- Unused loop variable
}

-- Allow unused function arguments (Redis function signatures require keys and args)
unused_args = false

-- Standard Lua version (Redis uses Lua 5.1 compatibility)
std = "lua51"