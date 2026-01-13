"""
查询服务
从InfluxDB 3.x查询历史数据（使用SQL）
"""

from typing import List, Dict, Any, Optional, Tuple
from datetime import datetime, timezone, timedelta
from loguru import logger

from ..core.influxdb import influxdb_manager
from ..core.config_loader import config_loader
from ..models.data_models import (
    HistoryData, QueryRequest, QueryResponse
)

class QueryService:
    """查询服务"""
    
    def __init__(self):
        self.influxdb_client = influxdb_manager.get_client()
        self.database = influxdb_manager.get_database_name()
    
    def _get_measurement_from_redis_key(self, redis_key: str) -> str:
        """从Redis键提取measurement名称"""
        key_parts = redis_key.split(':')
        return key_parts[0] if key_parts else "data"
        
    def _build_time_filter(self, start_time: datetime, end_time: datetime) -> str:
        """构建SQL时间过滤条件"""
        # 确保时间戳有时区信息
        if start_time.tzinfo is None:
            start_time = start_time.replace(tzinfo=timezone.utc)
        if end_time.tzinfo is None:
            end_time = end_time.replace(tzinfo=timezone.utc)
            
        return f"time >= '{start_time.isoformat()}' AND time <= '{end_time.isoformat()}'"
    
    def _build_filters(self, request: QueryRequest) -> List[str]:
        """构建SQL过滤条件列表"""
        filters = []
        
        # Redis键过滤
        if request.redis_keys:
            redis_key_conditions = [f"redis_key = '{key}'" for key in request.redis_keys]
            filters.append(f"({' OR '.join(redis_key_conditions)})")
        
        # 点位ID过滤
        if request.point_ids:
            point_conditions = [f"point_id = '{pid}'" for pid in request.point_ids]
            filters.append(f"({' OR '.join(point_conditions)})")
        
        # 数据来源过滤
        if request.sources:
            source_conditions = [f"source = '{src}'" for src in request.sources]
            filters.append(f"({' OR '.join(source_conditions)})")
        
        return filters
    
    def _parse_interval_to_seconds(self, interval_str: str) -> int:
        """将间隔字符串转换为秒数"""
        # 支持格式：10s, 1m, 5m, 1h, 2h, 1d
        import re
        match = re.match(r'^(\d+)([smhd])$', interval_str)
        if not match:
            return 60  # 默认1分钟
        
        value = int(match.group(1))
        unit = match.group(2)
        
        multipliers = {
            's': 1,
            'm': 60,
            'h': 3600,
            'd': 86400
        }
        
        return value * multipliers.get(unit, 60)
    
    def query_history_data(self, request: QueryRequest) -> QueryResponse:
        """查询历史数据（使用SQL）"""
        try:
            # 设置默认时间范围（如果未提供）
            end_time = request.end_time or datetime.now(timezone.utc)
            start_time = request.start_time or (end_time - timedelta(hours=24))
            
            # 构建SQL查询
            where_conditions = [self._build_time_filter(start_time, end_time)]
            where_conditions.extend(self._build_filters(request))
            
            where_clause = " AND ".join(where_conditions)
            
            # 确定表名（measurement）
            # InfluxDB 3中，表名就是measurement名，从redis_key提取
            if request.redis_keys and len(request.redis_keys) > 0:
                # 如果指定了redis_key，从中提取measurement
                measurement = self._get_measurement_from_redis_key(request.redis_keys[0])
            else:
                # 如果没有指定，使用通配符或默认measurement
                measurement = "*"  # 查询所有measurement
            
            # 构建查询语句 - 直接查询原始数据
            query = f"""
                SELECT 
                    time,
                    redis_key,
                    point_id,
                    source,
                    value
                FROM {measurement}
                WHERE {where_clause}
                ORDER BY time DESC
                LIMIT {request.page_size} OFFSET {(request.page - 1) * request.page_size}
            """
            
            logger.debug(f"执行SQL查询: {query}")
            
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
            
            # 获取总数（简化实现）
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
        """转换SQL查询记录为历史数据"""
        try:
            # 解析时间戳
            timestamp_value = record.get('time')
            if timestamp_value:
                if isinstance(timestamp_value, str):
                    timestamp = datetime.fromisoformat(timestamp_value.replace('Z', '+00:00'))
                elif isinstance(timestamp_value, datetime):
                    timestamp = timestamp_value
                else:
                    timestamp = datetime.utcnow()
            else:
                timestamp = datetime.utcnow()
            
            # 获取数值
            value = record.get('value')
            if value is None:
                # 尝试获取其他字段的值
                value = record.get('string_value') or record.get('boolean_value', 0)
            
            # 创建历史数据对象
            redis_key = record.get('redis_key', 'unknown')
            
            return HistoryData(
                timestamp=timestamp,
                redis_key=redis_key,
                point_id=str(record.get('point_id', 'unknown')),
                value=value,
                source=record.get('source', 'unknown')
            )
            
        except Exception as e:
            logger.error(f"转换记录失败: {e}, 记录: {record}")
            return None
    
    def get_latest_data(self, redis_key: str, point_id: str) -> Optional[HistoryData]:
        """获取最新数据（使用SQL）"""
        try:
            # 从redis_key提取measurement
            measurement = self._get_measurement_from_redis_key(redis_key)
            
            # 只查询必要的字段
            query = f"""
                SELECT 
                    time,
                    redis_key,
                    point_id,
                    source,
                    value
                FROM {measurement}
                WHERE redis_key = '{redis_key}'
                    AND point_id = '{point_id}'
                ORDER BY time DESC
                LIMIT 1
            """
            
            results = influxdb_manager.query_data(query)
            if results:
                return self._convert_record_to_history_data(results[0])
                
        except Exception as e:
            logger.error(f"获取最新数据失败: {e}")
            
        return None
    
    def get_data_range_info(self) -> Dict[str, Any]:
        """获取数据范围信息（使用SQL）"""
        info = {
            "earliest_timestamp": None,
            "latest_timestamp": None,
            "total_points": 0,
            "channels": [],
            "data_types": []
        }
        
        try:
            # 使用 SHOW TABLES 获取所有表（InfluxDB 3-core 支持）
            tables_query = "SHOW TABLES"
            tables_result = influxdb_manager.query_data(tables_query)
            
            if not tables_result:
                logger.warning("未找到任何表")
                return info
            
            # 提取用户数据表（table_schema = 'iox'，排除系统表）
            measurements = [
                row.get('table_name') 
                for row in tables_result 
                if row.get('table_schema') == 'iox' and row.get('table_name')
            ]
            
            if not measurements:
                logger.warning("未找到任何用户数据表（schema=iox）")
                return info
            
            logger.info(f"找到 {len(measurements)} 个数据表: {measurements}")
            
            # 遍历所有存在的表
            for measurement in measurements:
                try:
                    # 获取最早时间戳
                    earliest_query = f"""
                        SELECT time
                        FROM {measurement}
                        ORDER BY time ASC
                        LIMIT 1
                    """
                    
                    results = influxdb_manager.query_data(earliest_query)
                    if results:
                        earliest_time = results[0].get('time')
                        if earliest_time and (not info["earliest_timestamp"] or earliest_time < info["earliest_timestamp"]):
                            info["earliest_timestamp"] = earliest_time
                    
                    # 获取最新时间戳
                    latest_query = f"""
                        SELECT time
                        FROM {measurement}
                        ORDER BY time DESC
                        LIMIT 1
                    """
                    
                    results = influxdb_manager.query_data(latest_query)
                    if results:
                        latest_time = results[0].get('time')
                        if latest_time and (not info["latest_timestamp"] or latest_time > info["latest_timestamp"]):
                            info["latest_timestamp"] = latest_time
                    
                    # 获取总数据点数
                    count_query = f"""
                        SELECT COUNT(*) as total
                        FROM {measurement}
                    """
                    
                    results = influxdb_manager.query_data(count_query)
                    if results:
                        count = results[0].get('total', 0)
                        if count is not None:
                            info["total_points"] += count
                    
                    # 获取唯一redis_key
                    channels_query = f"""
                        SELECT DISTINCT redis_key
                        FROM {measurement}
                        LIMIT 100
                    """
                    
                    results = influxdb_manager.query_data(channels_query)
                    for r in results:
                        redis_key = r.get('redis_key')
                        if redis_key and redis_key not in info["channels"]:
                            info["channels"].append(redis_key)
                    
                    # 获取唯一source
                    types_query = f"""
                        SELECT DISTINCT source
                        FROM {measurement}
                    """
                    
                    results = influxdb_manager.query_data(types_query)
                    for r in results:
                        source = r.get('source')
                        if source and source not in info["data_types"]:
                            info["data_types"].append(source)
                    
                except Exception as e:
                    # 某个measurement查询失败，记录日志但继续处理其他measurement
                    logger.warning(f"查询 measurement '{measurement}' 失败: {e}")
                    continue
            
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
