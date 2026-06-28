# rust-doc MCP server — vendor submodule lifecycle
# PromptExecution fork of Govcraft/rust-docs-mcp-server
# Namespaced: just rust-doc <recipe>
# Required by: skills/rust/SKILL.md (refuses to operate without it)

# Build the server
build:
    #!/bin/bash
    set -euo pipefail
    B00T_ROOT="$(git rev-parse --show-toplevel)"
    cargo build --release --manifest-path "${B00T_ROOT}/vendor/rust-docs-mcp-server/Cargo.toml"

# Build and start in background (stdio mode, for IDE MCP)
start crate="serde":
    #!/bin/bash
    set -euo pipefail
    BIN="vendor/rust-docs-mcp-server/target/release/rustdocs_mcp_server"
    if [ ! -f "$BIN" ]; then
        echo "🔨 Building rust-doc server..."
        just rust-doc build
    fi
    echo "🚀 Starting rust-doc MCP server ({{crate}})..."
    nohup "$BIN" "{{crate}}" > /tmp/rust-doc-mcp.log 2>&1 &
    echo "✅ PID: $!"
    echo "   Logs: tail -f /tmp/rust-doc-mcp.log"

# Start in HTTP/SSE mode (agentic rendering)
start-http crate="serde" port="3000":
    vendor/rust-docs-mcp-server/target/release/rustdocs_mcp_server "{{crate}}" --http --port {{port}}

# Check if running
status:
    @pgrep -f rustdocs_mcp_server > /dev/null && echo "✅ rust-doc running" || echo "❌ rust-doc not running"

# Stop all instances
stop:
    pkill -f rustdocs_mcp_server 2>/dev/null || true

# Update from upstream
update:
    #!/bin/bash
    set -euo pipefail
    cd vendor/rust-docs-mcp-server
    git pull origin main
    cd /home/brianh/.dotfiles
    cargo build --release --manifest-path vendor/rust-docs-mcp-server/Cargo.toml
