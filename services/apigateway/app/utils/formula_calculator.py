"""
计算公式解析和执行引擎
用于解析和执行类似 "comsrv:1:T:4+comsrv:6:T:2*10" 的公式
"""

import re
import logging
from typing import Dict, Any, Optional, List
from app.core.edge_data_client import EdgeDataClient

logger = logging.getLogger(__name__)


class FormulaCalculator:
    """公式计算器"""
    
    def __init__(self, edge_data_client: EdgeDataClient):
        self.edge_data_client = edge_data_client
        # 匹配模式：comsrv:channel_id:type:point_id
        self.point_pattern = re.compile(r'([a-zA-Z]+):(\d+):([A-Z]):(\d+)')
    
    async def calculate(self, formula: str) -> Optional[float]:
        """
        计算公式的值
        
        Args:
            formula: 计算公式，如 "comsrv:1:T:4+comsrv:6:T:2*10"
        
        Returns:
            计算结果，如果计算失败返回 None
        """
        if not formula or not formula.strip():
            return None
        
        try:
            # 1. 提取所有的点位引用
            matches = self.point_pattern.findall(formula)
            
            if not matches:
                # 没有匹配到点位，可能是纯数字表达式
                try:
                    return float(eval(formula))
                except:
                    logger.warning(f"无法计算公式: {formula}")
                    return None
            
            # 2. 替换公式中的点位引用为实际值
            expression = formula
            point_values = {}
            
            for match in matches:
                source, channel_id, data_type, point_id = match
                point_ref = f"{source}:{channel_id}:{data_type}:{point_id}"
                
                # 如果已经获取过这个点位的值，直接使用
                if point_ref in point_values:
                    continue
                
                # 从 Redis 获取点位值
                value = await self._get_point_value(source, int(channel_id), data_type, int(point_id))
                
                if value is None:
                    logger.warning(f"无法获取点位值: {point_ref}")
                    # 使用0作为默认值
                    value = 0.0
                
                point_values[point_ref] = value
            
            # 3. 替换表达式中的所有点位引用
            for point_ref, value in point_values.items():
                expression = expression.replace(point_ref, str(value))
            
            # 4. 计算表达式
            # 使用 eval 计算（注意：这里应该做安全检查）
            result = eval(expression, {"__builtins__": {}}, {})
            
            return float(result)
            
        except Exception as e:
            logger.error(f"计算公式失败: {formula}, 错误: {e}")
            return None
    
    async def _get_point_value(
        self, 
        source: str, 
        channel_id: int, 
        data_type: str, 
        point_id: int
    ) -> Optional[float]:
        """
        从 Redis 获取指定点位的值
        
        Args:
            source: 数据源（如 comsrv）
            channel_id: 通道ID
            data_type: 数据类型（如 T, S, C, A）
            point_id: 点位ID
        
        Returns:
            点位值，如果获取失败返回 None
        """
        try:
            # 获取通道数据
            data = await self.edge_data_client.get_data(channel_id, data_type, source)
            
            if not data:
                return None
            
            # 提取 values 字段
            values = data.get("values", data)
            
            # 获取指定点位的值
            point_value = values.get(str(point_id))
            
            if point_value is None:
                return None
            
            # 转换为浮点数
            return float(point_value)
            
        except Exception as e:
            logger.debug(f"获取点位值失败: {source}:{channel_id}:{data_type}:{point_id}, 错误: {e}")
            return None
    
    def validate_formula(self, formula: str) -> tuple[bool, str]:
        """
        验证公式的语法正确性（不执行计算）
        
        Returns:
            (是否有效, 错误信息)
        """
        if not formula or not formula.strip():
            return True, ""
        
        try:
            # 检查是否包含非法字符
            allowed_chars = set("0123456789+-*/().,:ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz ")
            if not set(formula).issubset(allowed_chars):
                return False, "公式包含非法字符"
            
            # 检查是否包含点位引用
            matches = self.point_pattern.findall(formula)
            
            if not matches:
                # 尝试作为数学表达式验证
                try:
                    eval(formula, {"__builtins__": {}}, {})
                    return True, ""
                except:
                    return False, "无效的数学表达式"
            
            # 验证点位引用格式
            for match in matches:
                source, channel_id, data_type, point_id = match
                if not source or not channel_id.isdigit() or not point_id.isdigit():
                    return False, f"无效的点位引用格式: {source}:{channel_id}:{data_type}:{point_id}"
            
            return True, ""
            
        except Exception as e:
            return False, f"验证失败: {str(e)}"
