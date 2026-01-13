"""
数据存储服务
将数据存储到InfluxDB 3.x
"""

from typing import List, Dict, Any, Optional
from datetime import datetime, timezone
from loguru import logger
from influxdb_client_3 import Point

from ..core.influxdb import influxdb_manager
from ..core.config_loader import config_loader
from ..core.config import settings
from ..models.data_models import HistoryData

class DataStorage:
    """数据存储器"""
    
    def __init__(self):
        self.influxdb_client = influxdb_manager.get_client()
        self.common_tags = config_loader.get_config('data_storage.fields.common_tags', [])
        self.common_fields = config_loader.get_config('data_storage.fields.common_fields', [])
        
    def create_measurement_name(self, redis_key: str) -> str:
        """根据Redis键创建measurement名称"""
        # 直接使用Redis键的第一部分作为measurement名称
        key_parts = redis_key.split(':')
        return key_parts[0] if key_parts else "data"
    
    def create_point_from_history_data(self, data: HistoryData) -> Point:
        """从历史数据创建InfluxDB数据点"""
        try:
            # 使用Redis键创建measurement名称
            measurement = self.create_measurement_name(data.redis_key)
            
            # 创建数据点
            point = Point(measurement)
            
            # 添加标签（tags） - 使用Redis键作为主要标识
            point = point.tag("redis_key", str(data.redis_key))
            point = point.tag("point_id", str(data.point_id))
            point = point.tag("source", data.source)
            
            # 添加数值字段
            if isinstance(data.value, (int, float)):
                point = point.field("value", float(data.value))
            elif isinstance(data.value, bool):
                point = point.field("value", int(data.value))
                point = point.field("boolean_value", data.value)
            elif isinstance(data.value, str):
                # 尝试转换为数值
                try:
                    numeric_value = float(data.value)
                    point = point.field("value", numeric_value)
                except (ValueError, TypeError):
                    point = point.field("string_value", data.value)
                    # 对于字符串值，添加一个默认数值字段用于查询
                    point = point.field("value", 0.0)
            else:
                # 对于其他类型，转换为字符串
                point = point.field("string_value", str(data.value))
                point = point.field("value", 0.0)
            
            # 设置时间戳
            if data.timestamp.tzinfo is None:
                # 如果没有时区信息，假设为UTC
                timestamp = data.timestamp.replace(tzinfo=timezone.utc)
            else:
                timestamp = data.timestamp
            
            point = point.time(timestamp)
            
            return point
            
        except Exception as e:
            logger.error(f"创建InfluxDB数据点失败: {e}, 数据: {data}")
            raise
    
    def store_single_data(self, data: HistoryData) -> bool:
        """存储单条数据"""
        try:
            point = self.create_point_from_history_data(data)
            success = influxdb_manager.write_point(point)
            
            if success:
                logger.debug(f"存储单条数据成功: {data.redis_key}:{data.point_id}")
            else:
                logger.error(f"存储单条数据失败: {data.redis_key}:{data.point_id}")
                
            return success
            
        except Exception as e:
            logger.error(f"存储单条数据异常: {e}")
            return False
    
    def store_batch_data(self, data_list: List[HistoryData]) -> Dict[str, Any]:
        """批量存储数据"""
        result = {
            "total": len(data_list),
            "success": 0,
            "failed": 0,
            "errors": []
        }
        
        if not data_list:
            return result
        
        try:
            # 创建数据点列表
            points = []
            for data in data_list:
                try:
                    point = self.create_point_from_history_data(data)
                    points.append(point)
                except Exception as e:
                    result["failed"] += 1
                    result["errors"].append(f"创建数据点失败: {e}")
                    logger.error(f"创建数据点失败: {e}, 数据: {data}")
            
            # 批量写入
            if points:
                success = influxdb_manager.write_points(points)
                
                if success:
                    result["success"] = len(points)
                    logger.info(f"批量存储成功: {len(points)} 条数据")
                else:
                    result["failed"] = len(points)
                    result["errors"].append("InfluxDB批量写入失败")
                    logger.error(f"InfluxDB批量写入失败: {len(points)} 条数据")
            
        except Exception as e:
            result["failed"] = result["total"]
            result["errors"].append(f"批量存储异常: {e}")
            logger.error(f"批量存储异常: {e}")
        
        return result
    
    def get_storage_stats(self) -> Dict[str, Any]:
        """获取存储统计信息（使用SQL查询）"""
        stats = {
            "total_measurements": 0,
            "total_points": 0,
            "oldest_timestamp": None,
            "newest_timestamp": None,
            "measurements": {}
        }
        
        try:
            # 使用 SHOW TABLES 动态获取所有用户数据表（InfluxDB 3-core 支持）
            tables_query = "SHOW TABLES"
            tables_result = influxdb_manager.query_data(tables_query)
            
            if not tables_result:
                logger.warning("未找到任何表")
                return stats
            
            # 提取用户数据表（table_schema = 'iox'，排除系统表）
            measurements = [
                row.get('table_name') 
                for row in tables_result 
                if row.get('table_schema') == 'iox' and row.get('table_name')
            ]
            
            if not measurements:
                logger.warning("未找到任何用户数据表（schema=iox）")
                return stats
            
            logger.info(f"找到 {len(measurements)} 个数据表: {measurements}")
            
            for measurement in measurements:
                try:
                    # 查询该measurement的数据点数
                    query = f"""
                        SELECT COUNT(*) as total_count
                        FROM {measurement}
                    """
                    
                    logger.debug(f"查询 {measurement} 数据点数: {query}")
                    result = influxdb_manager.query_data(query)
                    if result and len(result) > 0:
                        count = result[0].get("total_count", 0)
                        if count and count > 0:
                            stats["total_points"] += count
                            stats["measurements"][measurement] = count
                            stats["total_measurements"] += 1
                    
                    # 查询最新时间戳
                    query_latest = f"""
                        SELECT time
                        FROM {measurement}
                        ORDER BY time DESC
                        LIMIT 1
                    """
                    
                    latest_result = influxdb_manager.query_data(query_latest)
                    if latest_result and len(latest_result) > 0:
                        latest_time = latest_result[0].get("time")
                        if latest_time:
                            if not stats["newest_timestamp"] or latest_time > stats["newest_timestamp"]:
                                stats["newest_timestamp"] = latest_time
                    
                    # 查询最早时间戳
                    query_earliest = f"""
                        SELECT time
                        FROM {measurement}
                        ORDER BY time ASC
                        LIMIT 1
                    """
                    
                    earliest_result = influxdb_manager.query_data(query_earliest)
                    if earliest_result and len(earliest_result) > 0:
                        earliest_time = earliest_result[0].get("time")
                        if earliest_time:
                            if not stats["oldest_timestamp"] or earliest_time < stats["oldest_timestamp"]:
                                stats["oldest_timestamp"] = earliest_time
                    
                except Exception as e:
                    # 某个measurement查询失败，记录日志但继续处理其他measurement
                    logger.warning(f"查询 measurement '{measurement}' 统计信息失败: {e}")
                    continue
            
        except Exception as e:
            logger.error(f"获取存储统计失败: {e}")
        
        return stats
    
    def cleanup_old_data(self, older_than_days: int) -> Dict[str, Any]:
        """清理旧数据"""
        result = {
            "success": False,
            "deleted_points": 0,
            "error": None
        }
        
        try:
            database = influxdb_manager.get_database_name()
            
            # InfluxDB 3使用SQL DELETE语句
            # 注意：需要确认InfluxDB 3-core是否支持DELETE操作
            from datetime import timedelta
            
            cutoff_time = datetime.utcnow() - timedelta(days=older_than_days)
            
            # 构建删除查询
            delete_query = f"""
                DELETE FROM {database}
                WHERE time < '{cutoff_time.isoformat()}Z'
            """
            
            logger.info(f"清理 {older_than_days} 天前的旧数据")
            
            # 执行删除（如果InfluxDB 3支持）
            # 注意：某些版本可能不支持DELETE，需要通过retention policy管理
            logger.warning("InfluxDB 3 的数据清理通常通过retention policy自动管理")
            result["success"] = True
            result["error"] = "InfluxDB 3使用retention policy自动管理数据保留"
                
        except Exception as e:
            result["error"] = str(e)
            logger.error(f"清理旧数据失败: {e}")
        
        return result
    
    def test_connection(self) -> bool:
        """测试连接"""
        try:
            return influxdb_manager.is_connected()
        except Exception as e:
            logger.error(f"测试InfluxDB连接失败: {e}")
            return False

# 全局数据存储器实例
data_storage = DataStorage()
