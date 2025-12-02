"""
Redis数据库连接模块
"""

import redis
from typing import Optional
from loguru import logger
from .config import settings

class RedisManager:
    """Redis连接管理器"""
    
    def __init__(self):
        self.redis_client: Optional[redis.Redis] = None
        self._connect()
    
    def _connect(self):
        """建立Redis连接"""
        try:
            self.redis_client = redis.Redis(
                host=settings.REDIS_HOST,
                port=settings.REDIS_PORT,
                db=settings.REDIS_DB,
                password=settings.REDIS_PASSWORD,
                decode_responses=True,
                socket_connect_timeout=5,
                socket_timeout=5,
                retry_on_timeout=True
            )
            # 测试连接
            self.redis_client.ping()
            logger.info(f"Redis连接成功: {settings.REDIS_HOST}:{settings.REDIS_PORT}")
        except Exception as e:
            logger.error(f"Redis连接失败: {e}")
            self.redis_client = None
    
    def get_client(self) -> Optional[redis.Redis]:
        """获取Redis客户端"""
        if self.redis_client is None:
            self._connect()
        return self.redis_client
    
    def is_connected(self) -> bool:
        """检查连接状态"""
        try:
            if self.redis_client:
                self.redis_client.ping()
                return True
        except:
            pass
        return False
    
    def reconnect(self):
        """重新连接"""
        logger.info("尝试重新连接Redis...")
        self._connect()
    
    def close(self):
        """关闭连接"""
        if self.redis_client:
            self.redis_client.close()
            logger.info("Redis连接已关闭")

# 全局Redis管理器实例
redis_manager = RedisManager()
