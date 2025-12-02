"""
告警数据模型
定义alert表和alert_event表的数据结构
"""

import json
from datetime import datetime
from typing import List, Optional, Dict, Any
from dataclasses import dataclass
from enum import Enum


class AlertStatus(Enum):
    """告警状态枚举"""
    ACTIVE = "active"      # 活跃告警
    RESOLVED = "resolved"  # 已恢复


class EventType(Enum):
    """事件类型枚举"""
    TRIGGER = "trigger"
    RECOVERY = "recovery"


@dataclass
class Alert:
    """告警数据类"""
    id: Optional[int] = None
    rule_id: int = None
    rule_snapshot: str = ""  # JSON格式的规则快照
    service_type: str = ""
    channel_id: int = None
    data_type: str = ""
    point_id: int = None
    rule_name: str = ""
    warning_level: int = 1
    operator: str = ""
    threshold_value: float = 0.0
    current_value: float = 0.0
    status: str = "active"
    triggered_at: Optional[int] = None  # 时间戳（秒）
    
    def to_dict(self) -> Dict[str, Any]:
        """转换为字典格式"""
        return {
            "id": self.id,
            "rule_id": self.rule_id,
            "rule_snapshot": json.loads(self.rule_snapshot) if self.rule_snapshot else {},
            "service_type": self.service_type,
            "channel_id": self.channel_id,
            "data_type": self.data_type,
            "point_id": self.point_id,
            "rule_name": self.rule_name,
            "warning_level": self.warning_level,
            "operator": self.operator,
            "threshold_value": self.threshold_value,
            "current_value": self.current_value,
            "status": self.status,
            "triggered_at": self.triggered_at,
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "Alert":
        """从字典创建实例"""
        return cls(
            id=data.get("id"),
            rule_id=data.get("rule_id"),
            rule_snapshot=data.get("rule_snapshot", "{}"),
            service_type=data.get("service_type", ""),
            channel_id=data.get("channel_id"),
            data_type=data.get("data_type", ""),
            point_id=data.get("point_id"),
            rule_name=data.get("rule_name", ""),
            warning_level=data.get("warning_level", 1),
            operator=data.get("operator", ""),
            threshold_value=data.get("threshold_value", 0.0),
            current_value=data.get("current_value", 0.0),
            status=data.get("status", "active"),
            triggered_at=cls.isoformat_to_timestamp(data.get("triggered_at")),
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
    
    def duration_seconds(self) -> Optional[int]:
        """计算告警持续时间（秒）"""
        if self.triggered_at:
            return int((datetime.now().timestamp() - self.triggered_at))
        return None


@dataclass
class AlertEvent:
    """告警事件历史数据类"""
    id: Optional[int] = None
    rule_id: int = None
    rule_snapshot: str = ""  # JSON格式的规则快照
    service_type: str = ""
    channel_id: int = None
    data_type: str = ""
    point_id: int = None
    rule_name: str = ""
    warning_level: int = 1
    operator: str = ""
    threshold_value: float = 0.0
    trigger_value: float = 0.0
    recovery_value: Optional[float] = None
    event_type: str = "trigger"  # trigger/recovery
    triggered_at: Optional[int] = None  # 告警触发时间戳（秒）
    recovered_at: Optional[int] = None  # 告警结束时间戳（秒）
    duration: Optional[int] = None           # 持续时间（秒）
    
    def to_dict(self) -> Dict[str, Any]:
        """转换为字典格式"""
        return {
            "id": self.id,
            "rule_id": self.rule_id,
            "rule_snapshot": json.loads(self.rule_snapshot) if self.rule_snapshot else {},
            "service_type": self.service_type,
            "channel_id": self.channel_id,
            "data_type": self.data_type,
            "point_id": self.point_id,
            "rule_name": self.rule_name,
            "warning_level": self.warning_level,
            "operator": self.operator,
            "threshold_value": self.threshold_value,
            "trigger_value": self.trigger_value,
            "recovery_value": self.recovery_value,
            "event_type": self.event_type,
            "triggered_at": self.triggered_at,
            "recovered_at": self.recovered_at,
            "duration": self.duration,
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "AlertEvent":
        """从字典创建实例"""
        return cls(
            id=data.get("id"),
            rule_id=data.get("rule_id"),
            rule_snapshot=data.get("rule_snapshot", "{}"),
            service_type=data.get("service_type", ""),
            channel_id=data.get("channel_id"),
            data_type=data.get("data_type", ""),
            point_id=data.get("point_id"),
            rule_name=data.get("rule_name", ""),
            warning_level=data.get("warning_level", 1),
            operator=data.get("operator", ""),
            threshold_value=data.get("threshold_value", 0.0),
            trigger_value=data.get("trigger_value", 0.0),
            recovery_value=data.get("recovery_value"),
            event_type=data.get("event_type", "trigger"),
            triggered_at=cls.isoformat_to_timestamp(data.get("triggered_at")),
            recovered_at=cls.isoformat_to_timestamp(data.get("recovered_at")),
            duration=data.get("duration"),
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
    
    @classmethod
    def from_alert(cls, alert: "Alert", event_type: str, recovery_value: Optional[float] = None) -> "AlertEvent":
        """从Alert对象创建AlertEvent"""
        now = int(datetime.now().timestamp())
        
        # 计算持续时间
        duration = None
        if alert.triggered_at:
            duration = int(now - alert.triggered_at)
        
        return cls(
            rule_id=alert.rule_id,
            rule_snapshot=alert.rule_snapshot,
            service_type=alert.service_type,
            channel_id=alert.channel_id,
            data_type=alert.data_type,
            point_id=alert.point_id,
            rule_name=alert.rule_name,
            warning_level=alert.warning_level,
            operator=alert.operator,
            threshold_value=alert.threshold_value,
            trigger_value=alert.current_value,
            recovery_value=recovery_value,
            event_type=event_type,
            triggered_at=alert.triggered_at,
            recovered_at=now if event_type == "recovery" else None,
            duration=duration
        )
