# 🥾 b00t-tf: Infrastructure & Claude Desktop Extensions

Self-bootstrapping MCP server and OpenTofu infrastructure for b00t AI development toolkit.

## Overview

b00t-tf provides:
- **Self-bootstrapping MCP server** for Claude Desktop
- **Personalized .dxt file generation** via b00t.promptexecution.com
- **OpenTofu infrastructure modules** for multi-cloud provisioning
- **Intelligent tool detection** and auto-installation

## Quick Start

```bash
# Install dependencies and create base template
just setup

# Generate base .dxt template (for development)
just create-base-template

# Test MCP server locally
just test-server

# View all available commands
just --list
```

## Architecture

### 🎯 .dxt Generation Flow

1. **User visits** [b00t.promptexecution.com](https://b00t.promptexecution.com)
2. **Configures preferences**: AI providers, infrastructure, tools
3. **Downloads personalized .dxt**: `b00t-{username}-{timestamp}.dxt`
4. **Installs in Claude Desktop**: One-click MCP integration

### 🤖 MCP Server Features

- **Auto-installation**: Detects and installs missing tools (just, gh, tofu)
- **Multi-provider support**: Anthropic, OpenRouter, OpenAI
- **Infrastructure provisioning**: Uses local OpenTofu modules
- **Secure configuration**: API keys stored in OS keychain
- **Cross-platform**: Works on Linux, macOS, Windows

### ☁️ Infrastructure Modules

```
modules/
├── base/           # Core provider configuration
├── cloudflare/     # Workers, KV, R2 resources
├── aws/           # Lambda, S3, IAM resources
└── monitoring/    # Alerts and observability
```

## Configuration

### Environment Setup

The configuration system uses a layered approach:

1. **`.env`** (user overrides) - highest priority
2. **`_b00t_.toml`** (session defaults) - middle priority  
3. **Hardcoded fallbacks** - lowest priority

```bash
# Copy template and customize
cp .env.template .env
# Edit .env with your API keys and preferences
```

### Session Configuration

The `_b00t_.toml` file defines defaults for your b00t session:

```toml
[session]
project_name = "b00t"
aws_region = "us-east-1"

[providers]
anthropic_enabled = true
openrouter_enabled = false

[infrastructure]
cloudflare_workers = true
aws_lambda = false

[tools]
filesystem = true
git = true
terraform = true
```

## MCP Tools Available

| Tool | Description |
|------|-------------|
| `bootstrap_b00t` | Initialize and install required tools |
| `install_tool` | Install specific CLI tools (just, gh, tofu) |
| `provision_infrastructure` | Deploy infrastructure using OpenTofu |
| `configure_provider` | Setup AI provider credentials |
| `get_status` | Show system status and configuration |

## Development

### Local Testing

```bash
# Start development server
npm run dev

# Test MCP server directly
node proxy-server.js

# Validate configuration
just validate
```

### OpenTofu Commands

```bash
# Initialize providers
just tf-init

# Plan infrastructure changes
just tf-plan

# Apply changes (with confirmation)
just tf-apply

# Destroy resources (with confirmation)
just tf-destroy
```

## File Structure

```
b00t-tf/
├── proxy-server.js           # Main MCP server
├── manifest.json            # Claude Desktop extension manifest
├── package.json            # Node.js dependencies
├── main.tf                 # Root OpenTofu configuration
├── _b00t_.toml            # Default session configuration
├── .env.template          # Environment template
├── justfile               # Development commands
├── modules/               # OpenTofu infrastructure modules
│   ├── base/             # Provider configuration
│   ├── cloudflare/       # Cloudflare resources
│   └── aws/              # AWS resources
└── template/             # Generated .dxt template
    └── b00t-base.dxt    # Base template for personalization
```

## Contributing

1. **Follow b00t conventions**: Use justfile for repeatable tasks
2. **Test thoroughly**: Ensure MCP tools work cross-platform
3. **Update documentation**: Keep README and justfile help current
4. **Security first**: Never commit API keys or secrets

## Related Projects

- **[b00t-website](https://github.com/PromptExecution/b00t-website)**: Vue3 configurator for .dxt generation
- **[dotfiles](https://github.com/elasticdotventures/dotfiles)**: Personal development environment setup

---

## 🍰 Next Steps

See [NEXT-STEPS.md](./NEXT-STEPS.md) for deployment and testing roadmap.

---

*Part of the b00t AI development toolkit - Making infrastructure as easy as 🥾*