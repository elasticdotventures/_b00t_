"""
b00t LangChain Agent Service

LangChain v1.0 agent service with MCP tool discovery and k0mmand3r IPC.
"""

__version__ = "0.1.0"

from .agent_service import AgentService
from .mcp_tools import MCPToolDiscovery
from .k0mmand3r import K0mmand3rListener

__all__ = ["AgentService", "MCPToolDiscovery", "K0mmand3rListener"]
