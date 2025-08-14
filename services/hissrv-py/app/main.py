"""
Historical Data Service - Main Application
Collects data from Redis and stores in InfluxDB
"""

import logging
import sys
from datetime import datetime
import redis.asyncio as redis
from influxdb_client import InfluxDBClient, Point
from influxdb_client.client.write_api import SYNCHRONOUS
from apscheduler.schedulers.asyncio import AsyncIOScheduler
from fastapi import FastAPI
from contextlib import asynccontextmanager
import os

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(sys.stdout)],
)

logger = logging.getLogger(__name__)


class Config:
    """Simple configuration class"""

    def __init__(self):
        self.service_name = os.getenv("SERVICE_NAME", "hissrv")
        self.service_port = int(os.getenv("SERVICE_PORT", "6004"))
        self.redis_url = os.getenv("REDIS_URL", "redis://localhost:6379")
        self.influxdb_url = os.getenv("INFLUXDB_URL", "http://localhost:8086")
        self.influxdb_token = os.getenv("INFLUXDB_TOKEN", "")
        self.influxdb_org = os.getenv("INFLUXDB_ORG", "voltage")
        self.influxdb_bucket = os.getenv("INFLUXDB_BUCKET", "voltage")
        self.collection_interval = int(os.getenv("COLLECTION_INTERVAL", "60"))
        self.batch_size = int(os.getenv("BATCH_SIZE", "1000"))
        self.data_patterns = ["comsrv:*:T", "modsrv:realtime:*"]


class HistoricalDataService:
    """Historical data collection service"""

    def __init__(self, config: Config):
        self.config = config
        self.redis_client = None
        self.influx_client = None
        self.write_api = None
        self.scheduler = AsyncIOScheduler()
        self.stats = {
            "collections": 0,
            "points_written": 0,
            "last_collection": None,
            "errors": 0,
        }

    async def connect(self):
        """Connect to Redis and InfluxDB"""
        # Connect to Redis
        try:
            self.redis_client = redis.from_url(
                self.config.redis_url, decode_responses=True
            )
            await self.redis_client.ping()
            logger.info(f"Connected to Redis at {self.config.redis_url}")
        except Exception as e:
            logger.error(f"Failed to connect to Redis: {e}")
            raise

        # Connect to InfluxDB (if configured)
        if self.config.influxdb_token:
            try:
                self.influx_client = InfluxDBClient(
                    url=self.config.influxdb_url,
                    token=self.config.influxdb_token,
                    org=self.config.influxdb_org,
                )
                self.write_api = self.influx_client.write_api(write_options=SYNCHRONOUS)
                logger.info(f"Connected to InfluxDB at {self.config.influxdb_url}")
            except Exception as e:
                logger.warning(f"InfluxDB connection failed: {e}")
                logger.info("Running without InfluxDB storage")

    async def collect_data(self):
        """Collect data from Redis and store in InfluxDB"""
        try:
            logger.info("Starting data collection")
            points_collected = 0

            for pattern in self.config.data_patterns:
                # Scan for keys
                keys = []
                async for key in self.redis_client.scan_iter(pattern):
                    keys.append(key)

                logger.debug(f"Found {len(keys)} keys for pattern {pattern}")

                # Process each key
                for key in keys[: self.config.batch_size]:
                    try:
                        # Get data type (hash or string)
                        key_type = await self.redis_client.type(key)

                        if key_type == "hash":
                            data = await self.redis_client.hgetall(key)
                            if data:
                                # Parse key to get measurement info
                                parts = key.split(":")
                                measurement = parts[0] if parts else "unknown"

                                # Create InfluxDB point
                                point = Point(measurement)

                                # Add tags from key
                                if len(parts) > 1:
                                    point.tag("channel_id", parts[1])
                                if len(parts) > 2:
                                    point.tag("data_type", parts[2])

                                # Add fields
                                for field_key, value in data.items():
                                    if not field_key.startswith("_"):
                                        try:
                                            # Try to convert to float
                                            point.field(field_key, float(value))
                                        except (ValueError, TypeError):
                                            point.field(field_key, str(value))

                                # Write to InfluxDB if configured
                                if self.write_api:
                                    self.write_api.write(
                                        bucket=self.config.influxdb_bucket, record=point
                                    )

                                points_collected += 1

                    except Exception as e:
                        logger.error(f"Error processing key {key}: {e}")
                        self.stats["errors"] += 1

            # Update stats
            self.stats["collections"] += 1
            self.stats["points_written"] += points_collected
            self.stats["last_collection"] = datetime.utcnow().isoformat()

            logger.info(f"Collection complete: {points_collected} points written")

        except Exception as e:
            logger.error(f"Collection error: {e}")
            self.stats["errors"] += 1

    async def start(self):
        """Start the historical data service"""
        await self.connect()

        # Schedule data collection
        self.scheduler.add_job(
            self.collect_data,
            "interval",
            seconds=self.config.collection_interval,
            id="data_collection",
            replace_existing=True,
        )

        self.scheduler.start()
        logger.info(
            f"Scheduled data collection every {self.config.collection_interval} seconds"
        )

        # Run initial collection
        await self.collect_data()

    async def stop(self):
        """Stop the service"""
        self.scheduler.shutdown()
        if self.redis_client:
            await self.redis_client.close()
        if self.influx_client:
            self.influx_client.close()


# Global service instance
service = None


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application lifespan manager"""
    global service

    # Startup
    config = Config()
    service = HistoricalDataService(config)
    await service.start()

    yield

    # Shutdown
    await service.stop()


# Create FastAPI app
app = FastAPI(
    title="Historical Data Service",
    description="Collects and stores time-series data",
    version="0.0.1",
    lifespan=lifespan,
)


@app.get("/health")
async def health_check():
    """Health check endpoint"""
    return {"status": "healthy", "service": "hissrv-py"}


@app.get("/stats")
async def get_stats():
    """Get service statistics"""
    if service:
        return service.stats
    return {"error": "Service not initialized"}


if __name__ == "__main__":
    import uvicorn

    config = Config()
    uvicorn.run(
        "app.main:app", host="0.0.0.0", port=config.service_port, log_level="info"
    )
