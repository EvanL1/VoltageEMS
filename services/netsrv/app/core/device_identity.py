"""
设备标识管理器
用于读取和管理设备序列号、产品序列号等标识信息
"""

import os
from pathlib import Path
from typing import Optional
from loguru import logger
from .config_loader import config_loader

class DeviceIdentityManager:
    """设备标识管理器"""
    
    def __init__(self):
        self.product_sn: str = ""
        self.device_sn: str = ""
        self.device_type: str = ""
        self.is_gateway: bool = False
        self._load_identity()
    
    def _load_identity(self):
        """加载设备标识信息"""
        try:
            # 从配置文件加载产品序列号
            self.product_sn = config_loader.get_config('device_identity.product_sn', 'voltageems_netsrv')
            self.device_type = config_loader.get_config('device_identity.device_type', 'gateway')
            self.is_gateway = config_loader.get_config('device_identity.is_gateway', True)
            
            # 获取设备序列号
            config_device_sn = config_loader.get_config('device_identity.device_sn', 'auto')
            
            if config_device_sn == 'auto':
                self.device_sn = self._read_device_serial_number()
            else:
                self.device_sn = config_device_sn
            
            logger.info(f"设备标识加载完成:")
            logger.info(f"  产品序列号: {self.product_sn}")
            logger.info(f"  设备序列号: {self.device_sn}")
            logger.info(f"  设备类型: {self.device_type}")
            logger.info(f"  是否网关: {self.is_gateway}")
            
        except Exception as e:
            logger.error(f"加载设备标识失败: {e}")
            # 使用默认值
            self.product_sn = "voltageems_netsrv"
            self.device_sn = "dev_001"
            self.device_type = "gateway"
            self.is_gateway = True
    
    def _read_device_serial_number(self) -> str:
        """读取设备序列号"""
        try:
            # 尝试从配置文件读取
            config_device_sn = config_loader.get_device_identity_config().get('device_sn', '')
            if config_device_sn and config_device_sn != 'auto':
                logger.info(f"从配置文件读取设备序列号: {config_device_sn}")
                return config_device_sn
            
            # 尝试从设备树读取
            device_tree_path = "/proc/device-tree/serial-number"
            if os.path.exists(device_tree_path):
                with open(device_tree_path, 'r') as f:
                    device_sn = f.read().strip()
                    if device_sn:
                        logger.info(f"从设备树读取设备序列号: {device_sn}")
                        return device_sn
            
            # 尝试从环境变量读取
            env_device_sn = os.environ.get('DEVICE_SN', '')
            if env_device_sn:
                logger.info(f"从环境变量读取设备序列号: {env_device_sn}")
                return env_device_sn
            
            # 尝试从Docker容器ID生成
            if os.path.exists('/proc/self/cgroup'):
                try:
                    with open('/proc/self/cgroup', 'r') as f:
                        for line in f:
                            if 'docker' in line:
                                # 提取容器ID的最后12位作为设备序列号
                                container_id = line.strip().split('/')[-1]
                                if len(container_id) >= 12:
                                    device_sn = f"container_{container_id[-12:]}"
                                    logger.info(f"从容器ID生成设备序列号: {device_sn}")
                                    return device_sn
                except Exception as e:
                    logger.debug(f"读取容器ID失败: {e}")
            
            # 使用开发环境固定值
            logger.warning("无法读取设备序列号，使用开发环境固定值")
            return "dev_001"
            
        except Exception as e:
            logger.error(f"读取设备序列号异常: {e}")
            return "dev_001"
    
    def get_product_sn(self) -> str:
        """获取产品序列号"""
        return self.product_sn
    
    def get_device_sn(self) -> str:
        """获取设备序列号"""
        return self.device_sn
    
    def get_device_type(self) -> str:
        """获取设备类型"""
        return self.device_type
    
    def is_gateway_device(self) -> bool:
        """是否为网关设备"""
        return self.is_gateway
    
    def format_topic(self, topic_template: str) -> str:
        """格式化MQTT主题，替换占位符"""
        try:
            formatted_topic = topic_template.replace('{productSN}', self.product_sn)
            formatted_topic = formatted_topic.replace('{deviceSN}', self.device_sn)
            return formatted_topic
        except Exception as e:
            logger.error(f"格式化主题失败: {e}")
            return topic_template
    
    def get_formatted_topics(self) -> dict:
        """获取所有格式化的MQTT主题"""
        try:
            topics = config_loader.get_config('mqtt_topics', {})
            formatted_topics = {}
            
            for topic_name, topic_template in topics.items():
                formatted_topics[topic_name] = self.format_topic(topic_template)
            
            return formatted_topics
            
        except Exception as e:
            logger.error(f"获取格式化主题失败: {e}")
            return {}
    
    def get_device_info(self) -> dict:
        """获取设备信息"""
        return {
            'product_sn': self.product_sn,
            'device_sn': self.device_sn,
            'device_type': self.device_type,
            'is_gateway': self.is_gateway,
            'topics': self.get_formatted_topics()
        }
    
    def reload_identity(self):
        """重新加载设备标识"""
        logger.info("重新加载设备标识...")
        self._load_identity()

# 全局设备标识管理器实例
device_identity = DeviceIdentityManager()
