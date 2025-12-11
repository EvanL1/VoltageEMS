"""
Edge数据结构模型
基于Edge数据结构文档定义的数据模型
"""

from pydantic import BaseModel, Field
from typing import Dict, List, Any, Optional, Union
from datetime import datetime
from enum import Enum
import time

# DataType 不再定义为枚举，支持任意字符串类型（如 T/S/C/A/M 等）
# 数据类型会随着项目发展动态增加

class WebSocketMessageType(str, Enum):
    """WebSocket消息类型"""
    SUBSCRIBE = "subscribe"
    UNSUBSCRIBE = "unsubscribe"
    CONTROL = "control"
    PING = "ping"
    DATA_UPDATE = "data_update"
    DATA_BATCH = "data_batch"
    ALARM = "alarm"
    SUBSCRIBE_ACK = "subscribe_ack"
    UNSUBSCRIBE_ACK = "unsubscribe_ack"
    CONTROL_ACK = "control_ack"
    ERROR = "error"
    PONG = "pong"

class AlarmLevel(str, Enum):
    """告警级别"""
    LOW = "Low"
    MEDIUM = "Medium"
    HIGH = "High"
    CRITICAL = "Critical"

class AlarmStatus(str, Enum):
    """告警状态"""
    ACTIVE = "Active"
    ACKNOWLEDGED = "Acknowledged"
    RESOLVED = "Resolved"

# 基础WebSocket消息结构
class BaseWebSocketMessage(BaseModel):
    """基础WebSocket消息"""
    type: WebSocketMessageType
    id: str
    timestamp: int = Field(default_factory=lambda: int(time.time()))
    data: Optional[Dict[str, Any]] = None

# 客户端发送的消息
class SubscribeMessage(BaseWebSocketMessage):
    """订阅消息"""
    type: WebSocketMessageType = WebSocketMessageType.SUBSCRIBE
    data: Dict[str, Any] = Field(..., description="订阅数据")

class UnsubscribeMessage(BaseWebSocketMessage):
    """取消订阅消息"""
    type: WebSocketMessageType = WebSocketMessageType.UNSUBSCRIBE
    data: Dict[str, Any] = Field(..., description="取消订阅数据")

class ControlMessage(BaseWebSocketMessage):
    """控制命令消息"""
    type: WebSocketMessageType = WebSocketMessageType.CONTROL
    data: Dict[str, Any] = Field(..., description="控制命令数据")

class PingMessage(BaseWebSocketMessage):
    """心跳消息"""
    type: WebSocketMessageType = WebSocketMessageType.PING

# 服务端推送的消息
class DataUpdateMessage(BaseWebSocketMessage):
    """实时数据更新消息"""
    type: WebSocketMessageType = WebSocketMessageType.DATA_UPDATE
    data: Dict[str, Any] = Field(..., description="数据更新内容")

class DataBatchMessage(BaseWebSocketMessage):
    """批量数据更新消息"""
    type: WebSocketMessageType = WebSocketMessageType.DATA_BATCH
    data: Dict[str, Any] = Field(..., description="批量数据内容")

class AlarmMessage(BaseWebSocketMessage):
    """告警事件消息"""
    type: WebSocketMessageType = WebSocketMessageType.ALARM
    data: Dict[str, Any] = Field(..., description="告警事件内容")

class SubscribeAckMessage(BaseWebSocketMessage):
    """订阅确认消息"""
    type: WebSocketMessageType = WebSocketMessageType.SUBSCRIBE_ACK
    data: Dict[str, Any] = Field(..., description="订阅确认内容")

class UnsubscribeAckMessage(BaseWebSocketMessage):
    """取消订阅确认消息"""
    type: WebSocketMessageType = WebSocketMessageType.UNSUBSCRIBE_ACK
    data: Dict[str, Any] = Field(..., description="取消订阅确认内容")

class ControlAckMessage(BaseWebSocketMessage):
    """控制命令确认消息"""
    type: WebSocketMessageType = WebSocketMessageType.CONTROL_ACK
    data: Dict[str, Any] = Field(..., description="控制命令确认内容")

class ErrorMessage(BaseWebSocketMessage):
    """错误消息"""
    type: WebSocketMessageType = WebSocketMessageType.ERROR
    data: Dict[str, Any] = Field(..., description="错误信息内容")

