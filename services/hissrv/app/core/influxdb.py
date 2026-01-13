"""
InfluxDB 3.x 数据库连接模块
"""

from influxdb_client_3 import InfluxDBClient3, Point, WriteOptions
from typing import Optional, List, Dict, Any
from loguru import logger
from .config import settings
from .config_loader import config_loader

class InfluxDBManager:
    """InfluxDB 3.x 连接管理器"""
    
    def __init__(self):
        self.client: Optional[InfluxDBClient3] = None
        self._database = None
        self._connect()
    
    def _connect(self):
        """建立InfluxDB 3.x连接"""
        try:
            # 从配置文件读取配置
            influxdb_config = config_loader.get_influxdb_config()
            url = influxdb_config.get('url') or settings.INFLUXDB_URL
            token = influxdb_config.get('token') or settings.INFLUXDB_TOKEN
            database = config_loader.get_database_name()
            
            # 解析URL，提取host和port
            # 支持格式: http://host:port 或 https://host:port
            import re
            url_pattern = r'^(https?://)?([^:/]+)(?::(\d+))?'
            match = re.match(url_pattern, url)
            
            if match:
                protocol = match.group(1) or 'http://'
                host = match.group(2)
                port = match.group(3)
                
                # InfluxDB 3客户端需要完整的URL格式（包括协议和端口）
                if port:
                    full_url = f"{protocol}{host}:{port}"
                else:
                    # 如果没有指定端口，使用默认端口
                    default_port = '443' if 'https' in protocol else '80'
                    full_url = f"{protocol}{host}:{default_port}"
            else:
                # 如果无法解析，直接使用原始URL
                full_url = url
            
            logger.info(f"InfluxDB 3 连接参数 - URL: {full_url}, Database: {database}")
            
            # 初始化InfluxDB 3客户端
            # 注意：根据 influxdb3-python 的版本，参数名可能是 host 或 url
            try:
                self.client = InfluxDBClient3(
                    host=full_url,
                    token=token,
                    database=database
                )
            except TypeError:
                # 如果 host 参数不工作，尝试使用其他参数名
                self.client = InfluxDBClient3(
                    url=full_url,
                    token=token,
                    database=database
                )
            
            self._database = database
            
            # 测试连接
            try:
                # InfluxDB 3客户端使用简单的查询测试连接
                self.client.query(f"SELECT * FROM {database} LIMIT 1")
                logger.info(f"InfluxDB 3.x 连接成功: {full_url}, database: {database}")
            except Exception as test_error:
                # 如果查询失败（比如表不存在），也算连接成功
                logger.info(f"InfluxDB 3.x 连接成功: {full_url}, database: {database}")
                logger.debug(f"初始查询测试: {test_error}")
                
        except Exception as e:
            logger.error(f"InfluxDB 3.x 连接失败: {e}")
            logger.exception("详细错误信息:")
            self.client = None
            self._database = None
    
    def get_client(self) -> Optional[InfluxDBClient3]:
        """获取InfluxDB客户端"""
        if self.client is None:
            self._connect()
        return self.client
    
    def is_connected(self) -> bool:
        """检查连接状态"""
        try:
            if self.client and self._database:
                # 尝试执行简单查询测试连接
                self.client.query(f"SELECT 1")
                return True
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
            try:
                self.client.close()
                logger.info("InfluxDB连接已关闭")
            except Exception as e:
                logger.warning(f"关闭InfluxDB连接时出错: {e}")
            finally:
                self.client = None
    
    def get_database_name(self) -> str:
        """获取当前database名称"""
        return self._database or config_loader.get_database_name()
    
    def write_point(self, point: Point) -> bool:
        """写入单个数据点"""
        try:
            if self.client:
                self.client.write(record=point)
                return True
        except Exception as e:
            logger.error(f"写入数据点失败: {e}")
        return False
    
    def write_points(self, points: List[Point]) -> bool:
        """批量写入数据点"""
        try:
            if self.client and points:
                self.client.write(record=points)
                return True
        except Exception as e:
            logger.error(f"批量写入数据点失败: {e}")
        return False
    
    def query_data(self, query: str) -> List[Dict[str, Any]]:
        """执行SQL查询并返回结果"""
        try:
            if self.client:
                # InfluxDB 3使用SQL查询，返回结果为PyArrow Table
                result = self.client.query(query=query)
                
                # 处理不同类型的返回结果
                if result is None:
                    return []
                
                # 检查是否是 PyArrow Table
                if hasattr(result, 'to_pydict'):
                    # PyArrow Table转换为字典列表
                    # to_pydict() 返回列名->值列表的字典，需要转换为行记录列表
                    pydict = result.to_pydict()
                    if not pydict:
                        return []
                    
                    # 将列式数据转换为行式数据
                    num_rows = len(next(iter(pydict.values())))
                    records = []
                    for i in range(num_rows):
                        record = {key: values[i] for key, values in pydict.items()}
                        records.append(record)
                    return records
                    
                # pandas DataFrame 格式
                elif hasattr(result, 'to_dict'):
                    # 转换为records格式的字典列表
                    return result.to_dict('records')
                    
                # 已经是列表格式
                elif isinstance(result, list):
                    return result
                    
                else:
                    logger.warning(f"未知的查询结果格式: {type(result)}")
                    return []
        except Exception as e:
            logger.error(f"查询数据失败: {e}")
        return []
    
    def create_point(self, measurement: str, tags: Dict[str, str], 
                    fields: Dict[str, Any], timestamp=None) -> Point:
        """创建数据点"""
        point = Point(measurement)
        
        # 添加标签
        for key, value in tags.items():
            point = point.tag(key, str(value))
        
        # 添加字段
        for key, value in fields.items():
            point = point.field(key, value)
        
        # 添加时间戳
        if timestamp:
            point = point.time(timestamp)
        
        return point
    
    def get_database_info(self) -> Optional[Dict[str, Any]]:
        """获取数据库信息"""
        try:
            if self.client:
                # InfluxDB 3中可以通过查询系统表获取信息
                # 这里返回基本信息
                return {
                    "name": self._database,
                    "type": "InfluxDB 3.x",
                    "connected": self.is_connected()
                }
        except Exception as e:
            logger.error(f"获取数据库信息失败: {e}")
        return None

# 全局InfluxDB管理器实例
influxdb_manager = InfluxDBManager()
