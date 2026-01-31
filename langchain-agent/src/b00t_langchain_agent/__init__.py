"""
b00t LangChain Agent Service

LangChain v1.0 agent service with MCP tool discovery and k0mmand3r IPC.
"""

__version__ = "0.1.0"

from .agent_service import AgentService
from .job_executor import JobExecutor
from .k0mmand3r import K0mmand3rListener
from .mcp_tools import MCPToolDiscovery

__all__ = ["AgentService", "JobExecutor", "K0mmand3rListener", "MCPToolDiscovery"]
