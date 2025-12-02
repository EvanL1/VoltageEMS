"""
Edge数据客户端
从Redis获取Edge设备数据
"""

import json
import logging
from typing import Dict, List, Any, Optional, Tuple, Union
from datetime import datetime
import redis.asyncio as redis

from app.models.edge_data import (
    ComsrvData, ModsrvModel, ModsrvMeasurement, 
    ModsrvAction, AlarmRecord, RuleDefinition
)

logger = logging.getLogger(__name__)


def round_float_value(value: float, decimal_places: int = 4) -> float:
    """限制浮点数的小数位数
    
    Args:
        value: 要处理的浮点数
        decimal_places: 保留的小数位数，默认4位
        
    Returns:
        限制小数位数后的浮点数
    """
    return round(value, decimal_places)


class EdgeDataClient:
    """Edge数据客户端"""
    
    def __init__(self, redis_client: redis.Redis):
        self.redis_client = redis_client
        
    async def get_data(self, channel_id: int, data_type: str, source: str = "inst") -> Dict[str, Any]:
        """获取数据源数据
        
        Args:
            channel_id: 通道ID
            data_type: 数据类型字符串 (如 T/S/C/A/M 等，无限制)
            source: 数据源名称，默认"inst"
            
        Returns:
            通道数据字典
        """
        try:
            # data_type 直接使用字符串，无需转换
            key = f"{source}:{channel_id}:{data_type}"
            
            # 首先检查键的类型
            key_type = await self.redis_client.type(key)
            
            if key_type == b'none':
                logger.debug(f"键不存在: {key}")
                return {}
            
            # 根据键类型使用不同的读取方法
            logger.debug(f"键 {key} 的类型: {key_type} (类型: {type(key_type)})")
            
            # 转换为字符串进行比较
            key_type_str = key_type.decode('utf-8') if isinstance(key_type, bytes) else str(key_type)
            
            if key_type_str == 'hash':
                # 使用HGETALL读取hash类型
                logger.info(f"使用HGETALL读取hash类型数据: {key}")
                data = await self.redis_client.hgetall(key)
            elif key_type_str == 'string':
                # 使用GET读取string类型，然后尝试解析JSON
                logger.info(f"使用GET读取string类型数据: {key}")
                raw_data = await self.redis_client.get(key)
                if raw_data:
                    try:
                        data = json.loads(raw_data)
                        if not isinstance(data, dict):
                            logger.warning(f"键 {key} 的数据不是字典格式")
                            return {}
                    except json.JSONDecodeError:
                        logger.error(f"键 {key} 的JSON数据解析失败")
                        return {}
                else:
                    return {}
            elif key_type_str == 'none':
                logger.debug(f"键不存在: {key}")
                return {}
            else:
                logger.warning(f"不支持的键类型: {key_type_str} for key: {key}")
                return {}
            
            if not data:
                return {}
            
            # 转换数据类型，确保键和值都是字符串
            result = {}
            for point_id, value in data.items():
                try:
                    # 确保键是字符串
                    str_point_id = point_id.decode('utf-8') if isinstance(point_id, bytes) else str(point_id)
                    # 确保值是字符串
                    str_value = value.decode('utf-8') if isinstance(value, bytes) else str(value)
                    
                    # 尝试转换为数值
                    if '.' in str_value:
                        result[str_point_id] = round_float_value(float(str_value))
                    else:
                        try:
                            result[str_point_id] = int(str_value)
                        except ValueError:
                            result[str_point_id] = str_value
                except (ValueError, AttributeError) as e:
                    logger.warning(f"数据类型转换失败 {point_id}={value}: {e}")
                    # 保持原始字符串值
                    str_point_id = point_id.decode('utf-8') if isinstance(point_id, bytes) else str(point_id)
                    str_value = value.decode('utf-8') if isinstance(value, bytes) else str(value)
                    result[str_point_id] = str_value
            
            return result
            
        except Exception as e:
            logger.error(f"获取数据源[{source}]数据失败 {key}: {e}")
            return {}
    
    async def get_comsrv_data(self, channel_id: int, data_type: str) -> Dict[str, Any]:
        """获取通信服务数据（向后兼容方法）
        
        Args:
            channel_id: 通道ID
            data_type: 数据类型 (T/S/C/A)
            
        Returns:
            通道数据字典
        """
        return await self.get_data(channel_id, data_type, source="comsrv")
    
    async def get_modsrv_model(self, model_id: str) -> Optional[ModsrvModel]:
        """获取模型定义
        
        Args:
            model_id: 模型ID
            
        Returns:
            模型定义对象
        """
        try:
            key = f"modsrv:model:{model_id}"
            data = await self.redis_client.get(key)
            
            if not data:
                return None
            
            model_data = json.loads(data)
            return ModsrvModel(**model_data)
            
        except Exception as e:
            logger.error(f"获取模型定义失败: {e}")
            return None
    
    async def get_modsrv_measurement(self, model_id: str) -> Optional[ModsrvMeasurement]:
        """获取模型测量值
        
        Args:
            model_id: 模型ID
            
        Returns:
            模型测量值对象
        """
        try:
            key = f"modsrv:model:{model_id}:measurement"
            data = await self.redis_client.hgetall(key)
            
            if not data:
                return None
            
            # 转换数据类型
            values = {}
            updated = 0
            
            for key, value in data.items():
                if key == "__updated":
                    updated = int(value)
                else:
                    try:
                        if '.' in value:
                            values[key] = round_float_value(float(value))
                        else:
                            values[key] = int(value)
                    except ValueError:
                        values[key] = value
            
            return ModsrvMeasurement(
                model_id=model_id,
                values=values,
                updated=updated
            )
            
        except Exception as e:
            logger.error(f"获取模型测量值失败: {e}")
            return None
    
    async def get_modsrv_action(self, model_id: str) -> Optional[ModsrvAction]:
        """获取模型控制值
        
        Args:
            model_id: 模型ID
            
        Returns:
            模型控制值对象
        """
        try:
            key = f"modsrv:model:{model_id}:action"
            data = await self.redis_client.hgetall(key)
            
            if not data:
                return None
            
            # 转换数据类型
            values = {}
            updated = 0
            
            for key, value in data.items():
                if key == "__updated":
                    updated = int(value)
                else:
                    try:
                        if '.' in value:
                            values[key] = round_float_value(float(value))
                        else:
                            values[key] = int(value)
                    except ValueError:
                        values[key] = value
            
            return ModsrvAction(
                model_id=model_id,
                values=values,
                updated=updated
            )
            
        except Exception as e:
            logger.error(f"获取模型控制值失败: {e}")
            return None
    
    async def get_alarm_record(self, alarm_id: str) -> Optional[AlarmRecord]:
        """获取告警记录
        
        Args:
            alarm_id: 告警ID
            
        Returns:
            告警记录对象
        """
        try:
            key = f"alarmsrv:{alarm_id}"
            data = await self.redis_client.hgetall(key)
            
            if not data:
                return None
            
            # 转换数据类型
            alarm_data = {}
            for key, value in data.items():
                if key in ["timestamp", "acknowledged_at"]:
                    alarm_data[key] = int(value) if value else None
                elif key == "acknowledged":
                    alarm_data[key] = value.lower() == "true"
                else:
                    alarm_data[key] = value
            
            return AlarmRecord(**alarm_data)
            
        except Exception as e:
            logger.error(f"获取告警记录失败: {e}")
            return None
    
    async def get_active_alarms(self) -> List[AlarmRecord]:
        """获取所有活跃告警
        
        Returns:
            活跃告警列表
        """
        try:
            key = "alarmsrv:status:Active"
            alarm_ids = await self.redis_client.smembers(key)
            
            alarms = []
            for alarm_id in alarm_ids:
                alarm = await self.get_alarm_record(alarm_id)
                if alarm:
                    alarms.append(alarm)
            
            return alarms
            
        except Exception as e:
            logger.error(f"获取活跃告警失败: {e}")
            return []
    
    async def get_rule_definition(self, rule_id: str) -> Optional[RuleDefinition]:
        """获取规则定义
        
        Args:
            rule_id: 规则ID
            
        Returns:
            规则定义对象
        """
        try:
            key = f"rulesrv:rule:{rule_id}"
            data = await self.redis_client.get(key)
            
            if not data:
                return None
            
            rule_data = json.loads(data)
            return RuleDefinition(**rule_data)
            
        except Exception as e:
            logger.error(f"获取规则定义失败: {e}")
            return None
    
    async def get_all_channels(self, source: str = "inst", data_type: str = "M") -> List[int]:
        """获取所有通道ID
        
        Args:
            source: 数据源名称，默认"inst"
            data_type: 数据类型，用于模式匹配，默认"M"
        
        Returns:
            通道ID列表
        """
        try:
            pattern = f"{source}:*:{data_type}"  # 以指定数据类型为基准获取通道
            keys = await self.redis_client.keys(pattern)
            
            channels = []
            for key in keys:
                # 解析key: source:channel_id:data_type
                parts = key.decode('utf-8') if isinstance(key, bytes) else key
                parts = parts.split(":")
                if len(parts) >= 2:
                    try:
                        channel_id = int(parts[1])
                        if channel_id not in channels:
                            channels.append(channel_id)
                    except ValueError:
                        continue
            
            return sorted(channels)
            
        except Exception as e:
            logger.error(f"获取数据源[{source}]所有通道失败: {e}")
            return []
    
    async def get_channel_data_summary(self, channel_id: int, source: str = "inst", data_types: List[str] = None) -> Dict[str, Any]:
        """获取通道数据摘要
        
        Args:
            channel_id: 通道ID
            source: 数据源名称，默认"inst"
            data_types: 数据类型列表，默认为 ["T", "S", "C", "A"]
            
        Returns:
            通道数据摘要
        """
        try:
            if data_types is None:
                data_types = ["T", "S", "C", "A"]  # 默认常用类型
            
            summary = {
                "channel_id": channel_id,
                "source": source,
            }
            
            # 获取各种类型的数据
            for data_type in data_types:
                data = await self.get_data(channel_id, data_type, source)
                if data:
                    summary[data_type] = data
            
            return summary
            
        except Exception as e:
            logger.error(f"获取数据源[{source}]通道数据摘要失败: {e}")
            return {"channel_id": channel_id, "source": source}
    
    async def get_model_by_channel_point(self, channel_id: int, point_id: int, is_action: bool = False) -> Optional[str]:
        """根据通道和点位获取模型ID
        
        Args:
            channel_id: 通道ID
            point_id: 点位ID
            is_action: 是否为控制点
            
        Returns:
            模型ID和点位名称
        """
        try:
            if is_action:
                key = f"modsrv:reverse:action:{channel_id}:{point_id}"
            else:
                key = f"modsrv:reverse:{channel_id}:{point_id}"
            
            value = await self.redis_client.get(key)
            return value
            
        except Exception as e:
            logger.error(f"获取模型映射失败: {e}")
            return None
    
    async def get_models_by_template(self, template_name: str) -> List[str]:
        """根据模板获取模型列表
        
        Args:
            template_name: 模板名称
            
        Returns:
            模型ID列表
        """
        try:
            key = f"modsrv:models:by_template:{template_name}"
            models = await self.redis_client.smembers(key)
            return list(models)
            
        except Exception as e:
            logger.error(f"获取模板模型失败: {e}")
            return []
    
    async def get_command_queue(self, channel_id: int, data_type: str, source: str = "inst") -> List[Dict[str, Any]]:
        """获取命令队列
        
        Args:
            channel_id: 通道ID
            data_type: 数据类型
            source: 数据源名称，默认"inst"
            
        Returns:
            命令列表
        """
        try:
            key = f"{source}:trigger:{channel_id}:{data_type}"
            commands = []
            
            # 获取队列中的所有命令
            while True:
                command = await self.redis_client.lpop(key)
                if not command:
                    break
                
                try:
                    command_data = json.loads(command)
                    commands.append(command_data)
                except json.JSONDecodeError:
                    logger.warning(f"解析命令数据失败: {command}")
                    continue
            
            return commands
            
        except Exception as e:
            logger.error(f"获取数据源[{source}]命令队列失败: {e}")
            return []
    
    async def publish_command(self, channel_id: int, data_type: str, command_data: Dict[str, Any], source: str = "inst") -> bool:
        """发布命令到队列
        
        Args:
            channel_id: 通道ID
            data_type: 数据类型
            command_data: 命令数据
            source: 数据源名称，默认"inst"
            
        Returns:
            是否成功
        """
        try:
            key = f"{source}:trigger:{channel_id}:{data_type}"
            command_json = json.dumps(command_data)
            await self.redis_client.rpush(key, command_json)
            return True
            
        except Exception as e:
            logger.error(f"发布命令到数据源[{source}]失败: {e}")
            return False
