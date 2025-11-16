# GitHub MCP Server - Toolsets Configuration

Master GitHub API access through Model Context Protocol with granular toolset control.

## Overview

GitHub MCP Server enables AI tools to interact with GitHub APIs. Toolsets provide granular control over which GitHub capabilities are exposed to the AI agent.

**Recommended**: Use official `github-mcp-server` (supports toolsets) instead of archived `@modelcontextprotocol/server-github`

## Authentication (Security First!)

⚠️ **PATs are a security vector** - Avoid storing Personal Access Tokens when possible

### Recommended: Use gh CLI Token (No Storage)

```bash
# Get token dynamically from gh CLI (no storage, uses your gh auth)
export GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)"
npx -y github-mcp-server
```

**Benefits:**
- ✅ No token storage (security vector eliminated)
- ✅ Uses existing gh CLI authentication
- ✅ Token auto-refreshes with gh auth
- ✅ Centralized auth management via gh

### Alternative: Personal Access Token

Only if gh CLI unavailable:

```bash
# Create PAT: https://github.com/settings/tokens
export GITHUB_PERSONAL_ACCESS_TOKEN="ghp_xxxxxxxxxxxx"
```

### Required Token Scopes

Minimum scopes depend on toolsets:
- **repos**: `repo` scope
- **issues**: `repo` scope
- **pull_requests**: `repo` scope
- **actions**: `repo`, `workflow` scopes
- **code_security**: `repo`, `security_events` scopes

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
# Use gh CLI token (secure, no storage)
export GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)"
export GITHUB_TOOLSETS="issues,pull_requests,actions"
npx -y github-mcp-server
```

#### 2. CLI Argument
```bash
# Official github-mcp-server supports --toolsets flag
GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)" \
  github-mcp-server --toolsets repos,issues,pull_requests,actions
```

⚠️ Environment variable takes precedence over CLI argument

#### 3. MCP Server Configuration (Claude Desktop, VS Code, etc.)

**With gh CLI (Recommended):**
```json
{
  "mcpServers": {
    "github": {
      "command": "sh",
      "args": ["-c", "GITHUB_PERSONAL_ACCESS_TOKEN=$(gh auth token) npx -y github-mcp-server"],
      "env": {
        "GITHUB_TOOLSETS": "issues,pull_requests,actions,code_security"
      }
    }
  }
}
```

**With stored PAT (Less secure):**
```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "github-mcp-server"],
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
# Use gh CLI token (no PAT storage in env files)
docker run -i --rm \
  -e GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)" \
  -e GITHUB_TOOLSETS="all" \
  ghcr.io/github/github-mcp-server
```

### Special Toolset Values

#### "all" - Enable Everything
```bash
GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)" \
  GITHUB_TOOLSETS="all" npx -y github-mcp-server
```
Enables all available toolsets regardless of other settings.

#### "default" - Baseline Configuration
Default includes: `context`, `repos`, `issues`, `pull_requests`, `users`

Extend default with additional toolsets:
```bash
GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)" \
  GITHUB_TOOLSETS="default,code_security,experiments" \
  npx -y github-mcp-server
```

## Recommended Configurations

### Development & Issue Management
```bash
GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)" \
  GITHUB_TOOLSETS="context,repos,issues,pull_requests" \
  npx -y github-mcp-server
```

### CI/CD & Security
```bash
GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)" \
  GITHUB_TOOLSETS="context,repos,actions,code_security" \
  npx -y github-mcp-server
```

### Full Access (Development)
```bash
GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)" \
  GITHUB_TOOLSETS="all" \
  npx -y github-mcp-server
```

### Production (Minimal)
```bash
GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)" \
  GITHUB_TOOLSETS="context,repos,issues" \
  npx -y github-mcp-server
