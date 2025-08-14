"""
Network Service - Main Application
Forwards data from Redis to external networks
"""

import asyncio
import logging
import signal
import sys
import json
from typing import Dict, Any, List
import redis.asyncio as redis
import aiohttp
from app.config import Config

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(sys.stdout)],
)

logger = logging.getLogger(__name__)


class NetworkService:
    """Main network service class"""

    def __init__(self, config: Config):
        self.config = config
        self.redis_client = None
        self.running = False
        self.tasks = []

    async def connect_redis(self):
        """Connect to Redis"""
        try:
            self.redis_client = redis.from_url(
                self.config.redis.url, decode_responses=True
            )
            await self.redis_client.ping()
            logger.info(f"Connected to Redis at {self.config.redis.url}")
        except Exception as e:
            logger.error(f"Failed to connect to Redis: {e}")
            raise

    async def scan_keys(self, pattern: str) -> List[str]:
        """Scan Redis keys matching pattern"""
        keys = []
        async for key in self.redis_client.scan_iter(pattern):
            keys.append(key)
        return keys

    async def get_data(self, key: str) -> Dict[str, Any]:
        """Get data from Redis hash"""
        try:
            data = await self.redis_client.hgetall(key)
            # Try to parse JSON values
            result = {}
            for k, v in data.items():
                try:
                    result[k] = json.loads(v) if isinstance(v, str) else v
                except (json.JSONDecodeError, TypeError):
                    result[k] = v
            return result
        except Exception as e:
            logger.error(f"Error getting data for key {key}: {e}")
            return {}

    async def forward_to_http(self, config: Dict[str, Any], data: Dict[str, Any]):
        """Forward data via HTTP"""
        http_config = config["http"]

        try:
            async with aiohttp.ClientSession() as session:
                # Prepare request
                headers = http_config.headers or {}
                headers["Content-Type"] = "application/json"

                # Send request
                async with session.request(
                    method=http_config.method,
                    url=http_config.url,
                    json=data,
                    headers=headers,
                    timeout=aiohttp.ClientTimeout(total=http_config.timeout_secs),
                ) as response:
                    if response.status >= 200 and response.status < 300:
                        logger.debug(f"HTTP forward successful to {http_config.name}")
                    else:
                        logger.warning(
                            f"HTTP forward failed with status {response.status}"
                        )
        except Exception as e:
            logger.error(f"HTTP forward error to {http_config.name}: {e}")

    async def forward_to_mqtt(self, config: Dict[str, Any], data: Dict[str, Any]):
        """Forward data via MQTT (simplified version)"""
        mqtt_config = config["mqtt"]
        logger.info(
            f"MQTT forward to {mqtt_config.name} (topic: {mqtt_config.topic_prefix})"
        )
        # TODO: Implement actual MQTT publishing
        # For now, just log the action

    async def data_collection_loop(self):
        """Main data collection and forwarding loop"""
        logger.info("Starting data collection loop")

        while self.running:
            try:
                # Collect data from Redis
                all_data = {}

                for pattern in self.config.redis.data_keys:
                    keys = await self.scan_keys(pattern)
                    logger.debug(f"Found {len(keys)} keys for pattern {pattern}")

                    for key in keys:
                        data = await self.get_data(key)
                        if data:
                            all_data[key] = data

                if all_data:
                    logger.info(f"Collected data from {len(all_data)} keys")

                    # Forward to configured networks
                    for network_config in self.config.networks:
                        if "http" in network_config:
                            await self.forward_to_http(network_config, all_data)
                        elif "mqtt" in network_config:
                            await self.forward_to_mqtt(network_config, all_data)

                # Wait for next poll interval
                await asyncio.sleep(self.config.redis.poll_interval_secs)

            except Exception as e:
                logger.error(f"Error in data collection loop: {e}")
                await asyncio.sleep(5)  # Wait before retry

    async def start(self):
        """Start the network service"""
        logger.info("Starting Network Service")

        # Connect to Redis
        await self.connect_redis()

        # Start running
        self.running = True

        # Start data collection task
        collection_task = asyncio.create_task(self.data_collection_loop())
        self.tasks.append(collection_task)

        logger.info("Network Service started successfully")

        # Wait for tasks
        try:
            await asyncio.gather(*self.tasks)
        except asyncio.CancelledError:
            logger.info("Tasks cancelled")

    async def stop(self):
        """Stop the network service"""
        logger.info("Stopping Network Service")
        self.running = False

        # Cancel all tasks
        for task in self.tasks:
            task.cancel()

        # Wait for tasks to complete
        await asyncio.gather(*self.tasks, return_exceptions=True)

        # Disconnect from Redis
        if self.redis_client:
            await self.redis_client.close()

        logger.info("Network Service stopped")


async def main():
    """Main entry point"""
    # Load configuration
    config = Config.load()

    logger.info(f"Starting {config.service.name} service")
    logger.info(f"Redis URL: {config.redis.url}")
    logger.info(f"Configured networks: {len(config.networks)}")

    # Create service
    service = NetworkService(config)

    # Handle shutdown signals
    loop = asyncio.get_running_loop()

    def signal_handler():
        logger.info("Received shutdown signal")
        asyncio.create_task(service.stop())

    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, signal_handler)

    # Start service
    try:
        await service.start()
    except KeyboardInterrupt:
        logger.info("Interrupted by user")
    except Exception as e:
        logger.error(f"Service error: {e}")
    finally:
        await service.stop()


if __name__ == "__main__":
    asyncio.run(main())
