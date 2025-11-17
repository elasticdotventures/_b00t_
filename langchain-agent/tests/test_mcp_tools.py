"""Tests for MCP tool discovery."""

import pytest
from pathlib import Path
from b00t_langchain_agent.mcp_tools import MCPToolDiscovery


@pytest.mark.asyncio
async def test_mcp_discovery_initialization():
    """Test MCP tool discovery initialization."""
    # Use _b00t_ directory
    datum_path = Path.home() / ".dotfiles" / "_b00t_"
    if not datum_path.exists():
        pytest.skip("_b00t_ directory not found")

    discovery = MCPToolDiscovery(datum_path=datum_path)
    await discovery.initialize()

    # Should discover some MCP servers
    assert len(discovery.mcp_servers) > 0
    assert len(discovery.tools) > 0


@pytest.mark.asyncio
async def test_mcp_parse_datum():
    """Test parsing MCP datum files."""
    datum_path = Path.home() / ".dotfiles" / "_b00t_"
    if not datum_path.exists():
        pytest.skip("_b00t_ directory not found")

    discovery = MCPToolDiscovery(datum_path=datum_path)

    # Find a sample MCP datum
    mcp_datums = list(datum_path.glob("*.mcp.toml"))
    if not mcp_datums:
        pytest.skip("No MCP datums found")

    # Parse first datum
    servers = discovery._parse_mcp_datum(mcp_datums[0])
    assert isinstance(servers, list)


@pytest.mark.asyncio
async def test_get_tools_by_name():
    """Test getting tools by name."""
    datum_path = Path.home() / ".dotfiles" / "_b00t_"
    if not datum_path.exists():
        pytest.skip("_b00t_ directory not found")

    discovery = MCPToolDiscovery(datum_path=datum_path)
    await discovery.initialize()

    # Try to get tools by common names
    if discovery.tools:
        # Get first tool name
        first_tool = discovery.tools[0]
        tools = discovery.get_tools_by_name([first_tool.name])
        assert len(tools) > 0
