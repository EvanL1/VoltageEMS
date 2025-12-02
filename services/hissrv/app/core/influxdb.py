"""
InfluxDB数据库连接模块
"""

from influxdb_client import InfluxDBClient, Point
from influxdb_client.client.write_api import SYNCHRONOUS, ASYNCHRONOUS
from typing import Optional, List, Dict, Any
from loguru import logger
from .config import settings
from .config_loader import config_loader
import asyncio

class InfluxDBManager:
    """InfluxDB连接管理器"""
    
    def __init__(self):
        self.client: Optional[InfluxDBClient] = None
        self.write_api = None
        self.query_api = None
        self._connect()
    
    def _connect(self):
        """建立InfluxDB连接"""
        try:
            # 优先从YAML配置文件读取，如果没有则使用settings的默认值
            influxdb_config = config_loader.get_influxdb_config()
            url = influxdb_config.get('url') or settings.INFLUXDB_URL
            token = influxdb_config.get('token') or settings.INFLUXDB_TOKEN
            org = influxdb_config.get('org') or settings.INFLUXDB_ORG
            timeout = influxdb_config.get('timeout', settings.INFLUXDB_TIMEOUT)
            
            self.client = InfluxDBClient(
                url=url,
                token=token,
                org=org,
                timeout=timeout * 1000  # 转换为毫秒
            )
            
            # 创建写入和查询API
            self.write_api = self.client.write_api(write_options=SYNCHRONOUS)
            self.query_api = self.client.query_api()
            
            # 测试连接
            health = self.client.health()
            if health.status == "pass":
                logger.info(f"InfluxDB连接成功: {url}")
                
                # 自动创建bucket
                self._ensure_bucket_exists()
            else:
                logger.error(f"InfluxDB健康检查失败: {health}")
                
        except Exception as e:
            logger.error(f"InfluxDB连接失败: {e}")
            self.client = None
            self.write_api = None
            self.query_api = None
    
    def _ensure_bucket_exists(self):
        """确保bucket存在，如果不存在则自动创建"""
        try:
            if not self.client:
                return False
            
            # 从配置文件读取配置
            influxdb_config = config_loader.get_influxdb_config()
            bucket_name = influxdb_config.get('bucket') or settings.INFLUXDB_BUCKET
            org_name = influxdb_config.get('org') or settings.INFLUXDB_ORG
            retention_policy = config_loader.get_retention_policy()
            retention_days = retention_policy.get('default_retention', '30d') if retention_policy.get('enabled') else '30d'
            # 解析保留天数（如 "30d" -> 30）
            retention_days_int = int(retention_days.replace('d', '')) if isinstance(retention_days, str) else settings.DATA_RETENTION_DAYS
            
            buckets_api = self.client.buckets_api()
            
            # 检查bucket是否存在
            buckets = buckets_api.find_buckets()
            bucket_names = [bucket.name for bucket in buckets.buckets] if buckets.buckets else []
            
            if bucket_name in bucket_names:
                logger.info(f"Bucket '{bucket_name}' 已存在")
                return True
            
            # bucket不存在，创建新的
            logger.info(f"创建bucket: {bucket_name}")
            
            from influxdb_client.domain.bucket import Bucket
            from influxdb_client.domain.bucket_retention_rules import BucketRetentionRules
            
            # 设置保留策略
            retention_rules = BucketRetentionRules(
                type="expire",
                every_seconds=retention_days_int * 24 * 3600
            )
            
            # 获取组织ID
            orgs_api = self.client.organizations_api()
            orgs_result = orgs_api.find_organizations()
            
            org_id = None
            # 处理不同版本的API返回格式
            orgs_list = orgs_result.orgs if hasattr(orgs_result, 'orgs') else orgs_result
            
            for org in orgs_list:
                if org.name == org_name:
                    org_id = org.id
                    break
            
            if not org_id:
                logger.error(f"找不到组织: {org_name}")
                return False
            
            bucket = Bucket(
                name=bucket_name,
                org_id=org_id,
                retention_rules=[retention_rules]
            )
            
            created_bucket = buckets_api.create_bucket(bucket=bucket)
            
            if created_bucket:
                logger.info(f"✅ Bucket '{bucket_name}' 创建成功")
                logger.info(f"📅 保留策略: {retention_days_int}天")
                return True
            else:
                logger.error(f"❌ 创建bucket '{bucket_name}' 失败")
                return False
                
        except Exception as e:
            if "already exists" in str(e):
                bucket_name = influxdb_config.get('bucket') or settings.INFLUXDB_BUCKET
                logger.info(f"Bucket '{bucket_name}' 已存在")
                return True
            else:
                logger.error(f"确保bucket存在时出错: {e}")
                return False
    
    def get_client(self) -> Optional[InfluxDBClient]:
        """获取InfluxDB客户端"""
        if self.client is None:
            self._connect()
        return self.client
    
    def is_connected(self) -> bool:
        """检查连接状态"""
        try:
            if self.client:
                health = self.client.health()
                return health.status == "pass"
        except Exception as e:
            logger.warning(f"InfluxDB连接检查失败: {e}")
        return False
    
    def reconnect(self):
        """重新连接"""
        logger.info("尝试重新连接InfluxDB...")
        self.close()
        self._connect()
    
    def close(self):
        """关闭连接"""
        if self.client:
            self.client.close()
            logger.info("InfluxDB连接已关闭")
    
    def _get_bucket_name(self) -> str:
        """获取bucket名称"""
        influxdb_config = config_loader.get_influxdb_config()
        return influxdb_config.get('bucket') or settings.INFLUXDB_BUCKET
    
    def write_point(self, point: Point) -> bool:
        """写入单个数据点"""
        try:
            if self.write_api:
                self.write_api.write(bucket=self._get_bucket_name(), record=point)
                return True
        except Exception as e:
            logger.error(f"写入数据点失败: {e}")
        return False
    
    def write_points(self, points: List[Point]) -> bool:
        """批量写入数据点"""
        try:
            if self.write_api and points:
                self.write_api.write(bucket=self._get_bucket_name(), record=points)
                return True
        except Exception as e:
            logger.error(f"批量写入数据点失败: {e}")
        return False
    
    def query_data(self, query: str) -> List[Dict[str, Any]]:
        """查询数据"""
        try:
            if self.query_api:
                result = self.query_api.query(query)
                data = []
                for table in result:
                    for record in table.records:
                        data.append(record.values)
                return data
        except Exception as e:
            logger.error(f"查询数据失败: {e}")
        return []
    
    def create_point(self, measurement: str, tags: Dict[str, str], 
                    fields: Dict[str, Any], timestamp=None) -> Point:
        """创建数据点"""
        point = Point(measurement)
        
        # 添加标签
        for key, value in tags.items():
            point.tag(key, value)
        
        # 添加字段
        for key, value in fields.items():
            point.field(key, value)
        
        # 添加时间戳
        if timestamp:
            point.time(timestamp)
        
        return point
    
    def get_bucket_info(self) -> Optional[Dict[str, Any]]:
        """获取存储桶信息"""
        try:
            if self.client:
                bucket_name = self._get_bucket_name()
                buckets_api = self.client.buckets_api()
                buckets = buckets_api.find_buckets()
                for bucket in buckets.buckets:
                    if bucket.name == bucket_name:
                        return {
                            "name": bucket.name,
                            "id": bucket.id,
                            "retention_rules": bucket.retention_rules,
                            "created_at": bucket.created_at,
                            "updated_at": bucket.updated_at
                        }
        except Exception as e:
            logger.error(f"获取存储桶信息失败: {e}")
        return None

# 全局InfluxDB管理器实例
influxdb_manager = InfluxDBManager()
