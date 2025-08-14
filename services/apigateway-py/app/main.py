"""
API Gateway Service - Main Application
"""

import logging
import sys
from contextlib import asynccontextmanager
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from app.config import Config
from app.utils.redis import RedisClient
from app.routers import health, channels, data

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(sys.stdout)],
)

logger = logging.getLogger(__name__)

# Global Redis client
redis_client: RedisClient = None


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application lifespan manager"""
    global redis_client

    # Startup
    logger.info("Starting API Gateway Service")

    # Load configuration
    config = Config.load()
    app.state.config = config

    # Initialize Redis client
    redis_client = RedisClient(config.redis.url, config.redis.decode_responses)
    await redis_client.connect()
    app.state.redis = redis_client

    logger.info(f"API Gateway started on {config.server.host}:{config.server.port}")

    yield

    # Shutdown
    logger.info("Shutting down API Gateway Service")
    if redis_client:
        await redis_client.disconnect()


# Create FastAPI app
app = FastAPI(
    title="VoltageEMS API Gateway",
    description="REST API Gateway for VoltageEMS System",
    version="0.0.1",
    lifespan=lifespan,
)

# Add CORS middleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


# Override the get_redis_client dependency
def get_redis_override():
    """Override function to provide Redis client"""
    return redis_client


# Include routers
app.include_router(health.router)
app.include_router(channels.router)
app.include_router(data.router)

# Override dependencies
health.get_redis_client = get_redis_override
channels.get_redis_client = get_redis_override
data.get_redis_client = get_redis_override


@app.get("/")
async def root():
    """Root endpoint"""
    return {
        "service": "apigateway-py",
        "version": "0.0.1",
        "description": "VoltageEMS API Gateway Service",
        "docs": "/docs",
        "health": "/health",
    }


if __name__ == "__main__":
    import uvicorn

    # Load configuration
    config = Config.load()

    # Run the application
    uvicorn.run(
        "app.main:app",
        host=config.server.host,
        port=config.server.port,
        reload=True,
        log_level="info",
    )
