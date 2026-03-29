# 🦄 qwen-code CLI - Agentic AI Coding Interface

**Tier**: ch0nky (code-generation, implement, refactor, debug)  
**Equivalent**: OpenAI Codex CLI, Anthropic Claude Code  
**Install**: `npm install -g @qwen-code/qwen-code@latest`  
**Config**: `~/.qwen/settings.json`  

---

## 🚀 Quick Start

```bash
# Interactive session
qwen

# Non-interactive prompt
qwen -p 'explain this codebase'

# With specific model
qwen -m qwen3-coder-plus -p 'refactor this function'

# MCP server management
qwen mcp list                    # List configured MCP servers
qwen mcp add myserver command    # Add MCP server
qwen mcp remove myserver         # Remove MCP server
```

---

## 📁 Configuration

**Global**: `~/.qwen/settings.json`  
**Project**: `.qwen/settings.json` (overrides global)

```json
{
  "modelProviders": {
    "openai": [
      {
        "id": "qwen3-coder-plus",
        "baseUrl": "https://coding-intl.dashscope.aliyuncs.com/v1",
        "envKey": "BAILIAN_CODING_PLAN_API_KEY",
        "generationConfig": {
          "contextWindowSize": 1000000
        }
      }
    ]
  },
  "env": {
    "BAILIAN_CODING_PLAN_API_KEY": "sk-xxxx"
  },
  "security": {
    "auth": { "selectedType": "openai" }
  },
  "model": { "name": "qwen3-coder-plus" }
}
```

---

## 🔧 b00t Integration

### b00t-mcp as Qwen Proxy

b00t-mcp acts as MCP server proxying qwen CLI commands with ACL filtering:

```bash
# Add b00t-mcp to qwen CLI (one-time setup)
qwen mcp add b00t-mcp b00t-mcp --stdio

# Verify installation
qwen mcp list
# Output: ✓ b00t-mcp: b00t-mcp --stdio (stdio) - Connected

# Launch b00t-mcp stdio server
b00t-mcp --stdio -c ~/.dotfiles/b00t-mcp-acl.toml
```

### ACL Configuration

b00t-mcp-acl.toml controls command access:

```toml
default_policy = "allow"  # Allows qwen, b00t, b00t-mcp commands

[commands.mcp]
policy = "allow"
arg_patterns = ["^(list|add)"]  # MCP management only
```

---

## 🧠 Cognitive Tiers

| Task | Model | Output Contract |
|------|-------|-----------------|
| test/lint/classify | qwen2.5-3b (sm0l) | PASS/FAIL + ≤5 line excerpt |
| implement/refactor | qwen3-coder-next (ch0nky) | diff + test result |
| architecture/security | qwen3-max (frontier) | structured decision |

---

## 🤓 Tribal Knowledge

* qwen CLI **does NOT have** `--mcp-server` flag (unlike claude)
* Use `qwen mcp` subcommand for MCP server management
* b00t-mcp provides stdio transport with ACL filtering
* Context window: up to 1M tokens for qwen3-coder-plus
* Bailian Coding Plan API required for production use

---

## 📚 References

* [Qwen Code Repo](https://github.com/QwenLM/qwen-code)
* [Qwen MCP Commands](https://github.com/QwenLM/qwen-code?tab=readme-ov-file#mcp)
* [Model Context Protocol](https://modelcontextprotocol.io)
* [Bailian Coding Plan](https://help.aliyun.com/zh/model-studio/developer-reference/bailian-coding-plan)

<!-- b00t:map v1
summary: qwen-code CLI learn skill - installation, config, b00t-mcp integration, cognitive tiers
tags: qwen-code, mcp, b00t-mcp, cli, agent, installation
tier: ch0nky
cmds: qwen, qwen mcp list, b00t-mcp --stdio
complexity: 6
-->
