"""
系统监控数据收集模块
用于获取宿主机系统信息，如CPU占用、内存占用、磁盘空间等
"""

import time
import platform
from typing import Dict, Any, Optional
from loguru import logger
import json

# 尝试导入psutil，如果不存在则使用系统命令
try:
    import psutil
    HAS_PSUTIL = True
except ImportError:
    HAS_PSUTIL = False
    logger.warning("psutil未安装，将使用系统命令获取系统信息")

class SystemMonitor:
    """系统监控数据收集器"""
    
    def __init__(self):
        self.last_collect_time = 0
        self._validate_environment()
    
    def _validate_environment(self):
        """验证运行环境"""
        try:
            system = platform.system()
            logger.info(f"运行在 {system} {platform.release()} 上")
            
            if system != 'Linux':
                logger.warning(f"当前系统 {system} 可能不完全支持所有监控功能")
        except Exception as e:
            logger.error(f"验证环境失败: {e}")
    
    def get_system_data(self) -> Optional[Dict[str, Any]]:
        """获取系统监控数据"""
        try:
            current_time = time.time()
            
            if HAS_PSUTIL:
                data = self._get_data_with_psutil()
            else:
                data = self._get_data_with_commands()
            
            self.last_collect_time = current_time
            
            if data:
                logger.debug(f"系统监控数据收集成功，包含 {len(data)} 项指标")
            
            return data
            
        except Exception as e:
            logger.error(f"获取系统数据失败: {e}")
            return None
    
    def _get_data_with_psutil(self) -> Dict[str, Any]:
        """使用psutil获取系统数据"""
        # 定义固定的数据结构，确保每次上报字段一致
        data = {
            # CPU相关 (固定字段)
            "cpu_usage_percent": 0.0,
            
            # 内存相关 (固定字段)
            "memory_total_gb": 0.0,
            "memory_used_gb": 0.0,
            "memory_available_gb": 0.0,
            "memory_usage_percent": 0.0,
            
            # 磁盘相关 (固定字段)
            "disk_total_gb": 0.0,
            "disk_used_gb": 0.0,
            "disk_free_gb": 0.0,
            "disk_usage_percent": 0.0,
            
            # 网络相关 (固定字段)
            "network_bytes_sent": 0,
            "network_bytes_recv": 0,
            "network_packets_sent": 0,
            "network_packets_recv": 0,
            
            # 系统相关 (固定字段)
            "system_uptime_hours": 0.0,
            "boot_time": 0,
        }
        
        try:
            # CPU信息
            cpu_percent = psutil.cpu_percent(interval=0.1)
            data["cpu_usage_percent"] = round(cpu_percent, 2)
            
            # 内存信息
            memory = psutil.virtual_memory()
            data["memory_total_gb"] = round(memory.total / (1024**3), 2)
            data["memory_used_gb"] = round(memory.used / (1024**3), 2)
            data["memory_available_gb"] = round(memory.available / (1024**3), 2)
            data["memory_usage_percent"] = round(memory.percent, 2)
            
            # 磁盘信息
            disk_usage = psutil.disk_usage('/')
            data["disk_total_gb"] = round(disk_usage.total / (1024**3), 2)
            data["disk_used_gb"] = round(disk_usage.used / (1024**3), 2)
            data["disk_free_gb"] = round(disk_usage.free / (1024**3), 2)
            data["disk_usage_percent"] = round((disk_usage.used / disk_usage.total) * 100, 2)
            
            # 网络信息
            net_io = psutil.net_io_counters()
            data["network_bytes_sent"] = net_io.bytes_sent
            data["network_bytes_recv"] = net_io.bytes_recv
            data["network_packets_sent"] = net_io.packets_sent
            data["network_packets_recv"] = net_io.packets_recv
            
            # 系统启动时间
            boot_time = psutil.boot_time()
            uptime_seconds = time.time() - boot_time
            data["system_uptime_hours"] = round(uptime_seconds / 3600, 2)
            data["boot_time"] = int(boot_time)
            
        except Exception as e:
            logger.error(f"使用psutil获取系统数据失败: {e}")
        
        return data
    
    def _get_data_with_commands(self) -> Dict[str, Any]:
        """使用系统命令获取数据（fallback方案）"""
        # 定义与psutil方法相同的固定数据结构
        data = {
            # CPU相关 (固定字段)
            "cpu_usage_percent": 0.0,
            
            # 内存相关 (固定字段)
            "memory_total_gb": 0.0,
            "memory_used_gb": 0.0,
            "memory_available_gb": 0.0,
            "memory_usage_percent": 0.0,
            
            # 磁盘相关 (固定字段)
            "disk_total_gb": 0.0,
            "disk_used_gb": 0.0,
            "disk_free_gb": 0.0,
            "disk_usage_percent": 0.0,
            
            # 网络相关 (固定字段)
            "network_bytes_sent": 0,
            "network_bytes_recv": 0,
            "network_packets_sent": 0,
            "network_packets_recv": 0,
            
            # 系统相关 (固定字段)
            "system_uptime_hours": 0.0,
            "boot_time": 0,
        }
        
        try:
            # CPU使用率
            cpu_usage = self._get_cpu_usage()
            if cpu_usage is not None:
                data["cpu_usage_percent"] = cpu_usage
            
            # 内存信息
            memory_info = self._get_memory_info()
            if memory_info:
                data.update(memory_info)
            
            # 磁盘信息
            disk_info = self._get_disk_info()
            if disk_info:
                data.update(disk_info)
            
            # 系统运行时间
            uptime_info = self._get_uptime_info()
            if uptime_info:
                data.update(uptime_info)
                
        except Exception as e:
            logger.error(f"使用系统命令获取数据失败: {e}")
        
        return data
    
    def _get_cpu_usage(self) -> Optional[float]:
        """获取CPU使用率"""
        try:
            # 读取 /proc/stat
            with open('/proc/stat', 'r') as f:
                line = f.readline()
                cpu_times = [int(x) for x in line.split()[1:]]
                
            # 计算总时间和空闲时间
            total_time = sum(cpu_times)
            idle_time = cpu_times[3]  # idle time
            
            # 简单计算使用率（注意：这是瞬时值，不如psutil准确）
            if total_time > 0:
                cpu_usage = round((1 - idle_time / total_time) * 100, 2)
                return max(0, min(100, cpu_usage))
                
        except Exception as e:
            logger.debug(f"获取CPU使用率失败: {e}")
            
        return None
    
    def _get_memory_info(self) -> Dict[str, Any]:
        """获取内存信息"""
        try:
            with open('/proc/meminfo', 'r') as f:
                meminfo = {}
                for line in f:
                    key, value = line.split(':')
                    meminfo[key.strip()] = int(value.strip().split()[0]) * 1024  # 转换为字节
            
            total = meminfo.get('MemTotal', 0)
            free = meminfo.get('MemFree', 0)
            buffers = meminfo.get('Buffers', 0)
            cached = meminfo.get('Cached', 0)
            
            available = free + buffers + cached
            used = total - available
            
            return {
                "memory_total_gb": round(total / (1024**3), 2),
                "memory_used_gb": round(used / (1024**3), 2),
                "memory_available_gb": round(available / (1024**3), 2),
                "memory_usage_percent": round((used / total) * 100, 2) if total > 0 else 0,
            }
            
        except Exception as e:
            logger.debug(f"获取内存信息失败: {e}")
            
        return {}
    
    def _get_disk_info(self) -> Dict[str, Any]:
        """获取磁盘信息"""
        try:
            import subprocess
            result = subprocess.run(['df', '-h', '/'], capture_output=True, text=True)
            lines = result.stdout.strip().split('\n')
            
            if len(lines) >= 2:
                fields = lines[1].split()
                if len(fields) >= 6:
                    # 解析df输出 (假设格式: Filesystem Size Used Avail Use% Mounted)
                    size_str = fields[1]
                    used_str = fields[2]
                    avail_str = fields[3]
                    use_percent_str = fields[4].rstrip('%')
                    
                    # 转换大小单位到GB
                    size_gb = self._parse_size_to_gb(size_str)
                    used_gb = self._parse_size_to_gb(used_str)
                    avail_gb = self._parse_size_to_gb(avail_str)
                    
                    return {
                        "disk_total_gb": size_gb,
                        "disk_used_gb": used_gb,
                        "disk_free_gb": avail_gb,
                        "disk_usage_percent": float(use_percent_str) if use_percent_str.replace('.', '').isdigit() else 0,
                    }
                    
        except Exception as e:
            logger.debug(f"获取磁盘信息失败: {e}")
            
        return {}
    
    def _parse_size_to_gb(self, size_str: str) -> float:
        """解析大小字符串到GB"""
        try:
            if size_str.endswith('T'):
                return round(float(size_str[:-1]) * 1024, 2)
            elif size_str.endswith('G'):
                return round(float(size_str[:-1]), 2)
            elif size_str.endswith('M'):
                return round(float(size_str[:-1]) / 1024, 2)
            elif size_str.endswith('K'):
                return round(float(size_str[:-1]) / (1024 * 1024), 2)
            else:
                return round(float(size_str) / (1024**3), 2)
        except:
            return 0.0
    
    def _get_uptime_info(self) -> Dict[str, Any]:
        """获取系统运行时间"""
        try:
            with open('/proc/uptime', 'r') as f:
                uptime_seconds = float(f.read().split()[0])
            
            # 计算启动时间
            boot_time = int(time.time() - uptime_seconds)
                
            return {
                "system_uptime_hours": round(uptime_seconds / 3600, 2),
                "boot_time": boot_time,
            }
        except Exception as e:
            logger.debug(f"获取系统运行时间失败: {e}")
            
        return {}
    
    def format_for_mqtt(self, system_data: Dict[str, Any], device_sn: str) -> Dict[str, Any]:
        """格式化系统监控数据为MQTT上报格式"""
        try:
            if not system_data:
                return {}
            
            current_timestamp = int(time.time())
            
            # 按照用户提供的格式构建数据
            message = {
                "timestamp": current_timestamp,
                "property": [
                    {
                        "source": "gateway",  # 固定值
                        "device": device_sn,  # 设备序列号
                        "data_type": "T",     # 固定值
                        "value": system_data
                    }
                ]
            }
            
            return message
            
        except Exception as e:
            logger.error(f"格式化系统监控数据失败: {e}")
            return {}

# 全局系统监控实例
system_monitor = SystemMonitor()
