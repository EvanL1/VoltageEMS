"""
告警规则数据模型
定义alert_rule表的数据结构和操作方法
"""

import sqlite3
from datetime import datetime
from typing import List, Optional, Dict, Any
from dataclasses import dataclass
from enum import Enum


class WarningLevel(Enum):
    """告警级别枚举"""
    LOW = 1      # 低级告警
    MEDIUM = 2   # 中级告警
    HIGH = 3     # 高级告警


class ComparisonOperator(Enum):
    """比较操作符枚举"""
    GREATER_THAN = ">"
    LESS_THAN = "<"
    GREATER_EQUAL = ">="
    LESS_EQUAL = "<="
    EQUAL = "=="
    NOT_EQUAL = "!="


class ServiceType(Enum):
    """服务类型枚举"""
    COMSRV = "comsrv"      # 通信服务
    RULESRV = "rulesrv"    # 规则服务
    MODSRV = "modsrv"      # 模型服务
    ALARMSRV = "alarmsrv"  # 告警服务
    HISSRV = "hissrv"      # 历史服务
    NETSRV = "netsrv"      # 网络服务


class DataType(Enum):
    """数据类型枚举"""
    TELEMETRY = "T"  # 遥测
    SIGNAL = "S"     # 遥信
    CONTROL = "C"    # 遥控
    ADJUSTMENT = "A" # 遥调


@dataclass
class AlertRule:
    """告警规则数据类"""
    id: Optional[int] = None
    service_type: str = "comsrv"  # 服务类型：comsrv, rulesrv, modsrv等
    channel_id: int = None
    data_type: str = None  # T, S, C, A
    point_id: int = None
    rule_name: str = ""
    warning_level: int = 1
    operator: str = ">"
    value: float = 0.0
    enabled: bool = True
    description: str = ""
    created_at: Optional[int] = None  # 时间戳（秒）
    updated_at: Optional[int] = None  # 时间戳（秒）

    def to_dict(self) -> Dict[str, Any]:
        """转换为字典格式"""
        return {
            "id": self.id,
            "service_type": self.service_type,
            "channel_id": self.channel_id,
            "data_type": self.data_type,
            "point_id": self.point_id,
            "rule_name": self.rule_name,
            "warning_level": self.warning_level,
            "operator": self.operator,
            "value": self.value,
            "enabled": self.enabled,
            "description": self.description,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "AlertRule":
        """从字典创建实例"""
        return cls(
            id=data.get("id"),
            service_type=data.get("service_type", "comsrv"),
            channel_id=data.get("channel_id"),
            data_type=data.get("data_type"),
            point_id=data.get("point_id"),
            rule_name=data.get("rule_name", ""),
            warning_level=data.get("warning_level", 1),
            operator=data.get("operator", ">"),
            value=data.get("value", 0.0),
            enabled=data.get("enabled", True),
            description=data.get("description", ""),
            created_at=cls.isoformat_to_timestamp(data.get("created_at")),
            updated_at=cls.isoformat_to_timestamp(data.get("updated_at")),
        )
    
    @staticmethod
    def timestamp_to_isoformat(timestamp: Optional[int]) -> Optional[str]:
        """将时间戳转换为ISO格式字符串"""
        if timestamp is None:
            return None
        return datetime.fromtimestamp(timestamp).isoformat()
    
    @staticmethod
    def isoformat_to_timestamp(iso_string: Optional[str]) -> Optional[int]:
        """将ISO格式字符串转换为时间戳"""
        if iso_string is None:
            return None
        try:
            # 支持多种时间格式
            if 'T' in iso_string:
                dt = datetime.fromisoformat(iso_string)
            else:
                dt = datetime.fromisoformat(iso_string.replace(' ', 'T'))
            return int(dt.timestamp())
        except ValueError:
            return None

    def redis_key(self) -> str:
        """生成对应的Redis键"""
        return f"{self.service_type}:{self.channel_id}:{self.data_type}"

    def validate(self) -> bool:
        """验证规则有效性"""
        if not self.service_type or self.service_type.strip() == "":
            return False
            
        # 暂时只支持这些服务类型
        valid_services = ["comsrv", "rulesrv", "modsrv", "alarmsrv", "hissrv", "netsrv"]
        if self.service_type not in valid_services:
            return False
        
        if not self.channel_id or self.channel_id <= 0:
            return False
        
        # data_type现在支持自定义，不再限制固定值
        if not self.data_type or self.data_type.strip() == "":
            return False
            
        if not self.point_id or self.point_id <= 0:
            return False
            
        if not self.rule_name or self.rule_name.strip() == "":
            return False
            
        if self.warning_level not in [1, 2, 3]:
            return False
            
        if self.operator not in [">", "<", ">=", "<=", "==", "!="]:
            return False
            
        return True

    def validate_detailed(self) -> tuple[bool, str]:
        """详细验证规则有效性，返回验证结果和错误信息"""
        if not self.service_type or self.service_type.strip() == "":
            return False, "服务类型不能为空"
            
        # 暂时只支持这些服务类型
        valid_services = ["comsrv", "rulesrv", "modsrv", "alarmsrv", "hissrv", "netsrv"]
        if self.service_type not in valid_services:
            return False, f"不支持的服务类型'{self.service_type}'，支持的类型: {', '.join(valid_services)}"
        
        if not self.channel_id or self.channel_id <= 0:
            return False, f"通道ID必须大于0，当前值: {self.channel_id}"
        
        # data_type现在支持自定义，不再限制固定值
        if not self.data_type or self.data_type.strip() == "":
            return False, f"数据类型不能为空，当前值: '{self.data_type}'"
            
        if not self.point_id or self.point_id <= 0:
            return False, f"点位ID必须大于0，当前值: {self.point_id}"
            
        if not self.rule_name or self.rule_name.strip() == "":
            return False, "规则名称不能为空"
            
        if self.warning_level not in [1, 2, 3]:
            return False, f"告警级别必须为1(一般)、2(重要)或3(紧急)，当前值: {self.warning_level}"
            
        if self.operator not in [">", "<", ">=", "<=", "==", "!="]:
            return False, f"不支持的比较操作符'{self.operator}'，支持的操作符: >, <, >=, <=, ==, !="
        
        # 检查规则名称长度
        if len(self.rule_name.strip()) > 100:
            return False, f"规则名称过长，最大100字符，当前: {len(self.rule_name)}字符"
        
        # 检查描述长度
        if self.description and len(self.description) > 500:
            return False, f"描述过长，最大500字符，当前: {len(self.description)}字符"
        
        try:
            # 验证阈值是否为有效数值
            float(self.value)
        except (ValueError, TypeError):
            return False, f"阈值必须为有效数值，当前值: {self.value}"
            
        return True, "验证通过"

    def evaluate(self, current_value: float) -> bool:
        """评估规则是否触发"""
        if not self.enabled:
            return False
            
        try:
            current_value = float(current_value)
            threshold = float(self.value)
            
            if self.operator == ">":
                return current_value > threshold
            elif self.operator == "<":
                return current_value < threshold
            elif self.operator == ">=":
                return current_value >= threshold
            elif self.operator == "<=":
                return current_value <= threshold
            elif self.operator == "==":
                return abs(current_value - threshold) < 1e-6  # 浮点数比较
            elif self.operator == "!=":
                return abs(current_value - threshold) >= 1e-6
                
        except (ValueError, TypeError):
            return False
            
        return False
