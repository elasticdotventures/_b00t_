"""Type definitions for b00t LangChain agent service."""

from enum import Enum
from typing import Any, Literal
from pydantic import BaseModel, Field


class AgentAction(str, Enum):
    """Agent slash command actions."""

    CREATE = "create"
    RUN = "run"
    BROADCAST = "broadcast"
    CALL = "call"
    STATUS = "status"
    DELETE = "delete"


class ChainAction(str, Enum):
    """Chain slash command actions."""

    RUN = "run"
    STATUS = "status"


class K0mmand3rMessage(BaseModel):
    """Message from k0mmand3r parser via Redis."""

    verb: Literal["agent", "chain"] | None = None
    params: dict[str, str] = Field(default_factory=dict)
    content: str | None = None
    timestamp: str | None = None
    agent_id: str | None = None


class AgentConfig(BaseModel):
    """Agent configuration from datum."""

    name: str
    description: str | None = None
    model: str
    tools: list[str] = Field(default_factory=list)
    middleware: list[str] = Field(default_factory=list)
    system_prompt: str
    max_iterations: int = 10
    timeout_seconds: int = 300
    peer_agents: list[str] = Field(default_factory=list)


class ChainConfig(BaseModel):
    """Chain configuration from datum."""

    name: str
    description: str | None = None
    steps: list[dict[str, Any]]


class AgentResult(BaseModel):
    """Result from agent execution."""

    success: bool
    agent_name: str | None = None
    output: str | None = None
    error: str | None = None
    message: str
    metadata: dict[str, Any] = Field(default_factory=dict)


class MCPServerConfig(BaseModel):
    """MCP server connection configuration."""

    name: str
    transport: Literal["http", "stdio", "docker"]
    url: str | None = None
    command: str | None = None
    args: list[str] = Field(default_factory=list)


class MiddlewareConfig(BaseModel):
    """Middleware configuration."""

    enabled: bool = True
    config: dict[str, Any] = Field(default_factory=dict)
