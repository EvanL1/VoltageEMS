"""Pydantic models for API Gateway"""

from typing import Dict, Optional, Any
from pydantic import BaseModel, Field
from datetime import datetime


class ApiResponse(BaseModel):
    """Standard API response wrapper"""

    success: bool
    message: Optional[str] = None
    data: Optional[Any] = None
    timestamp: datetime = Field(default_factory=datetime.utcnow)


class ChannelStatus(BaseModel):
    """Channel status model"""

    channel_id: int
    status: str  # active, inactive, error
    last_update: Optional[int] = None  # Unix timestamp
    active_points: int = 0


class DataQuery(BaseModel):
    """Query parameters for data endpoints"""

    data_type: Optional[str] = None  # T, S, C, A
    point_id: Optional[int] = None
    limit: int = Field(default=100, ge=1, le=1000)
    start_time: Optional[int] = None  # Unix timestamp
    end_time: Optional[int] = None  # Unix timestamp


class RealtimeData(BaseModel):
    """Real-time data model"""

    channel_id: int
    data_type: str
    timestamp: int  # Unix timestamp
    values: Dict[str, Any]  # point_id -> value


class HealthStatus(BaseModel):
    """Health check status"""

    status: str  # healthy, degraded, unhealthy
    service: str
    timestamp: datetime
    dependencies: Optional[Dict[str, Dict[str, Any]]] = None