```

## Server Implementations

### Official github-mcp-server (Recommended)

**Package**: `github-mcp-server` (official GitHub implementation)

**Features:**
- ✅ Full toolsets support
- ✅ Active development
- ✅ All configuration options
- ✅ Production ready

```bash
npx -y github-mcp-server
```

### @modelcontextprotocol/server-github (Fallback)

**Package**: `@modelcontextprotocol/server-github` (archived)

**Use only when:**
- Official server unavailable
- Legacy compatibility needed
- No toolsets required

⚠️ **Limitations**: No toolsets support, archived repository

```bash
npx -y @modelcontextprotocol/server-github
```

## Common Patterns

### b00t MCP Configuration

The `_b00t_/github.mcp.toml` already configures:
- ✅ Official github-mcp-server (priority 0, preferred)
- ✅ Fallback to @modelcontextprotocol/server-github (priority 10)
- ✅ Default toolsets: `issues,pull_requests,actions`
- ✅ gh CLI requirement for secure token access
- ✅ **RHAI pre-start validation** (fail-fast credential check)

**RHAI Validation Script:**

The configuration includes a `pre_start` rhai script that validates GitHub credentials before starting the MCP server:

```toml
[[b00t.mcp.stdio]]
priority = 0
command = "npx"
args = ["-y", "github-mcp-server"]
pre_start = "github-auth-validate.rhai"  # Runs before server starts
```

**What the validation script does:**
1. Checks if `GITHUB_PERSONAL_ACCESS_TOKEN` already set in environment
2. Checks for token in `.env` file
3. Falls back to `gh auth token` if gh CLI available
4. **Fails fast** with actionable error if no credentials found
5. Sets token in environment for MCP server

**Benefits:**
- ✅ Prevents server startup failures due to missing credentials
- ✅ Provides clear error messages (vs cryptic MCP server errors)
- ✅ Avoids cascading stack failures
- ✅ Validates `gh auth login` state before attempting server start

**Manual validation:**
```bash
# Test the validation script manually
b00t script run github-auth-validate

# View validation script
cat _b00t_/scripts/github-auth-validate.rhai
```

**Override toolsets:**
```toml
# Edit _b00t_/github.mcp.toml
[b00t.mcp.stdio.env]
GITHUB_TOOLSETS = "all"  # or specific toolsets
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

# Use gh CLI token (no PAT storage)
export GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)"
npx -y github-mcp-server
```

### Multi-Instance Configuration

Run multiple instances with different toolsets:

```bash
# Shared gh CLI token
export GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)"

# Instance 1: Issue management
GITHUB_TOOLSETS="issues" npx -y github-mcp-server &

# Instance 2: PR operations
GITHUB_TOOLSETS="pull_requests,actions" npx -y github-mcp-server &
```

## Troubleshooting

### Authentication Failed

**Symptom**: "Authentication required" or "401 Unauthorized"

**Solution**: Use gh CLI token
```bash
# Check gh auth status
gh auth status

# Login if needed
gh auth login

# Export token
export GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)"
```

### Toolset Not Enabled

**Symptom**: Tool calls fail with "not found" or "permission denied"

**Solution**: Verify toolsets configuration
```bash
# Check environment
echo $GITHUB_TOOLSETS

# Enable required toolset
export GITHUB_TOOLSETS="issues,pull_requests"

# Verify with gh token
GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)" \
  npx -y github-mcp-server
```

### Token Insufficient Scopes

**Symptom**: Authentication errors or 403 responses

**Solution**: gh auth refresh with required scopes
```bash
# Refresh with additional scopes
gh auth refresh -s workflow,security_events

# Or create new PAT at https://github.com/settings/tokens
```

### gh CLI Not Available

**Symptom**: `gh: command not found`

**Solution**: Install gh CLI or use PAT fallback
```bash
# Install gh CLI (preferred)
# See: https://cli.github.com/manual/installation

