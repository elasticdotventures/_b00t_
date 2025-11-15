# GitHub MCP Server - Toolsets Configuration

Master GitHub API access through Model Context Protocol with granular toolset control.

## Overview

GitHub MCP Server enables AI tools to interact with GitHub APIs. Toolsets provide granular control over which GitHub capabilities are exposed to the AI agent.

## Toolsets Configuration

### Available Toolsets

- **context** - Current user and GitHub environment (strongly recommended)
- **repos** - Repository management and operations
- **issues** - GitHub Issues management
- **pull_requests** - Pull request operations
- **actions** - GitHub Actions and CI/CD workflows
- **code_security** - Code scanning and security features
- **experiments** - Unstable/experimental features
- **users** - User profile operations
- **stargazers** - Repository starring operations

### Configuration Methods

#### 1. Environment Variable (Recommended)
```bash
export GITHUB_TOOLSETS="issues,pull_requests,actions"
npx -y @modelcontextprotocol/server-github
```

#### 2. CLI Argument
```bash
github-mcp-server --toolsets repos,issues,pull_requests,actions
```

⚠️ Environment variable takes precedence over CLI argument

#### 3. MCP Server Configuration (Claude Desktop, VS Code, etc.)
```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "ghp_xxx",
        "GITHUB_TOOLSETS": "issues,pull_requests,actions,code_security"
      }
    }
  }
}
```

#### 4. Docker Configuration
```bash
docker run -i --rm \
  -e GITHUB_PERSONAL_ACCESS_TOKEN=ghp_xxx \
  -e GITHUB_TOOLSETS="all" \
  ghcr.io/github/github-mcp-server
```

### Special Toolset Values

#### "all" - Enable Everything
```bash
GITHUB_TOOLSETS="all" npx -y @modelcontextprotocol/server-github
```
Enables all available toolsets regardless of other settings.

#### "default" - Baseline Configuration
Default includes: `context`, `repos`, `issues`, `pull_requests`, `users`

Extend default with additional toolsets:
```bash
GITHUB_TOOLSETS="default,code_security,experiments"
```

## Recommended Configurations

### Development & Issue Management
```bash
GITHUB_TOOLSETS="context,repos,issues,pull_requests"
```

### CI/CD & Security
```bash
GITHUB_TOOLSETS="context,repos,actions,code_security"
```

### Full Access (Development)
```bash
GITHUB_TOOLSETS="all"
```

### Production (Minimal)
```bash
GITHUB_TOOLSETS="context,repos,issues"
```

## Authentication

GitHub MCP Server requires a Personal Access Token:

```bash
export GITHUB_PERSONAL_ACCESS_TOKEN=ghp_xxxxxxxxxxxx
```

### Token Scopes Required

Minimum scopes depend on toolsets:
- **repos**: `repo` scope
- **issues**: `repo` scope
- **pull_requests**: `repo` scope
- **actions**: `repo`, `workflow` scopes
- **code_security**: `repo`, `security_events` scopes

## Common Patterns

### b00t MCP Configuration

Update `_b00t_/github.mcp.toml`:

```toml
[b00t.mcp.stdio.env]
GITHUB_TOOLSETS = "issues,pull_requests,actions"
```

### Conditional Toolsets

Enable toolsets based on context:

```bash
# Development: all tools
if [[ "$ENV" == "dev" ]]; then
  export GITHUB_TOOLSETS="all"
# Production: limited tools
else
  export GITHUB_TOOLSETS="context,repos,issues"
fi
```

### Multi-Instance Configuration

Run multiple instances with different toolsets:

```bash
# Instance 1: Issue management
GITHUB_TOOLSETS="issues" npx -y @modelcontextprotocol/server-github &

# Instance 2: PR operations
GITHUB_TOOLSETS="pull_requests,actions" npx -y @modelcontextprotocol/server-github &
```

## Troubleshooting

### Toolset Not Enabled

**Symptom**: Tool calls fail with "not found" or "permission denied"

**Solution**: Verify toolsets configuration
```bash
# Check environment
echo $GITHUB_TOOLSETS

# Enable required toolset
export GITHUB_TOOLSETS="issues,pull_requests"
```

### Token Insufficient Scopes

**Symptom**: Authentication errors or 403 responses

**Solution**: Update token scopes at https://github.com/settings/tokens

### Environment Variable Override

**Symptom**: CLI argument ignored

**Solution**: Environment variable takes precedence
```bash
# Unset env var to use CLI arg
unset GITHUB_TOOLSETS
github-mcp-server --toolsets repos,issues
```

## Best Practices

1. **Use `context` toolset**: Always include for environment awareness
2. **Minimal toolsets**: Enable only required toolsets for security
3. **Environment-specific**: Use different toolsets for dev/prod
4. **Token scoping**: Match token scopes to enabled toolsets
5. **Documentation**: Document toolset requirements in project

## Integration Examples

### VS Code MCP Settings
```json
{
  "mcp.servers": {
    "github-issues": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_TOOLSETS": "context,issues,pull_requests"
      }
    }
  }
}
```

### Claude Desktop Configuration
```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_TOOLSETS": "all"
      }
    }
  }
}
```

### b00t Justfile Integration
```just
# Start GitHub MCP with specific toolsets
github-mcp toolsets="issues,pull_requests":
    #!/usr/bin/env bash
    export GITHUB_TOOLSETS="{{toolsets}}"
    npx -y @modelcontextprotocol/server-github
```

## References

- [GitHub MCP Server](https://github.com/github/github-mcp-server)
- [MCP Documentation](https://modelcontextprotocol.io)
- [GitHub PAT Settings](https://github.com/settings/tokens)
- [MCP Specification](https://spec.modelcontextprotocol.io)

## LFMF Integration

Record lessons about GitHub MCP toolsets:

```bash
b00t lfmf github-mcp "toolset priority: Always include 'context' toolset for environment awareness"
b00t lfmf github-mcp "env override: GITHUB_TOOLSETS env var takes precedence over CLI --toolsets flag"
b00t lfmf github-mcp "token scopes: Match PAT scopes to enabled toolsets (actions requires workflow scope)"
```

Get advice:

```bash
b00t advice github-mcp "toolset not working"
b00t advice github-mcp "authentication failed"
b00t advice github-mcp list
```
