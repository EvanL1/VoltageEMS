-- VoltageEMS 配置框架 SQLite 数据库架构
-- 用于存储动态配置、点表数据和配置历史

-- 配置主表
-- 存储所有服务的键值对配置
CREATE TABLE IF NOT EXISTS configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service TEXT NOT NULL,              -- 服务名称 (comsrv, modsrv, etc.)
    key TEXT NOT NULL,                  -- 配置键 (支持嵌套，如 'redis.host')
    value TEXT NOT NULL,                -- 配置值 (JSON 格式)
    type TEXT NOT NULL DEFAULT 'json',  -- 值类型: json/yaml/toml/string/number/boolean
    description TEXT,                   -- 配置项说明
    version INTEGER DEFAULT 1,          -- 配置版本号
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_by TEXT,                    -- 创建者
    updated_by TEXT,                    -- 最后更新者
    is_active BOOLEAN DEFAULT 1,        -- 是否激活
    UNIQUE(service, key)
);

-- 配置历史表
-- 记录所有配置变更历史
CREATE TABLE IF NOT EXISTS config_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    config_id INTEGER NOT NULL,         -- 关联 configs 表
    service TEXT NOT NULL,
    key TEXT NOT NULL,
    old_value TEXT,
    new_value TEXT,
    old_type TEXT,
    new_type TEXT,
    operation TEXT NOT NULL,            -- create/update/delete
    changed_by TEXT,
    changed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    change_reason TEXT,                 -- 变更原因说明
    FOREIGN KEY(config_id) REFERENCES configs(id)
);

-- CSV 点表存储
-- 存储四遥（YC/YX/YK/YT）点表数据
CREATE TABLE IF NOT EXISTS point_tables (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_id INTEGER NOT NULL,        -- 通道 ID
    channel_name TEXT NOT NULL,         -- 通道名称
    point_id TEXT NOT NULL,             -- 点位 ID
    point_name TEXT NOT NULL,           -- 点位名称
    point_type TEXT NOT NULL,           -- 点位类型: YC/YX/YK/YT
    data_type TEXT,                     -- 数据类型: float/int/bool/string
    unit TEXT,                          -- 单位
    scale REAL DEFAULT 1.0,             -- 比例系数
    offset REAL DEFAULT 0.0,            -- 偏移量
    min_value REAL,                     -- 最小值
    max_value REAL,                     -- 最大值
    description TEXT,                   -- 描述
    metadata TEXT,                      -- JSON 格式的额外元数据
    is_active BOOLEAN DEFAULT 1,        -- 是否激活
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(channel_id, point_id)
);

-- 协议映射表
-- 存储点位到具体协议地址的映射
CREATE TABLE IF NOT EXISTS protocol_mappings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_id INTEGER NOT NULL,
    point_id TEXT NOT NULL,
    protocol TEXT NOT NULL,             -- 协议类型: modbus/iec104/can/gpio
    address TEXT NOT NULL,              -- 协议地址 (JSON 格式)
    params TEXT,                        -- JSON 格式的协议参数
    
    -- Modbus 特定字段（可选）
    slave_id INTEGER,                   -- Modbus 从站地址
    function_code INTEGER,              -- 功能码
    register_address INTEGER,           -- 寄存器地址
    register_count INTEGER DEFAULT 1,   -- 寄存器数量
    byte_order TEXT,                    -- 字节序: ABCD/DCBA/BADC/CDAB
    
    -- IEC104 特定字段（可选）
    ioa_address INTEGER,                -- 信息对象地址
    type_id INTEGER,                    -- 类型标识
    
    -- CAN 特定字段（可选）
    can_id INTEGER,                     -- CAN ID
    start_bit INTEGER,                  -- 起始位
    bit_length INTEGER,                 -- 位长度
    
    is_active BOOLEAN DEFAULT 1,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(channel_id, point_id) REFERENCES point_tables(channel_id, point_id)
);

-- 配置模板表
-- 存储可重用的配置模板
CREATE TABLE IF NOT EXISTS config_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,          -- 模板名称
    service TEXT NOT NULL,              -- 适用服务
    description TEXT,
    template_data TEXT NOT NULL,        -- JSON 格式的模板数据
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_by TEXT,
    is_active BOOLEAN DEFAULT 1
);

-- 配置验证规则表
-- 存储配置项的验证规则
CREATE TABLE IF NOT EXISTS config_validators (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service TEXT NOT NULL,
    key TEXT NOT NULL,
    rule_type TEXT NOT NULL,            -- regex/range/enum/custom
    rule_data TEXT NOT NULL,            -- JSON 格式的规则数据
    error_message TEXT,
    is_active BOOLEAN DEFAULT 1,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(service, key, rule_type)
);

-- 创建索引以提高查询性能
CREATE INDEX IF NOT EXISTS idx_configs_service_key ON configs(service, key);
CREATE INDEX IF NOT EXISTS idx_configs_active ON configs(is_active);
CREATE INDEX IF NOT EXISTS idx_config_history_config_id ON config_history(config_id);
CREATE INDEX IF NOT EXISTS idx_config_history_changed_at ON config_history(changed_at);
CREATE INDEX IF NOT EXISTS idx_point_tables_channel ON point_tables(channel_id, channel_name);
CREATE INDEX IF NOT EXISTS idx_point_tables_type ON point_tables(point_type);
CREATE INDEX IF NOT EXISTS idx_protocol_mappings_channel_point ON protocol_mappings(channel_id, point_id);
CREATE INDEX IF NOT EXISTS idx_protocol_mappings_protocol ON protocol_mappings(protocol);

-- 创建触发器以自动更新 updated_at 字段
CREATE TRIGGER IF NOT EXISTS update_configs_timestamp 
AFTER UPDATE ON configs
BEGIN
    UPDATE configs SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS update_point_tables_timestamp 
AFTER UPDATE ON point_tables
BEGIN
    UPDATE point_tables SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS update_protocol_mappings_timestamp 
AFTER UPDATE ON protocol_mappings
BEGIN
    UPDATE protocol_mappings SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- 创建视图以简化查询
-- 完整的点位信息视图（包含协议映射）
CREATE VIEW IF NOT EXISTS v_point_full AS
SELECT 
    pt.id,
    pt.channel_id,
    pt.channel_name,
    pt.point_id,
    pt.point_name,
    pt.point_type,
    pt.data_type,
    pt.unit,
    pt.scale,
    pt.offset,
    pt.min_value,
    pt.max_value,
    pt.description,
    pt.metadata,
    pm.protocol,
    pm.address,
    pm.params,
    pm.slave_id,
    pm.function_code,
    pm.register_address,
    pm.register_count,
    pm.byte_order
FROM point_tables pt
LEFT JOIN protocol_mappings pm ON pt.channel_id = pm.channel_id AND pt.point_id = pm.point_id
WHERE pt.is_active = 1 AND (pm.is_active = 1 OR pm.is_active IS NULL);

-- 配置变更统计视图
CREATE VIEW IF NOT EXISTS v_config_change_stats AS
SELECT 
    service,
    COUNT(*) as total_changes,
    COUNT(DISTINCT key) as unique_keys_changed,
    COUNT(DISTINCT changed_by) as unique_users,
    DATE(changed_at) as change_date
FROM config_history
GROUP BY service, DATE(changed_at)
ORDER BY change_date DESC;