"""
b00t-py: High-performance Python bindings for b00t-cli

This package provides native Rust performance for b00t ecosystem operations
with a fluent, chainable Python API.
"""

from typing import List, Dict, Any, Optional, Union
import json
try:
    import b00t_py as _core
except ImportError:
    # Fallback for development/testing without compiled module
    _core = None

from .exceptions import B00tError

# Version info
__version__ = _core.version() if _core else "dev"

# Direct functional exports
def mcp_list(path: str = "~/.dotfiles/_b00t_", json_output: bool = False) -> str:
    """List all MCP servers available in the b00t configuration."""
    if _core is None:
        raise B00tError("Native b00t_py module not available. Install with: pip install b00t-py")
    return _core.mcp_list_py(path, json_output)

def mcp_output(servers: str, path: str = "~/.dotfiles/_b00t_", json_format: bool = False) -> str:
    """Get MCP server output in specified format."""
    if _core is None:
        raise B00tError("Native b00t_py module not available. Install with: pip install b00t-py")
    return _core.mcp_output_py(servers, path, json_format)

# Fluent interface classes
class McpQuery:
    """Fluent interface for MCP operations."""
    
    def __init__(self, path: str = "~/.dotfiles/_b00t_"):
        self.path = path
        self._servers: Optional[List[str]] = None
        self._json_format = False
    
    def servers(self, server_list: List[str]) -> 'McpQuery':
        """Filter to specific servers."""
        self._servers = server_list
        return self
    
    def json(self) -> 'McpQuery':
        """Use JSON format output."""
        self._json_format = True
        return self
    
    def list(self) -> str:
        """Execute list operation."""
        return mcp_list(self.path, self._json_format)
    
    def output(self) -> str:
        """Execute output operation."""
        if self._servers is None:
            raise B00tError("No servers specified. Use .servers() first.")
        
        server_str = ",".join(self._servers)
        return mcp_output(server_str, self.path, self._json_format)

class AiQuery:
    """Fluent interface for AI operations (placeholder for future implementation)."""
    
    def __init__(self, path: str = "~/.dotfiles/_b00t_"):
        self.path = path
    
    def list(self) -> List[Dict[str, Any]]:
        """List AI providers (placeholder)."""
        # TODO: Implement when AI functions are added to Rust side
        return []
    
    def providers(self, provider_list: List[str]) -> 'AiQuery':
        """Filter to specific providers."""
        return self

class CliQuery:
    """Fluent interface for CLI operations (placeholder for future implementation)."""
    
    def __init__(self, path: str = "~/.dotfiles/_b00t_"):
        self.path = path
    
    def detect(self, tool: str) -> str:
        """Detect tool version (placeholder)."""
        # TODO: Implement when CLI functions are added to Rust side
        return f"Tool {tool} detection not yet implemented"

# Factory functions for fluent interface
def mcp(path: str = "~/.dotfiles/_b00t_") -> McpQuery:
    """Create MCP query builder."""
    return McpQuery(path)

def ai(path: str = "~/.dotfiles/_b00t_") -> AiQuery:
    """Create AI query builder."""
    return AiQuery(path)

def cli(path: str = "~/.dotfiles/_b00t_") -> CliQuery:
    """Create CLI query builder."""
    return CliQuery(path)

# Re-export exception
__all__ = [
    'mcp_list', 'mcp_output', 
    'mcp', 'ai', 'cli',
    'McpQuery', 'AiQuery', 'CliQuery',
    'B00tError', '__version__'
]