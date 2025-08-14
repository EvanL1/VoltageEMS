"""Data query endpoints"""

import time
from fastapi import APIRouter, Depends, HTTPException, Path, Query
from app.models import ApiResponse, RealtimeData
from app.utils.redis import RedisClient

router = APIRouter(prefix="/api/channels", tags=["data"])


async def get_redis_client():
    """Dependency to get Redis client (will be injected from main)"""
    pass


@router.get("/{channel_id}/realtime", response_model=ApiResponse)
async def get_realtime_data(
    channel_id: int = Path(..., description="Channel ID"),
    data_type: str = Query(None, description="Data type (T/S/C/A)"),
    point_id: int = Query(None, description="Specific point ID"),
    limit: int = Query(100, ge=1, le=1000, description="Max results"),
    redis: RedisClient = Depends(get_redis_client),
):
    """Get real-time data for a channel"""
    try:
        # Determine which data types to query
        if data_type:
            data_types = [data_type]
        else:
            data_types = ["T", "S", "C", "A"]

        results = []

        for dt in data_types:
            data = await redis.get_channel_data(channel_id, dt)

            if not data:
                continue

            # Extract timestamp if available
            timestamp = data.get("_timestamp", int(time.time()))

            # Filter out metadata fields
            values = {k: v for k, v in data.items() if not k.startswith("_")}

            # Filter by point_id if specified
            if point_id is not None:
                point_key = str(point_id)
                if point_key in values:
                    values = {point_key: values[point_key]}
                else:
                    continue  # Skip if point not found

            if values:
                results.append(
                    RealtimeData(
                        channel_id=channel_id,
                        data_type=dt,
                        timestamp=timestamp,
                        values=values,
                    )
                )

            if len(results) >= limit:
                break

        if not results:
            raise HTTPException(
                status_code=404, detail=f"No data found for channel {channel_id}"
            )

        return ApiResponse(
            success=True,
            data=results[:limit],
            message=f"Retrieved {len(results)} data records",
        )
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.get("/{channel_id}/history", response_model=ApiResponse)
async def get_historical_data(
    channel_id: int = Path(..., description="Channel ID"),
    data_type: str = Query(None, description="Data type (T/S/C/A)"),
    point_id: int = Query(None, description="Specific point ID"),
    start_time: int = Query(None, description="Start time (Unix timestamp)"),
    end_time: int = Query(None, description="End time (Unix timestamp)"),
    limit: int = Query(100, ge=1, le=1000, description="Max results"),
    redis: RedisClient = Depends(get_redis_client),
):
    """
    Get historical data for a channel.
    Note: This currently returns empty data as it requires InfluxDB integration.
    """
    # TODO: Integrate with hissrv or directly with InfluxDB
    return ApiResponse(
        success=True,
        data=[],
        message="Historical data endpoint - requires InfluxDB integration",
    )
