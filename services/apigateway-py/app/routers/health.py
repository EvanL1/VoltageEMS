"""Health check endpoints"""

from datetime import datetime
from fastapi import APIRouter, Depends
from app.models import ApiResponse, HealthStatus
from app.utils.redis import RedisClient

router = APIRouter(tags=["health"])


async def get_redis_client():
    """Dependency to get Redis client (will be injected from main)"""
    # This will be overridden in main.py
    pass


@router.get("/health", response_model=ApiResponse)
async def health_check():
    """Basic health check endpoint"""
    return ApiResponse(
        success=True, data={"status": "healthy", "service": "apigateway-py"}
    )


@router.get("/health/detailed", response_model=ApiResponse)
async def detailed_health(redis: RedisClient = Depends(get_redis_client)):
    """Detailed health check with dependency status"""
    health = HealthStatus(
        status="healthy",
        service="apigateway-py",
        timestamp=datetime.utcnow(),
        dependencies={},
    )

    # Check Redis connection
    redis_healthy = await redis.ping()
    health.dependencies["redis"] = {
        "status": "healthy" if redis_healthy else "unhealthy",
        "message": "Redis connection successful"
        if redis_healthy
        else "Redis connection failed",
    }

    # Determine overall health
    if not redis_healthy:
        health.status = "degraded"

    return ApiResponse(success=True, data=health.model_dump())
