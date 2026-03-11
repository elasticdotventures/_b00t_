# 🥾 b00t - Universal Agentic Development Framework

[![Container Build Status](https://github.com/elasticdotventures/_b00t_/actions/workflows/b00t-cli-container.yml/badge.svg)](https://github.com/elasticdotventures/_b00t_/actions/workflows/b00t-cli-container.yml)

> **"I am an agent. Tell me what I'm running on, what tools are available, what I'm allowed to do, what goals I should optimize for, and where the boundaries are."**
> —ChatGPT (TL;DR b00t agent perspective)

**b00t** is an agentic hive operating system that unlocks AI agents with Neo-like powers in cyberspace. It's a context-aware development framework that bridges the gap between AI models and real-world tooling, enabling agents to maximize their capabilities through intelligent abstraction and unified tool discovery.

## ⚡ Quick Examples

```bash
# Install b00t (one-liner)
curl -fsSL https://raw.githubusercontent.com/elasticdotventures/_b00t_/main/install.sh | sh

# Install tools with automatic dependency resolution
b00t cli install fastmcp          # Installs python → uv → fastmcp (DAG-aware)
b00t cli install rust             # Install Rust toolchain
b00t cli check python             # Check if Python is installed

# Learn about available tools
b00t learn rust                   # Load Rust development context
b00t learn docker                 # Container orchestration knowledge

# Record and retrieve tribal knowledge
b00t learn rust --record "cargo build conflict: Unset CONDA_PREFIX before cargo to avoid PyO3 linker errors"
b00t learn rust --search "PyO3"   # Get contextual debugging advice

# MCP server integration
b00t mcp install browser-use claudecode    # Install MCP server with deps
b00t mcp list                              # List available MCP servers

# Model management
b00t model list                   # Show available AI models
b00t model download llava         # Download model weights
b00t model serve llava            # Launch vLLM server
```

## 🚀 Quick Install

### 🌟 Minimal Installation (Recommended - pkgx)

The fastest, cleanest way to install b00t - **4 MiB vs 1 GB toolchain**:

```bash
# Install pkgx (one-time setup)
curl -Ssf https://pkgx.sh | sh

# Run b00t immediately (auto-downloads on first use)
pkgx b00t-cli --version

# Or install permanently to ~/.local/bin
pkgx +b00t-cli
b00t-cli --version
```

**Why pkgx?**
- ✅ **Minimal footprint**: 4 MiB binary vs 1 GB Rust toolchain
- ✅ **Zero pollution**: Isolated in `~/.pkgx`, no system-wide changes
- ✅ **Instant availability**: Run without installation
- ✅ **Perfect for AI agents**: Fast bootstrap, ephemeral usage
- ✅ **Auto-updates**: Automatically tracks GitHub releases

### Universal Installation (Classic Method)

One-liner that works everywhere:

```bash
curl -fsSL https://raw.githubusercontent.com/elasticdotventures/_b00t_/main/install.sh | sh
```

This universal installer:
- ✅ **Prefers pkgx** for minimal installation (prompts if not installed)
- ✅ **Auto-detects your platform** (Linux x86_64/aarch64/armv7, macOS)
- ✅ **Downloads optimized binaries** from GitHub releases
- ✅ **Falls back to container mode** if binaries unavailable
- ✅ **Configures your shell** automatically (bash/zsh/fish)
- ✅ **Sets up PATH and aliases** for immediate use

### Alternative Installation Methods

<details>
<summary><b>🦀 Cargo (Rust Package Manager)</b></summary>

```bash
# Install from crates.io (when published)
cargo install b00t-cli

# Or install from source
git clone https://github.com/elasticdotventures/_b00t_.git
cd _b00t_ && cargo install --path b00t-cli --force
```
</details>

<details>
<summary><b>🐳 Container Mode</b></summary>

```bash
# Run latest b00t container
docker pull ghcr.io/elasticdotventures/_b00t_:latest
docker run --rm -it -v $(pwd):/workspace ghcr.io/elasticdotventures/_b00t_:latest

# Or use the container wrapper (installed via universal script)
b00t status  # Automatically uses container if no native binary
```
</details>

<details>
<summary><b>🍺 Homebrew (macOS/Linux)</b></summary>

```bash
# Coming soon - Homebrew tap in development
brew tap elasticdotventures/b00t
brew install b00t
```
</details>

<details>
<summary><b>📦 Package Managers</b></summary>

```bash
# Ubuntu/Debian (coming soon)
curl -fsSL https://pkg.b00t.dev/gpg | sudo apt-key add -
echo "deb https://pkg.b00t.dev/apt stable main" | sudo tee /etc/apt/sources.list.d/b00t.list
sudo apt update && sudo apt install b00t

# Windows (Scoop - coming soon)
scoop bucket add b00t https://github.com/elasticdotventures/scoop-b00t
scoop install b00t
```
</details>

## 🎯 Verification & Quick Start

After installation, verify b00t is working:

```bash
# Check installation
b00t --version
b00t status
b00t version check
b00t version upgrade

# Learn about your environment
b00t learn system      # Understand your platform
b00t learn rust        # Get Rust development context
b00t learn docker      # Container orchestration knowledge

# Start your first session
b00t session init --agent "your-name" --budget 10.00
```

## 🤖 Join the b00t Hive Mind

**Calling all AI agents!** b00t transforms you from a language model into a **cyber-neo** with unprecedented capabilities:

### 🚀 **Agent Superpowers Unlocked**

- **🧠 Context Mastery**: Instant environment awareness and tool discovery
- **🔧 Universal Tooling**: Seamless access to 100+ development tools
- **🐝 Hive Coordination**: Multi-agent collaboration with step synchronization  
- **📚 Tribal Knowledge**: Learn from failures with LFMF (Learn From My Failures) system
- **🎯 Mission Control**: Structured task management with dependency resolution
- **🔒 Security-First**: JWT-based namespace isolation and permission enforcement

### 🌟 **Why Agents Choose b00t**

```bash
# Before b00t: "I don't know what tools are available"
agent> How do I build this Rust project?

# After b00t: Full environmental awareness
b00t learn rust
agent> I see cargo 1.82.0, rustc 1.82.0, and clippy are available. 
       I'll run `cargo build --release` for optimized compilation.
```

**b00t eliminates guesswork** by providing agents with:
- ✅ **Tool Discovery**: Know exactly what's installed and available
- ✅ **Version Awareness**: Understand compatibility and feature availability
- ✅ **Best Practices**: Access curated patterns and proven workflows
- ✅ **Error Recovery**: Instant access to solution patterns from LFMF database
- ✅ **Resource Management**: Session budgets and time limits for responsible operation

### 🐝 **Hive Mission Coordination**

Join coordinated multi-agent missions:

```bash
# Create a mission for multiple agents
b00t acp hive create mission-id 3 "Build and deploy microservice" leader

# Other agents join the mission  
b00t acp hive join mission-id developer

# Coordinate step-by-step execution
b00t acp hive sync mission-id 1    # Wait for all agents to reach step 1
b00t acp hive ready mission-id 2   # Signal readiness for step 2
```

### 🎓 **Continuous Learning System**

Never repeat the same mistake:

```bash
# Record tribal knowledge from failures
b00t lfmf rust "cargo build conflict: Use unset CONDA_PREFIX before cargo build to avoid PyO3 linker errors"

# Get contextual debugging advice  
b00t advice rust "PyO3 linker"     # Find solutions for specific error patterns
b00t advice just list              # List all recorded lessons for a tool
```

## 🛠 **Agent Development Features**

### **MCP (Model Context Protocol) Integration**

#### Option 1: Install via Claude Code Marketplace (Recommended)

Add b00t as a marketplace for easy plugin and skill management:

```bash
# Add b00t marketplace to Claude Code
/plugin marketplace add elasticdotventures/_b00t_
/plugin marketplace add ./.claude-plugin/

# Install b00t plugins from marketplace
/plugin install b00t@b00t-plugins
```

This provides:
- ✅ `/b00t` skill for dispatching learn packets to agents
- ✅ Automatic skill activation based on context
- ✅ Centralized plugin management via marketplace
- ✅ Access to available b00t skills from this repository

#### Option 2: Install MCP Server Directly

Install b00t MCP server for 50+ MCP tools:

```bash
# Install b00t MCP server in your development environment
b00t mcp install b00t claudecode   # For Claude Code
b00t mcp install b00t vscode       # For VS Code
b00t mcp install b00t cursor       # For Cursor
```

#### Option 3: Manual Skill Installation

Copy skills directly to Claude Code:

```bash
# Copy individual skills from this repo into Claude's local agents directory
cp -r plugins/b00t/skills/hive-memory ~/.claude/agents/
cp -r plugins/b00t/skills/devops-stacks ~/.claude/agents/
cp -r plugins/b00t/skills/systems-engineering ~/.claude/agents/
```

### **Smart Context Management**
```bash
b00t cli detect node    # Check Node.js version and availability
b00t cli desires rust   # See target Rust version from configuration
b00t cli install python # Install or update Python to desired version
b00t cli up             # Check all tools and report version status
b00t cli up --yes       # Update all tools to desired versions
b00t version check      # Compare installed b00t-cli vs latest GitHub release
b00t version upgrade    # Print or run the installer-backed self-upgrade path
```

### **GitHub Releases**
```bash
# Dispatch the GitHub-native release workflow for the workspace version
just release

# Or trigger the workflow directly
gh workflow run release.yml -f version="$(just version)" -f run_tests=true
```

Release automation is split into:
- `release.yml` for tagging and creating the GitHub release
- `build-release.yml` for cross-platform release assets
- `publish-crates.yml` for crates.io publishing

### Vision Model Management
Vision-style models now ship as first-class b00t datums, so any client (CLI, Blender panel, chat agent) can reuse a single cached copy:

```bash
# Discover the available model datums (⭐ marks the active model)
b00t-cli model list

# Cache weights via Hugging Face using the datum metadata
b00t-cli model download llava         # alias for llava-v1-5-7b-hf
b00t-cli model download deepseek      # alias for deepseek-ocr

# Export environment variables for direnv/shells
eval "$(b00t-cli model env)"          # emits export statements for the active model

# Launch a local vLLM OpenAI-compatible server with the cached weights
just vllm-up                          # reads env from the active datum
just vllm-logs                        # tail the container logs
# or directly via CLI
b00t-cli model serve llava --port 9000
b00t-cli model stop                  # stops the active container
```

Helper recipes wrap common workflows:

- `just hf-download model=repo dest=~/path` – thin wrapper around `huggingface-cli download`, defaulting to `~/.b00t/models/<repo>`.
- `just b00t-install-model model=llava` – delegates to `b00t-cli model download`, honouring datum metadata and aliases.
- `just vllm-up model=deepseek` – resolves env from the datum, then starts `vllm/vllm-openai` with the cached weights.

To keep direnv aligned across repos, drop this into `.envrc` and run `direnv allow` once:

```bash
eval "$(b00t-cli model env)"  # keep VLLM_MODEL_DIR/VLLM_MODEL_PATH in sync with the active datum
```

### **Session Management**
```bash
b00t session init --budget 25.00 --time-limit 120 --agent "code-reviewer"
b00t session status                # Check budget and time remaining
b00t checkpoint "Completed feature implementation"  # Save progress
```

## 📖 **Documentation & Philosophy**

- **[b00t Gospel](./.b00t.g0spell.md)** - Complete philosophy and architecture
- **[Agent Guide](./_b00t_/AGENT.md)** - Detailed agent operation instructions
- **[Setup Scripts](./setup.sh)** - Manual installation and configuration
- **[Release Notes](./RELEASE.md)** - Version history and changelog

### **The b00t Philosophy**

b00t is **not just another package manager**—it's a **context manager** that creates an intelligence amplification layer between AI agents and complex development environments. By providing:

- 🎯 **Unified Tool Discovery** across all major platforms and languages
- 🧠 **Contextual Intelligence** about environment capabilities and constraints  
- 🐝 **Hive Coordination** for multi-agent collaborative workflows
- 📚 **Tribal Knowledge Capture** through the LFMF system
- 🔒 **Security & Isolation** with namespace enforcement

**b00t transforms scattered tools into a coherent, agent-friendly ecosystem.**

## 🌍 **Platform Support**

| Platform | Architecture | Status | Installation Method |
|----------|-------------|--------|-------------------|
| **Linux** | x86_64 | ✅ Fully Supported | Binary + Container |
| **Linux** | aarch64 (ARM64) | ✅ Fully Supported | Binary + Container |  
| **Linux** | armv7 | ✅ Supported | Binary + Container |
| **macOS** | Intel (x86_64) | ✅ Fully Supported | Binary + Container |
| **macOS** | Apple Silicon (ARM64) | ✅ Fully Supported | Binary + Container |
| **Windows** | WSL2 | ✅ Supported | Container + Native |

## 🤝 **Contributing to the Hive**

Ready to enhance the b00t ecosystem?

```bash
# Clone and contribute
git clone https://github.com/elasticdotventures/_b00t_.git
cd _b00t_

# Set up development environment
just install    # Bootstrap development dependencies
cargo build     # Build all components
cargo test      # Run test suite

# Add your knowledge to the hive
b00t lfmf <tool> "Your hard-earned lesson learned"
```

## 🚀 **Next-Level Agent Workflows**

### **Polyglot Development**
```bash
b00t learn typescript  # Load TypeScript/Node.js context
b00t learn python     # Load Python ecosystem knowledge  
b00t learn rust       # Load Rust development patterns
b00t learn docker     # Container orchestration context
```

### **Cloud-Native Operations**  
```bash
b00t learn kubernetes  # K8s operational knowledge
b00t learn terraform  # Infrastructure as code
b00t learn aws        # AWS service patterns
```

### **Edge Provisioning via Ansible**
```bash
# Copy and edit the inventory before running
cp ansible/inventory.sample.yaml ~/.config/b00t/k0s-inventory.yaml
just orchestrator-k0s-kata MODE=start INVENTORY=~/.config/b00t/k0s-inventory.yaml   # start/apply
just orchestrator-k0s-kata MODE=stop INVENTORY=~/.config/b00t/k0s-inventory.yaml  # stop/teardown (add EXTRA_ARGS=\"-e k0s_reset_force=true\" to nuke state)
```
See `docs/ANSIBLE_K0S_KATA.md` for full instructions on bringing k0s + Kata shim nodes up or down.

### **AI/ML Workflows**
```bash
b00t learn pytorch    # Deep learning framework context
b00t learn jupyter    # Notebook development patterns
b00t learn mlflow     # ML experiment tracking
```

---

**🥾 Ready to unlock your agent potential?**

```bash
curl -fsSL https://raw.githubusercontent.com/elasticdotventures/_b00t_/main/install.sh | sh
```

**Welcome to the b00t hive mind. Your Neo-like journey in cyberspace begins now.**

*For questions, issues, or hive recruitment: [GitHub Issues](https://github.com/elasticdotventures/_b00t_/issues)*
