# AWS Tool Generator Refactoring Plan

## 1. Assessment of Current Implementations

An analysis of the provided code snippets for `AwsHelper` and `AWSToolGenerator` reveals several inconsistencies and opportunities for improvement. The goal of this refactoring is to create a single, unified, best-practice implementation that is DRY, robust, and maintainable.

### `AwsHelper` Analysis

-   **Caching:** Multiple snippets use a class-level dictionary (`_clients`) to cache `boto3` clients, which is a performance best practice. One snippet explicitly omits this.
-   **Configuration & Inheritance:** One implementation inherits from `pydantic.BaseModel`, enabling structured configuration, while others are plain Python objects.
-   **Region/Credential Management:** The source for AWS region and credentials varies. Some take it as a method argument, while others pull from a central config function (`get_config().aws`). The latter is a more robust and DRY pattern.
-   **User Agent:** The `user_agent_extra` string is inconsistent, with some versions hardcoding a version number and others omitting it entirely. Using a dynamic `__version__` variable is the best practice.

### `AWSToolGenerator` Analysis

-   **Helper Instantiation:** The method for creating and using the `AwsHelper` is inconsistent. Some instantiate it directly in `__init__`, one allows for dependency injection, one creates a client without a helper, and one uses a shared, class-level `AwsHelper` instance. The shared class-level instance is the most efficient pattern as it ensures the client cache is shared across all generated tools.

## 2. Proposed Unified Implementation

Based on the analysis, the following unified implementations are proposed.

### Unified `AwsHelper`

This version combines the best practices into a single, robust class.

-   Inherits from `pydantic.BaseModel` for structured configuration.
-   Uses a shared singleton pattern to ensure a single source of truth for clients and configuration.
-   Caches clients in a class-level dictionary for performance.
-   Retrieves configuration (e.g., region) from a central, reliable source.
-   Uses a dynamic version string for the user agent.

```python
# proposed in a new file, e.g., b00t_aws_tools/helpers.py

from typing import Any, ClassVar, Dict, Optional
from pydantic import BaseModel
import boto3
from botocore.config import Config

# Assume __version__ is available, e.g., from __init__.py
# from . import __version__
__version__ = "0.2.0" # Placeholder

# Assume a central config system exists
# from b00t.config import get_config

class AwsConfig(BaseModel):
    region_name: Optional[str] = "us-east-1"

def get_config():
    # Placeholder for the actual config loader
    return {"aws": AwsConfig()}

class AwsHelper(BaseModel):
    """
    Unified helper class for AWS operations.

    - Provides boto3 clients with performance caching.
    - Manages configuration from a central source.
    - Implemented as a singleton to ensure a single client cache.
    """
    _instance: ClassVar[Optional["AwsHelper"]] = None
    _clients: ClassVar[Dict[str, Any]] = {}
    
    region_name: Optional[str]

    def __new__(cls, *args, **kwargs):
        if not cls._instance:
            # In a real scenario, configuration would be loaded here
            aws_config = get_config()["aws"]
            cls._instance = super().__new__(cls)
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

```

### Unified `AWSToolGenerator`

This version is simplified to use the singleton `AwsHelper`, ensuring efficiency and consistency.

```python
# proposed in a new file, e.g., b00t_aws_tools/generator.py

# from .helpers import AwsHelper
# from b00t.mcp import AbstractToolGenerator

class AWSToolGenerator(AbstractToolGenerator):
    """
    Generates MCP tools for AWS services using a shared, unified AwsHelper.
    """
    # Use a single, shared helper instance for all tool generators
    # This ensures the client cache is shared.
    aws_helper = AwsHelper()

    def __init__(self, service_name: str):
        self.service_name = service_name
        self.client = self.aws_helper.get_client(service_name)
        self.service_model = self.client.meta.service_model
        
        super().__init__(
            tool_name=f"aws-{service_name}",
            pretty_name=f"AWS {self.service_model.service_id}",
            description=self.service_model.documentation,
        )

    def generate_tools(self) -> list:
        # Implementation for generating tools from the client would go here
        return []

```

## 3. Refactoring Steps

1.  Create a new, dedicated Python package for AWS tools (e.g., `b00t_aws_tools`).
2.  Implement the unified `AwsHelper` and `AWSToolGenerator` classes in this new package as defined above.
3.  Replace all existing, inconsistent implementations throughout the codebase with imports from the new, unified package.
4.  Ensure a central configuration system (`get_config()`) is in place and used by the `AwsHelper`.
5.  Verify that the `__version__` is correctly sourced within the package.
6.  Add unit and integration tests for the new unified components.