class PongMessage(BaseWebSocketMessage):
    """心跳响应消息"""
    type: WebSocketMessageType = WebSocketMessageType.PONG
    data: Dict[str, Any] = Field(..., description="心跳响应内容")

# Redis数据结构模型
class ComsrvData(BaseModel):
    """通信服务数据结构"""
    channel_id: int
    data_type: str  # 数据类型字符串 (如 T/S/C/A/M 等，无限制)
    point_id: int
    value: Union[str, float, int]
    timestamp: Optional[int] = None

class ModsrvModel(BaseModel):
    """模型服务数据结构"""
    model_id: str
    name: str
    template: str
    properties: Dict[str, Any]
    mappings: Dict[str, Any]

class ModsrvMeasurement(BaseModel):
    """模型测量值"""
    model_id: str
    values: Dict[str, Union[str, float, int]]
    updated: int

class ModsrvAction(BaseModel):
    """模型控制值"""
    model_id: str
    values: Dict[str, Union[str, float, int]]
    updated: int

class AlarmRecord(BaseModel):
    """告警记录"""
    alarm_id: str
    title: str
    description: str
    level: AlarmLevel
    status: AlarmStatus
    source: str
    timestamp: int
    acknowledged: bool = False
    acknowledged_by: Optional[str] = None
    acknowledged_at: Optional[int] = None

class RuleDefinition(BaseModel):
    """规则定义"""
    rule_id: str
    name: str
    enabled: bool
    condition: str
    actions: List[Dict[str, Any]]
    cooldown: int

# 数据转换函数
def create_data_update_message(channel_id: int, data_type: str, values: Dict[str, Union[str, float, int]]) -> DataUpdateMessage:
    """创建数据更新消息"""
    return DataUpdateMessage(
        id=f"update_{channel_id}_{data_type}_{int(datetime.now().timestamp())}",
        data={
            "channel_id": channel_id,
            "data_type": data_type,
            "values": values
        }
    )

def create_alarm_message(alarm_id: str, channel_id: int, point_id: int, status: int, level: int, value: float, message: str) -> AlarmMessage:
    """创建告警消息"""
    return AlarmMessage(
        id=f"alarm_{alarm_id}",
        data={
            "alarm_id": alarm_id,
            "channel_id": channel_id,
            "point_id": point_id,
            "status": status,
            "level": level,
            "value": value,
            "message": message
        }
    )

def create_subscribe_ack_message(request_id: str, subscribed: List[int], failed: List[int]) -> dict:
    """创建订阅确认消息（简化格式）"""
    return {
        "type": "subscribe_ack",
        "timestamp": int(time.time()),
        "data": {
            "request_id": request_id,
            "subscribed": subscribed,
            "failed": failed,
            "total": len(subscribed)
        }
    }

def create_unsubscribe_ack_message(request_id: str, unsubscribed: List[int], failed: List[int]) -> dict:
    """创建取消订阅确认消息（简化格式）"""
    return {
        "type": "unsubscribe_ack",
        "timestamp": int(time.time()),
        "data": {
            "request_id": request_id,
            "unsubscribed": unsubscribed,
            "failed": failed,
            "total": len(unsubscribed)
        }
    }

def create_control_ack_message(request_id: str, command_id: str, status: str, success: bool, actual_value: Optional[float] = None) -> ControlAckMessage:
    """创建控制命令确认消息"""
    return ControlAckMessage(
        id=f"{request_id}_ack",
        data={
            "request_id": request_id,
            "command_id": command_id,
            "status": status,
            "result": {
                "success": success,
                "actual_value": actual_value
            }
        }
    )

def create_error_message(code: str, message: str, details: str, request_id: Optional[str] = None) -> ErrorMessage:
    """创建错误消息"""
    return ErrorMessage(
        id=f"err_{int(datetime.now().timestamp())}",
        data={
            "code": code,
            "message": message,
            "details": details,
            "request_id": request_id
        }
    )

def create_pong_message(request_id: str, latency: int) -> PongMessage:
    """创建心跳响应消息"""
    return PongMessage(
        id=f"{request_id}_pong",
        data={
            "server_time": int(time.time()),
            "latency": latency
        }
    )
