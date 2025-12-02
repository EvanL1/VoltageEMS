"""
告警广播服务模块
处理告警数据并通过MQTT发布
"""

import json
import time
from typing import Dict, Any, Optional
from loguru import logger
from app.core.mqtt_client import mqtt_client
from app.core.device_identity import device_identity
from app.core.config_loader import config_loader

class AlarmBroadcaster:
    """告警广播服务"""
    
    def __init__(self):
        self.alarm_topic_template = None
        self.formatted_alarm_topic = None
        self._setup_topic()
        
    def _setup_topic(self):
        """设置告警主题"""
        try:
            # 获取告警主题配置模板
            self.alarm_topic_template = config_loader.get_config('mqtt_topics.alarm')
            
            if not self.alarm_topic_template:
                logger.error("告警主题配置缺失")
                return False
            
            # 格式化主题，替换占位符
            self.formatted_alarm_topic = device_identity.format_topic(self.alarm_topic_template)
            
            logger.info(f"告警广播服务初始化完成，发布主题: {self.formatted_alarm_topic}")
            return True
            
        except Exception as e:
            logger.error(f"设置告警主题失败: {e}")
            return False
    
    def broadcast_alarm(self, alarm_data: Dict[str, Any]) -> bool:
        """
        广播告警消息
        
        Args:
            alarm_data: 告警数据，必须是字典格式
            
        Returns:
            bool: 发送成功返回True，失败返回False
        """
        try:
            if not self.formatted_alarm_topic:
                logger.error("告警主题未正确配置，无法发送告警")
                return False
                
            if not isinstance(alarm_data, dict):
                logger.error(f"告警数据格式错误，必须是字典格式: {type(alarm_data)}")
                return False
            
            # 发布告警消息（使用QoS 1确保消息传输）
            # 注意：直接转发原始数据，不做任何修改
            success = mqtt_client.publish(
                topic=self.formatted_alarm_topic,
                payload=alarm_data,
                qos=1,
                retain=False  # 告警消息通常不需要保留
            )
            
            if success:
                logger.info(f"告警消息发送成功: {self.formatted_alarm_topic}")
                logger.debug(f"告警数据: {json.dumps(alarm_data, ensure_ascii=False, indent=2)}")
                return True
            else:
                logger.error(f"告警消息发送失败: {self.formatted_alarm_topic}")
                return False
                
        except Exception as e:
            logger.error(f"广播告警消息异常: {e}")
            return False
    
    def get_alarm_topic(self) -> Optional[str]:
        """获取当前使用的告警主题"""
        return self.formatted_alarm_topic
    
    def validate_alarm_data(self, data: Any) -> tuple[bool, str]:
        """
        验证告警数据格式
        
        Args:
            data: 要验证的数据
            
        Returns:
            tuple: (是否有效, 错误消息)
        """
        try:
            # 检查数据类型
            if not isinstance(data, dict):
                return False, f"数据必须是JSON对象格式，当前类型: {type(data).__name__}"
            
            # 检查是否为空
            if not data:
                return False, "告警数据不能为空"
            
            # 检查必需字段（可根据业务需求调整）
            # 这里暂时不设置必需字段，由调用方决定数据内容
            
            # 检查数据大小（避免消息过大）
            data_json = json.dumps(data, ensure_ascii=False)
            if len(data_json.encode('utf-8')) > 1024 * 100:  # 100KB限制
                return False, "告警数据过大，请控制在100KB以内"
            
            return True, ""
            
        except Exception as e:
            return False, f"数据验证异常: {e}"

# 全局告警广播服务实例
alarm_broadcaster = AlarmBroadcaster()
