# ── OpenCode plugin management ──────────────────────────────────────────────
# Extracted from root justfile for module isolation.

# Install opencode-goal-plugin (canonical willytop8 fork)
opencode-goal-install:
    @echo "📦 Installing opencode-goal-plugin..."
    npm install -g opencode-goal-plugin 2>/dev/null || \
        npm install --save-dev opencode-goal-plugin
    @echo "✅ Add to opencode.json: {\"plugin\": [\"opencode-goal-plugin\"]}"

# Install all b00t opencode plugins
opencode-plugins-install: opencode-goal-install
    @echo "✅ All opencode plugins installed"

# Run OpenCode via podman with full b00t environment mounted
# 🤓 Mounts: workspace, config, MCP binaries, git/ssh, API keys.
#    b00t-mcp + codebase-memory-mcp binaries from host PATH.
opencode-run workspace=".":
    @echo "🚀 Launching OpenCode (podman)..."
    @mkdir -p {{workspace}}/.opencode/skills
    podman run -it --rm \
        --security-opt label=disable \
        -v {{workspace}}:/workspace \
        -v ~/.config/opencode/opencode.json:/root/.config/opencode/opencode.json:ro \
        -v {{workspace}}/.opencode/skills:/root/.opencode/skills:ro \
        -v ~/.local/bin/b00t-mcp:/home/brianh/.local/bin/b00t-mcp:ro \
        -v ~/.local/bin/codebase-memory-mcp:/home/brianh/.local/bin/codebase-memory-mcp:ro \
        -v ${HOME}/.gitconfig:/root/.gitconfig:ro \
        -v ${HOME}/.ssh:/root/.ssh:ro \
        -e OPENAI_API_KEY \
        -e ANTHROPIC_API_KEY \
        -w /workspace \
        ghcr.io/anomalyco/opencode

# Run OpenCode for app4dog with full game development environment
opencode-app4dog:
    @echo "🐶 Launching OpenCode for app4dog..."
    @mkdir -p ~/promptexecution/app4dog/.opencode/skills
    podman run -it --rm \
        --security-opt label=disable \
        --network host \
        -v ~/promptexecution/app4dog:/workspace \
        -v ~/promptexecution/app4dog/.opencode/skills:/root/.opencode/skills:ro \
        -v ~/.config/opencode/opencode.json:/root/.config/opencode/opencode.json:ro \
        -v ~/.local/bin/b00t-mcp:/home/brianh/.local/bin/b00t-mcp:ro \
        -v ~/.local/bin/codebase-memory-mcp:/home/brianh/.local/bin/codebase-memory-mcp:ro \
        -v ${HOME}/.gitconfig:/root/.gitconfig:ro \
        -v ${HOME}/.ssh:/root/.ssh:ro \
        -e OPENAI_API_KEY \
        -e ANTHROPIC_API_KEY \
        -w /workspace \
        ghcr.io/anomalyco/opencode
