"""
通用工具函数
"""

import hashlib
import json
import logging
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional, Union
import uuid

logger = logging.getLogger(__name__)

def generate_uuid() -> str:
    """生成UUID"""
    return str(uuid.uuid4())

def generate_hash(data: str) -> str:
    """生成数据的MD5哈希值"""
    return hashlib.md5(data.encode()).hexdigest()

def safe_json_dumps(obj: Any) -> str:
    """安全的JSON序列化"""
    try:
        return json.dumps(obj, ensure_ascii=False, default=str)
    except Exception as e:
        logger.error(f"JSON序列化失败: {e}")
        return str(obj)

def safe_json_loads(data: str) -> Any:
    """安全的JSON反序列化"""
    try:
        return json.loads(data)
    except Exception as e:
        logger.error(f"JSON反序列化失败: {e}")
        return None

def format_timestamp(timestamp: int) -> str:
    """格式化时间戳为ISO字符串"""
    dt = datetime.fromtimestamp(timestamp, tz=timezone.utc)
    return dt.isoformat()

def parse_timestamp(timestamp_input: Union[str, int]) -> Optional[datetime]:
    """解析时间戳（支持字符串或数字格式）"""
    try:
        if isinstance(timestamp_input, int):
            return datetime.fromtimestamp(timestamp_input, tz=timezone.utc)
        elif isinstance(timestamp_input, str):
            return datetime.fromisoformat(timestamp_input.replace('Z', '+00:00'))
        else:
            logger.error(f"不支持的时间戳类型: {type(timestamp_input)}")
            return None
    except Exception as e:
        logger.error(f"解析时间戳失败: {e}")
        return None

def validate_email(email: str) -> bool:
    """验证邮箱格式"""
    import re
    pattern = r'^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$'
    return re.match(pattern, email) is not None

def sanitize_string(text: str, max_length: int = 1000) -> str:
    """清理字符串"""
    if not text:
        return ""
    
    # 移除危险字符
    dangerous_chars = ['<', '>', '"', "'", '&']
    for char in dangerous_chars:
        text = text.replace(char, '')
    
    # 限制长度
    if len(text) > max_length:
        text = text[:max_length] + "..."
    
    return text.strip()

def chunk_list(lst: List[Any], chunk_size: int) -> List[List[Any]]:
    """将列表分块"""
    return [lst[i:i + chunk_size] for i in range(0, len(lst), chunk_size)]

def merge_dicts(dict1: Dict[str, Any], dict2: Dict[str, Any]) -> Dict[str, Any]:
    """合并字典，dict2的值会覆盖dict1的值"""
    result = dict1.copy()
    result.update(dict2)
    return result

def get_nested_value(data: Dict[str, Any], keys: List[str], default: Any = None) -> Any:
    """获取嵌套字典的值"""
    try:
        for key in keys:
            data = data[key]
        return data
    except (KeyError, TypeError):
        return default

def set_nested_value(data: Dict[str, Any], keys: List[str], value: Any) -> bool:
    """设置嵌套字典的值"""
    try:
        current = data
        for key in keys[:-1]:
            if key not in current:
                current[key] = {}
            current = current[key]
        current[keys[-1]] = value
        return True
    except Exception as e:
        logger.error(f"设置嵌套值失败: {e}")
        return False

def filter_dict(data: Dict[str, Any], allowed_keys: List[str]) -> Dict[str, Any]:
    """过滤字典，只保留指定的键"""
    return {k: v for k, v in data.items() if k in allowed_keys}

def remove_none_values(data: Dict[str, Any]) -> Dict[str, Any]:
    """移除值为None的键"""
    return {k: v for k, v in data.items() if v is not None}

def format_bytes(bytes_value: int) -> str:
    """格式化字节数"""
    for unit in ['B', 'KB', 'MB', 'GB', 'TB']:
        if bytes_value < 1024.0:
            return f"{bytes_value:.2f} {unit}"
        bytes_value /= 1024.0
    return f"{bytes_value:.2f} PB"

def format_duration(seconds: float) -> str:
    """格式化持续时间"""
    if seconds < 60:
        return f"{seconds:.1f}秒"
    elif seconds < 3600:
        minutes = seconds / 60
        return f"{minutes:.1f}分钟"
    elif seconds < 86400:
        hours = seconds / 3600
        return f"{hours:.1f}小时"
    else:
        days = seconds / 86400
        return f"{days:.1f}天"

def retry_on_exception(func, max_retries: int = 3, delay: float = 1.0):
    """异常重试装饰器"""
    def wrapper(*args, **kwargs):
        for attempt in range(max_retries):
            try:
                return func(*args, **kwargs)
            except Exception as e:
                if attempt == max_retries - 1:
                    raise e
                logger.warning(f"函数 {func.__name__} 执行失败，第 {attempt + 1} 次重试: {e}")
                import time
                time.sleep(delay)
    return wrapper

def async_retry_on_exception(func, max_retries: int = 3, delay: float = 1.0):
    """异步异常重试装饰器"""
    async def wrapper(*args, **kwargs):
        for attempt in range(max_retries):
            try:
                return await func(*args, **kwargs)
            except Exception as e:
                if attempt == max_retries - 1:
                    raise e
                logger.warning(f"异步函数 {func.__name__} 执行失败，第 {attempt + 1} 次重试: {e}")
                import asyncio
                await asyncio.sleep(delay)
    return wrapper
