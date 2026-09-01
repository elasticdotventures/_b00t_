# ── h3rmes — b00t-integrated Hermes Agent variant ───────────────────────────
# Extracted from root justfile for module isolation.

# Install/verify h3rmes (PromptExecution Hermes fork with b00t integration).
# Checks the vendor/hermes-agent-b00t submodule, registers MCP servers,
# and installs the guard interposition plugin. Asks before destructive ops.

# Self-referential MCP: run h3rmes as MCP server that exposes itself as a
# discoverable tool surface. Other h3rmes instances can connect via:
#   hermes --mcp-server h3rmes-mcp
h3rmes-mcp-serve port="8002":
    #!/bin/bash
    set -euo pipefail
    port={{port}}
    HERMES_BIN="$(command -v hermes || echo '')"
    if [ -z "$HERMES_BIN" ]; then
        echo "✗ hermes not in PATH — run 'just h3rmes install' first"
        exit 1
    fi
    echo "🥾 h3rmes MCP serve on :{{port}}"
    echo "  Connect from another agent:"
    echo "    h3rmes --mcp-server http://localhost:{{port}}/mcp"
    echo
    exec "$HERMES_BIN" mcp serve --port {{port}}

# Register h3rmes as a self-discoverable MCP server in the b00t MCP registry.
# This lets h3rmes discover itself via `b00t mcp list` and lets other
# h3rmes instances connect to this one via `h3rmes --mcp-server h3rmes-mcp`.
h3rmes-mcp-register port="8002":
    #!/bin/bash
    set -euo pipefail
    port={{port}}
    HERMES_BIN="$(command -v hermes || echo '')"
    if [ -z "$HERMES_BIN" ]; then
        echo "✗ hermes not in PATH — run 'just h3rmes install' first"
        exit 1
    fi
    echo "🥾 Registering h3rmes MCP server..."
    # Register in Hermes config as an MCP server pointing to itself
    HERMES_CONFIG="$HOME/.hermes/config.yaml"
    export HERMES_CONFIG HERMES_BIN
    python3 -c 'import os,yaml; path=os.environ["HERMES_CONFIG"]; cfg=yaml.safe_load(open(path)) or {}; cfg.setdefault("mcp_servers", {})["h3rmes-mcp"]={"command": os.environ["HERMES_BIN"], "args": ["mcp", "serve", "--port", "{{port}}"]}; yaml.dump(cfg, open(path, "w"), default_flow_style=False); print("h3rmes-mcp registered in Hermes config")' 2>/dev/null || echo 'yaml write failed'
    echo "  Run: just h3rmes-mcp-serve port={{port}}"
    CONNECT_STR="--mcp-server h3rmes-mcp"
    echo "  Connect: h3rmes $CONNECT_STR"

