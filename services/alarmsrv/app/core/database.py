"""
数据库连接和管理模块
处理SQLite数据库的创建、连接和初始化
"""

import sqlite3
import os
import logging
from pathlib import Path
from typing import Optional
from contextlib import contextmanager

from app.core.config import settings

logger = logging.getLogger(__name__)


class DatabaseManager:
    """数据库管理器"""
    
    def __init__(self):
        self.db_path = settings.DATABASE_PATH
        self.timeout = settings.DATABASE_TIMEOUT
        self._connection: Optional[sqlite3.Connection] = None
    
    def ensure_database_exists(self) -> bool:
        """确保数据库文件存在，如果不存在则创建"""
        try:
            # 确保目录存在
            db_dir = Path(self.db_path).parent
            if not db_dir.exists():
                db_dir.mkdir(parents=True, exist_ok=True)
                logger.info(f"创建数据库目录: {db_dir}")
            
            # 检查数据库文件是否存在
            db_exists = Path(self.db_path).exists()
            
            if not db_exists:
                # 创建数据库文件
                with sqlite3.connect(self.db_path, timeout=self.timeout) as conn:
                    logger.info(f"创建新的数据库文件: {self.db_path}")
                    
                    # 启用WAL模式
                    self.enable_wal_mode(conn)
                    
                    # 创建表
                    self.create_tables(conn)
                    
                    logger.info("数据库初始化完成")
            else:
                # 数据库已存在，确保WAL模式开启和表结构正确
                with sqlite3.connect(self.db_path, timeout=self.timeout) as conn:
                    self.enable_wal_mode(conn)
                    self.create_tables(conn)
                    logger.info(f"使用现有数据库: {self.db_path}")
            
            return True
            
        except Exception as e:
            logger.error(f"数据库初始化失败: {e}")
            return False
    
    def enable_wal_mode(self, conn: sqlite3.Connection):
        """启用WAL (Write-Ahead Logging) 模式"""
        try:
            # 检查当前模式
            cursor = conn.execute("PRAGMA journal_mode;")
            current_mode = cursor.fetchone()[0]
            
            if current_mode.lower() != 'wal':
                # 启用WAL模式
                conn.execute("PRAGMA journal_mode=WAL;")
                logger.info("WAL模式已启用")
            else:
                logger.info("WAL模式已经启用")
                
            # 设置其他WAL相关参数
            conn.execute("PRAGMA synchronous=NORMAL;")  # 更好的性能
            conn.execute("PRAGMA cache_size=10000;")    # 增大缓存
            conn.execute("PRAGMA temp_store=memory;")   # 临时表存储在内存中
            conn.execute("PRAGMA foreign_keys=ON;")     # 启用外键约束
            
        except Exception as e:
            logger.error(f"启用WAL模式失败: {e}")
            raise
    
    def create_tables(self, conn: sqlite3.Connection):
        """创建表结构"""
        try:
            # 创建alert表（当前告警）
            create_alert_sql = """
            CREATE TABLE IF NOT EXISTS alert (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                rule_id INTEGER NOT NULL,
                rule_snapshot TEXT NOT NULL DEFAULT '{}',
                service_type TEXT NOT NULL,
                channel_id INTEGER NOT NULL,
                data_type TEXT NOT NULL,
                point_id INTEGER NOT NULL,
                rule_name TEXT NOT NULL,
                warning_level INTEGER NOT NULL,
                operator TEXT NOT NULL,
                threshold_value REAL NOT NULL,
                current_value REAL NOT NULL,
                status TEXT NOT NULL DEFAULT 'active',
                triggered_at INTEGER NOT NULL,
                UNIQUE(rule_id),
                FOREIGN KEY (rule_id) REFERENCES alert_rule (id) ON DELETE CASCADE
            );
            """
            
            conn.execute(create_alert_sql)
            
            # 创建alert_event表（告警历史）
            create_alert_event_sql = """
            CREATE TABLE IF NOT EXISTS alert_event (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                rule_id INTEGER NOT NULL,
                rule_snapshot TEXT NOT NULL DEFAULT '{}',
                service_type TEXT NOT NULL,
                channel_id INTEGER NOT NULL,
                data_type TEXT NOT NULL,
                point_id INTEGER NOT NULL,
                rule_name TEXT NOT NULL,
                warning_level INTEGER NOT NULL,
                operator TEXT NOT NULL,
                threshold_value REAL NOT NULL,
                trigger_value REAL NOT NULL,
                recovery_value REAL,
                event_type TEXT NOT NULL CHECK(event_type IN ('trigger', 'recovery')),
                triggered_at INTEGER NOT NULL,
                recovered_at INTEGER,
                duration INTEGER,
                FOREIGN KEY (rule_id) REFERENCES alert_rule (id) ON DELETE CASCADE
            );
            """
            
            conn.execute(create_alert_event_sql)
            
            # 创建alert_rule表
            create_alert_rule_sql = """
            CREATE TABLE IF NOT EXISTS alert_rule (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_type TEXT NOT NULL DEFAULT 'comsrv' CHECK(service_type IN ('comsrv', 'rulesrv', 'modsrv', 'alarmsrv', 'hissrv', 'netsrv')),
                channel_id INTEGER NOT NULL,
                data_type TEXT NOT NULL,
                point_id INTEGER NOT NULL,
                rule_name TEXT NOT NULL,
                warning_level INTEGER NOT NULL CHECK(warning_level IN (1, 2, 3)),
                operator TEXT NOT NULL CHECK(operator IN ('>', '<', '>=', '<=', '==', '!=')),
                value REAL NOT NULL,
                enabled BOOLEAN NOT NULL DEFAULT 1,
                description TEXT DEFAULT '',
                created_at INTEGER DEFAULT (strftime('%s', 'now')),
                updated_at INTEGER DEFAULT (strftime('%s', 'now')),
                UNIQUE(service_type, channel_id, data_type, point_id, rule_name)
            );
            """
            
            conn.execute(create_alert_rule_sql)
            
            # 创建索引以提高查询性能
            indexes = [
                # alert_rule表索引
                "CREATE INDEX IF NOT EXISTS idx_alert_rule_service_channel_type_point ON alert_rule(service_type, channel_id, data_type, point_id);",
                "CREATE INDEX IF NOT EXISTS idx_alert_rule_service_type ON alert_rule(service_type);",
                "CREATE INDEX IF NOT EXISTS idx_alert_rule_enabled ON alert_rule(enabled);",
                "CREATE INDEX IF NOT EXISTS idx_alert_rule_warning_level ON alert_rule(warning_level);",
                "CREATE INDEX IF NOT EXISTS idx_alert_rule_created_at ON alert_rule(created_at);",
                "CREATE INDEX IF NOT EXISTS idx_alert_rule_rule_name ON alert_rule(rule_name);",
                "CREATE INDEX IF NOT EXISTS idx_alert_rule_description ON alert_rule(description);",
                
                # alert表索引
                "CREATE INDEX IF NOT EXISTS idx_alert_rule_id ON alert(rule_id);",
                "CREATE INDEX IF NOT EXISTS idx_alert_service_channel ON alert(service_type, channel_id);",
                "CREATE INDEX IF NOT EXISTS idx_alert_warning_level ON alert(warning_level);",
                "CREATE INDEX IF NOT EXISTS idx_alert_triggered_at ON alert(triggered_at);",
                "CREATE INDEX IF NOT EXISTS idx_alert_status ON alert(status);",
                
                # alert_event表索引
                "CREATE INDEX IF NOT EXISTS idx_alert_event_rule_id ON alert_event(rule_id);",
                "CREATE INDEX IF NOT EXISTS idx_alert_event_service_channel ON alert_event(service_type, channel_id);",
                "CREATE INDEX IF NOT EXISTS idx_alert_event_event_type ON alert_event(event_type);",
                "CREATE INDEX IF NOT EXISTS idx_alert_event_warning_level ON alert_event(warning_level);",
                "CREATE INDEX IF NOT EXISTS idx_alert_event_triggered_at ON alert_event(triggered_at);",
                "CREATE INDEX IF NOT EXISTS idx_alert_event_recovered_at ON alert_event(recovered_at);",
                "CREATE INDEX IF NOT EXISTS idx_alert_event_rule_name ON alert_event(rule_name);"
            ]
            
            for index_sql in indexes:
                conn.execute(index_sql)
            
            # 创建触发器以自动更新updated_at字段
            trigger_sql = """
            CREATE TRIGGER IF NOT EXISTS update_alert_rule_timestamp 
            AFTER UPDATE ON alert_rule
            BEGIN
                UPDATE alert_rule SET updated_at = strftime('%s', 'now') WHERE id = NEW.id;
            END;
            """
            conn.execute(trigger_sql)
            
            logger.info("数据库表结构创建完成")
            
        except Exception as e:
            logger.error(f"创建表结构失败: {e}")
            raise
    
    @contextmanager
    def get_connection(self):
        """获取数据库连接的上下文管理器"""
        conn = None
        try:
            conn = sqlite3.connect(self.db_path, timeout=self.timeout)
            conn.row_factory = sqlite3.Row  # 使结果可以按列名访问
            conn.execute("PRAGMA foreign_keys=ON;")  # 确保每个连接都启用外键约束
            yield conn
        except Exception as e:
            if conn:
                conn.rollback()
            logger.error(f"数据库操作失败: {e}")
            raise
        finally:
            if conn:
                conn.close()
    
    def execute_query(self, sql: str, params: tuple = ()) -> list:
        """执行查询并返回结果"""
        with self.get_connection() as conn:
            cursor = conn.execute(sql, params)
            return cursor.fetchall()
    
    def execute_insert(self, sql: str, params: tuple = ()) -> int:
        """执行插入操作并返回插入的行ID"""
        with self.get_connection() as conn:
            cursor = conn.execute(sql, params)
            conn.commit()
            return cursor.lastrowid
    
    def execute_update(self, sql: str, params: tuple = ()) -> int:
        """执行更新操作并返回受影响的行数"""
        with self.get_connection() as conn:
            cursor = conn.execute(sql, params)
            conn.commit()
            return cursor.rowcount
    
    def execute_delete(self, sql: str, params: tuple = ()) -> int:
        """执行删除操作并返回受影响的行数"""
        with self.get_connection() as conn:
            cursor = conn.execute(sql, params)
            conn.commit()
            return cursor.rowcount


# 创建全局数据库管理器实例
db_manager = DatabaseManager()


def init_database() -> bool:
    """初始化数据库"""
    logger.info("开始初始化数据库...")
    return db_manager.ensure_database_exists()


def get_db_manager() -> DatabaseManager:
    """获取数据库管理器实例"""
    return db_manager
