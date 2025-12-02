"""
数据模型定义
"""

from pydantic import BaseModel, Field
from typing import Optional, Dict, Any, List
from datetime import datetime

class HistoryData(BaseModel):
    """历史数据模型"""
    timestamp: datetime = Field(..., description="时间戳")
    redis_key: str = Field(..., description="Redis键")
    point_id: str = Field(..., description="点位ID")
    value: Any = Field(..., description="数值")
    source: str = Field(..., description="数据来源")
    
    class Config:
        json_encoders = {
            datetime: lambda v: v.isoformat()
        }

class RedisDataPoint(BaseModel):
    """Redis数据点模型"""
    key: str = Field(..., description="Redis键")
    field: str = Field(..., description="字段名")
    value: Any = Field(..., description="值")
    timestamp: Optional[datetime] = None
    
    def to_history_data(self, parsed_key: Dict[str, str]) -> HistoryData:
        """转换为历史数据"""
        return HistoryData(
            timestamp=self.timestamp or datetime.utcnow(),
            redis_key=parsed_key.get("channel_id", "unknown"),  # channel_id实际是完整的Redis键
            point_id=self.field,
            value=self.value,
            source=parsed_key.get("source", "unknown")
        )

class QueryRequest(BaseModel):
    """查询请求模型"""
    start_time: Optional[datetime] = Field(None, description="开始时间，如果不提供则默认查询最近24小时")
    end_time: Optional[datetime] = Field(None, description="结束时间，如果不提供则默认为当前时间")
    redis_keys: Optional[List[str]] = Field(None, description="Redis键列表")
    point_ids: Optional[List[str]] = Field(None, description="点位ID列表")
    sources: Optional[List[str]] = Field(None, description="数据来源列表")
    interval: int = Field(600, ge=1, description="数据采样间隔（秒），默认600秒（10分钟）")
    page: int = Field(1, ge=1, description="页码")
    page_size: int = Field(100, ge=1, le=1000, description="每页大小")
    
    class Config:
        json_encoders = {
            datetime: lambda v: v.isoformat()
        }

class QueryResponse(BaseModel):
    """查询响应模型"""
    status: str = Field("success", description="响应状态：success-成功, error-错误, partial-部分成功")
    message: Optional[str] = Field(None, description="状态消息，错误时包含错误信息")
    data: List[HistoryData] = Field(..., description="数据列表")
    total: int = Field(..., description="总数量")
    page: int = Field(..., description="当前页码")
    page_size: int = Field(..., description="每页大小")
    has_more: bool = Field(..., description="是否有更多数据")
    
class StatisticsRequest(BaseModel):
    """统计请求模型"""
    start_time: datetime = Field(..., description="开始时间")
    end_time: datetime = Field(..., description="结束时间")
    redis_key: str = Field(..., description="Redis键")
    point_id: str = Field(..., description="点位ID")
    aggregation: str = Field("mean", description="聚合方式：mean, sum, min, max, count")
    interval: str = Field("1h", description="时间间隔：1m, 5m, 1h, 1d")

class StatisticsResponse(BaseModel):
    """统计响应模型"""
    status: str = Field("success", description="响应状态：success-成功, error-错误")
    message: Optional[str] = Field(None, description="状态消息，错误时包含错误信息")
    redis_key: str = Field(..., description="Redis键")
    point_id: str = Field(..., description="点位ID")
    aggregation: str = Field(..., description="聚合方式")
    interval: str = Field(..., description="时间间隔")
    data: List[Dict[str, Any]] = Field(..., description="统计数据")
    
class HealthStatus(BaseModel):
    """健康状态模型"""
    status: str = Field(..., description="状态：healthy, unhealthy")
    timestamp: datetime = Field(..., description="检查时间")
    components: Dict[str, Dict[str, Any]] = Field(..., description="组件状态")
    

class DataMetrics(BaseModel):
    """数据指标模型"""
    total_points: int = Field(..., description="数据库中实际存储的数据条数")
    latest_timestamp: Optional[datetime] = Field(None, description="最新数据时间戳")
    channels_count: int = Field(..., description="通道数量")
    active_channels: int = Field(..., description="活跃通道数量")
