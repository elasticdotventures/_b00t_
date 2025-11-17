"""
MCP Tool Discovery for LangChain agents.

Discovers MCP servers from b00t datums and converts tools to LangChain BaseTool.
"""

import asyncio
import logging
import tomllib
from pathlib import Path
from typing import Any, Callable

from fastmcp import Client
from langchain_core.tools import BaseTool, StructuredTool
from pydantic import BaseModel, Field, create_model

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
        self.mcp_clients: dict[str, Client] = {}  # Active MCP client connections

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
        Discover tools from MCP servers using FastMCP client.

        Connects to each MCP server, lists available tools, and converts them
        to LangChain BaseTool instances with real execution.
        """
        log.info("🛠️  Discovering tools from MCP servers...")

        for server in self.mcp_servers:
            try:
                await self._connect_and_discover(server)
            except Exception as e:
                log.warning(f"⚠️  Failed to connect to {server.name}: {e}")
                # Continue with other servers

        log.info(f"Total tools available: {len(self.tools)}")

    async def _connect_and_discover(self, server: MCPServerConfig) -> None:
        """
        Connect to a single MCP server and discover its tools.

        Args:
            server: MCP server configuration
        """
        log.info(f"  Connecting to {server.name} ({server.transport})...")

        try:
            # Create MCP client based on transport
            if server.transport == "stdio":
                # FastMCP Client for stdio: command with args
                client_input = [server.command] + server.args
                client = Client(client_input)
            elif server.transport == "http":
                # FastMCP Client for HTTP
                client = Client(server.url)
            else:
                log.warning(f"  ⚠️  Unsupported transport: {server.transport}")
                return

            # Connect and list tools
            async with client:
                # List available tools from MCP server
                mcp_tools = await client.list_tools()

                log.info(f"  ✅ {server.name}: {len(mcp_tools.tools)} tools")

                # Convert each MCP tool to LangChain BaseTool
                for mcp_tool in mcp_tools.tools:
                    try:
                        lc_tool = self._create_langchain_tool(
                            server_name=server.name,
                            mcp_tool=mcp_tool,
                            client=client,
                        )
                        self.tools.append(lc_tool)
                        log.debug(f"    • {mcp_tool.name}: {mcp_tool.description}")
                    except Exception as e:
                        log.warning(f"    ⚠️  Failed to convert tool {mcp_tool.name}: {e}")

        except Exception as e:
            log.error(f"  ❌ Failed to connect to {server.name}: {e}")
            raise

    def _create_langchain_tool(
        self,
        server_name: str,
        mcp_tool: Any,
        client: Client,
    ) -> BaseTool:
        """
        Create a LangChain BaseTool from an MCP tool definition.

        Args:
            server_name: Name of the MCP server
            mcp_tool: MCP tool definition from list_tools()
            client: Active MCP client connection

        Returns:
            LangChain BaseTool that executes via MCP
        """
        # Extract tool metadata
        tool_name = mcp_tool.name
        tool_description = mcp_tool.description or f"Tool from {server_name}"

        # Convert MCP inputSchema (JSON Schema) to Pydantic model
        input_schema = getattr(mcp_tool, "inputSchema", None)
        if input_schema:
            args_model = self._json_schema_to_pydantic(tool_name, input_schema)
        else:
            # No input schema - create empty model
            args_model = create_model(f"{tool_name}Input")

        # Create async function that calls MCP tool
        async def async_tool_func(**kwargs: Any) -> str:
            """Call MCP tool via FastMCP client."""
            try:
                # Call the MCP tool through the client
                async with client:
                    result = await client.call_tool(tool_name, kwargs)

                    # Extract text content from result
                    if hasattr(result, "content") and result.content:
                        # result.content is a list of content blocks
                        text_parts = []
                        for content_block in result.content:
                            if hasattr(content_block, "text"):
                                text_parts.append(content_block.text)
                            else:
                                text_parts.append(str(content_block))
                        return "\n".join(text_parts)
                    else:
                        return str(result)

            except Exception as e:
                log.error(f"Error calling MCP tool {tool_name}: {e}")
                return f"Error: {str(e)}"

        # Sync wrapper for LangChain (which expects sync functions)
        def sync_tool_func(**kwargs: Any) -> str:
            """Synchronous wrapper around async MCP tool call."""
            loop = asyncio.get_event_loop()
            if loop.is_running():
                # If event loop is running, create a new task
                # 🤓: This is tricky - LangGraph runs in async context
                # We need to use asyncio.create_task instead
                raise RuntimeError(
                    f"Cannot call MCP tool {tool_name} from running event loop. "
                    "Use async tools with LangGraph."
                )
            else:
                # Create new event loop if none exists
                return asyncio.run(async_tool_func(**kwargs))

        # Create StructuredTool with async support
        # 🤓: LangChain/LangGraph supports async tools via coroutine_func
        return StructuredTool(
            name=tool_name,
            description=tool_description,
            args_schema=args_model,
            coroutine=async_tool_func,  # Async execution
        )

    def _json_schema_to_pydantic(
        self, tool_name: str, json_schema: dict[str, Any]
    ) -> type[BaseModel]:
        """
        Convert JSON Schema to Pydantic model.

        Args:
            tool_name: Name of the tool (for model naming)
            json_schema: JSON Schema definition

        Returns:
            Pydantic BaseModel class
        """
        # Get properties from JSON Schema
        properties = json_schema.get("properties", {})
        required_fields = json_schema.get("required", [])

        # Build Pydantic field definitions
        field_defs: dict[str, Any] = {}

        for field_name, field_schema in properties.items():
            field_type = self._json_type_to_python(field_schema)
            field_description = field_schema.get("description", "")

            # Determine if field is required
            if field_name in required_fields:
                field_defs[field_name] = (field_type, Field(description=field_description))
            else:
                field_defs[field_name] = (
                    field_type | None,
                    Field(default=None, description=field_description),
                )

        # Create dynamic Pydantic model
        model_name = f"{tool_name.replace('-', '_').title()}Input"
        return create_model(model_name, **field_defs)

    def _json_type_to_python(self, schema: dict[str, Any]) -> type:
        """
        Convert JSON Schema type to Python type.

        Args:
            schema: JSON Schema field definition

        Returns:
            Python type
        """
        json_type = schema.get("type", "string")

        type_mapping = {
            "string": str,
            "integer": int,
            "number": float,
            "boolean": bool,
            "array": list,
            "object": dict,
        }

        return type_mapping.get(json_type, str)

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
            # First try exact match
            exact_matches = [tool for tool in self.tools if tool.name == tool_name]
            if exact_matches:
                matched_tools.extend(exact_matches)
            else:
                # Fallback to prefix match (e.g., "crawl4ai" matches "crawl4ai_crawl")
                prefix_matches = [tool for tool in self.tools if tool.name.startswith(tool_name)]
                matched_tools.extend(prefix_matches)

        return matched_tools

    async def shutdown(self) -> None:
        """Cleanup MCP client connections."""
        log.info("🛑 Shutting down MCP clients...")

        # Close all active clients
        for name, client in self.mcp_clients.items():
            try:
                # FastMCP clients are managed by context managers
                # No explicit cleanup needed
                log.debug(f"  Closed {name}")
            except Exception as e:
                log.warning(f"  ⚠️  Error closing {name}: {e}")

        self.mcp_clients.clear()
        log.info("✅ MCP clients stopped")
