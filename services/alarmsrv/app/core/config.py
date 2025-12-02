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
    APP_NAME: str = Field("告警服务", description="应用名称")
    APP_VERSION: str = Field("1.0.0", description="应用版本")
    DEBUG: bool = Field(False, description="调试模式")
    
    # 服务器设置
    HOST: str = "0.0.0.0"
    PORT: int = 6007
    
    # Redis设置 - 生产环境默认本地
    REDIS_HOST: str = "localhost"  # 生产环境默认本地
    REDIS_PORT: int = 6379
    REDIS_DB: int = 0
    REDIS_PASSWORD: Optional[str] = None
    REDIS_PREFIX: str = "alarmsrv:"
    
    # 开发环境Redis设置（通过.env文件覆盖）
    # REDIS_HOST: str = "192.168.30.62"  # 开发环境
    
    # JWT设置
    JWT_SECRET_KEY: str = "your-secret-key-here-change-in-production"
    JWT_ALGORITHM: str = "HS256"
    ACCESS_TOKEN_EXPIRE_MINUTES: int = 30
    
    # WebSocket设置
    WEBSOCKET_HEARTBEAT_INTERVAL: int = 30
    WEBSOCKET_MAX_CONNECTIONS: int = 1000
    
    # 数据调度设置
    DATA_FETCH_INTERVAL: int = 5  # 秒
    DATA_BATCH_SIZE: int = 100
    
    # 数据库设置
    DATABASE_PATH: str = Field("/app/config/voltageems-alarm.db", description="数据库文件路径")
    DATABASE_TIMEOUT: int = Field(30, description="数据库连接超时时间（秒）")
    
    # 日志设置
    LOG_LEVEL: str = "INFO"
    LOG_FILE: str = "logs/alarmsrv.log"
    
    # 安全设置
    CORS_ORIGINS: List[str] = ["*"]
    RATE_LIMIT_PER_MINUTE: int = 100
    
    class Config:
        env_file = ".env"
        env_file_encoding = "utf-8"
        case_sensitive = True
        # 允许从环境变量覆盖配置
        env_prefix = ""

# 创建全局设置实例
settings = Settings()
