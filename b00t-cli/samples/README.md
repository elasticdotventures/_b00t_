# MCP Server Samples

This directory contains sample JSON configurations for MCP servers that can be used with the `b00t-cli mcp add` command.

## Architecture: One Source, Many Targets

b00t-cli uses a "one import, many export" architecture for MCP server configurations:

1. **Import**: Use `b00t-cli mcp add` to create `.mcp.toml` files (single source of truth)
2. **Export**: Use various subcommands to install to different MCP-compatible clients

## Usage Examples

### Step 1: Add MCP server configurations

```bash
# Basic usage (Playwright server)
b00t-cli mcp add "$(cat samples/mcp/playwright.json)"

# With --dwiw flag to auto-clean comments
b00t-cli mcp add --dwiw "$(cat samples/mcp/with-comments.json)"

# Direct format
b00t-cli mcp add "$(cat samples/mcp/direct-format.json)"

# List available MCP servers
b00t-cli mcp list
b00t-cli mcp list --json  # JSON output format
```

### Step 2: Install to specific clients

```bash
# Install to VSCode
b00t-cli vscode install mcp playwright
b00t-cli vscode install mcp filesystem

# Install to Claude Code
b00t-cli claude-code install mcp playwright
b00t-cli claude-code install mcp filesystem

# Future: Install to other MCP clients (planned)
# b00t-cli cursor install mcp playwright
```

## Sample Files

- `playwright.json` - Playwright MCP server configuration
- `direct-format.json` - Direct format with name/command/args at top level  
- `with-comments.json` - Example with JavaScript-style comments (use --dwiw flag)

## Generated Files

When you run `b00t-cli mcp add`, it creates a `.mcp.toml` file in `~/.dotfiles/_b00t_/`:

```toml
[mcp]
name = "playwright"
command = "npx"
args = ["-y", "@executeautomation/playwright-mcp-server"]
```

This serves as the single source of truth that can be exported to any MCP-compatible client.