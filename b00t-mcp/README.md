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

## Executing External MCP Tools via stdio

b00t provides two mechanisms for agents and scripts to execute MCP tools via stdio transport when they can't run MCP directly:

### `b00t-cli mcp execute` - Datum-based and Direct Execution

Execute MCP tools from registered servers (datums) or via direct command specification:

```bash
# Execute from registered MCP server datum
b00t-cli mcp execute filesystem read_file '{"path":"/tmp/test.txt"}'
b00t-cli mcp execute brave-search search '{"query":"rust programming"}'

# Direct execution without datum registration
b00t-cli mcp execute --command npx \
  --args '-y,@modelcontextprotocol/server-filesystem' \
  read_file '{"path":"/file.txt"}'

b00t-cli mcp execute -c uvx \
  -a 'mcp-server-playwright' \
  screenshot '{"url":"https://example.com"}'

# Discover available tools from a server
b00t-cli mcp execute filesystem --discover
b00t-cli mcp execute --command npx --args '-y,@mcp/server-filesystem' --discover
```

### `mcp-user` - Standalone Utility for Simplified Access

Lightweight standalone binary for quick MCP tool execution:

```bash
# Discover tools from an MCP server
mcp-user -c npx -a '-y,@modelcontextprotocol/server-filesystem' --discover

# Execute a tool (concise output)
mcp-user -c npx -a '-y,@modelcontextprotocol/server-filesystem' \
  read_file '{"path":"/tmp/test.txt"}'

# Use with Python-based MCP servers (uvx)
mcp-user -c uvx -a 'mcp-server-playwright' \
  screenshot '{"url":"https://example.com"}'

# Verbose mode for debugging
mcp-user -v -c npx -a '-y,@mcp/server-git' \
  git_status '{"repo_path":"."}'

# JSON output format
mcp-user -c npx -a '-y,@mcp/server-filesystem' \
  --format json read_file '{"path":"/file.txt"}'
```

**Use cases for stdio MCP execution:**
- Agents that can't run MCP directly (e.g., restricted environments)
- Shell scripts and automation workflows
- Quick testing of MCP servers during development
- Piping MCP tool outputs to other commands
- Integration with non-MCP-aware tools

**Key features:**
- ✅ No datum registration required (direct command mode)
- ✅ Automatic tool discovery and validation
- ✅ JSON and text output formats
- ✅ Supports all MCP stdio transports (npx, uvx, docker, etc.)
- ✅ Verbose mode for debugging connections
- ✅ Working directory customization

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
## Identity & r0le gating (SP3)

b00t-mcp can gate its tools per calling agent. **With none of these env vars set
it runs exactly as before — fully local, `anon` caller, no filtering.**

| var | effect |
|---|---|
| `B00T_MCP_REQUIRE_AUTH=1` | HTTP: a request without a valid `Authorization: Bearer <jwt>` gets `401` (else `anon`) |
| `B00T_AGENT_JWT` | stdio: the JWT for this process (overridable by `initialize._meta.b00t_jwt`) |
| `B00T_IDENTITY_JWKS` | inline JWKS JSON used to verify JWTs (else fetched) |
| `B00T_IDENTITY_URL` | base for `GET /.well-known/jwks.json` (default `https://b00t.promptexecution.com`) |
| `B00T_MCP_REQUIRE_SIGNED_R0LE=1` | reject r0le datums whose `[b00t.agent_profile].signature` doesn't verify |
| `B00T_LEDGRRR_MODE=http` + `B00T_LEDGRRR_URL` | meter `call_tool` against a real ledgrrr (else an always-ok mock) |
| `B00T_MCP_ROUTING_DOMAIN` / `B00T_MCP_ROUTING_BASE_URL` | base for `<svc>__<tool>` remote routing |
| `B00T_MCP_JUDGE=grant` | allow `b00t_r0le_request_escalation` grants (default: deny) |

An identified caller sees only the tools its r0le package (`DatumType::AgentProfile`)
allows; a gated tool returns JSON-RPC `-32003` until `b00t_learn('<skill>')`;
`call_tool` is metered and returns `-32004` when ledgrrr denies the spend;
`b00t_r0le_request_escalation` widens the live allow-list (session-scoped) and
fires `tools/list_changed`.
