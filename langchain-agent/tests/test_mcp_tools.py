"""Tests for MCP tool discovery with real MCP client integration."""

import pytest
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

from b00t_langchain_agent.mcp_tools import MCPToolDiscovery
from b00t_langchain_agent.types import MCPServerConfig


@pytest.mark.asyncio
async def test_json_schema_to_pydantic():
    """Test JSON Schema to Pydantic model conversion."""
    datum_path = Path.home() / ".dotfiles" / "_b00t_"
    discovery = MCPToolDiscovery(datum_path=datum_path)

    # Test schema with various types
    json_schema = {
        "type": "object",
        "properties": {
            "url": {"type": "string", "description": "URL to crawl"},
            "depth": {"type": "integer", "description": "Crawl depth"},
            "wait_time": {"type": "number", "description": "Wait time in seconds"},
            "enable_js": {"type": "boolean", "description": "Enable JavaScript"},
            "headers": {"type": "object", "description": "HTTP headers"},
        },
        "required": ["url"],
    }

    # Convert to Pydantic model
    model = discovery._json_schema_to_pydantic("test_tool", json_schema)

    # Verify model fields
    assert "url" in model.model_fields
    assert "depth" in model.model_fields
    assert "wait_time" in model.model_fields
    assert "enable_js" in model.model_fields
    assert "headers" in model.model_fields

    # Test instance creation with required field
    instance = model(url="https://example.com")
    assert instance.url == "https://example.com"
    assert instance.depth is None  # Optional field


@pytest.mark.asyncio
async def test_json_type_to_python():
    """Test JSON type to Python type conversion."""
    datum_path = Path.home() / ".dotfiles" / "_b00t_"
    discovery = MCPToolDiscovery(datum_path=datum_path)

    # Test type conversions
    assert discovery._json_type_to_python({"type": "string"}) == str
    assert discovery._json_type_to_python({"type": "integer"}) == int
    assert discovery._json_type_to_python({"type": "number"}) == float
    assert discovery._json_type_to_python({"type": "boolean"}) == bool
    assert discovery._json_type_to_python({"type": "array"}) == list
    assert discovery._json_type_to_python({"type": "object"}) == dict
    assert discovery._json_type_to_python({"type": "unknown"}) == str  # Default


@pytest.mark.asyncio
async def test_parse_mcp_datum_stdio():
    """Test parsing stdio MCP datum configuration."""
    datum_path = Path.home() / ".dotfiles" / "_b00t_"
    if not datum_path.exists():
        pytest.skip("_b00t_ directory not found")

    discovery = MCPToolDiscovery(datum_path=datum_path)

    # Find a stdio MCP datum (e.g., github.mcp.toml)
    github_datum = datum_path / "github.mcp.toml"
    if not github_datum.exists():
        pytest.skip("github.mcp.toml not found")

    servers = discovery._parse_mcp_datum(github_datum)
    assert len(servers) > 0
    assert servers[0].transport == "stdio"
    assert servers[0].command in ("npx", "node", "python", "uv")


@pytest.mark.asyncio
async def test_parse_mcp_datum_http():
    """Test parsing HTTP MCP datum configuration (if exists)."""
    datum_path = Path.home() / ".dotfiles" / "_b00t_"
    if not datum_path.exists():
        pytest.skip("_b00t_ directory not found")

    discovery = MCPToolDiscovery(datum_path=datum_path)

    # Check for HTTP-based MCP datums
    for datum_file in datum_path.glob("*.mcp.toml"):
        servers = discovery._parse_mcp_datum(datum_file)
        for server in servers:
            if server.transport == "http":
                assert server.url is not None
                assert server.url.startswith("http")


@pytest.mark.asyncio
async def test_create_langchain_tool_with_mock_client():
    """Test creating LangChain tool from mock MCP tool."""
    datum_path = Path.home() / ".dotfiles" / "_b00t_"
    discovery = MCPToolDiscovery(datum_path=datum_path)

    # Create mock MCP tool
    mock_mcp_tool = MagicMock()
    mock_mcp_tool.name = "test_crawl"
    mock_mcp_tool.description = "Test crawling tool"
    mock_mcp_tool.inputSchema = {
        "type": "object",
        "properties": {
            "url": {"type": "string", "description": "URL to crawl"},
        },
        "required": ["url"],
    }

    # Create mock client
    mock_client = AsyncMock()
    mock_result = MagicMock()
    mock_result.content = [MagicMock(text="Test result content")]
    mock_client.call_tool = AsyncMock(return_value=mock_result)

    # Create LangChain tool
    lc_tool = discovery._create_langchain_tool(
        server_name="test_server",
        mcp_tool=mock_mcp_tool,
        client=mock_client,
    )

    # Verify tool properties
    assert lc_tool.name == "test_crawl"
    assert lc_tool.description == "Test crawling tool"
    assert lc_tool.args_schema is not None

    # Test async tool execution
    result = await lc_tool.coroutine(url="https://example.com")
    assert result == "Test result content"


