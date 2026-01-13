"""
API路由定义
提供历史数据查询接口
"""

from fastapi import APIRouter, HTTPException, Query, Depends
from typing import List, Optional
from datetime import datetime, timedelta
import dateutil.parser

from ..models.data_models import (
    QueryRequest, QueryResponse,
    HistoryData, HealthStatus, DataMetrics
)
from ..services.query_service import query_service
from ..services.data_collector import data_collector
from ..services.data_storage import data_storage
from ..services.scheduler import scheduler_service
from ..core.config import settings

# 创建路由器
router = APIRouter(prefix=settings.API_PREFIX)

def parse_datetime(dt_str: str) -> datetime:
    """
    解析多种格式的时间字符串
    支持格式：
    - 2025-08-21
    - 2025-08-21 23:59:59  
    - 2025-08-21T23:59:59
    - 2025-08-21T23:59:59Z
    - 2025-08-21T23:59:59.123Z
    """
    try:
        # 使用dateutil.parser解析多种时间格式
        parsed_dt = dateutil.parser.parse(dt_str)
        # 如果没有时区信息，假设为UTC
        if parsed_dt.tzinfo is None:
            parsed_dt = parsed_dt.replace(tzinfo=datetime.now().astimezone().tzinfo)
        return parsed_dt.astimezone().replace(tzinfo=None)  # 转换为本地时间并移除时区信息
    except (ValueError, TypeError) as e:
        raise HTTPException(
            status_code=400, 
            detail=f"无效的时间格式: {dt_str}。支持格式: 2025-08-21, 2025-08-21 23:59:59, 2025-08-21T23:59:59等"
        )

@router.get("/health", response_model=HealthStatus, summary="健康检查")
async def health_check():
    """系统健康检查"""
    try:
        # 检查各组件状态
        redis_ok = data_collector.test_connection()
        influxdb_ok = data_storage.test_connection()
        scheduler_running = scheduler_service.is_running
        
        components = {
            "redis": {
                "status": "healthy" if redis_ok else "unhealthy",
                "message": "Redis连接正常" if redis_ok else "Redis连接异常"
            },
            "influxdb": {
                "status": "healthy" if influxdb_ok else "unhealthy", 
                "message": "InfluxDB连接正常" if influxdb_ok else "InfluxDB连接异常"
            },
            "scheduler": {
                "status": "healthy" if scheduler_running else "unhealthy",
                "message": f"调度器运行状态: {'运行中' if scheduler_running else '已停止'}"
            }
        }
        
        # 判断整体状态
        overall_status = "healthy" if all(
            comp["status"] == "healthy" for comp in components.values()
        ) else "unhealthy"
        
        return HealthStatus(
            status=overall_status,
            timestamp=datetime.utcnow(),
            components=components
        )
        
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"健康检查失败: {e}")


@router.get("/data/query", response_model=QueryResponse, summary="查询历史数据")
async def query_history_data(
    redis_key: str = Query(..., description="Redis键，必填参数", example="comsrv:device001:sensors"),
    point_id: str = Query(..., description="点位ID，必填参数", example="temperature"),
    start_time: Optional[str] = Query(None, description="开始时间，可选参数。支持多种格式：2025-08-21、2025-08-21 23:59:59、2025-08-21T23:59:59等", example="2025-08-21"),
    end_time: Optional[str] = Query(None, description="结束时间，可选参数。支持多种格式：2025-08-21、2025-08-21 23:59:59、2025-08-21T23:59:59等", example="2025-08-22"),
    page: int = Query(1, ge=1, description="页码，从1开始", example=1),
    page_size: int = Query(100, ge=1, le=1000, description="每页大小，最大1000条", example=100)
):
    """
    查询单个点位的历史数据（返回原始数据）
    
    **参数说明:**
    - redis_key: Redis键，如 "inst:1:M"（必填）
    - point_id: 点位ID，如 "1"（必填）
    - start_time: 开始时间（可选，不提供则默认为最近24小时），支持格式：
      - 日期: 2025-08-21
      - 日期时间: 2025-08-21 23:59:59
      - ISO格式: 2025-08-21T23:59:59
      - 带时区: 2025-08-21T23:59:59Z
    - end_time: 结束时间（可选，不提供则默认为当前时间），格式同start_time
    - page: 页码，从1开始（默认1）
    - page_size: 每页数据量，最大1000条（默认100）
    
    **注意:** InfluxDB 3 返回原始数据，不支持时间窗口聚合。如需降采样，请在客户端处理。
    
    **示例:**
    ```
    # 查询最近24小时数据
    GET /hisApi/data/query?redis_key=inst:1:M&point_id=1
    
    # 查询指定时间范围数据
    GET /hisApi/data/query?redis_key=inst:1:M&point_id=1&start_time=2025-01-09&end_time=2025-01-10
    
    # 分页查询
    GET /hisApi/data/query?redis_key=inst:1:M&point_id=1&page=1&page_size=100
    ```
    """
    try:
        # 解析时间参数
        parsed_start_time = None
        parsed_end_time = None
        
        if start_time:
            parsed_start_time = parse_datetime(start_time)
            
        if end_time:
            parsed_end_time = parse_datetime(end_time)
            
        # 验证时间范围
        if parsed_start_time and parsed_end_time:
            if parsed_end_time <= parsed_start_time:
                raise HTTPException(status_code=400, detail="结束时间必须大于开始时间")
            
            time_diff = parsed_end_time - parsed_start_time
            if time_diff > timedelta(days=settings.MAX_TIME_RANGE_DAYS):
                raise HTTPException(
                    status_code=400, 
                    detail=f"查询时间范围不能超过 {settings.MAX_TIME_RANGE_DAYS} 天"
                )
        
        # 创建查询请求（单个点位查询）
        request = QueryRequest(
            start_time=parsed_start_time,
            end_time=parsed_end_time,
            redis_keys=[redis_key],  # 单个Redis键
            point_ids=[point_id],    # 单个点位ID
            sources=None,           # 不需要来源过滤
            page=page,
            page_size=page_size
        )
        
        # 执行查询
        response = query_service.query_history_data(request)
        return response
        
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"查询失败: {e}")

