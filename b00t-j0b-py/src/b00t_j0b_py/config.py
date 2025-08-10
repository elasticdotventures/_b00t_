"""Configuration management for b00t-j0b-py."""

import os
from pydantic_settings import BaseSettings
from pydantic import Field
from typing import Optional


class CrawlerConfig(BaseSettings):
    """Configuration for the web crawler."""
    
    # Redis Configuration
    redis_url: str = Field(default="redis://localhost:6379/0", env="REDIS_URL")
    
    # Crawler Configuration
    user_agent: str = Field(
        default="b00t-j0b-py/0.1.0 (+https://github.com/elasticdotventures/dotfiles)",
        env="CRAWLER_USER_AGENT"
    )
    delay: float = Field(default=1.0, env="CRAWLER_DELAY")
    max_depth: int = Field(default=3, env="CRAWLER_MAX_DEPTH")
    timeout: int = Field(default=30, env="CRAWLER_TIMEOUT")
    
    # Content Processing
    max_content_size: int = Field(default=10485760, env="MAX_CONTENT_SIZE")  # 10MB
    chunk_size: int = Field(default=8192, env="CHUNK_SIZE")
    
    # Job Configuration
    default_queue: str = Field(default="default", env="RQ_DEFAULT_QUEUE")
    high_queue: str = Field(default="high", env="RQ_HIGH_QUEUE")
    low_queue: str = Field(default="low", env="RQ_LOW_QUEUE")
    
    class Config:
        env_file = ".env"
        case_sensitive = False
        extra = "allow"  # Allow extra fields from environment


# Global config instance
config = CrawlerConfig()