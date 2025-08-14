"""Configuration management for Network Service"""

import os
from typing import List, Optional, Dict, Any
from pydantic import BaseModel, Field
import yaml


class ServiceConfig(BaseModel):
    """Service configuration"""

    name: str = Field(default="netsrv", description="Service name")
    port: int = Field(default=6006, description="Service port")


class RedisConfig(BaseModel):
    """Redis configuration"""

    url: str = Field(default="redis://localhost:6379", description="Redis URL")
    data_keys: List[str] = Field(
        default_factory=lambda: ["comsrv:*:T"], description="Data key patterns"
    )
    poll_interval_secs: int = Field(
        default=5, description="Polling interval in seconds"
    )


class DataConfig(BaseModel):
    """Data forwarding configuration"""

    redis_data_key: str = Field(
        default="comsrv:*:T", description="Redis data key pattern"
    )
    redis_polling_interval_secs: int = Field(default=5, description="Polling interval")
    batch_size: int = Field(default=100, description="Batch size for data collection")


class MqttConfig(BaseModel):
    """MQTT configuration"""

    name: str = Field(default="mqtt1", description="MQTT client name")
    broker: str = Field(default="mqtt://localhost:1883", description="MQTT broker URL")
    client_id: str = Field(default="netsrv", description="MQTT client ID")
    username: Optional[str] = None
    password: Optional[str] = None
    topic_prefix: str = Field(default="voltage/data", description="Topic prefix")
    qos: int = Field(default=1, ge=0, le=2, description="QoS level")
    format_type: str = Field(default="json", description="Data format (json/ascii)")


class HttpConfig(BaseModel):
    """HTTP configuration"""

    name: str = Field(default="http1", description="HTTP client name")
    url: str = Field(..., description="HTTP endpoint URL")
    method: str = Field(default="POST", description="HTTP method")
    headers: Dict[str, str] = Field(default_factory=dict, description="HTTP headers")
    timeout_secs: int = Field(default=30, description="Request timeout")
    format_type: str = Field(default="json", description="Data format (json/ascii)")


class Config(BaseModel):
    """Main configuration"""

    service: ServiceConfig = Field(default_factory=ServiceConfig)
    redis: RedisConfig = Field(default_factory=RedisConfig)
    data: DataConfig = Field(default_factory=DataConfig)
    networks: List[Dict[str, Any]] = Field(
        default_factory=list, description="Network configurations"
    )

    @classmethod
    def load(cls, config_path: Optional[str] = None) -> "Config":
        """Load configuration from file or environment"""
        # Default config path
        if config_path is None:
            config_path = os.getenv("CONFIG_PATH", "/app/config/netsrv/netsrv.yaml")

        # Load from file if exists
        if os.path.exists(config_path):
            with open(config_path, "r") as f:
                data = yaml.safe_load(f)
                if data:
                    # Process network configurations
                    if "networks" in data:
                        processed_networks = []
                        for net in data["networks"]:
                            if "mqtt" in net:
                                processed_networks.append(
                                    {"mqtt": MqttConfig(**net["mqtt"])}
                                )
                            elif "http" in net:
                                processed_networks.append(
                                    {"http": HttpConfig(**net["http"])}
                                )
                        data["networks"] = processed_networks
                    return cls(**data)

        # Load from environment variables
        config = cls()

        # Override with env vars
        if redis_url := os.getenv("REDIS_URL"):
            config.redis.url = redis_url
        if poll_interval := os.getenv("REDIS_POLL_INTERVAL"):
            config.redis.poll_interval_secs = int(poll_interval)

        return config
