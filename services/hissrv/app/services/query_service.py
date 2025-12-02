"""
查询服务
从InfluxDB查询历史数据
"""

from typing import List, Dict, Any, Optional, Tuple
from datetime import datetime, timezone, timedelta
from loguru import logger

from ..core.influxdb import influxdb_manager
from ..core.config_loader import config_loader
from ..models.data_models import (
    HistoryData, QueryRequest, QueryResponse, 
    StatisticsRequest, StatisticsResponse
)

class QueryService:
    """查询服务"""
    
    def __init__(self):
        self.influxdb_client = influxdb_manager.get_client()
        self.query_api = influxdb_manager.query_api
        self.bucket = getattr(influxdb_manager.client, '_bucket', 'history_data') if influxdb_manager.client else 'history_data'
        
    def _build_time_filter(self, start_time: datetime, end_time: datetime) -> str:
        """构建时间过滤器"""
        # 确保时间戳有时区信息
        if start_time.tzinfo is None:
            start_time = start_time.replace(tzinfo=timezone.utc)
        if end_time.tzinfo is None:
            end_time = end_time.replace(tzinfo=timezone.utc)
            
        return f'|> range(start: {start_time.isoformat()}, stop: {end_time.isoformat()})'
    
    def _build_filters(self, request: QueryRequest) -> List[str]:
        """构建过滤器列表"""
        filters = []
        
        # Redis键过滤
        if request.redis_keys:
            redis_key_filter = ' or '.join([f'r.redis_key == "{key}"' for key in request.redis_keys])
            filters.append(f'|> filter(fn: (r) => {redis_key_filter})')
        
        # 点位ID过滤
        if request.point_ids:
            point_filter = ' or '.join([f'r.point_id == "{pid}"' for pid in request.point_ids])
            filters.append(f'|> filter(fn: (r) => {point_filter})')
        
        # 数据来源过滤
        if request.sources:
            source_filter = ' or '.join([f'r.source == "{src}"' for src in request.sources])
            filters.append(f'|> filter(fn: (r) => {source_filter})')
        
        return filters
    
    def query_history_data(self, request: QueryRequest) -> QueryResponse:
        """查询历史数据"""
        try:
            if not self.query_api:
                logger.error("InfluxDB查询API不可用")
                return QueryResponse(
                    status="error",
                    message="InfluxDB查询API不可用",
                    data=[], 
                    total=0, 
                    page=request.page, 
                    page_size=request.page_size, 
                    has_more=False
                )
            
            # 设置默认时间范围（如果未提供）
            end_time = request.end_time or datetime.now(timezone.utc)
            start_time = request.start_time or (end_time - timedelta(hours=24))
            
            # 构建基础查询
            query_parts = [
                f'from(bucket: "{self.bucket}")',
                self._build_time_filter(start_time, end_time)
            ]
            
            # 添加过滤器
            query_parts.extend(self._build_filters(request))
            
            # 获取数据收集间隔（配置文件中的采样间隔）
            collection_interval = config_loader.get_data_collection_interval()
            
            # 只有当请求的采样间隔大于等于数据收集间隔时，才进行窗口聚合
            # 如果请求间隔小于数据收集间隔，说明数据库中没有更细粒度的数据，无需聚合
            if request.interval and request.interval > collection_interval:
                # 将秒转换为 InfluxDB 的时间格式
                interval_str = f"{request.interval}s"
                query_parts.append(f'|> aggregateWindow(every: {interval_str}, fn: mean, createEmpty: false)')
            
            # 添加排序和分页
            offset = (request.page - 1) * request.page_size
            query_parts.extend([
                '|> sort(columns: ["_time"], desc: true)',
                f'|> limit(n: {request.page_size}, offset: {offset})'
            ])
            
            # 执行查询
            query = '\n'.join(query_parts)
            logger.debug(f"执行查询: {query}")
            
            results = influxdb_manager.query_data(query)
            
            # 转换结果
            history_data = []
            for record in results:
                try:
                    data = self._convert_record_to_history_data(record)
                    if data:
                        history_data.append(data)
                except Exception as e:
                    logger.error(f"转换记录失败: {e}, 记录: {record}")
            
            # 获取总数（简化实现，实际项目中可能需要单独的计数查询）
            total = len(history_data)
            has_more = len(history_data) == request.page_size
            
            return QueryResponse(
                status="success",
                message=f"成功查询到 {total} 条数据" if total > 0 else "未查询到数据",
                data=history_data,
                total=total,
                page=request.page,
                page_size=request.page_size,
                has_more=has_more
            )
            
        except Exception as e:
            logger.error(f"查询历史数据失败: {e}")
            return QueryResponse(
                status="error",
                message=f"查询失败: {str(e)}",
                data=[], 
                total=0, 
                page=request.page, 
                page_size=request.page_size, 
                has_more=False
            )
    
    def _convert_record_to_history_data(self, record: Dict[str, Any]) -> Optional[HistoryData]:
        """转换记录为历史数据"""
        try:
            # 解析时间戳
            timestamp_str = record.get('_time')
            if timestamp_str:
                if isinstance(timestamp_str, str):
                    timestamp = datetime.fromisoformat(timestamp_str.replace('Z', '+00:00'))
                else:
                    timestamp = timestamp_str
            else:
                timestamp = datetime.utcnow()
            
            # 获取数值
            value = record.get('_value')
            if value is None:
                # 尝试获取其他字段的值
                value = record.get('string_value') or record.get('boolean_value', 0)
            
            # 创建历史数据对象 - 使用简化的结构
            redis_key = record.get('redis_key', 'unknown')
            
            return HistoryData(
                timestamp=timestamp,
                redis_key=redis_key,  # 直接使用Redis键
                point_id=str(record.get('point_id', 'unknown')),
                value=value,
                source=record.get('source', 'unknown')
            )
            
        except Exception as e:
            logger.error(f"转换记录失败: {e}, 记录: {record}")
            return None
    
    def query_statistics(self, request: StatisticsRequest) -> StatisticsResponse:
        """查询统计数据"""
        try:
            if not self.query_api:
                logger.error("InfluxDB查询API不可用")
                return StatisticsResponse(
                    status="error",
                    message="InfluxDB查询API不可用",
                    redis_key=request.redis_key,
                    point_id=request.point_id,
                    aggregation=request.aggregation,
                    interval=request.interval,
                    data=[]
                )
            
            # 构建统计查询
            aggregation_func = {
                'mean': 'mean',
                'sum': 'sum',
                'min': 'min',
                'max': 'max',
                'count': 'count'
            }.get(request.aggregation, 'mean')
            
            query = f'''from(bucket: "{self.bucket}")
|> range(start: {request.start_time.isoformat()}Z, stop: {request.end_time.isoformat()}Z)
|> filter(fn: (r) => r.redis_key == "{request.redis_key}")
|> filter(fn: (r) => r.point_id == "{request.point_id}")
|> filter(fn: (r) => r._field == "value")
|> aggregateWindow(every: {request.interval}, fn: {aggregation_func}, createEmpty: false)
|> sort(columns: ["_time"])'''
            
            logger.debug(f"执行统计查询: {query}")
            results = influxdb_manager.query_data(query)
            
            # 转换结果
            data = []
            for record in results:
                data.append({
                    'timestamp': record.get('_time'),
                    'value': record.get('_value')
                })
            
            return StatisticsResponse(
                status="success",
                message=f"成功查询到 {len(data)} 条统计数据" if data else "未查询到统计数据",
                redis_key=request.redis_key,
                point_id=request.point_id,
                aggregation=request.aggregation,
                interval=request.interval,
                data=data
            )
            
        except Exception as e:
            logger.error(f"查询统计数据失败: {e}")
            return StatisticsResponse(
                status="error",
                message=f"查询统计数据失败: {str(e)}",
                redis_key=request.redis_key,
                point_id=request.point_id,
                aggregation=request.aggregation,
                interval=request.interval,
                data=[]
            )
    
    def get_latest_data(self, redis_key: str, point_id: str) -> Optional[HistoryData]:
        """获取最新数据"""
        try:
            if not self.query_api:
                return None
            
            query = f'''
            from(bucket: "{self.bucket}")
            |> range(start: -24h)
            |> filter(fn: (r) => r.redis_key == "{redis_key}")
            |> filter(fn: (r) => r.point_id == "{point_id}")
            |> filter(fn: (r) => r._field == "value")
            |> sort(columns: ["_time"], desc: true)
            |> limit(n: 1)
            '''
            
            results = influxdb_manager.query_data(query)
            if results:
                return self._convert_record_to_history_data(results[0])
                
        except Exception as e:
            logger.error(f"获取最新数据失败: {e}")
            
        return None
    
    def get_data_range_info(self) -> Dict[str, Any]:
        """获取数据范围信息"""
        info = {
            "earliest_timestamp": None,
            "latest_timestamp": None,
            "total_points": 0,
            "channels": [],
            "data_types": []
        }
        
        try:
            if not self.query_api:
                return info
            
            # 获取最早时间戳
            earliest_query = f'''
            from(bucket: "{self.bucket}")
            |> range(start: 0)
            |> sort(columns: ["_time"])
            |> limit(n: 1)
            '''
            
            results = influxdb_manager.query_data(earliest_query)
            if results:
                info["earliest_timestamp"] = results[0].get('_time')
            
            # 获取最新时间戳
            latest_query = f'''
            from(bucket: "{self.bucket}")
            |> range(start: 0)
            |> sort(columns: ["_time"], desc: true)
            |> limit(n: 1)
            '''
            
            results = influxdb_manager.query_data(latest_query)
            if results:
                info["latest_timestamp"] = results[0].get('_time')
            
            # 获取唯一通道ID
            channels_query = f'''
            from(bucket: "{self.bucket}")
            |> range(start: 0)
            |> distinct(column: "channel_id")
            |> limit(n: 1000)
            '''
            
            results = influxdb_manager.query_data(channels_query)
            info["channels"] = [r.get('_value') for r in results if r.get('_value')]
            
            # 获取数据类型
            types_query = f'''
            from(bucket: "{self.bucket}")
            |> range(start: 0)
            |> distinct(column: "data_type")
            '''
            
            results = influxdb_manager.query_data(types_query)
            info["data_types"] = [r.get('_value') for r in results if r.get('_value')]
            
        except Exception as e:
            logger.error(f"获取数据范围信息失败: {e}")
        
        return info
    
    def test_connection(self) -> bool:
        """测试连接"""
        try:
            return influxdb_manager.is_connected()
        except Exception as e:
            logger.error(f"测试InfluxDB连接失败: {e}")
            return False

# 全局查询服务实例
query_service = QueryService()
