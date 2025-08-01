# b00t-mcp

MCP (Model Context Protocol) server for b00t-cli command proxy with ACL filtering.

## Overview

b00t-mcp is a lightweight wrapper around b00t-cli that exposes its functionality through the Model Context Protocol (MCP). It provides secure command execution with configurable Access Control Lists (ACL) to restrict which commands and arguments can be executed.

## Features

- **100% b00t-cli compatibility**: Supports all b00t-cli commands through MCP interface
- **ACL filtering**: TOML-based configuration for allow/deny command policies
- **Regex pattern matching**: Fine-grained control over command arguments
- **Security-first design**: Dangerous commands denied by default
- **MCP-native**: Built with rmcp for efficient MCP protocol handling

## Installation

From the workspace root:

```bash
cargo build --release
```

## Configuration

ACL configuration is stored in `~/.dotfiles/b00t-mcp-acl.toml`:

```toml
# Default policy when no specific rule matches
default_policy = "allow"

# Command-specific rules
[commands.detect]
policy = "allow"
description = "Detect installed versions of tools"

[commands.install]
policy = "deny"
description = "Install commands denied by default for security"

# Global regex patterns
[patterns]
deny = [
    ".*\\b(rm|delete|destroy|kill)\\b.*",  # Prevent destructive operations
    ".*--force.*",                          # Prevent forced operations
]
```

## Usage

### As MCP Server

```bash
# Run with stdio transport (typical MCP usage)
b00t-mcp stdio
# OR
b00t-mcp --stdio

# Run in specific directory
b00t-mcp --directory /path/to/project stdio
# OR
b00t-mcp --directory /path/to/project --stdio

# Use custom ACL config
b00t-mcp --config /path/to/custom-acl.toml stdio
# OR
b00t-mcp --config /path/to/custom-acl.toml --stdio
```

### Available MCP Tools

- `b00t_detect` - Detect currently installed tool versions
- `b00t_desires` - Show desired versions from configuration  
- `b00t_learn` - Display learning resources for topics
- `b00t_mcp` - Manage MCP servers (list/add only)
- `b00t_status` - Show status of all tools

### Example MCP Client Usage

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "b00t_detect",
    "arguments": {
      "tool": "git"
    }
  }
}
```

## Security Model

### Default Security Posture

- **Allow by default**: Non-destructive read operations are permitted
- **Deny dangerous commands**: install, update, up commands blocked by default
- **Pattern filtering**: Regex patterns block destructive arguments
- **No privilege escalation**: sudo and similar commands blocked

### ACL Policy Evaluation

1. **Deny patterns** are checked first (highest priority)
2. **Allow patterns** override command-specific denials
3. **Command-specific rules** are evaluated
4. **Default policy** is used as fallback

### Recommended Production Configuration

For production use, consider a more restrictive policy:

```toml
default_policy = "deny"

[commands.detect]
policy = "allow"

[commands.learn]  
policy = "allow"

[commands.mcp]
policy = "allow"
arg_patterns = ["^list$"]  # Only allow 'mcp list'
```

## Integration with MCP Clients

### Claude Code Integration

Add to `.mcp.json`:

```json
{
  "mcpServers": {
    "b00t-mcp": {
      "command": "b00t-mcp",
      "args": ["stdio"],
      "cwd": "/home/user/.dotfiles"
    }
  }
}
```

### VSCode Integration

```bash
# Install as MCP server for VSCode
b00t-cli vscode install mcp b00t-mcp
```

## Development

### Running Tests

```bash
cargo test
```

### Building

```bash
# Debug build
cargo build

# Release build  
cargo build --release
```

### Workspace Integration

This project is part of the ~/.dotfiles Cargo workspace:

```bash
# Build all workspace members
cargo build --workspace

# Test all workspace members
cargo test --workspace
```

## Architecture

### Components

- **`main.rs`**: CLI interface and MCP server startup
- **`mcp_server.rs`**: MCP protocol implementation and command execution
- **`acl.rs`**: Access Control List filtering and policy evaluation

### Command Execution Flow

1. MCP client calls tool through protocol
2. ACL filter evaluates command + arguments against policy
3. If allowed, b00t-cli subprocess is executed
4. Output is returned through MCP protocol
5. If denied, error is returned with ACL violation message

### Security Considerations

- All commands are executed as subprocesses (no shell injection)
- Working directory is controlled and validated
- ACL configuration is loaded once at startup
- No dynamic code execution or eval functions

## Contributing

1. Follow existing code patterns from just-mcp reference implementation
2. Maintain 100% b00t-cli command compatibility
3. Add tests for new ACL rules or command mappings
4. Update documentation for configuration changes

## License

MIT License - see LICENSE file for details.