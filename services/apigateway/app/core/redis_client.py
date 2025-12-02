"""
Redis客户端模块
提供Redis连接和数据操作功能
"""

import asyncio
import json
import logging
from typing import Any, Optional, Dict, List
import redis.asyncio as redis
from app.core.config import settings

logger = logging.getLogger(__name__)

class RedisClient:
    """Redis客户端类"""
    
    def __init__(self):
        self.redis_client: Optional[redis.Redis] = None
        self.connection_pool: Optional[redis.ConnectionPool] = None
        
    async def connect(self):
        """连接到Redis"""
        try:
            self.connection_pool = redis.ConnectionPool(
                host=settings.REDIS_HOST,
                port=settings.REDIS_PORT,
                db=settings.REDIS_DB,
                password=settings.REDIS_PASSWORD,
                decode_responses=True,
                max_connections=20
            )
            
            self.redis_client = redis.Redis(connection_pool=self.connection_pool)
            
            # 测试连接
            await self.redis_client.ping()
            logger.info(f"Redis连接成功: {settings.REDIS_HOST}:{settings.REDIS_PORT}")
            
        except Exception as e:
            logger.error(f"Redis连接失败: {e}")
            raise
    
    async def close(self):
        """关闭Redis连接"""
        if self.redis_client:
            await self.redis_client.close()
        if self.connection_pool:
            await self.connection_pool.disconnect()
        logger.info("Redis连接已关闭")
    
    async def get(self, key: str) -> Optional[str]:
        """获取值"""
        try:
            return await self.redis_client.get(key)
        except Exception as e:
            logger.error(f"Redis GET操作失败: {e}")
            return None
    
    async def set(self, key: str, value: str, expire: Optional[int] = None) -> bool:
        """设置值"""
        try:
            if expire:
                return await self.redis_client.setex(key, expire, value)
            else:
                return await self.redis_client.set(key, value)
        except Exception as e:
            logger.error(f"Redis SET操作失败: {e}")
            return False
    
    async def delete(self, key: str) -> bool:
        """删除键"""
        try:
            result = await self.redis_client.delete(key)
            return result > 0
        except Exception as e:
            logger.error(f"Redis DELETE操作失败: {e}")
            return False
    
    async def exists(self, key: str) -> bool:
        """检查键是否存在"""
        try:
            return await self.redis_client.exists(key)
        except Exception as e:
            logger.error(f"Redis EXISTS操作失败: {e}")
            return False
    
    async def expire(self, key: str, seconds: int) -> bool:
        """设置过期时间"""
        try:
            return await self.redis_client.expire(key, seconds)
        except Exception as e:
            logger.error(f"Redis EXPIRE操作失败: {e}")
            return False
    
    async def lpush(self, key: str, value: str) -> bool:
        """左推入列表"""
        try:
            await self.redis_client.lpush(key, value)
            return True
        except Exception as e:
            logger.error(f"Redis LPUSH操作失败: {e}")
            return False
    
    async def rpop(self, key: str) -> Optional[str]:
        """右弹出列表"""
        try:
            return await self.redis_client.rpop(key)
        except Exception as e:
            logger.error(f"Redis RPOP操作失败: {e}")
            return None
    
    async def llen(self, key: str) -> int:
        """获取列表长度"""
        try:
            return await self.redis_client.llen(key)
        except Exception as e:
            logger.error(f"Redis LLEN操作失败: {e}")
            return 0
    
    async def publish(self, channel: str, message: str) -> int:
        """发布消息到频道"""
        try:
            return await self.redis_client.publish(channel, message)
        except Exception as e:
            logger.error(f"Redis PUBLISH操作失败: {e}")
            return 0
    
    async def subscribe(self, channel: str):
        """订阅频道"""
        try:
            pubsub = self.redis_client.pubsub()
            await pubsub.subscribe(channel)
            return pubsub
        except Exception as e:
            logger.error(f"Redis SUBSCRIBE操作失败: {e}")
            return None
    
    async def get_all_keys(self, pattern: str = "*") -> List[str]:
        """获取所有匹配的键"""
        try:
            keys = []
            async for key in self.redis_client.scan_iter(match=pattern):
                keys.append(key)
            return keys
        except Exception as e:
            logger.error(f"Redis SCAN操作失败: {e}")
            return []
    
    async def get_data_for_websocket(self, data_type: str) -> List[Dict[str, Any]]:
        """获取WebSocket需要的数据"""
        try:
            # 根据数据类型从Redis获取数据
            key = f"{settings.REDIS_PREFIX}{data_type}"
            
            if await self.exists(key):
                data = await self.get(key)
                if data:
                    return json.loads(data)
            
            return []
            
        except Exception as e:
            logger.error(f"获取WebSocket数据失败: {e}")
            return []
