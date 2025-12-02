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
    APP_NAME: str = Field("历史数据服务", description="应用名称")
    APP_VERSION: str = Field("1.0.0", description="应用版本")
    DEBUG: bool = Field(False, description="调试模式")
    
    # 服务器设置
    HOST: str = "0.0.0.0"
    PORT: int = 6004
    
    # Redis设置 - 生产环境默认本地
    REDIS_HOST: str = "localhost"  # 生产环境默认本地
    REDIS_PORT: int = 6379
    REDIS_DB: int = 0
    REDIS_PASSWORD: Optional[str] = None
    REDIS_PREFIX: str = "hissrv:"
    
    # 开发环境Redis设置（通过.env文件覆盖）
    # REDIS_HOST: str = "192.168.30.62"  # 开发环境
    
    # InfluxDB设置 - 从配置文件读取
    INFLUXDB_URL: str = ""
    INFLUXDB_TOKEN: str = ""
    INFLUXDB_ORG: str = ""
    INFLUXDB_BUCKET: str = ""
    INFLUXDB_TIMEOUT: int = 30
    
    # 数据存储设置
    DATA_COLLECTION_INTERVAL: int = 5  # 秒
    DATA_BATCH_SIZE: int = 1000
    DATA_FLUSH_INTERVAL: int = 10  # 秒
    DATA_RETENTION_DAYS: int = 30
    
    # 定时任务设置
    SCHEDULER_ENABLED: bool = True
    CLEANUP_CRON: str = "0 2 * * *"
    STATISTICS_CRON: str = "0 1 * * *"
    HEALTH_CHECK_INTERVAL: int = 60
    
    # 日志设置
    LOG_LEVEL: str = "INFO"
    LOG_FILE: str = "logs/hissrv.log"
    
    # 安全设置
    CORS_ORIGINS: List[str] = ["*"]
    RATE_LIMIT_PER_MINUTE: int = 100
    
    # API设置
    API_PREFIX: str = "/hisApi"
    DEFAULT_PAGE_SIZE: int = 100
    MAX_PAGE_SIZE: int = 1000
    MAX_TIME_RANGE_DAYS: int = 365
    
    # 缓存设置
    CACHE_ENABLED: bool = True
    CACHE_TTL: int = 300
    
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