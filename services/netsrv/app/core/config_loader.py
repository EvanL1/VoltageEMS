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
        self.config_file = "netsrv.yaml"
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
    
    def get_mqtt_targets(self) -> Dict[str, Any]:
        """获取MQTT转发目标配置"""
        return self.get_config('mqtt_targets', {})
    
    def get_http_targets(self) -> Dict[str, Any]:
        """获取HTTP转发目标配置"""
        return self.get_config('http_targets', {})
    
    def get_aliyun_iot_config(self) -> Dict[str, Any]:
        """获取阿里云IoT配置"""
        return self.get_config('aliyun_iot', {})
    
    def get_forward_strategy(self) -> Dict[str, Any]:
        """获取转发策略配置"""
        return self.get_config('forward_strategy', {})
    
    def get_redis_source_config(self) -> Dict[str, Any]:
        """获取Redis数据源配置"""
        return self.get_config('redis_source', {})
    
    def get_monitoring_config(self) -> Dict[str, Any]:
        """获取监控配置"""
        return self.get_config('monitoring', {})
    
    def get_logging_config(self) -> Dict[str, Any]:
        """获取日志配置"""
        return self.get_config('logging', {})
    
    def is_target_enabled(self, target_type: str, target_name: str) -> bool:
        """检查目标是否启用"""
        targets = self.get_config(f'{target_type}_targets', {})
        target = targets.get(target_name, {})
        return target.get('enabled', False)
    
    def get_enabled_targets(self, target_type: str) -> Dict[str, Any]:
        """获取启用的目标配置"""
        targets = self.get_config(f'{target_type}_targets', {})
        return {name: config for name, config in targets.items() 
                if config.get('enabled', False)}
    
    def validate_config(self) -> bool:
        """验证配置文件"""
        try:
            required_sections = [
                'mqtt_targets', 'http_targets', 'forward_strategy'
            ]
            
            for section in required_sections:
                if not self.get_config(section):
                    logger.warning(f"缺少配置节: {section}")
            
            # 检查至少有一个启用的转发目标
            enabled_mqtt = len(self.get_enabled_targets('mqtt')) > 0
            enabled_http = len(self.get_enabled_targets('http')) > 0
            
            if not enabled_mqtt and not enabled_http:
                logger.warning("没有启用的转发目标")
                return False
            
            logger.info("配置文件验证通过")
            return True
            
        except Exception as e:
            logger.error(f"配置文件验证失败: {e}")
            return False
    
    def get_config_summary(self) -> Dict[str, Any]:
        """获取配置摘要"""
        return {
            'mqtt_targets_count': len(self.get_mqtt_targets()),
            'http_targets_count': len(self.get_http_targets()),
            'enabled_mqtt_targets': len(self.get_enabled_targets('mqtt')),
            'enabled_http_targets': len(self.get_enabled_targets('http')),
            'forward_interval': self.get_config('forward_strategy.frequency.interval', 5),
            'batch_size': self.get_config('forward_strategy.frequency.batch_size', 100),
            'config_file': str(Path(settings.CONFIG_DIR) / self.config_file)
        }

    def get_mqtt_connection_config(self) -> Dict[str, Any]:
        """获取MQTT连接配置（用于MQTT客户端连接，统一使用full_path）"""
        mqtt_config = self.get_config('mqtt_connection', {})
        
        # 处理SSL证书路径，统一转换为绝对路径（用于MQTT客户端连接）
        if mqtt_config.get('broker', {}).get('ssl', {}).get('enabled', False):
            ssl_config = mqtt_config['broker']['ssl']
            
            # 检查是否在容器环境中运行
            if os.path.exists('/app/config') and os.path.isdir('/app/config'):
                # 容器环境：使用绝对路径
                config_dir = Path('/app/config')
                logger.debug("检测到容器环境，使用绝对路径")
            else:
                # 本地开发环境：使用相对路径
                config_dir = Path("config")
                logger.debug("检测到本地开发环境，使用相对路径")
            
            # 转换证书路径，统一使用绝对路径
            for cert_key in ['ca_cert', 'client_cert', 'client_key']:
                if ssl_config.get(cert_key):
                    # 如果是双路径结构，使用full_path；否则转换相对路径
                    if isinstance(ssl_config[cert_key], dict) and 'full_path' in ssl_config[cert_key]:
                        ssl_config[cert_key] = ssl_config[cert_key]['full_path']
                    else:
                        ssl_config[cert_key] = str(config_dir / ssl_config[cert_key])
            
            logger.debug(f"SSL证书路径: CA={ssl_config.get('ca_cert')}, Cert={ssl_config.get('client_cert')}, Key={ssl_config.get('client_key')}")
        
        # 配置重连参数
        reconnect_config = mqtt_config.get('broker', {}).get('reconnect', {})
        if reconnect_config.get('enabled', True):
            mqtt_config['reconnect_enabled'] = True
            mqtt_config['reconnect_delay'] = reconnect_config.get('delay', 5)
            mqtt_config['max_reconnect_attempts'] = reconnect_config.get('max_attempts', 10)
        else:
            mqtt_config['reconnect_enabled'] = False
        
        return mqtt_config
    
    def get_mqtt_connection_config_for_api(self) -> Dict[str, Any]:
        """获取MQTT连接配置（用于API接口，返回双路径：path和full_path）"""
        # 直接从配置文件读取原始数据，避免使用可能被修改过的内存数据
        try:
            import yaml
            config_path = Path("config/netsrv.yaml")
            if not config_path.exists():
                config_path = Path("config/netsrv.yaml")
            
            with open(config_path, 'r', encoding='utf-8') as file:
                config_data = yaml.safe_load(file)
            
            mqtt_config = config_data.get('mqtt_connection', {})
            
            # 为SSL证书添加双路径：path（原始相对路径）和full_path（绝对路径）
            if mqtt_config.get('broker', {}).get('ssl', {}).get('enabled', False):
                ssl_config = mqtt_config['broker']['ssl']
                
                # 检查是否在容器环境中运行
                if os.path.exists('/app/config') and os.path.isdir('/app/config'):
                    # 容器环境：使用绝对路径
                    config_dir = Path('/app/config')
                else:
                    # 本地开发环境：使用相对路径
                    config_dir = Path("config")
                
                # 为每个证书添加双路径
                for cert_key in ['ca_cert', 'client_cert', 'client_key']:
                    if ssl_config.get(cert_key):
                        original_path = ssl_config[cert_key]
                        full_path = str(config_dir / original_path)
                        
                        # 创建双路径结构
                        ssl_config[cert_key] = {
                            'path': original_path,        # 原始相对路径
                            'full_path': full_path       # 绝对路径
                        }
            
            # 配置重连参数
            reconnect_config = mqtt_config.get('broker', {}).get('reconnect', {})
            if reconnect_config.get('enabled', True):
                mqtt_config['reconnect_enabled'] = True
                mqtt_config['reconnect_delay'] = reconnect_config.get('delay', 5)
                mqtt_config['max_reconnect_attempts'] = reconnect_config.get('max_attempts', 10)
            else:
                mqtt_config['reconnect_enabled'] = False
            
            return mqtt_config
            
        except Exception as e:
            logger.error(f"读取配置文件失败: {e}")
            # 如果读取文件失败，回退到内存数据
            return self.get_config('mqtt_connection', {})
    
    def get_mqtt_topics_config(self) -> Dict[str, Any]:
        """获取MQTT主题配置"""
        return self.get_config('mqtt_topics', {})
    
    def get_device_identity_config(self) -> Dict[str, Any]:
        """获取设备身份配置"""
        return self.get_config('device_identity', {})
    
    def get_data_report_config(self) -> Dict[str, Any]:
        """获取数据上报配置"""
        return self.get_config('data_report', {})
    
    def get_device_status_config(self) -> Dict[str, Any]:
        """获取设备状态配置"""
        return self.get_config('device_status', {})
    
    def get_system_monitor_config(self) -> Dict[str, Any]:
        """获取系统监控配置"""
        return self.get_config('system_monitor', {})
    
    def update_mqtt_connection_config(self, mqtt_config: Dict[str, Any]) -> bool:
        """更新MQTT连接配置"""
        try:
            # 更新内存中的配置
            if 'mqtt_connection' not in self.config_data:
                self.config_data['mqtt_connection'] = {}
            
            self.config_data['mqtt_connection'] = mqtt_config
            
            # 只更新文件中的MQTT配置部分，保持其他配置和注释不变
            if self.update_mqtt_config_in_file(mqtt_config):
                logger.info("MQTT连接配置更新成功")
                return True
            else:
                logger.error("保存MQTT连接配置失败")
                return False
                
        except Exception as e:
            logger.error(f"更新MQTT连接配置失败: {e}")
            return False
    
    def update_mqtt_config_in_file(self, mqtt_config: Dict[str, Any]) -> bool:
        """简单方式：只替换配置值，保持原有格式和注释完全不变"""
        try:
            # 构建配置文件路径
            config_path = Path(settings.CONFIG_DIR) / self.config_file
            
            # 如果配置文件在当前目录，也使用当前目录
            if not config_path.exists():
                current_dir_config = Path("config") / self.config_file
                if current_dir_config.exists():
                    config_path = current_dir_config
            
            if not config_path.exists():
                logger.error(f"配置文件不存在: {config_path}")
                return False
            
            # 创建备份
            backup_path = config_path.with_suffix('.bak')
            import shutil
            shutil.copy2(config_path, backup_path)
            logger.info(f"创建配置文件备份: {backup_path}")
            
            # 读取原始文件内容
            with open(config_path, 'r', encoding='utf-8') as file:
                lines = file.readlines()
            
            # 扁平化新配置，便于查找
            flat_config = self._flatten_mqtt_config(mqtt_config)
            logger.debug(f"扁平化配置: {flat_config}")
            
            # 逐行处理，只替换需要更新的配置值
            modified = False
            current_section = None  # 跟踪当前配置节
            
            for i, line in enumerate(lines):
                # 跟踪当前配置节
                current_section = self._get_current_section(line, current_section)
                
                if self._should_update_line(line, flat_config, current_section):
                    new_line = self._update_config_line(line, flat_config, current_section)
                    if new_line != line:
                        lines[i] = new_line
                        modified = True
                        logger.debug(f"更新行 {i+1}: {line.strip()} -> {new_line.strip()}")
            
            if modified:
                # 写入更新后的配置文件
                with open(config_path, 'w', encoding='utf-8') as file:
                    file.writelines(lines)
                
                logger.info(f"MQTT配置更新成功: {config_path}")
            else:
                logger.info("MQTT配置无需更新")
            
            return True
            
        except Exception as e:
            logger.error(f"更新MQTT配置文件失败: {e}")
            return False
    
    def _flatten_mqtt_config(self, mqtt_config: Dict[str, Any]) -> Dict[str, Any]:
        """将嵌套的MQTT配置扁平化，便于查找"""
        flat = {}
        
        try:
            broker = mqtt_config.get('broker', {})
            
            # 基础配置
            if 'host' in broker:
                flat['host'] = broker['host']
            if 'port' in broker:
                flat['port'] = broker['port']
            if 'username' in broker:
                flat['username'] = broker['username']
            if 'password' in broker:
                flat['password'] = broker['password']
            if 'client_id' in broker:
                flat['client_id'] = broker['client_id']
            if 'keepalive' in broker:
                flat['keepalive'] = broker['keepalive']
            
            # SSL配置
            ssl = broker.get('ssl', {})
            if 'enabled' in ssl:
                flat['enabled'] = ssl['enabled']  # SSL enabled
            if 'ca_cert' in ssl:
                flat['ca_cert'] = ssl['ca_cert']
            if 'client_cert' in ssl:
                flat['client_cert'] = ssl['client_cert']
            if 'client_key' in ssl:
                flat['client_key'] = ssl['client_key']
            
            # 重连配置
            reconnect = broker.get('reconnect', {})
            if 'enabled' in reconnect:
                flat['reconnect_enabled'] = reconnect['enabled']  # 避免与SSL的enabled冲突
            if 'max_attempts' in reconnect:
                flat['max_attempts'] = reconnect['max_attempts']
            if 'delay' in reconnect:
                flat['delay'] = reconnect['delay']
            
            # 状态配置
            status = broker.get('status', {})
            if 'will_message_enabled' in status:
                flat['will_message_enabled'] = status['will_message_enabled']
            if 'auto_online_message' in status:
                flat['auto_online_message'] = status['auto_online_message']
                
            return flat
            
        except Exception as e:
            logger.warning(f"扁平化MQTT配置失败: {e}")
            return {}
    
    def _get_current_section(self, line: str, previous_section: str) -> str:
        """跟踪当前配置节"""
        stripped = line.strip()
        
        # 跳过注释行和空行
        if not stripped or stripped.startswith('#'):
            return previous_section
        
        # 检查是否是配置节开始
        if ':' in stripped and not stripped.startswith(' ') and not stripped.startswith('\t'):
            section_name = stripped.split(':')[0].strip()
            return section_name
        elif stripped.endswith(':'):
            # 处理嵌套配置节，如 ssl: 或 reconnect:
            indent = len(line) - len(line.lstrip())
            if indent > 0:  # 有缩进，说明是子节
                section_name = stripped[:-1].strip()  # 去除冒号
                return section_name
        
        return previous_section
    
    def _should_update_line(self, line: str, flat_config: Dict[str, Any], current_section: str) -> bool:
        """判断这一行是否需要更新"""
        stripped = line.strip()
        
        # 跳过注释行和空行
        if not stripped or stripped.startswith('#'):
            return False
        
        # 检查是否包含配置项
        if ':' not in stripped or stripped.endswith(':'):
            return False
        
        # 提取配置项名称
        config_name = stripped.split(':')[0].strip()
        
        # 处理特殊情况：enabled字段根据当前节来区分
        if config_name == 'enabled':
            if current_section == 'ssl':
                return 'enabled' in flat_config
            elif current_section == 'reconnect':
                return 'reconnect_enabled' in flat_config
            else:
                return 'enabled' in flat_config or 'reconnect_enabled' in flat_config
        
        return config_name in flat_config
    
    def _update_config_line(self, line: str, flat_config: Dict[str, Any], current_section: str) -> str:
        """更新配置行的值，保持格式和注释"""
        try:
            stripped = line.strip()
            if ':' not in stripped:
                return line
            
            # 提取配置项名称
            config_name = stripped.split(':')[0].strip()
            
            # 获取新值
            new_value = None
            if config_name == 'enabled':
                # 根据当前节来区分不同的enabled
                if current_section == 'ssl':
                    new_value = flat_config.get('enabled')
                elif current_section == 'reconnect':
                    new_value = flat_config.get('reconnect_enabled')
                else:
                    # 默认情况
                    new_value = flat_config.get('enabled') or flat_config.get('reconnect_enabled')
            else:
                new_value = flat_config.get(config_name)
            
            if new_value is None:
                return line
            
            # 保持原有的缩进
            indent = len(line) - len(line.lstrip())
            indent_str = line[:indent]
            
            # 检查是否有注释
            if '#' in line:
                # 有注释：只替换冒号和#之间的部分
                parts = line.split('#', 1)
                config_part = parts[0]
                comment_part = '#' + parts[1]
                
                # 格式化新值
                if isinstance(new_value, str):
                    formatted_value = f'"{new_value}"'
                elif isinstance(new_value, bool):
                    formatted_value = str(new_value).lower()
                else:
                    formatted_value = str(new_value)
                
                # 重构配置部分，保持适当的空格
                new_config_part = f"{indent_str}{config_name}: {formatted_value}  "
                new_line = new_config_part + comment_part
                
            else:
                # 无注释：替换冒号后面的所有内容
                if isinstance(new_value, str):
                    formatted_value = f'"{new_value}"'
                elif isinstance(new_value, bool):
                    formatted_value = str(new_value).lower()
                else:
                    formatted_value = str(new_value)
                
                new_line = f"{indent_str}{config_name}: {formatted_value}\n"
            
            return new_line
            
        except Exception as e:
            logger.warning(f"更新配置行失败: {e}")
            return line
    
    
    def validate_mqtt_connection_config(self, mqtt_config: Dict[str, Any]) -> tuple[bool, str]:
        """验证MQTT连接配置"""
        try:
            # 检查必需字段
            broker = mqtt_config.get('broker', {})
            if not broker:
                return False, "缺少broker配置"
            
            # 检查主机地址
            if not broker.get('host'):
                return False, "缺少host配置"
            
            # 检查端口
            port = broker.get('port', 1883)
            if not isinstance(port, int) or port <= 0 or port > 65535:
                return False, "port配置无效，必须是1-65535之间的整数"
            
            # 检查客户端ID
            if not broker.get('client_id'):
                return False, "缺少client_id配置"
            
            # 检查keepalive
            keepalive = broker.get('keepalive', 60)
            if not isinstance(keepalive, int) or keepalive < 10 or keepalive > 3600:
                return False, "keepalive配置无效，必须是10-3600之间的整数"
            
            # 检查SSL配置
            ssl_config = broker.get('ssl', {})
            if ssl_config.get('enabled', False):
                ca_cert = ssl_config.get('ca_cert')
                client_cert = ssl_config.get('client_cert')
                client_key = ssl_config.get('client_key')
                
                if not ca_cert or not client_cert or not client_key:
                    return False, "SSL启用时必须提供ca_cert、client_cert和client_key"
            
            # 检查重连配置
            reconnect_config = broker.get('reconnect', {})
            if reconnect_config.get('enabled', True):
                max_attempts = reconnect_config.get('max_attempts', 10)
                delay = reconnect_config.get('delay', 5)
                
                if not isinstance(max_attempts, int) or max_attempts < 1 or max_attempts > 100:
                    return False, "max_attempts配置无效，必须是1-100之间的整数"
                
                if not isinstance(delay, int) or delay < 1 or delay > 300:
                    return False, "delay配置无效，必须是1-300之间的整数"
            
            return True, "配置验证通过"
            
        except Exception as e:
            return False, f"配置验证异常: {str(e)}"
    
    def get_config_file_path(self) -> str:
        """获取配置文件路径"""
        config_path = Path(settings.CONFIG_DIR) / self.config_file
        
        # 如果配置文件在当前目录，也使用当前目录
        if not config_path.exists():
            current_dir_config = Path("config") / self.config_file
            if current_dir_config.exists():
                config_path = current_dir_config
        
        return str(config_path)

# 全局配置加载器实例
config_loader = ConfigLoader()