@router.get("/data/latest", response_model=HistoryData, summary="获取最新数据")
async def get_latest_data(
    redis_key: str = Query(..., description="Redis键，必填参数", example="comsrv:device001:sensors"),
    point_id: str = Query(..., description="点位ID，必填参数", example="temperature")
):
    """
    获取指定点位的最新数据
    
    **参数说明:**
    - redis_key: Redis键，如 "comsrv:device001:sensors"  
    - point_id: 点位ID，如 "temperature"
    
    **示例:**
    ```
    GET /hisApi/data/latest?redis_key=comsrv:device001:sensors&point_id=temperature
    ```
    
    **返回:** 单条最新的历史数据记录
    """
    try:
        data = query_service.get_latest_data(redis_key, point_id)
        
        if not data:
            raise HTTPException(status_code=404, detail="未找到数据")
        
        return data
        
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"获取最新数据失败: {e}")

@router.get("/data/range", summary="获取数据范围信息")
async def get_data_range():
    """
    获取数据范围信息
    
    **功能:** 获取数据库中存储数据的时间范围信息
    
    **示例:**
    ```
    GET /hisApi/data/range
    ```
    
    **返回:** 包含最早和最晚数据时间戳的信息
    """
    try:
        range_info = query_service.get_data_range_info()
        return range_info
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"获取数据范围信息失败: {e}")

@router.get("/channels", summary="获取通道列表")
async def get_channels():
    """
    获取所有Redis键通道列表
    
    **功能:** 获取当前系统监控的所有Redis键通道
    
    **示例:**
    ```
    GET /hisApi/channels
    ```
    
    **返回:** 包含所有通道名称和数量的列表
    ```json
    {
      "channels": ["comsrv:device001:sensors", "comsrv:device002:sensors", "modsrv:plc001:data"],
      "count": 3
    }
    ```
    """
    try:
        channels = data_collector.get_channel_list()
        return {"channels": channels, "count": len(channels)}
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"获取通道列表失败: {e}")

@router.get("/config/influxdb", summary="获取InfluxDB配置信息")
async def get_influxdb_config():
    """
    获取InfluxDB连接配置信息
    
    **功能:** 返回当前InfluxDB的连接配置
    
    **示例:**
    ```
    GET /hisApi/config/influxdb
    ```
    
    **返回:**
    ```json
    {
      "status": "success",
      "url": "http://192.168.30.21:18181",
      "database": "history_data",
      "token": "apiv3_ureLplELJaYvOZjBEWCwMDXERyhMSIgm5hJo_lXLUpI02yNyx-x8MPv7XqtmIpYXH0zDXdz-dCEojm5G_rIjTQ",
      "connected": true
    }
    ```
    """
    try:
        from ..core.config_loader import config_loader
        from ..core.influxdb import influxdb_manager
        
        # 获取配置
        influxdb_config = config_loader.get_influxdb_config()
        url = influxdb_config.get('url', '')
        database = config_loader.get_database_name()
        token = influxdb_config.get('token', '')
        
        # 检查连接状态
        is_connected = influxdb_manager.is_connected()
        
        return {
            "status": "success",
            "url": url,
            "database": database,
            "token": token,
            "connected": is_connected
        }
    except Exception as e:
        return {
            "status": "error",
            "message": f"获取配置失败: {str(e)}",
            "url": "",
            "database": "",
            "token": "",
            "connected": False
        }

@router.get("/metrics", response_model=DataMetrics, summary="获取数据指标")
async def get_data_metrics():
    """
    获取系统数据指标
    
    **功能:** 获取历史数据服务的运行指标和统计信息
    
    **示例:**
    ```
    GET /hisApi/metrics
    ```
    
    **返回:** 包含数据点数量、通道数量等关键指标
    ```json
    {
      "total_points": 125000,
      "latest_timestamp": "2025-09-02T09:30:00",
      "channels_count": 15,
      "active_channels": 15
    }
    ```
    """
    try:
        # 获取存储统计
        storage_stats = data_storage.get_storage_stats()
        
        # 获取通道信息
        channels = data_collector.get_channel_list()
        
        # 获取数据范围信息
        range_info = query_service.get_data_range_info()
        
        return DataMetrics(
            total_points=storage_stats.get("total_points", 0),
            latest_timestamp=range_info.get("latest_timestamp"),
            channels_count=len(channels),
            active_channels=len(channels)
        )
        
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"获取数据指标失败: {e}")


