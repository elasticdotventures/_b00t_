from typing import Any, ClassVar, Dict, Optional
from pydantic import BaseModel
import boto3
from botocore.config import Config

from . import __version__

# This is a placeholder for the actual b00t configuration system.
# In a real implementation, this would read from a central TOML file
# or another configuration management tool.
class AwsConfig(BaseModel):
    region_name: Optional[str] = "us-east-1"

def get_config() -> dict:
    """Placeholder for the actual b00t config loader."""
    return {"aws": AwsConfig()}


class AwsHelper:
    """
    Unified helper class for AWS operations.

    - Provides boto3 clients with performance caching.
    - Manages configuration from a central source.
    - Implemented as a singleton to ensure a single client cache across the application.
    """
    _instance: ClassVar[Optional["AwsHelper"]] = None
    _clients: ClassVar[Dict[str, Any]] = {}

    def __new__(cls, *args, **kwargs):
        if not cls._instance:
            cls._instance = super().__new__(cls)
            # Load configuration only once when the first instance is created.
            aws_config = get_config()["aws"]
            cls._instance.region_name = aws_config.region_name
        return cls._instance

    def get_client(self, service_name: str) -> Any:
        """
        Get a boto3 client for a given service.
        Clients are cached for performance.
        """
        if service_name not in self._clients:
            print(f"Creating new boto3 client for {service_name} in region {self.region_name}")
            config = Config(user_agent_extra=f"b00t-aws-mcp/{__version__}")
            self._clients[service_name] = boto3.client(
                service_name,
                region_name=self.region_name,
                config=config
            )
        return self._clients[service_name]
