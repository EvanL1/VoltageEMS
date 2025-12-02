"""
配置加载器模块
用于加载YAML格式的业务配置文件
"""

import os
import yaml
from typing import Dict, Any, Optional, List
from pathlib import Path
from loguru import logger
from .config import settings

class ConfigLoader:
    """配置加载器"""
    
    def __init__(self):
        self.config_data: Dict[str, Any] = {}
        self.config_file = "hissrv.yaml"
        self.load_config()
    
    def load_config(self):
        """加载配置文件"""
        try:
            # 构建配置文件路径
            config_path = Path(settings.CONFIG_DIR) / self.config_file
            
            # 如果配置文件不存在，尝试从当前目录加载
            if not config_path.exists():
                current_dir_config = Path("config") / self.config_file
                if current_dir_config.exists():
                    config_path = current_dir_config
                    logger.info(f"从当前目录加载配置文件: {config_path}")
                else:
                    logger.warning(f"配置文件不存在: {config_path}")
                    return
            
            # 读取YAML文件
            with open(config_path, 'r', encoding='utf-8') as file:
                self.config_data = yaml.safe_load(file)
            
            logger.info(f"配置文件加载成功: {config_path}")
            
        except FileNotFoundError:
            logger.error(f"配置文件未找到: {self.config_file}")
        except yaml.YAMLError as e:
            logger.error(f"YAML解析错误: {e}")
        except Exception as e:
            logger.error(f"加载配置文件失败: {e}")
    
    def reload_config(self):
        """重新加载配置文件"""
        logger.info("重新加载配置文件...")
        self.load_config()
    
    def get_config(self, key: str, default: Any = None) -> Any:
        """获取配置值"""
        try:
            keys = key.split('.')
            value = self.config_data
            
            for k in keys:
                if isinstance(value, dict) and k in value:
                    value = value[k]
                else:
                    return default
            
            return value
        except Exception as e:
            logger.warning(f"获取配置失败: {key}, {e}")
            return default
    
    def get_influxdb_config(self) -> Dict[str, Any]:
        """获取InfluxDB配置"""
        return self.get_config('influxdb', {})
    
    def get_scheduler_config(self) -> Dict[str, Any]:
        """获取定时任务配置"""
        return self.get_config('scheduler', {})
    
    def get_api_config(self) -> Dict[str, Any]:
        """获取API配置"""
        return self.get_config('api', {})
    
    def get_data_storage_config(self) -> Dict[str, Any]:
        """获取数据存储配置"""
        return self.get_config('data_storage', {})
    
    
    def get_redis_source_config(self) -> Dict[str, Any]:
        """获取Redis数据源配置"""
        return self.get_config('redis_source', {})
    
    def get_monitoring_config(self) -> Dict[str, Any]:
        """获取监控配置"""
        return self.get_config('monitoring', {})
    
    def get_logging_config(self) -> Dict[str, Any]:
        """获取日志配置"""
        return self.get_config('logging', {})
    
    def is_scheduler_task_enabled(self, task_name: str) -> bool:
        """检查定时任务是否启用"""
        task_config = self.get_config(f'scheduler.{task_name}', {})
        return task_config.get('enabled', False)
    
    def get_enabled_scheduler_tasks(self) -> Dict[str, Any]:
        """获取启用的定时任务配置"""
        scheduler_config = self.get_scheduler_config()
        return {name: config for name, config in scheduler_config.items() 
                if isinstance(config, dict) and config.get('enabled', False)}
    
    def validate_config(self) -> bool:
        """验证配置文件"""
        try:
            required_sections = [
                'influxdb', 'scheduler', 'redis_source', 'api'
            ]
            
            for section in required_sections:
                if not self.get_config(section):
                    logger.warning(f"缺少配置节: {section}")
            
            # 检查InfluxDB配置
            influxdb_config = self.get_influxdb_config()
            if not influxdb_config.get('url') or not influxdb_config.get('token'):
                logger.warning("InfluxDB配置不完整")
                return False
            
            # 检查Redis数据源配置
            redis_config = self.get_redis_source_config()
            if not redis_config.get('subscribe_patterns'):
                logger.warning("缺少Redis数据订阅模式")
                return False
            
            logger.info("配置文件验证通过")
            return True
            
        except Exception as e:
            logger.error(f"配置文件验证失败: {e}")
            return False
    
    def get_config_summary(self) -> Dict[str, Any]:
        """获取配置摘要"""
        return {
            'influxdb_url': self.get_config('influxdb.url'),
            'influxdb_bucket': self.get_config('influxdb.bucket'),
            'data_collection_interval': self.get_config('scheduler.data_collection.interval', 5),
            'data_flush_interval': self.get_config('scheduler.data_collection.flush_interval', 5),
            'data_batch_size': self.get_config('scheduler.data_collection.batch_size', 1000),
            'redis_patterns_count': len(self.get_config('redis_source.subscribe_patterns', [])),
            'api_prefix': self.get_config('api.prefix', '/hisApi'),
            'config_file': str(Path(settings.CONFIG_DIR) / self.config_file)
        }

    def get_data_collection_interval(self) -> int:
        """获取数据收集间隔"""
        return self.get_config('scheduler.data_collection.interval', 5)
    
    def get_data_flush_interval(self) -> int:
        """获取数据写入InfluxDB的间隔"""
        return self.get_config('scheduler.data_collection.flush_interval', 5)
    
    def get_data_batch_size(self) -> int:
        """获取数据批量大小"""
        return self.get_config('scheduler.data_collection.batch_size', 1000)
    
    def get_retention_policy(self) -> Dict[str, Any]:
        """获取数据保留策略"""
        return self.get_config('influxdb.retention_policy', {})
    
    def get_api_pagination_config(self) -> Dict[str, Any]:
        """获取API分页配置"""
        return self.get_config('api.pagination', {})
    
    def get_subscribe_patterns(self) -> List[str]:
        """获取Redis订阅模式"""
        return self.get_config('redis_source.subscribe_patterns', [])
    

# 全局配置加载器实例
config_loader = ConfigLoader()
