"""
数据收集服务
定时从Redis获取数据
"""

import re
import json
from typing import Dict, List, Optional, Any, Tuple
from datetime import datetime
from loguru import logger

from ..core.database import redis_manager
from ..core.config_loader import config_loader
from ..models.data_models import RedisDataPoint, HistoryData

class DataCollector:
    """数据收集器"""
    
    def __init__(self):
        self.redis_client = redis_manager.get_client()
        self.subscribe_patterns = config_loader.get_subscribe_patterns()
        self.exclude_patterns = config_loader.get_config('redis_source.filters.exclude_patterns', [])
        
    def parse_redis_key(self, key: str) -> Dict[str, str]:
        """解析Redis键 - 简化版本，直接使用Redis键"""
        try:
            # 按冒号分割键，取第一部分作为source
            key_parts = key.split(':')
            source = key_parts[0] if key_parts else "unknown"
            
            # 直接返回键的信息，不做复杂判断
            return {
                "source": source,
                "channel_id": key  # 直接使用完整的Redis键作为channel_id
            }
                
        except Exception as e:
            logger.error(f"解析Redis键失败: {key}, {e}")
            return {
                "source": "unknown",
                "channel_id": key
            }
    
    def should_exclude_key(self, key: str) -> bool:
        """检查是否应该排除该键"""
        for pattern in self.exclude_patterns:
            if re.match(pattern.replace('*', '.*'), key):
                return True
        return False
    
    def collect_data_from_pattern(self, pattern: str) -> List[RedisDataPoint]:
        """根据模式收集数据"""
        data_points = []
        
        try:
            if not self.redis_client:
                logger.error("Redis客户端未连接")
                return data_points
            
            # 获取匹配的键
            keys = self.redis_client.keys(pattern)
            logger.debug(f"模式 {pattern} 匹配到 {len(keys)} 个键")
            
            for key in keys:
                # 检查是否应该排除
                if self.should_exclude_key(key):
                    continue
                
                # 解析键 - 现在总是返回结果，不再跳过
                parsed_key = self.parse_redis_key(key)
                
                try:
                    # 获取Hash数据
                    hash_data = self.redis_client.hgetall(key)
                    if not hash_data:
                        continue
                    
                    # 获取时间戳
                    timestamp_str = hash_data.pop('_timestamp', None) or hash_data.pop('__updated', None)
                    timestamp = None
                    if timestamp_str:
                        try:
                            timestamp = datetime.fromtimestamp(float(timestamp_str))
                        except (ValueError, TypeError):
                            timestamp = datetime.utcnow()
                    else:
                        timestamp = datetime.utcnow()
                    
                    # 处理每个字段
                    for field, value in hash_data.items():
                        # 跳过以下划线开头的系统字段
                        if field.startswith('_'):
                            continue
                            
                        data_point = RedisDataPoint(
                            key=key,
                            field=field,
                            value=self._convert_value(value),
                            timestamp=timestamp
                        )
                        data_points.append(data_point)
                        
                except Exception as e:
                    logger.error(f"处理键 {key} 失败: {e}")
                    continue
                    
        except Exception as e:
            logger.error(f"收集模式 {pattern} 数据失败: {e}")
            
        return data_points
    
    def _convert_value(self, value: Any) -> Any:
        """转换数值"""
        if isinstance(value, str):
            # 尝试转换为数字
            try:
                if '.' in value:
                    return float(value)
                else:
                    return int(value)
            except (ValueError, TypeError):
                # 尝试转换布尔值
                if value.lower() in ('true', 'false'):
                    return value.lower() == 'true'
                # 保持字符串
                return value
        return value
    
    def collect_all_data(self) -> List[HistoryData]:
        """收集所有数据"""
        all_history_data = []
        
        logger.info(f"开始收集数据，监听 {len(self.subscribe_patterns)} 个模式")
        
        for pattern in self.subscribe_patterns:
            try:
                # 收集数据点
                data_points = self.collect_data_from_pattern(pattern)
                logger.debug(f"模式 {pattern} 收集到 {len(data_points)} 个数据点")
                
                # 转换为历史数据
                for data_point in data_points:
                    parsed_key = self.parse_redis_key(data_point.key)
                    if parsed_key:
                        history_data = data_point.to_history_data(parsed_key)
                        all_history_data.append(history_data)
                        
            except Exception as e:
                logger.error(f"处理模式 {pattern} 失败: {e}")
                continue
        
        logger.info(f"总共收集到 {len(all_history_data)} 条历史数据")
        return all_history_data
    
    def get_latest_data(self, channel_id: str, data_type: str = None) -> Optional[Dict[str, Any]]:
        """获取最新数据 - channel_id就是完整的Redis键"""
        try:
            if not self.redis_client:
                return None
            
            # 如果channel_id是完整的Redis键，直接获取
            if ':' in channel_id:
                try:
                    data = self.redis_client.hgetall(channel_id)
                    if data:
                        data['_key'] = channel_id
                        data['_channel_id'] = channel_id
                        return data
                except Exception as e:
                    logger.debug(f"直接获取键 {channel_id} 失败: {e}")
            
            # 否则尝试在所有订阅模式中查找包含channel_id的键
            for pattern in self.subscribe_patterns:
                try:
                    keys = self.redis_client.keys(pattern)
                    for key in keys:
                        if channel_id in key:
                            data = self.redis_client.hgetall(key)
                            if data:
                                data['_key'] = key
                                data['_channel_id'] = key
                                return data
                except Exception as e:
                    logger.debug(f"模式匹配失败: {e}")
                    continue
            
            return None
            
        except Exception as e:
            logger.error(f"获取最新数据失败: {channel_id}, {e}")
            return None
    
    def get_channel_list(self) -> List[str]:
        """获取通道列表"""
        channels = set()
        
        try:
            if not self.redis_client:
                return list(channels)
            
            # 扫描所有模式的键
            for pattern in self.subscribe_patterns:
                keys = self.redis_client.keys(pattern)
                for key in keys:
                    parsed = self.parse_redis_key(key)
                    if parsed and 'channel_id' in parsed:
                        channels.add(parsed['channel_id'])
                        
        except Exception as e:
            logger.error(f"获取通道列表失败: {e}")
            
        return sorted(list(channels))
    
    def test_connection(self) -> bool:
        """测试连接"""
        try:
            return self.redis_client and redis_manager.is_connected()
        except Exception as e:
            logger.error(f"测试Redis连接失败: {e}")
            return False

# 全局数据收集器实例
data_collector = DataCollector()
