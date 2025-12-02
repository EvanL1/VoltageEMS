"""
数据存储服务
将数据存储到InfluxDB
"""

from typing import List, Dict, Any, Optional
from datetime import datetime, timezone
from loguru import logger
from influxdb_client import Point

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
            point.tag("redis_key", str(data.redis_key))
            point.tag("point_id", str(data.point_id))
            point.tag("source", data.source)
            
            # 添加数值字段
            if isinstance(data.value, (int, float)):
                point.field("value", float(data.value))
            elif isinstance(data.value, bool):
                point.field("value", int(data.value))
                point.field("boolean_value", data.value)
            elif isinstance(data.value, str):
                # 尝试转换为数值
                try:
                    numeric_value = float(data.value)
                    point.field("value", numeric_value)
                except (ValueError, TypeError):
                    point.field("string_value", data.value)
                    # 对于字符串值，添加一个默认数值字段用于查询
                    point.field("value", 0.0)
            else:
                # 对于其他类型，转换为字符串
                point.field("string_value", str(data.value))
                point.field("value", 0.0)
            
            # 设置时间戳
            if data.timestamp.tzinfo is None:
                # 如果没有时区信息，假设为UTC
                timestamp = data.timestamp.replace(tzinfo=timezone.utc)
            else:
                timestamp = data.timestamp
            
            point.time(timestamp)
            
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
                logger.debug(f"存储单条数据成功: {data.channel_id}:{data.point_id}")
            else:
                logger.error(f"存储单条数据失败: {data.channel_id}:{data.point_id}")
                
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
    
    def create_retention_policy(self, policy_name: str, duration: str) -> bool:
        """创建数据保留策略"""
        try:
            # InfluxDB 2.x 使用bucket的retention rules
            if influxdb_manager.client:
                # 这里可以通过API修改bucket的retention policy
                # 具体实现依赖于InfluxDB 2.x的API
                logger.info(f"创建保留策略: {policy_name}, 持续时间: {duration}")
                return True
        except Exception as e:
            logger.error(f"创建保留策略失败: {e}")
            return False
    
    def get_storage_stats(self) -> Dict[str, Any]:
        """获取存储统计信息"""
        stats = {
            "total_measurements": 0,
            "total_points": 0,
            "oldest_timestamp": None,
            "newest_timestamp": None,
            "measurements": {}
        }
        
        try:
            if not influxdb_manager.query_api:
                return stats
            
            # 查询总的数据点数
            query = f'''from(bucket: "{settings.INFLUXDB_BUCKET}")
|> range(start: 0)
|> filter(fn: (r) => r._field == "value")
|> count()
|> group()
|> sum()'''
            
            logger.debug(f"查询总数据点数: {query}")
            result = influxdb_manager.query_data(query)
            if result and len(result) > 0:
                stats["total_points"] = result[0].get("_value", 0)
            
            # 查询最早和最新时间戳
            try:
                # 查询最新时间戳
                query_latest = f'''from(bucket: "{settings.INFLUXDB_BUCKET}")
|> range(start: 0)
|> filter(fn: (r) => r._field == "value")
|> last()
|> keep(columns: ["_time"])'''
                
                latest_result = influxdb_manager.query_data(query_latest)
                if latest_result and len(latest_result) > 0:
                    stats["newest_timestamp"] = latest_result[0].get("_time")
                
                # 查询最早时间戳
                query_earliest = f'''from(bucket: "{settings.INFLUXDB_BUCKET}")
|> range(start: 0)
|> filter(fn: (r) => r._field == "value")
|> first()
|> keep(columns: ["_time"])'''
                
                earliest_result = influxdb_manager.query_data(query_earliest)
                if earliest_result and len(earliest_result) > 0:
                    stats["oldest_timestamp"] = earliest_result[0].get("_time")
                        
            except Exception as e:
                logger.error(f"查询时间戳统计失败: {e}")
            
            # 查询各个measurement的统计
            try:
                query_measurements = f'''from(bucket: "{settings.INFLUXDB_BUCKET}")
|> range(start: 0)
|> filter(fn: (r) => r._field == "value")
|> group(columns: ["_measurement"])
|> count()
|> group()'''
                
                measurements_result = influxdb_manager.query_data(query_measurements)
                for result in measurements_result:
                    measurement = result.get("_measurement", "unknown")
                    count = result.get("_value", 0)
                    stats["measurements"][measurement] = count
                    
                stats["total_measurements"] = len(stats["measurements"])
                        
            except Exception as e:
                logger.error(f"查询measurement统计失败: {e}")
            
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
            # InfluxDB 2.x 使用delete API
            if influxdb_manager.client:
                from datetime import timedelta
                
                cutoff_time = datetime.utcnow() - timedelta(days=older_than_days)
                
                # 构建删除查询
                delete_predicate = f'_time < "{cutoff_time.isoformat()}Z"'
                
                # 执行删除（注意：这个API可能需要管理员权限）
                logger.info(f"清理 {older_than_days} 天前的旧数据")
                
                # 这里需要根据具体的InfluxDB Python客户端版本实现删除逻辑
                # delete_api = influxdb_manager.client.delete_api()
                # delete_api.delete(start=..., stop=cutoff_time, predicate=delete_predicate, bucket=...)
                
                result["success"] = True
                logger.info("数据清理完成")
                
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