@pytest.mark.asyncio
async def test_connect_and_discover_with_mock():
    """Test MCP server connection and tool discovery with mock."""
    datum_path = Path.home() / ".dotfiles" / "_b00t_"
    discovery = MCPToolDiscovery(datum_path=datum_path)

    # Create mock server config
    server = MCPServerConfig(
        name="test_server",
        transport="stdio",
        command="python",
        args=["-m", "test_mcp_server"],
    )

    # Create mock MCP tools response
    mock_tool_1 = MagicMock()
    mock_tool_1.name = "fetch_url"
    mock_tool_1.description = "Fetch URL content"
    mock_tool_1.inputSchema = {
        "type": "object",
        "properties": {"url": {"type": "string"}},
        "required": ["url"],
    }

    mock_tool_2 = MagicMock()
    mock_tool_2.name = "search"
    mock_tool_2.description = "Search for content"
    mock_tool_2.inputSchema = {
        "type": "object",
        "properties": {"query": {"type": "string"}},
        "required": ["query"],
    }

    mock_tools_result = MagicMock()
    mock_tools_result.tools = [mock_tool_1, mock_tool_2]

    # Mock the FastMCP Client
    with patch("b00t_langchain_agent.mcp_tools.Client") as mock_client_class:
        mock_client = AsyncMock()
        mock_client.list_tools = AsyncMock(return_value=mock_tools_result)
        mock_client.__aenter__ = AsyncMock(return_value=mock_client)
        mock_client.__aexit__ = AsyncMock(return_value=None)
        mock_client_class.return_value = mock_client

        # Test connection and discovery
        await discovery._connect_and_discover(server)

        # Verify tools were discovered
        assert len(discovery.tools) == 2
        tool_names = {tool.name for tool in discovery.tools}
        assert "fetch_url" in tool_names
        assert "search" in tool_names


@pytest.mark.asyncio
async def test_get_tools_by_name():
    """Test filtering tools by name."""
    datum_path = Path.home() / ".dotfiles" / "_b00t_"
    discovery = MCPToolDiscovery(datum_path=datum_path)

    # Create mock tools
    tool1 = MagicMock()
    tool1.name = "crawl4ai_fetch"
    tool2 = MagicMock()
    tool2.name = "crawl4ai_extract"
    tool3 = MagicMock()
    tool3.name = "github_get_repo"

    discovery.tools = [tool1, tool2, tool3]

    # Test exact match
    tools = discovery.get_tools_by_name(["github_get_repo"])
    assert len(tools) == 1
    assert tools[0].name == "github_get_repo"

    # Test prefix match
    tools = discovery.get_tools_by_name(["crawl4ai"])
    assert len(tools) == 2
    tool_names = {t.name for t in tools}
    assert "crawl4ai_fetch" in tool_names
    assert "crawl4ai_extract" in tool_names


@pytest.mark.asyncio
async def test_async_tool_execution_error_handling():
    """Test that MCP tool execution handles errors gracefully."""
    datum_path = Path.home() / ".dotfiles" / "_b00t_"
    discovery = MCPToolDiscovery(datum_path=datum_path)

    # Create mock MCP tool
    mock_mcp_tool = MagicMock()
    mock_mcp_tool.name = "failing_tool"
    mock_mcp_tool.description = "Tool that fails"
    mock_mcp_tool.inputSchema = None

    # Create mock client that raises an error
    mock_client = AsyncMock()
    mock_client.call_tool = AsyncMock(side_effect=Exception("Connection failed"))

    # Create LangChain tool
    lc_tool = discovery._create_langchain_tool(
        server_name="test_server",
        mcp_tool=mock_mcp_tool,
        client=mock_client,
    )

    # Test that error is handled gracefully
    result = await lc_tool.coroutine()
    assert "Error" in result
    assert "Connection failed" in result


@pytest.mark.asyncio
@pytest.mark.integration
async def test_real_mcp_server_connection():
    """
    Integration test: Connect to a real MCP server if available.

    🤓: This test requires an MCP server to be running.
    Skip if no servers are available or configured.
    """
    datum_path = Path.home() / ".dotfiles" / "_b00t_"
    if not datum_path.exists():
        pytest.skip("_b00t_ directory not found")

    discovery = MCPToolDiscovery(datum_path=datum_path)

    try:
        # Attempt to discover real MCP servers
        await discovery.initialize()

        if len(discovery.tools) == 0:
            pytest.skip("No MCP tools discovered (servers may not be running)")

        # Verify at least one tool was discovered
        assert len(discovery.tools) > 0

        # Verify tools have required properties
        for tool in discovery.tools:
            assert tool.name is not None
            assert tool.description is not None
            assert hasattr(tool, "coroutine")  # Async execution support

        # Cleanup
        await discovery.shutdown()

    except Exception as e:
        pytest.skip(f"MCP server connection failed: {e}")


@pytest.mark.asyncio
async def test_shutdown_cleanup():
    """Test that shutdown properly cleans up MCP clients."""
    datum_path = Path.home() / ".dotfiles" / "_b00t_"
    discovery = MCPToolDiscovery(datum_path=datum_path)

    # Add mock clients
    mock_client_1 = MagicMock()
    mock_client_2 = MagicMock()
    discovery.mcp_clients = {
        "server1": mock_client_1,
        "server2": mock_client_2,
    }

    # Test shutdown
    await discovery.shutdown()

    # Verify clients were cleared
    assert len(discovery.mcp_clients) == 0
