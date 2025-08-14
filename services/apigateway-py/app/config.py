"""Configuration management for API Gateway"""

import os
from typing import Optional
from pydantic import BaseModel, Field
import yaml


class ServerConfig(BaseModel):
    """Server configuration"""

    host: str = Field(default="0.0.0.0", description="Server host")
    port: int = Field(default=6005, description="Server port")


class RedisConfig(BaseModel):
    """Redis configuration"""

    url: str = Field(default="redis://localhost:6379", description="Redis URL")
    decode_responses: bool = Field(default=True, description="Decode Redis responses")
    socket_connect_timeout: int = Field(default=5, description="Connection timeout")


class Config(BaseModel):
    """Main configuration"""

    server: ServerConfig = Field(default_factory=ServerConfig)
    redis: RedisConfig = Field(default_factory=RedisConfig)

    @classmethod
    def load(cls, config_path: Optional[str] = None) -> "Config":
        """Load configuration from file or environment"""
        # Default config path
        if config_path is None:
            config_path = os.getenv(
                "CONFIG_PATH", "/app/config/apigateway/apigateway.yaml"
            )

        # Load from file if exists
        if os.path.exists(config_path):
            with open(config_path, "r") as f:
                data = yaml.safe_load(f)
                return cls(**data) if data else cls()

        # Load from environment variables
        config = cls()

        # Override with env vars
        if host := os.getenv("SERVER_HOST"):
            config.server.host = host
        if port := os.getenv("SERVER_PORT"):
            config.server.port = int(port)
        if redis_url := os.getenv("REDIS_URL"):
            config.redis.url = redis_url

        return config