# OR use PAT fallback (less secure)
export GITHUB_PERSONAL_ACCESS_TOKEN="ghp_xxxxxxxxxxxx"
```

### Environment Variable Override

**Symptom**: CLI argument ignored

**Solution**: Environment variable takes precedence
```bash
# Unset env var to use CLI arg
unset GITHUB_TOOLSETS
GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)" \
  github-mcp-server --toolsets repos,issues
```

## Best Practices

1. **Use gh CLI token**: Avoid storing PATs (security vector eliminated)
2. **Use `context` toolset**: Always include for environment awareness
3. **Minimal toolsets**: Enable only required toolsets for security
4. **Environment-specific**: Use different toolsets for dev/prod
5. **Token scoping**: Match gh auth scopes to enabled toolsets
6. **Prefer official server**: Use `github-mcp-server` over archived `@modelcontextprotocol/server-github`
7. **Documentation**: Document toolset requirements in project

## Integration Examples

### VS Code MCP Settings
```json
{
  "mcp.servers": {
    "github": {
      "command": "sh",
      "args": ["-c", "GITHUB_PERSONAL_ACCESS_TOKEN=$(gh auth token) npx -y github-mcp-server"],
      "env": {
        "GITHUB_TOOLSETS": "context,issues,pull_requests,actions"
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
      "command": "sh",
      "args": ["-c", "GITHUB_PERSONAL_ACCESS_TOKEN=$(gh auth token) npx -y github-mcp-server"],
      "env": {
        "GITHUB_TOOLSETS": "all"
      }
    }
  }
}
```

### b00t Justfile Integration
```just
# Start GitHub MCP with specific toolsets using gh CLI token
github-mcp toolsets="issues,pull_requests":
    #!/usr/bin/env bash
    export GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)"
    export GITHUB_TOOLSETS="{{toolsets}}"
    npx -y github-mcp-server

# Quick start with default toolsets
github-mcp-start:
    #!/usr/bin/env bash
    export GITHUB_PERSONAL_ACCESS_TOKEN="$(gh auth token)"
    export GITHUB_TOOLSETS="issues,pull_requests,actions"
    npx -y github-mcp-server
```

## References

- [GitHub MCP Server](https://github.com/github/github-mcp-server)
- [MCP Documentation](https://modelcontextprotocol.io)
- [GitHub PAT Settings](https://github.com/settings/tokens)
- [MCP Specification](https://spec.modelcontextprotocol.io)

## LFMF Integration

Record lessons about GitHub MCP configuration:

```bash
# Security best practices
b00t lfmf github-mcp "gh CLI token: Use \$(gh auth token) instead of storing PATs (eliminates security vector)"
b00t lfmf github-mcp "server preference: Use official github-mcp-server (toolsets support) over archived @modelcontextprotocol/server-github"

# RHAI validation pattern
b00t lfmf github-mcp "pre-start validation: Use pre_start rhai script for fail-fast credential checks before MCP server starts"
b00t lfmf github-mcp "rhai script location: Validation scripts live in _b00t_/scripts/ directory (e.g. github-auth-validate.rhai)"
b00t lfmf github-mcp "validation benefits: Prevents cascading stack failures with clear error messages vs cryptic server errors"

# Toolset configuration
b00t lfmf github-mcp "context toolset: Always include 'context' toolset for environment awareness in GITHUB_TOOLSETS"
b00t lfmf github-mcp "env precedence: GITHUB_TOOLSETS env var takes precedence over CLI --toolsets flag"

# Authentication
b00t lfmf github-mcp "gh auth scopes: Use gh auth refresh -s workflow,security_events to add scopes for actions/code_security toolsets"
b00t lfmf github-mcp "credential priority: Check env → .env file → gh CLI → fail (validation script pattern)"
b00t lfmf github-mcp "fallback auth: If gh CLI unavailable, use PAT but document as technical debt for security review"
```

Get advice:

```bash
b00t advice github-mcp "authentication"
b00t advice github-mcp "toolset not working"
b00t advice github-mcp "rhai validation"
b00t advice github-mcp "pre-start script"
b00t advice github-mcp list
```
