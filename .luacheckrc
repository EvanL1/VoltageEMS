-- Luacheck configuration for VoltageEMS Redis Lua Functions
-- 针对 Redis Functions 优化的配置

-- Redis 提供的全局变量和函数
globals = {
    "redis",
    "KEYS",
    "ARGV"
}

-- Redis 只读全局变量
read_globals = {
    -- Redis API
    "redis.call",
    "redis.pcall",
    "redis.log",
    "redis.LOG_DEBUG",
    "redis.LOG_VERBOSE", 
    "redis.LOG_NOTICE",
    "redis.LOG_WARNING",
    "redis.error_reply",
    "redis.status_reply",
    
    -- Lua 标准库（Redis 中可用的部分）
    "string",
    "table",
    "math",
    "tonumber",
    "tostring",
    "type",
    "pairs",
    "ipairs",
    "next",
    "unpack",
    "pcall",
    "xpcall"
}

-- 允许未使用的参数（Redis Functions 常见）
unused_args = false
unused_secondaries = false

-- 允许的最大行长度
max_line_length = 120

-- 允许重定义局部变量
redefined = false

-- 允许最大循环复杂度（Redis Functions 逻辑复杂是正常的）
max_cyclomatic_complexity = 20

-- 忽略特定警告
ignore = {
    "212", -- 未使用的参数
    "213", -- 未使用的循环变量
}

-- 忽略特定文件或目录
exclude_files = {
    ".venv/**/*",
    "node_modules/**/*",
    "target/**/*",
    "**/*.min.lua"
}