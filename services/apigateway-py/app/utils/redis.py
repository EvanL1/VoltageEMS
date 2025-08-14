"""Redis client utilities"""

import json
import logging
from typing import Any, Dict, List, Optional
import redis.asyncio as redis
from redis.exceptions import RedisError

logger = logging.getLogger(__name__)


class RedisClient:
    """Async Redis client wrapper"""

    def __init__(self, url: str, decode_responses: bool = True):
        self.url = url
        self.decode_responses = decode_responses
        self._client: Optional[redis.Redis] = None

    async def connect(self) -> None:
        """Connect to Redis"""
        try:
            self._client = redis.from_url(
                self.url,
                decode_responses=self.decode_responses,
                socket_connect_timeout=5,
            )
            # Test connection
            await self._client.ping()
            logger.info(f"Connected to Redis at {self.url}")
        except RedisError as e:
            logger.error(f"Failed to connect to Redis: {e}")
            raise

    async def disconnect(self) -> None:
        """Disconnect from Redis"""
        if self._client:
            await self._client.close()
            logger.info("Disconnected from Redis")

    async def ping(self) -> bool:
        """Check Redis connection"""
        try:
            if self._client:
                await self._client.ping()
                return True
        except RedisError:
            pass
        return False

    async def get(self, key: str) -> Optional[str]:
        """Get value by key"""
        try:
            return await self._client.get(key)
        except RedisError as e:
            logger.error(f"Redis GET error for key {key}: {e}")
            return None

    async def hgetall(self, key: str) -> Dict[str, Any]:
        """Get all fields from hash"""
        try:
            data = await self._client.hgetall(key)
            # Try to parse JSON values
            result = {}
            for k, v in data.items():
                try:
                    result[k] = json.loads(v) if isinstance(v, str) else v
                except (json.JSONDecodeError, TypeError):
                    result[k] = v
            return result
        except RedisError as e:
            logger.error(f"Redis HGETALL error for key {key}: {e}")
            return {}

    async def keys(self, pattern: str) -> List[str]:
        """Get keys matching pattern"""
        try:
            return await self._client.keys(pattern)
        except RedisError as e:
            logger.error(f"Redis KEYS error for pattern {pattern}: {e}")
            return []

    async def scan_iter(self, pattern: str) -> List[str]:
        """Scan keys matching pattern (better than KEYS for production)"""
        try:
            keys = []
            async for key in self._client.scan_iter(pattern):
                keys.append(key)
            return keys
        except RedisError as e:
            logger.error(f"Redis SCAN error for pattern {pattern}: {e}")
            return []

    async def get_channel_data(self, channel_id: int, data_type: str) -> Dict[str, Any]:
        """Get channel data from Redis"""
        key = f"comsrv:{channel_id}:{data_type}"
        return await self.hgetall(key)

    async def get_channel_ids(self) -> List[int]:
        """Get all available channel IDs"""
        try:
            # Scan for comsrv:*:T pattern to find channels
            keys = await self.scan_iter("comsrv:*:T")
            channel_ids = set()

            for key in keys:
                # Extract channel ID from key like "comsrv:1001:T"
                parts = key.split(":")
                if len(parts) == 3:
                    try:
                        channel_id = int(parts[1])
                        channel_ids.add(channel_id)
                    except ValueError:
                        continue

            return sorted(list(channel_ids))
        except Exception as e:
            logger.error(f"Error getting channel IDs: {e}")
            return []
