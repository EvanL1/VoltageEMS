"""
增强的时间解析工具
支持多种时间格式的输入，自动转换为标准格式
"""

import re
from datetime import datetime, time
from typing import Optional, Union

class TimeParser:
    """时间解析器，支持多种时间格式"""
    
    @staticmethod
    def parse_time(time_str: str) -> Optional[datetime]:
        """
        解析时间字符串，支持多种格式
        
        支持的格式：
        1. 完整ISO格式: 2025-08-21T00:18:56.273109
        2. 空格分隔: 2025-08-21 00:18:56.273109
        3. 仅日期: 2025-08-21
        4. 日期+时间: 2025-08-21 00:00:00
        5. 日期+小时: 2025-08-21 00
        6. 日期+小时分钟: 2025-08-21 00:00
        7. 相对时间: today, yesterday, now
        """
        if not time_str or not isinstance(time_str, str):
            return None
        
        time_str = time_str.strip()
        
        try:
            # 1. 尝试解析完整ISO格式
            if 'T' in time_str:
                return datetime.fromisoformat(time_str)
            
            # 2. 尝试解析空格分隔格式
            if ' ' in time_str:
                return datetime.fromisoformat(time_str.replace(' ', 'T'))
            
            # 3. 检查是否为仅日期格式 (YYYY-MM-DD)
            if re.match(r'^\d{4}-\d{2}-\d{2}$', time_str):
                # 自动添加时间部分
                return datetime.fromisoformat(f"{time_str}T00:00:00")
            
            # 4. 检查是否为日期+时间格式 (YYYY-MM-DD HH:MM:SS)
            if re.match(r'^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$', time_str):
                return datetime.fromisoformat(time_str.replace(' ', 'T'))
            
            # 5. 检查是否为日期+小时格式 (YYYY-MM-DD HH)
            if re.match(r'^\d{4}-\d{2}-\d{2} \d{2}$', time_str):
                return datetime.fromisoformat(f"{time_str}:00:00")
            
            # 6. 检查是否为日期+小时分钟格式 (YYYY-MM-DD HH:MM)
            if re.match(r'^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$', time_str):
                return datetime.fromisoformat(f"{time_str}:00")
            
            # 7. 处理相对时间
            if time_str.lower() in ['today', '今天']:
                today = datetime.now().replace(hour=0, minute=0, second=0, microsecond=0)
                return today
            
            if time_str.lower() in ['yesterday', '昨天']:
                from datetime import timedelta
                yesterday = datetime.now().replace(hour=0, minute=0, second=0, microsecond=0) - timedelta(days=1)
                return yesterday
            
            if time_str.lower() in ['now', '现在']:
                return datetime.now()
            
            # 8. 尝试其他可能的格式
            # 处理 YYYY-MM-DD HH:MM:SS.xxxxxx 格式
            if re.match(r'^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d+$', time_str):
                return datetime.fromisoformat(time_str.replace(' ', 'T'))
            
            # 处理 YYYY-MM-DD HH:MM:SS,xxxxxx 格式
            if re.match(r'^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2},\d+$', time_str):
                time_str = time_str.replace(',', '.')
                return datetime.fromisoformat(time_str.replace(' ', 'T'))
            
            # 如果都不匹配，返回None
            return None
            
        except ValueError as e:
            print(f"时间解析失败: {time_str}, 错误: {e}")
            return None
    
    @staticmethod
    def parse_time_range(start_time: Optional[str], end_time: Optional[str]) -> tuple[Optional[datetime], Optional[datetime]]:
        """
        解析时间范围，支持智能填充
        
        Args:
            start_time: 开始时间字符串
            end_time: 结束时间字符串
            
        Returns:
            (start_datetime, end_datetime) 元组
        """
        start_dt = None
        end_dt = None
        
        # 解析开始时间
        if start_time:
            start_dt = TimeParser.parse_time(start_time)
        
        # 解析结束时间
        if end_time:
            end_dt = TimeParser.parse_time(end_time)
        
        # 智能填充逻辑
        if start_dt and not end_dt:
            # 如果只有开始时间，结束时间设为当天结束
            end_dt = start_dt.replace(hour=23, minute=59, second=59, microsecond=999999)
        
        elif end_dt and not start_dt:
            # 如果只有结束时间，开始时间设为当天开始
            start_dt = end_dt.replace(hour=0, minute=0, second=0, microsecond=0)
        
        elif start_dt and end_dt:
            # 如果两个时间都有，确保开始时间 <= 结束时间
            if start_dt > end_dt:
                # 交换时间
                start_dt, end_dt = end_dt, start_dt
        
        return start_dt, end_dt
    
    @staticmethod
    def format_time_for_display(dt: datetime) -> str:
        """格式化时间为显示字符串"""
        return dt.strftime("%Y-%m-%d %H:%M:%S")
    
    @staticmethod
    def format_time_for_iso(dt: datetime) -> str:
        """格式化时间为ISO字符串"""
        return dt.isoformat()
    
    @staticmethod
    def get_time_examples() -> list[str]:
        """获取支持的时间格式示例"""
        return [
            "2025-08-21T00:18:56.273109",  # 完整ISO格式
            "2025-08-21 00:18:56.273109",  # 空格分隔
            "2025-08-21",                   # 仅日期
            "2025-08-21 00:00:00",         # 日期+时间
            "2025-08-21 00",                # 日期+小时
            "2025-08-21 00:00",            # 日期+小时分钟
            "today",                        # 今天
            "yesterday",                    # 昨天
            "now"                           # 现在
        ]

# 便捷函数
def parse_time(time_str: str) -> Optional[datetime]:
    """解析时间字符串"""
    return TimeParser.parse_time(time_str)

def parse_time_range(start_time: Optional[str], end_time: Optional[str]) -> tuple[Optional[datetime], Optional[datetime]]:
    """解析时间范围"""
    return TimeParser.parse_time_range(start_time, end_time)
