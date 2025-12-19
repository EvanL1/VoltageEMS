"""
数据库初始化和管理服务
"""

import os
import sqlite3
import logging
from datetime import datetime
from typing import Optional, List, Dict, Any
import asyncio
from contextlib import asynccontextmanager

logger = logging.getLogger(__name__)


class DatabaseManager:
    """数据库管理器"""
    
    def __init__(self, db_path: str = None):
        if db_path is None:
            # 优先使用环境变量（与 Rust 服务一致）
            env_path = os.environ.get("VOLTAGE_DB_PATH")
            if env_path:
                self.db_path = env_path
            elif os.path.exists("/app/data"):
                self.db_path = "/app/data/voltage.db"
            else:
                # 本机开发使用临时目录
                import tempfile
                temp_dir = os.path.join(tempfile.gettempdir(), "voltageems")
                os.makedirs(temp_dir, exist_ok=True)
                self.db_path = os.path.join(temp_dir, "voltageems.db")
        else:
            self.db_path = db_path
        self.connection: Optional[sqlite3.Connection] = None
        
    async def initialize(self):
        """初始化数据库"""
        try:
            # 确保目录存在
            os.makedirs(os.path.dirname(self.db_path), exist_ok=True)
            
            # 检查数据库是否存在
            db_exists = os.path.exists(self.db_path)
            
            # 连接数据库
            self.connection = sqlite3.connect(self.db_path, check_same_thread=False)
            self.connection.row_factory = sqlite3.Row  # 使结果可以按列名访问
            
            # 尝试启用WAL模式（可选）
            try:
                await self._enable_wal_mode()
            except Exception as e:
                logger.warning(f"WAL模式初始化失败，继续使用默认模式: {e}")
            
            if not db_exists:
                logger.info("数据库不存在，正在创建新数据库...")
                
            # 创建表
            await self._create_tables()
            
            # 初始化角色数据
            await self._initialize_roles()
            
            logger.info(f"数据库初始化完成: {self.db_path}")
            
        except Exception as e:
            logger.error(f"数据库初始化失败: {e}")
            raise
    
    async def _enable_wal_mode(self):
        """启用WAL模式"""
        try:
            cursor = self.connection.cursor()
            cursor.execute("PRAGMA journal_mode=WAL")
            result = cursor.fetchone()
            logger.info(f"WAL模式启用状态: {result[0]}")
            cursor.close()
        except Exception as e:
            logger.warning(f"启用WAL模式失败，使用默认模式: {e}")
            # 在WSL环境下WAL模式可能不支持，继续使用默认模式
            # 重新连接数据库，使用默认模式
            try:
                self.connection.close()
                self.connection = sqlite3.connect(self.db_path, check_same_thread=False)
                self.connection.row_factory = sqlite3.Row
                logger.info("已切换到默认journal模式")
            except Exception as e2:
                logger.error(f"重新连接数据库失败: {e2}")
                raise
    
    async def _create_tables(self):
        """创建数据库表"""
        try:
            cursor = self.connection.cursor()
            
            # 创建角色表
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS roles (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name_en VARCHAR(50) NOT NULL UNIQUE,
                    name_zh VARCHAR(50) NOT NULL UNIQUE,
                    description TEXT,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
            """)
            
            # 创建用户表
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS users (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    username VARCHAR(50) NOT NULL UNIQUE,
                    password_hash VARCHAR(255) NOT NULL,
                    role_id INTEGER NOT NULL,
                    is_active BOOLEAN DEFAULT 1,
                    last_login TIMESTAMP NULL,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (role_id) REFERENCES roles (id)
                )
            """)
            
            # 创建索引
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_users_username ON users(username)")
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_users_role_id ON users(role_id)")
            
            self.connection.commit()
            cursor.close()
            logger.info("数据库表创建完成")
            
        except Exception as e:
            logger.error(f"创建数据库表失败: {e}")
            raise
    
    async def _initialize_roles(self):
        """初始化角色数据"""
        try:
            cursor = self.connection.cursor()
            
            # 检查是否已有角色数据
            cursor.execute("SELECT COUNT(*) FROM roles")
            count = cursor.fetchone()[0]
            
            if count == 0:
                # 插入默认角色
                roles = [
                    (1, "Admin", "管理员", "系统管理员，拥有所有权限"),
                    (2, "Engineer", "工程师", "工程师，可以进行设备操作和配置"),
                    (3, "Viewer", "查看者", "只读用户，只能查看数据")
                ]
                
                cursor.executemany("""
                    INSERT INTO roles (id, name_en, name_zh, description)
                    VALUES (?, ?, ?, ?)
                """, roles)
                
                self.connection.commit()
                logger.info("默认角色数据初始化完成")
            else:
                logger.info(f"角色表已存在 {count} 条记录，跳过初始化")
            
            cursor.close()
            
        except Exception as e:
            logger.error(f"初始化角色数据失败: {e}")
            raise
    
    async def execute_query(self, query: str, params: tuple = ()) -> List[sqlite3.Row]:
        """执行查询语句"""
        try:
            cursor = self.connection.cursor()
            cursor.execute(query, params)
            result = cursor.fetchall()
            cursor.close()
            return result
        except Exception as e:
            logger.error(f"执行查询失败: {query}, 参数: {params}, 错误: {e}")
            raise
    
    async def execute_update(self, query: str, params: tuple = ()) -> int:
        """执行更新语句"""
        try:
            cursor = self.connection.cursor()
            cursor.execute(query, params)
            self.connection.commit()
            affected_rows = cursor.rowcount
            cursor.close()
            return affected_rows
        except Exception as e:
            logger.error(f"执行更新失败: {query}, 参数: {params}, 错误: {e}")
            raise
    
    async def execute_insert(self, query: str, params: tuple = ()) -> int:
        """执行插入语句，返回新记录ID"""
        try:
            cursor = self.connection.cursor()
            cursor.execute(query, params)
            self.connection.commit()
            last_row_id = cursor.lastrowid
            cursor.close()
            return last_row_id
        except Exception as e:
            logger.error(f"执行插入失败: {query}, 参数: {params}, 错误: {e}")
            raise
    
    async def get_user_by_username(self, username: str) -> Optional[sqlite3.Row]:
        """根据用户名获取用户"""
        result = await self.execute_query(
            "SELECT * FROM users WHERE username = ?", 
            (username,)
        )
        return result[0] if result else None
    
    
    async def get_user_by_id(self, user_id: int) -> Optional[sqlite3.Row]:
        """根据ID获取用户"""
        result = await self.execute_query(
            "SELECT * FROM users WHERE id = ?", 
            (user_id,)
        )
        return result[0] if result else None
    
    async def get_user_with_role(self, user_id: int) -> Optional[Dict[str, Any]]:
        """获取包含角色信息的用户数据"""
        result = await self.execute_query("""
            SELECT u.*, r.name_en as role_name_en, r.name_zh as role_name_zh,
                   r.description as role_description
            FROM users u
            JOIN roles r ON u.role_id = r.id
            WHERE u.id = ?
        """, (user_id,))
        
        if not result:
            return None
            
        row = result[0]
        return {
            "id": row["id"],
            "username": row["username"],
            "is_active": bool(row["is_active"]),
            "last_login": row["last_login"],
            "created_at": row["created_at"],
            "updated_at": row["updated_at"],
            "role": {
                "id": row["role_id"],
                "name_en": row["role_name_en"],
                "name_zh": row["role_name_zh"],
                "description": row["role_description"]
            }
        }
    
    async def create_user(self, username: str, password_hash: str, role_id: int = 3) -> int:
        """创建用户"""
        return await self.execute_insert("""
            INSERT INTO users (username, password_hash, role_id)
            VALUES (?, ?, ?)
        """, (username, password_hash, role_id))
    
    async def update_user_login_time(self, user_id: int):
        """更新用户最后登录时间"""
        await self.execute_update("""
            UPDATE users 
            SET last_login = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?
        """, (user_id,))
    
    async def delete_user(self, user_id: int) -> bool:
        """删除用户"""
        try:
            cursor = self.connection.cursor()
            cursor.execute("DELETE FROM users WHERE id = ?", (user_id,))
            affected_rows = cursor.rowcount
            self.connection.commit()
            cursor.close()
            return affected_rows > 0
        except Exception as e:
            logger.error(f"删除用户失败: {e}")
            raise
    
    async def get_all_roles(self) -> List[sqlite3.Row]:
        """获取所有角色"""
        return await self.execute_query("SELECT * FROM roles ORDER BY id")
    
    async def get_all_users_with_roles(self) -> List[sqlite3.Row]:
        """获取所有用户及其角色信息"""
        return await self.execute_query("""
            SELECT u.*, r.name_en as role_name_en, r.name_zh as role_name_zh,
                   r.description as role_description
            FROM users u
            JOIN roles r ON u.role_id = r.id
            ORDER BY u.id
        """)
    
    def close(self):
        """关闭数据库连接"""
        if self.connection:
            self.connection.close()
            self.connection = None
            logger.info("数据库连接已关闭")


# 全局数据库管理器实例
db_manager: Optional[DatabaseManager] = None


def get_database() -> DatabaseManager:
    """获取数据库管理器实例"""
    global db_manager
    if db_manager is None:
        raise RuntimeError("数据库管理器未初始化")
    return db_manager


async def initialize_database():
    """初始化数据库"""
    global db_manager
    db_manager = DatabaseManager()
    await db_manager.initialize()


async def close_database():
    """关闭数据库"""
    global db_manager
    if db_manager:
        db_manager.close()
        db_manager = None
