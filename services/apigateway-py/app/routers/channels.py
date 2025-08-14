"""Channel management endpoints"""

import time
from fastapi import APIRouter, Depends, HTTPException, Path
from app.models import ApiResponse, ChannelStatus
from app.utils.redis import RedisClient

router = APIRouter(prefix="/api/channels", tags=["channels"])


async def get_redis_client():
    """Dependency to get Redis client (will be injected from main)"""
    pass


@router.get("", response_model=ApiResponse)
async def list_channels(redis: RedisClient = Depends(get_redis_client)):
    """List all available channels"""
    try:
        channel_ids = await redis.get_channel_ids()
        channels = []

        for channel_id in channel_ids:
            # Get channel data to check status
            data = await redis.get_channel_data(channel_id, "T")

            # Check if channel has recent data
            last_update = None
            if data and "_timestamp" in data:
                last_update = data["_timestamp"]

            channels.append(
                ChannelStatus(
                    channel_id=channel_id,
                    status="active" if data else "inactive",
                    last_update=last_update,
                    active_points=len(data) - 1
                    if data and "_timestamp" in data
                    else len(data),
                )
            )

        return ApiResponse(
            success=True, data=channels, message=f"Found {len(channels)} channels"
        )
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.get("/{channel_id}/status", response_model=ApiResponse)
async def get_channel_status(
    channel_id: int = Path(..., description="Channel ID"),
    redis: RedisClient = Depends(get_redis_client),
):
    """Get status for a specific channel"""
    try:
        # Check all data types for this channel
        data_types = ["T", "S", "C", "A"]
        active_points = 0
        last_update = None
        has_data = False

        for data_type in data_types:
            data = await redis.get_channel_data(channel_id, data_type)
            if data:
                has_data = True
                # Count points (excluding metadata fields)
                points = len([k for k in data.keys() if not k.startswith("_")])
                active_points += points

                # Get timestamp if available
                if "_timestamp" in data and data["_timestamp"]:
                    ts = data["_timestamp"]
                    if last_update is None or ts > last_update:
                        last_update = ts

        if not has_data:
            raise HTTPException(
                status_code=404, detail=f"Channel {channel_id} not found"
            )

        status = ChannelStatus(
            channel_id=channel_id,
            status="active" if has_data else "inactive",
            last_update=last_update or int(time.time()),
            active_points=active_points,
        )

        return ApiResponse(success=True, data=status)
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))
