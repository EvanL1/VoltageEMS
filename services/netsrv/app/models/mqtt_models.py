"""
MQTT配置相关的数据模型
"""

from typing import Optional
from pydantic import BaseModel, Field


class SSLConfig(BaseModel):
    """SSL配置"""
    enabled: bool = Field(default=False, description="是否启用SSL")
    ca_cert: Optional[str] = Field(default=None, description="CA证书路径")
    client_cert: Optional[str] = Field(default=None, description="客户端证书路径")
    client_key: Optional[str] = Field(default=None, description="客户端私钥路径")


class ReconnectConfig(BaseModel):
    """重连配置"""
    enabled: bool = Field(default=True, description="是否启用自动重连")
    max_attempts: int = Field(default=10, ge=1, le=100, description="最大重连次数")
    delay: int = Field(default=5, ge=1, le=300, description="重连间隔（秒）")


class StatusConfig(BaseModel):
    """状态消息配置"""
    will_message_enabled: bool = Field(default=True, description="是否启用遗嘱消息")
    auto_online_message: bool = Field(default=True, description="连接成功后是否自动发送在线消息")


class BrokerConfig(BaseModel):
    """MQTT代理配置"""
    host: str = Field(..., min_length=1, max_length=255, description="MQTT代理地址")
    port: int = Field(default=1883, ge=1, le=65535, description="MQTT代理端口")
    username: Optional[str] = Field(default="", max_length=255, description="用户名")
    password: Optional[str] = Field(default="", max_length=255, description="密码")
    client_id: str = Field(..., min_length=1, max_length=255, description="客户端ID")
    keepalive: int = Field(default=60, ge=10, le=3600, description="保活时间（秒）")
    ssl: Optional[SSLConfig] = Field(default_factory=SSLConfig, description="SSL配置")
    reconnect: Optional[ReconnectConfig] = Field(default_factory=ReconnectConfig, description="重连配置")
    status: Optional[StatusConfig] = Field(default_factory=StatusConfig, description="状态消息配置")


class MQTTConnectionConfig(BaseModel):
    """完整的MQTT连接配置"""
    broker: BrokerConfig = Field(..., description="MQTT代理配置")


class MQTTConfigUpdateRequest(BaseModel):
    """MQTT配置更新请求"""
    mqtt_connection: MQTTConnectionConfig = Field(..., description="MQTT连接配置")


class MQTTConfigResponse(BaseModel):
    """MQTT配置响应"""
    status: str = Field(default="success", description="响应状态")
    message: str = Field(default="操作成功", description="响应消息")
    mqtt_connection: Optional[MQTTConnectionConfig] = Field(default=None, description="当前MQTT配置")


class MQTTReconnectRequest(BaseModel):
    """MQTT重连请求"""
    force_disconnect: bool = Field(default=True, description="是否强制断开当前连接")


class MQTTReconnectResponse(BaseModel):
    """MQTT重连响应"""
    status: str = Field(default="success", description="响应状态")
    message: str = Field(default="重连操作已启动", description="响应消息")
    connection_status: bool = Field(default=False, description="当前连接状态")


class MQTTStatusResponse(BaseModel):
    """MQTT状态响应"""
    status: str = Field(default="success", description="响应状态")
    connected: bool = Field(default=False, description="是否已连接")
    current_config: Optional[dict] = Field(default=None, description="当前配置信息")
    reconnect_attempts: int = Field(default=0, description="当前重连尝试次数")
