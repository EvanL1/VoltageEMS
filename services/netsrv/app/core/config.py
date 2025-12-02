"""
配置文件
包含所有环境变量和应用设置
"""

import os
from typing import Optional, List
from pydantic import Field
from pydantic_settings import BaseSettings

class Settings(BaseSettings):
    """应用配置类"""
    
    # 应用基本设置
    APP_NAME: str = Field("网络服务", description="应用名称")
    APP_VERSION: str = Field("1.0.0", description="应用版本")
    DEBUG: bool = Field(False, description="调试模式")
    
    # 服务器设置
    HOST: str = "0.0.0.0"
    PORT: int = 6006
    
    # Redis设置 - 生产环境默认本地
    REDIS_HOST: str = "localhost"  # 生产环境默认本地
    REDIS_PORT: int = 6379
    REDIS_DB: int = 0
    REDIS_PASSWORD: Optional[str] = None
    REDIS_PREFIX: str = "netsrv:"
    
    # 开发环境Redis设置（通过.env文件覆盖）
    # REDIS_HOST: str = "192.168.30.62"  # 开发环境
    
    # MQTT设置 - 云端连接
    MQTT_BROKER_HOST: str = "localhost"
    MQTT_BROKER_PORT: int = 1883
    MQTT_USERNAME: Optional[str] = None
    MQTT_PASSWORD: Optional[str] = None
    MQTT_CLIENT_ID: str = "netsrv_client"
    MQTT_KEEPALIVE: int = 60
    MQTT_SSL_ENABLED: bool = False
    
    # HTTP设置 - 云端API
    HTTP_TIMEOUT: int = 30
    HTTP_RETRY_COUNT: int = 3
    HTTP_RETRY_DELAY: int = 5
    
    # 数据转发设置
    DATA_FORWARD_INTERVAL: int = 5  # 秒
    DATA_BATCH_SIZE: int = 100
    DATA_FORMAT: str = "json"
    DATA_COMPRESSION: bool = False
    
    # 连接池设置
    CONNECTION_POOL_SIZE: int = 10
    CONNECTION_TIMEOUT: int = 30
    RECONNECT_DELAY: int = 5
    
    # 日志设置
    LOG_LEVEL: str = "INFO"
    LOG_FILE: str = "logs/netsrv.log"
    
    # 安全设置
    CORS_ORIGINS: List[str] = ["*"]
    RATE_LIMIT_PER_MINUTE: int = 100
    
    # 云端平台配置
    CLOUD_PLATFORM: str = "aliyun"
    CLOUD_ACCESS_KEY: Optional[str] = None
    CLOUD_SECRET_KEY: Optional[str] = None
    CLOUD_REGION: str = "cn-shanghai"
    CLOUD_INSTANCE_ID: Optional[str] = None
    
    # 配置文件路径
    CONFIG_DIR: str = "/app/config"  # Docker容器中的配置目录
    
    class Config:
        env_file = ".env"
        env_file_encoding = "utf-8"
        case_sensitive = True
        # 允许从环境变量覆盖配置
        env_prefix = ""

# 创建全局设置实例
settings = Settings()