h3rmes action="status":
    #!/bin/bash
    set -euo pipefail

    # Resolve paths
    B00T_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || echo "$HOME/.b00t")"
    H3RMES_DIR="$B00T_ROOT/vendor/hermes-agent-b00t"
    B00T_CLI="$(command -v b00t-cli || command -v b00t || echo '')"

    case "{{action}}" in
        status)
            echo "🥾 h3rmes status"
            echo "  source:   $H3RMES_DIR"

            if [ -d "$H3RMES_DIR" ]; then
                echo "  submodule: ✓"
                (cd "$H3RMES_DIR" && echo "    branch: $(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo 'detached')")
                (cd "$H3RMES_DIR" && echo "    commit: $(git log --oneline -1 2>/dev/null || echo 'N/A')")

                # Check for b00t-specific patches
                PATCH_COUNT=$(cd "$H3RMES_DIR" && (git merge-base --is-ancestor origin/main HEAD 2>/dev/null && git log --oneline origin/main..HEAD 2>/dev/null || echo "") | wc -l || echo 0)
                PATCH_COUNT=${PATCH_COUNT//[[:space:]]/}
                if [ -n "${PATCH_COUNT:-0}" ] && [ "${PATCH_COUNT:-0}" -gt 0 ] 2>/dev/null; then
                    echo "    b00t patches: $PATCH_COUNT ✓"
                else
                    echo "    b00t patches: 0 ⚠️  (not on b00t branch)"
                fi
            else
                echo "  submodule: ✗ missing"
            fi

            # Check installed binary
            if command -v hermes &>/dev/null; then
                HERMES_PATH="$(command -v hermes)"
                echo "  binary:   ${HERMES_PATH}"
                hermes --version 2>/dev/null || echo "    (version check failed)"
            else
                echo "  binary:   ✗ not in PATH"
            fi

            # Check MCP servers registered
            HERMES_CONFIG="$HOME/.hermes/config.yaml"
            if [ -f "$HERMES_CONFIG" ]; then
                echo "  config:   $HERMES_CONFIG"
                for server in b00t-mcp codebase-memory irontology-mcp; do
                    if grep -q "$server" "$HERMES_CONFIG" 2>/dev/null; then
                        echo "    mcp/$server: ✓"
                    else
                        echo "    mcp/$server: ⚠️ not registered"
                    fi
                done
            else
                echo "  config:   ✗ missing"
            fi

            # Check guard plugin
            H3RMES_PLUGIN="$H3RMES_DIR/plugins/b00t"
            if [ -f "$H3RMES_PLUGIN/__init__.py" ] && [ -f "$H3RMES_PLUGIN/plugin.yaml" ]; then
                echo "  guard plugin: ✓"
            else
                echo "  guard plugin: ⚠️ not installed in submodule"
            fi
            ;;
        install|update)
            # ── Permission gate ──────────────────────────────────────────
            echo "🥾 h3rmes {{action}}"
            echo
            echo "  This will:"
            echo "    [1] Ensure vendor/hermes-agent-b00t submodule is checked out"
            echo "    [2] Register MCP servers in ~/.hermes/config.yaml"
            echo "    [3] Enable the b00t guard interposition plugin"
            echo "    [4] Install/verify b00t-mcp and codebase-memory MCP servers"
            echo
            read -r -p "  Continue? [y/N] " REPLY
            if [[ ! "$REPLY" =~ ^[Yy]$ ]]; then
                echo "  Aborted."
                exit 1
            fi
            echo

            # Ensure submodule is checked out on the right branch
            if [ ! -d "$H3RMES_DIR" ]; then
                echo "📦 Initializing hermes-agent-b00t submodule..."
                cd "$B00T_ROOT"
                git submodule update --init vendor/hermes-agent-b00t
            fi

            cd "$H3RMES_DIR"
            # Ensure we're on the b00t feature branch with patches
            git checkout b00t 2>/dev/null || true
            # Check if the b00t-specific branch exists
            if git show-ref --verify --quiet refs/heads/feat/pre-tool-rewrite-hook; then
                git checkout feat/pre-tool-rewrite-hook
                echo "    branch: feat/pre-tool-rewrite-hook (with b00t patches)"
            else
                echo "    branch: b00t (upstream base)"
            fi
            cd "$B00T_ROOT"

            # Register MCP servers via b00t-cli
            if [ -n "$B00T_CLI" ]; then
                echo "🔌 Registering MCP servers..."
                "$B00T_CLI" install hermes 2>/dev/null || echo "    (b00t-cli install hermes skipped)"
                "$B00T_CLI" mcp install b00t-mcp hermes 2>/dev/null || echo "    (b00t-mcp already registered)"

                # Install codebase-memory-mcp if built
                if [ -f "$B00T_ROOT/vendor/b00tyverse/codebase-memory-mcp-b00t-ir0n-ledg3rr/build/c/codebase-memory-mcp" ]; then
                    echo "🔌 Registering codebase-memory..."
                    "$B00T_CLI" mcp install codebase-memory hermes 2>/dev/null || true
                fi

                # Build and register irontology-mcp
                echo "🔌 Building irontology-mcp..."
                cargo build --release --manifest-path "$B00T_ROOT/vendor/irontology-mcp/Cargo.toml" -p mcp-server 2>&1 | tail -3
                IRONTOLOGY_BIN="$B00T_ROOT/vendor/irontology-mcp/target/release/irontology-mcp"
                if [ -f "$IRONTOLOGY_BIN" ]; then
                    echo "🔌 Registering irontology-mcp..."
                    mkdir -p "$B00T_ROOT/target/release"
                    ln -sf "$IRONTOLOGY_BIN" "$B00T_ROOT/target/release/irontology-mcp" 2>/dev/null || true
                    "$B00T_CLI" mcp install irontology-mcp hermes 2>/dev/null || echo "    (irontology-mcp already registered)"
                fi
            else
                echo "⚠️ b00t-cli not found; MCP registration skipped"
                echo "  Run: cd $B00T_ROOT && just install"
            fi

            echo
            echo "✅ h3rmes {{action}} complete"
            echo "  Restart Hermes or run /reset for changes to take effect."
            ;;
        doctor|check)
            just h3rmes::h3rmes status
            echo "---"
            echo "🔍 Running health checks..."
            # Verify b00t-cli works
            if [ -n "$B00T_CLI" ]; then
                "$B00T_CLI" --version 2>/dev/null && echo "  b00t-cli: ✓" || echo "  b00t-cli: ✗"
            else
                echo "  b00t-cli: ✗ not found"
            fi
            # Verify submodule MCP config
            HERMES_CONFIG="$HOME/.hermes/config.yaml"
            if [ -f "$HERMES_CONFIG" ]; then
                if grep -q "b00t-mcp" "$HERMES_CONFIG" 2>/dev/null; then
                    echo "  hermes-mcp-config: ✓"
                else
                    echo "  hermes-mcp-config: ⚠️ b00t-mcp not in config"
                fi
            fi
            # Verify guard interposition plugin
            if [ -d "$H3RMES_DIR/plugins/b00t" ]; then
                echo "  guard-plugin: ✓"
            else
                echo "  guard-plugin: ⚠️ not deployed"
            fi
            echo
            echo "✅ h3rmes check complete"
            ;;
        *)
            echo "Usage: just h3rmes [status|install|update|doctor|check]"
            echo "  status   — show current h3rmes integration state"
            echo "  install  — install/configure h3rmes (with permission gate)"
            echo "  update   — same as install (idempotent)"
            echo "  doctor   — run health checks"
            echo "  check    — alias for doctor"
            ;;
    esac

# Alias: just hermes -> just h3rmes
hermes action="status":
    just h3rmes::h3rmes {{action}}
