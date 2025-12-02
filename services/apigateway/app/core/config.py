"""
配置文件
包含所有环境变量和应用设置
"""

import os
from typing import Optional, List
from pydantic_settings import BaseSettings

class Settings(BaseSettings):
    """应用配置类"""
    
    # 应用基本设置
    APP_NAME: str = "API网关"
    APP_VERSION: str = "1.0.0"
    DEBUG: bool = False
    
    # 服务器设置
    HOST: str = "0.0.0.0"
    PORT: int = 6005
    
    # Redis设置 - 生产环境默认本地
    REDIS_HOST: str = "localhost"  # 生产环境默认本地
    REDIS_PORT: int = 6379
    REDIS_DB: int = 0
    REDIS_PASSWORD: Optional[str] = None
    REDIS_PREFIX: str = "apigateway:"
    
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
    
    # 日志设置
    LOG_LEVEL: str = "INFO"
    LOG_FILE: str = "logs/apigateway.log"
    
    # 安全设置
    CORS_ORIGINS: List[str] = ["*"]
    RATE_LIMIT_PER_MINUTE: int = 100
    
    model_config = {
        "env_file": ".env",
        "case_sensitive": True
    }

# 创建全局设置实例
settings = Settings()
