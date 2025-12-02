"""
证书管理服务
负责证书文件的上传、删除和配置更新
"""

import os
import shutil
from pathlib import Path
from typing import Dict, Any, Optional, Tuple
from loguru import logger
from app.core.config import settings
from app.core.config_loader import config_loader


class CertificateManager:
    """证书管理器"""
    
    def __init__(self):
        self.cert_dir = self._get_cert_directory()
        self._ensure_cert_directory()
    
    def _get_cert_directory(self) -> Path:
        """获取证书目录路径"""
        # 检查是否在容器环境中运行
        if os.path.exists('/app/config') and os.path.isdir('/app/config'):
            # 容器环境：使用绝对路径
            cert_dir = Path('/app/config/cert')
            logger.debug("检测到容器环境，使用绝对路径")
        else:
            # 本地开发环境：使用相对路径
            cert_dir = Path("config/cert")
            logger.debug("检测到本地开发环境，使用相对路径")
        
        return cert_dir
    
    def _ensure_cert_directory(self):
        """确保证书目录存在"""
        try:
            self.cert_dir.mkdir(parents=True, exist_ok=True)
            logger.info(f"证书目录已准备: {self.cert_dir}")
        except Exception as e:
            logger.error(f"创建证书目录失败: {e}")
            raise
    
    def upload_certificate(self, cert_type: str, file_content: bytes, filename: str) -> Tuple[bool, str, str]:
        """
        上传证书文件
        
        Args:
            cert_type: 证书类型 ('ca_cert', 'client_cert', 'client_key')
            file_content: 文件内容
            filename: 原始文件名
            
        Returns:
            (成功标志, 消息, 保存的文件名)
        """
        try:
            # 验证证书类型
            if cert_type not in ['ca_cert', 'client_cert', 'client_key']:
                return False, f"不支持的证书类型: {cert_type}", ""
            
            # 生成新的文件名
            new_filename = self._generate_cert_filename(cert_type, filename)
            new_file_path = self.cert_dir / new_filename
            
            # 获取旧证书文件路径（用于删除）
            old_cert_path = self._get_current_cert_path(cert_type)
            
            # 保存新证书文件
            with open(new_file_path, 'wb') as f:
                f.write(file_content)
            
            logger.info(f"证书文件上传成功: {new_file_path}")
            
            # 更新配置文件中的证书路径
            success = self._update_cert_config(cert_type, f"cert/{new_filename}")
            
            if success:
                # 删除旧证书文件（如果存在且不是同一个文件）
                if old_cert_path and old_cert_path.exists() and old_cert_path != new_file_path:
                    try:
                        old_cert_path.unlink()
                        logger.info(f"旧证书文件已删除: {old_cert_path}")
                    except Exception as e:
                        logger.warning(f"删除旧证书文件失败: {e}")
                
                return True, f"证书上传成功，配置文件已更新", new_filename
            else:
                # 如果配置更新失败，删除刚上传的文件
                try:
                    new_file_path.unlink()
                    logger.info(f"配置更新失败，已删除上传的文件: {new_file_path}")
                except Exception as e:
                    logger.warning(f"删除上传文件失败: {e}")
                
                return False, "证书上传成功，但配置文件更新失败", ""
                
        except Exception as e:
            logger.error(f"上传证书失败: {e}")
            return False, f"上传证书失败: {str(e)}", ""
    
    def _generate_cert_filename(self, cert_type: str, original_filename: str) -> str:
        """生成证书文件名，保留原始文件名"""
        # 直接使用原始文件名，但确保有扩展名
        original_path = Path(original_filename)
        
        # 如果没有扩展名，根据证书类型添加默认扩展名
        if not original_path.suffix:
            if cert_type == 'ca_cert':
                return f"{original_path.stem}.pem"
            elif cert_type == 'client_cert':
                return f"{original_path.stem}.crt"
            elif cert_type == 'client_key':
                return f"{original_path.stem}.key"
            else:
                return f"{original_path.stem}.pem"
        
        # 有扩展名就直接使用原始文件名
        return original_filename
    
    def _get_current_cert_path(self, cert_type: str) -> Optional[Path]:
        """获取当前证书文件的完整路径"""
        try:
            mqtt_config = config_loader.get_mqtt_connection_config()
            ssl_config = mqtt_config.get('broker', {}).get('ssl', {})
            
            cert_filename = ssl_config.get(cert_type)
            if not cert_filename:
                return None
            
            # 如果是相对路径，转换为绝对路径
            if not os.path.isabs(cert_filename):
                # 检查是否在容器环境中运行
                if os.path.exists('/app/config') and os.path.isdir('/app/config'):
                    config_dir = Path('/app/config')
                else:
                    config_dir = Path("config")
                
                # 检查路径是否已经包含config前缀，避免重复添加
                if cert_filename.startswith('config/'):
                    cert_path = Path(cert_filename)
                else:
                    cert_path = config_dir / cert_filename
            else:
                cert_path = Path(cert_filename)
            
            return cert_path if cert_path.exists() else None
            
        except Exception as e:
            logger.warning(f"获取当前证书路径失败: {e}")
            return None
    
    def _update_cert_config(self, cert_type: str, new_cert_path: str) -> bool:
        """更新配置文件中的证书路径"""
        try:
            # 直接读取配置文件，避免使用被修改过的config_data
            import yaml
            config_path = Path("config/netsrv.yaml")
            if not config_path.exists():
                config_path = Path("config/netsrv.yaml")
            
            with open(config_path, 'r', encoding='utf-8') as file:
                config_data = yaml.safe_load(file)
            
            mqtt_config = config_data.get('mqtt_connection', {})
            
            # 确保SSL配置存在
            if 'broker' not in mqtt_config:
                mqtt_config['broker'] = {}
            if 'ssl' not in mqtt_config['broker']:
                mqtt_config['broker']['ssl'] = {}
            
            # 更新证书路径，不涉及SSL enabled状态
            mqtt_config['broker']['ssl'][cert_type] = new_cert_path
            
            # 保存配置
            success = config_loader.update_mqtt_connection_config(mqtt_config)
            
            if success:
                logger.info(f"证书配置更新成功: {cert_type} -> {new_cert_path}")
            else:
                logger.error(f"证书配置更新失败: {cert_type}")
            
            return success
            
        except Exception as e:
            logger.error(f"更新证书配置失败: {e}")
            return False
    
    def get_certificate_info(self) -> Dict[str, Any]:
        """获取当前证书信息"""
        try:
            # 重新加载配置以确保获取最新数据
            config_loader.reload_config()
            # 直接获取原始配置，避免路径被重复处理
            mqtt_config = config_loader.config_data.get('mqtt_connection', {})
            ssl_config = mqtt_config.get('broker', {}).get('ssl', {})
            
            cert_info = {
                'ssl_enabled': ssl_config.get('enabled', False),
                'certificates': {}
            }
            
            for cert_type in ['ca_cert', 'client_cert', 'client_key']:
                cert_path = ssl_config.get(cert_type)
                if cert_path:
                    # 构建完整路径
                    if os.path.isabs(cert_path):
                        full_path = Path(cert_path)
                    else:
                        # 检查是否在容器环境中运行
                        if os.path.exists('/app/config') and os.path.isdir('/app/config'):
                            config_dir = Path('/app/config')
                        else:
                            config_dir = Path("config")
                        full_path = config_dir / cert_path
                    
                    cert_info['certificates'][cert_type] = {
                        'path': cert_path,
                        'full_path': str(full_path),
                        'exists': full_path.exists(),
                        'size': full_path.stat().st_size if full_path.exists() else 0
                    }
                else:
                    cert_info['certificates'][cert_type] = {
                        'path': None,
                        'full_path': None,
                        'exists': False,
                        'size': 0
                    }
            
            return cert_info
            
        except Exception as e:
            logger.error(f"获取证书信息失败: {e}")
            return {
                'ssl_enabled': False,
                'certificates': {},
                'error': str(e)
            }
    
    def delete_certificate(self, cert_type: str) -> Tuple[bool, str]:
        """删除证书文件"""
        try:
            # 验证证书类型
            if cert_type not in ['ca_cert', 'client_cert', 'client_key']:
                return False, f"不支持的证书类型: {cert_type}"
            
            # 获取当前证书文件路径
            cert_path = self._get_current_cert_path(cert_type)
            
            if not cert_path or not cert_path.exists():
                return False, f"证书文件不存在: {cert_type}"
            
            # 删除文件
            cert_path.unlink()
            logger.info(f"证书文件已删除: {cert_path}")
            
            # 更新配置文件，清空证书路径
            # 直接读取配置文件，避免使用被修改过的config_data
            import yaml
            config_path = Path("config/netsrv.yaml")
            if not config_path.exists():
                config_path = Path("config/netsrv.yaml")
            
            with open(config_path, 'r', encoding='utf-8') as file:
                config_data = yaml.safe_load(file)
            
            mqtt_config = config_data.get('mqtt_connection', {})
            if 'broker' in mqtt_config and 'ssl' in mqtt_config['broker']:
                # 清空证书路径，不涉及SSL enabled状态
                mqtt_config['broker']['ssl'][cert_type] = ""
                
                config_loader.update_mqtt_connection_config(mqtt_config)
                logger.info(f"配置文件已更新，清空{cert_type}路径")
            
            return True, f"证书文件删除成功: {cert_type}"
            
        except Exception as e:
            logger.error(f"删除证书失败: {e}")
            return False, f"删除证书失败: {str(e)}"


# 全局证书管理器实例
certificate_manager = CertificateManager()
