"""
MCP Tool Discovery for LangChain agents.

Discovers MCP servers from b00t datums and converts tools to LangChain BaseTool.
"""

import asyncio
import logging
import tomllib
from pathlib import Path
from typing import Any

from langchain_core.tools import BaseTool, StructuredTool
from pydantic import BaseModel, Field

from .types import MCPServerConfig

log = logging.getLogger(__name__)


class MCPToolDiscovery:
    """Discovers MCP tools from b00t datums and converts to LangChain BaseTool."""

    def __init__(self, datum_path: Path) -> None:
        """
        Initialize MCP tool discovery.

        Args:
            datum_path: Path to _b00t_ directory containing MCP datums
        """
        self.datum_path = datum_path
        self.tools: list[BaseTool] = []
        self.mcp_servers: list[MCPServerConfig] = []

    async def initialize(self) -> None:
        """Discover MCP servers and load tools."""
        log.info("🔍 Discovering MCP servers from datums...")

        # Find all .mcp.toml files
        mcp_datums = list(self.datum_path.glob("*.mcp.toml"))
        log.info(f"Found {len(mcp_datums)} MCP datum files")

        # Parse each datum
        for datum_file in mcp_datums:
            try:
                servers = self._parse_mcp_datum(datum_file)
                self.mcp_servers.extend(servers)
                log.info(f"  ✅ {datum_file.name}: {len(servers)} server(s)")
            except Exception as e:
                log.warning(f"  ⚠️  {datum_file.name}: {e}")

        log.info(f"Total MCP servers discovered: {len(self.mcp_servers)}")

        # Connect to servers and discover tools
        # 🤓: Full MCP protocol implementation would go here
        # For now, create placeholder tools for testing
        await self._discover_tools()

    def _parse_mcp_datum(self, datum_file: Path) -> list[MCPServerConfig]:
        """
        Parse MCP datum TOML file.

        Args:
            datum_file: Path to .mcp.toml file

        Returns:
            List of MCP server configurations
        """
        with open(datum_file, "rb") as f:
            data = tomllib.load(f)

        servers: list[MCPServerConfig] = []
        name = data.get("b00t", {}).get("name", datum_file.stem)

        # Parse stdio configurations
        stdio_configs = data.get("b00t", {}).get("mcp", {}).get("stdio", [])
        if isinstance(stdio_configs, dict):
            # Single stdio config (non-array form)
            stdio_configs = [stdio_configs]

        for idx, stdio in enumerate(stdio_configs):
            if not isinstance(stdio, dict):
                continue

            server = MCPServerConfig(
                name=f"{name}-{idx}" if len(stdio_configs) > 1 else name,
                transport="stdio",
                command=stdio.get("command", ""),
                args=stdio.get("args", []),
            )
            servers.append(server)

        # Parse HTTP configurations (if any)
        http_configs = data.get("b00t", {}).get("mcp", {}).get("http", [])
        if isinstance(http_configs, dict):
            http_configs = [http_configs]

        for idx, http in enumerate(http_configs):
            if not isinstance(http, dict):
                continue

            server = MCPServerConfig(
                name=f"{name}-http-{idx}" if len(http_configs) > 1 else f"{name}-http",
                transport="http",
                url=http.get("url", ""),
            )
            servers.append(server)

        return servers

    async def _discover_tools(self) -> None:
        """
        Discover tools from MCP servers.

        🤓: This is a placeholder implementation. Full MCP protocol integration
        requires fastmcp or similar client library to:
        1. Start stdio processes or connect to HTTP servers
        2. Call list_tools JSON-RPC method
        3. Convert JSON-Schema -> Pydantic -> LangChain BaseTool
        """
        log.info("🛠️  Discovering tools from MCP servers...")

        # Create placeholder tools for common MCP servers
        # 🤓: These will be replaced with actual tool discovery via MCP protocol
        placeholder_tools = self._create_placeholder_tools()
        self.tools.extend(placeholder_tools)

        log.info(f"Total tools available: {len(self.tools)}")

    def _create_placeholder_tools(self) -> list[BaseTool]:
        """
        Create placeholder tools for testing.

        🤓: Remove this once full MCP protocol is implemented.
        """
        tools: list[BaseTool] = []

        # Common server names to create placeholder tools for
        server_names = {server.name for server in self.mcp_servers}

        # crawl4ai placeholder
        if any("crawl4ai" in name for name in server_names):
            tools.append(
                StructuredTool.from_function(
                    func=lambda url: f"Crawled: {url}",
                    name="crawl4ai_crawl",
                    description="Crawl a URL and extract clean markdown content",
                )
            )

        # github placeholder
        if any("github" in name for name in server_names):
            tools.append(
                StructuredTool.from_function(
                    func=lambda repo: f"GitHub: {repo}",
                    name="github_get_repo",
                    description="Get repository information from GitHub",
                )
            )

        # grok placeholder
        if any("grok" in name for name in server_names):
            tools.append(
                StructuredTool.from_function(
                    func=lambda query: f"Grok search: {query}",
                    name="grok_search",
                    description="Search b00t knowledge base",
                )
            )

        return tools

    def get_tools_by_name(self, tool_names: list[str]) -> list[BaseTool]:
        """
        Get specific tools by name.

        Args:
            tool_names: List of tool names or MCP server names

        Returns:
            List of matching BaseTool instances
        """
        # Match tools by exact name or by MCP server prefix
        matched_tools: list[BaseTool] = []

        for tool_name in tool_names:
            # Exact match
            for tool in self.tools:
                if tool.name == tool_name:
                    matched_tools.append(tool)
                    continue

            # Prefix match (e.g., "crawl4ai" matches "crawl4ai_crawl")
            for tool in self.tools:
                if tool.name.startswith(tool_name):
                    matched_tools.append(tool)

        return matched_tools
